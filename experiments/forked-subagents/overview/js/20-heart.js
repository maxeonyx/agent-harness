// The heart: what a task call does to contexts, replayed from a real run, and the one finding that matters most.

function agentsInOrder(requests) {
  const paths = [...new Set(requests.map(r => r.agent))];
  const kids = p => paths.filter(q => q.startsWith(p + " › ") && q.split(" › ").length === p.split(" › ").length + 1).sort();
  const out = [];
  const walk = p => { out.push(p); kids(p).forEach(walk); };
  walk("root");
  return out;
}

function tapeModel(which) {
  const w = DATA.wire[which];
  const order = agentsInOrder(w.requests);
  const agents = order.map(path => {
    const reqs = w.requests.filter(r => r.agent === path).sort((a, b) => a.sent - b.sent);
    const parent = path.split(" › ").slice(0, -1).join(" › ") || null;
    const suspended = [];
    reqs.forEach((r, i) => { if (r.calls.some(c => c.name === "task") && reqs[i + 1]) suspended.push([r.returned, reqs[i + 1].sent]); });
    return { path, name: path.split(" › ").pop(), depth: path.split(" › ").length - 1, parent, reqs, start: reqs[0].sent, end: reqs[reqs.length - 1].returned,
      inherited: parent ? reqs[0].cached : 0, final: reqs[reqs.length - 1].prompt + reqs[reqs.length - 1].out, suspended };
  });
  const T = Math.max(...agents.map(a => a.end));
  const lengthAt = (a, t) => {
    let len = 0, flying = false;
    for (const r of a.reqs) {
      if (r.sent > t) break;
      len = r.prompt + (r.returned <= t ? r.out : 0);
      flying = r.returned > t;
    }
    return { len, flying };
  };
  const spentAt = t => w.requests.filter(r => r.returned <= t).reduce((x, r) => x + r.cost, 0);
  return { agents, T, lengthAt, spentAt };
}

function hatch(id, colour) {
  return s("pattern", { id, width: 6, height: 6, patternUnits: "userSpaceOnUse", patternTransform: "rotate(45)" },
    s("rect", { width: 6, height: 6, fill: colour, opacity: 0.85 }), s("line", { x1: 0, y1: 0, x2: 0, y2: 6, stroke: "#0a0c11", "stroke-width": 1.6, opacity: 0.35 }));
}

function tapes(which, width) {
  const model = tapeModel(which);
  const { agents, T } = model;
  const rowH = 34, x0 = 170, top = 40;
  const maxLen = Math.max(...agents.map(a => a.final));
  const scale = (width - x0 - 180) / maxLen;
  const height = top + agents.length * rowH + 70;
  const svg = s("svg", { width, height, viewBox: `0 0 ${width} ${height}` });
  svg.append(s("defs", {}, hatch("ice-" + which, "var(--ice)")));
  const rowY = new Map(agents.map((a, i) => [a.path, top + i * rowH + rowH / 2]));
  const layer = s("g");
  svg.append(layer);
  // axis: tokens
  const axis = s("g");
  for (let k = 0; k <= maxLen; k += 2000) {
    axis.append(s("line", { x1: x0 + k * scale, x2: x0 + k * scale, y1: top - 6, y2: height - 50, stroke: "#1a1f2b" }));
    axis.append(s("text", { x: x0 + k * scale, y: top - 12, "text-anchor": "middle", class: "tape-label" }, k ? fmt.tok(k) : "0 tokens"));
  }
  svg.prepend(axis);

  const leaves = agents.filter(a => a.depth === Math.max(...agents.map(x => x.depth)));
  const seam = Math.min(...leaves.map(a => a.inherited)), tail = Math.max(...leaves.map(a => a.final));
  const draw = t => {
    layer.replaceChildren();
    if (t >= T) {
      const y = height - 36;
      layer.append(s("path", { d: `M${x0} ${y - 4} v4 H ${x0 + seam * scale - 2} v-4`, fill: "none", stroke: "var(--ice)" }));
      layer.append(s("text", { x: x0 + (seam * scale) / 2, y: y + 13, "text-anchor": "middle", class: "tape-label", fill: "var(--ice)" }, "the parent's own bytes — every child read them from cache"));
      layer.append(s("path", { d: `M${x0 + seam * scale + 2} ${y - 4} v4 H ${x0 + tail * scale} v-4`, fill: "none", stroke: "var(--ember)" }));
      layer.append(s("text", { x: x0 + seam * scale + 4, y: y + 26, class: "tape-label", fill: "var(--ember)" }, "each child's own"));
    }
    for (const a of agents) {
      const y = rowY.get(a.path);
      const on = t >= a.start;
      const colour = HUE[lineage(a.name)];
      // tree guide + name
      if (a.parent) layer.append(s("path", { d: `M${14 + (a.depth - 1) * 14} ${rowY.get(a.parent) + 6} V ${y} h 8`, fill: "none", stroke: on ? "#3a4256" : "#1b2030" }));
      const label = s("text", { x: 14 + a.depth * 14 + (a.depth ? 0 : 0), y: y + 4, class: "tape-name", fill: on ? colour : "#2b3142" }, a.name);
      layer.append(label);
      if (!on) { layer.append(s("rect", { x: x0, y: y - 7, width: a.final * scale, height: 14, rx: 3, fill: "none", stroke: "#161a24", "stroke-dasharray": "2 3" })); continue; }
      const { len, flying } = model.lengthAt(a, t);
      const iceW = Math.min(a.inherited, len) * scale;
      const ownW = Math.max(0, len - a.inherited) * scale;
      layer.append(s("rect", { x: x0, y: y - 7, width: iceW, height: 14, rx: 3, fill: `url(#ice-${which})` }));
      layer.append(s("rect", { x: x0 + iceW, y: y - 7, width: ownW, height: 14, rx: 2, fill: "var(--ember)" }));
      if (flying) layer.append(s("rect", { x: x0 + iceW + ownW - 1, y: y - 10, width: 3, height: 20, rx: 1.5, fill: "var(--gold)", opacity: 0.6 + 0.4 * Math.sin(t * 9) }));
      const susp = a.suspended.find(([from, to]) => t >= from && t < to);
      if (susp) {
        layer.append(s("rect", { x: x0 - 3, y: y - 10, width: iceW + ownW + 6, height: 20, rx: 5, fill: "none", stroke: "var(--mute)", class: "suspended" }));
        layer.append(s("text", { x: x0 + iceW + ownW + 12, y: y + 4, class: "tape-label", fill: "var(--mute)" }, "suspended"));
      } else if (t >= a.end) {
        layer.append(s("text", { x: x0 + iceW + ownW + 12, y: y + 4, class: "tape-label" }, a.parent ? `${fmt.tok(a.inherited)} cached · ${fmt.tok(len - a.inherited)} own` : `${fmt.tok(len)} · done`));
      } else if (a.parent) {
        layer.append(s("text", { x: x0 + iceW + ownW + 12, y: y + 4, class: "tape-label", fill: "var(--ice)" }, `${fmt.tok(a.inherited)} from cache`));
      }
      // the fork: the child's cached prefix sits exactly under the parent's same bytes
      if (a.parent) {
        const age = t - a.start;
        layer.append(s("line", { x1: x0 + a.inherited * scale, x2: x0 + a.inherited * scale, y1: rowY.get(a.parent) + 7, y2: y - 7, stroke: "var(--ice)", "stroke-width": 1, opacity: Math.max(0.12, 1 - age / 3) }));
        // its report flies home
        const flight = (t - a.end) / 1.6;
        if (flight > 0 && flight < 1) {
          const parent = agents.find(p => p.path === a.parent);
          const px = x0 + model.lengthAt(parent, t).len * scale, py = rowY.get(a.parent);
          const cx = x0 + a.final * scale, cy = y;
          const e = flight * flight * (3 - 2 * flight);
          layer.append(s("circle", { cx: cx + (px - cx) * e, cy: cy + (py - cy) * e - Math.sin(e * Math.PI) * 24, r: 4, fill: "var(--gold)" }));
        }
      }
    }
  };
  return { svg, draw, T, model };
}

function player(host, which, width, opts = {}) {
  const tp = tapes(which, width);
  const speed = opts.speed || 5;
  let t = 0, playing = false, last = 0, raf = 0;
  const btn = h("button", { class: "play", title: "play / pause" }, "▶");
  const clock = h("span", { class: "clock" }, "0.0s");
  const cost = h("span", { class: "money" }, "$0.000");
  const scrub = h("input", { type: "range", min: 0, max: tp.T, step: 0.05, value: 0, style: { flex: 1, accentColor: "var(--ice)" } });
  const frame = now => {
    if (playing) { t = Math.max(0, Math.min(tp.T + 2, t + (Math.max(0, now - last) / 1000) * speed)); last = now; if (t >= tp.T + 2) playing = false; }
    tp.draw(t);
    clock.textContent = `${Math.min(t, tp.T).toFixed(1)}s`;
    cost.textContent = fmt.usd(tp.model.spentAt(t));
    scrub.value = t;
    btn.textContent = playing ? "❚❚" : t >= tp.T ? "↺" : "▶";
    if (playing) raf = requestAnimationFrame(frame);
  };
  const play = () => { if (t >= tp.T) t = 0; playing = true; last = performance.now(); cancelAnimationFrame(raf); raf = requestAnimationFrame(frame); };
  btn.onclick = () => (playing ? (playing = false) : play());
  scrub.oninput = () => { playing = false; t = +scrub.value; frame(performance.now()); };
  host.append(h("div", { class: "watch-top" }, btn, clock, cost, scrub, h("span", { class: "tape-label mono", style: { color: "var(--mute)", fontSize: "11px" } }, `real time ÷ ${speed}`)), tp.svg);
  tp.draw(0);
  return { play, stop: () => (playing = false), set: x => { t = Math.min(x, tp.T + 2); frame(performance.now()); } };
}

// --- the numbers the heart quotes, computed from the trials --------------------
function share(list, a, b) {
  const x = list.reduce((s, t) => s + t[a], 0), y = list.reduce((s, t) => s + t[b], 0);
  return x + y ? x / (x + y) : 0;
}
const STAT = (() => {
  const fork = valid.filter(t => t.mode === "fork"), fresh = valid.filter(t => t.mode === "fresh");
  const leaves = list => [list.reduce((s, t) => s + t.leaf_overreach, 0), list.reduce((s, t) => s + t.leaves, 0)];
  const sonnet = valid.filter(t => short(t.model) === "sonnet" && t.mode === "fork");
  return {
    forkFirst: share(fork, "child_first_cached_in", "child_first_uncached_in"),
    freshFirst: share(fresh, "child_first_cached_in", "child_first_uncached_in"),
    toolTail: leaves(sonnet.filter(t => t.cut !== "before")),
    userTail: leaves(sonnet.filter(t => t.cut === "before")),
    toolTrees: [sonnet.filter(t => t.cut !== "before" && t.structure_ok).length, sonnet.filter(t => t.cut !== "before").length],
    userTrees: [sonnet.filter(t => t.cut === "before" && t.structure_ok).length, sonnet.filter(t => t.cut === "before").length],
  };
})();

view("heart", "heart", el => {
  const left = h("div", { class: "pane" });
  const right = h("div", { class: "pane" });
  el.append(
    h("h1", {}, "Agents as ", h("em", {}, "structured concurrency")),
    h("p", { class: "lede" }, "A ", h("span", { class: "mono" }, "task"), " call suspends the parent, runs its children at once, and returns one result. A forked child begins as its parent's exact bytes — served from cache — plus one message telling it who it is."),
    h("div", { class: "heart" }, left, right));

  left.append(h("div", { class: "caption" }, "One real run, replayed"),
    h("div", { class: "sub" }, "sonnet · 9 agents · every bar is that agent's context, in tokens. ",
      h("span", { class: "swatch", style: { background: "var(--ice)" } }), " read from cache  ", h("span", { class: "swatch", style: { background: "var(--ember)" } }), " its own ",
      h("span", { class: "link", style: { marginLeft: "8px" }, onclick: () => show("watch") }, "request by request →")));
  const p = player(left, "clean", Math.max(560, Math.min(820, innerWidth * 0.52)), { speed: 5 });
  VIEWS.find(v => v.id === "heart").onShow = () => (STILL ? p.set(1e9) : setTimeout(p.play, 400));

  const runaway = trialById(DATA.featured.runaway), clean = trialById(DATA.featured.clean);
  const card = (trial, sample, k, verdict, colour) => h("div", { class: "card", onclick: () => openTrial(trial) },
    h("div", { class: "k" }, k),
    h("div", { class: "verdict", style: { color: colour } }, verdict),
    tailRows(sample.tail),
    drawTree(trial, 300, 150, { r: 5, labels: 9, pad: 22 }));
  right.append(
    h("div", { class: "caption" }, "Who does the child think it is?"),
    h("div", { class: "sub" }, "Same model, same task. Only the last message a region agent was shown differs:"),
    h("div", { class: "duo" },
      card(runaway, DATA.samples.full, "its own task call, then a tool result", "It became the parent — every region re-split the whole job", "#ff9db5"),
      card(clean, DATA.samples.before, "a user message", "Two regions, six leaves — as asked", "var(--tool)")),
    h("div", { class: "caption", style: { marginTop: "26px", fontSize: "18px" } }, "What the region agents said instead"),
    h("div", { class: "sub" }, "Handed their assignment as the result of a task call, sonnet's regions read it as their own call coming back broken:"),
    h("div", { class: "quotes" }, voices(t => short(t.model) === "sonnet" && t.mode === "fork" && t.cut !== "before").slice(0, 4).map(v => quote(v))),
    h("div", { class: "stats" },
      tipOn(h("div", { class: "stat", onclick: () => show("money") }, h("div", { class: "n" }, fmt.pct(STAT.forkFirst)), h("div", { class: "l" }, "of a forked child's first request read from cache")), "every valid fork trial, both models; fresh children: " + fmt.pct(STAT.freshFirst)),
      tipOn(h("div", { class: "stat", onclick: () => show("who") }, h("div", { class: "n" }, `${STAT.toolTail[0]}`, h("small", {}, ` / ${STAT.toolTail[1]}`)), h("div", { class: "l" }, "sonnet leaves over-reached — tool-result tail")), `trees built as asked: ${STAT.toolTrees[0]} of ${STAT.toolTrees[1]}`),
      tipOn(h("div", { class: "stat", onclick: () => show("who") }, h("div", { class: "n" }, `${STAT.userTail[0]}`, h("small", {}, ` / ${STAT.userTail[1]}`)), h("div", { class: "l" }, "— user-message tail")), `trees built as asked: ${STAT.userTrees[0]} of ${STAT.userTrees[1]}`)));
});
