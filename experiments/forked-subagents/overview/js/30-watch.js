// Watch: the same run on a clock — every request as a bar, suspensions, forks and reports as they happened.

const WATCH = { pick: () => {} };

function timeline(which, width) {
  const w = DATA.wire[which];
  const order = agentsInOrder(w.requests);
  const T = Math.max(...w.requests.map(r => r.returned));
  const x0 = 190, laneH = 30, top = 30;
  const X = t => x0 + (t / T) * (width - x0 - 30);
  const height = top + order.length * laneH + 44;
  const svg = s("svg", { width, height, viewBox: `0 0 ${width} ${height}` });
  svg.append(s("defs", {}, hatch("ice-w-" + which, "var(--ice)")));
  const Y = new Map(order.map((p, i) => [p, top + i * laneH + laneH / 2]));
  for (let k = 0; k <= T; k += 10) {
    svg.append(s("line", { x1: X(k), x2: X(k), y1: top - 10, y2: height - 34, stroke: "#171b26" }));
    svg.append(s("text", { x: X(k), y: height - 18, "text-anchor": "middle", class: "tape-label" }, k + "s"));
  }
  const dyn = s("g");
  const stat = s("g");
  svg.append(stat, dyn);
  order.forEach(path => {
    const depth = path.split(" › ").length - 1, name = path.split(" › ").pop();
    const lab = s("text", { x: 12 + depth * 16, y: Y.get(path) + 4, class: "lane-label", fill: HUE[lineage(name)] }, name);
    lab.addEventListener("click", () => openTrial(trialById(DATA.featured[which]), path));
    stat.append(s("line", { x1: x0, x2: width - 30, y1: Y.get(path), y2: Y.get(path), stroke: "#141823" }), lab);
  });
  // forks and reports: curves between lanes
  const byAgent = p => w.requests.filter(r => r.agent === p).sort((a, b) => a.sent - b.sent);
  const links = [];
  for (const path of order) {
    const reqs = byAgent(path);
    reqs.forEach((r, i) => {
      if (!r.calls.some(c => c.name === "task")) return;
      const kids = order.filter(q => q.startsWith(path + " › ") && q.split(" › ").length === path.split(" › ").length + 1);
      const resume = reqs[i + 1];
      if (resume) links.push({ kind: "suspend", path, from: r.returned, to: resume.sent });
      for (const k of kids) {
        const kr = byAgent(k);
        if (!kr.length) continue;
        links.push({ kind: "fork", from: [r.returned, Y.get(path)], to: [kr[0].sent, Y.get(k)] });
        if (resume) links.push({ kind: "report", from: [kr[kr.length - 1].returned, Y.get(k)], to: [resume.sent, Y.get(path)] });
      }
    });
  }
  const curve = ([ax, ay], [bx, by]) => `M${X(ax)} ${ay} C ${X(ax) + 18} ${ay}, ${X(bx) - 18} ${by}, ${X(bx)} ${by}`;

  const draw = t => {
    dyn.replaceChildren();
    for (const l of links) {
      if (l.kind === "suspend") {
        if (t < l.from) continue;
        const end = Math.min(t, l.to);
        dyn.append(s("rect", { x: X(l.from), y: Y.get(l.path) - 8, width: Math.max(0, X(end) - X(l.from)), height: 16, rx: 4, fill: "none", stroke: "var(--mute)", class: "suspended" }));
        if (end - l.from > T * 0.12) dyn.append(s("text", { x: (X(l.from) + X(end)) / 2, y: Y.get(l.path) + 4, "text-anchor": "middle", class: "tape-label" }, "suspended — inside its task call"));
      } else if (t >= l.from[0]) {
        dyn.append(s("path", { d: curve(l.from, l.to), fill: "none", stroke: l.kind === "fork" ? "var(--ice)" : "var(--gold)", "stroke-width": 1.2, opacity: t >= l.to[0] ? 0.55 : 0.25, "stroke-dasharray": t >= l.to[0] ? null : "3 3" }));
      }
    }
    w.requests.forEach((r, index) => {
      if (t < r.sent) return;
      const y = Y.get(r.agent);
      const end = Math.min(t, r.returned);
      const full = X(r.returned) - X(r.sent), wNow = Math.max(2, X(end) - X(r.sent));
      const g = s("g", { class: "bar" });
      const iceShare = r.prompt ? r.cached / r.prompt : 0;
      g.append(s("rect", { x: X(r.sent), y: y - 7, width: wNow * iceShare, height: 14, fill: `url(#ice-w-${which})`, rx: 2 }));
      g.append(s("rect", { x: X(r.sent) + wNow * iceShare, y: y - 7, width: wNow * (1 - iceShare), height: 14, fill: "var(--ember)", rx: 2 }));
      g.append(s("rect", { class: "outline", x: X(r.sent), y: y - 7, width: Math.max(2, full), height: 14, rx: 3, fill: "none", stroke: t < r.returned ? "var(--gold)" : "transparent", "stroke-width": 1.2 }));
      if (t >= r.returned) {
        const tools = r.calls.filter(c => c.name !== "task");
        tools.forEach((c, k) => g.append(s("path", { d: `M${X(r.returned) + 5 + k * 7} ${y - 4} l3.5 4 -3.5 4 -3.5 -4z`, fill: "var(--tool)" })));
        if (r.calls.some(c => c.name === "task")) g.append(s("circle", { cx: X(r.returned) + 4, cy: y, r: 3.5, fill: "var(--ice)" }));
      }
      tipOn(g, () => `${r.agent}\n${fmt.int(r.prompt)} in — ${fmt.int(r.cached)} from cache, ${fmt.int(r.written)} written\n${fmt.int(r.out)} out · ${fmt.usd(r.cost)} · ${(r.returned - r.sent).toFixed(1)}s\n${r.calls.map(c => "→ " + c.name).join("  ") || "(its report)"}\nclick: the exact request`);
      g.addEventListener("click", () => openRequest(which, index));
      dyn.append(g);
    });
    dyn.append(s("line", { x1: X(Math.min(t, T)), x2: X(Math.min(t, T)), y1: top - 14, y2: height - 34, stroke: "var(--gold)", opacity: t < T ? 0.7 : 0 }));
  };
  return { svg, draw, T, spentAt: t => w.requests.filter(r => r.returned <= t).reduce((x, r) => x + r.cost, 0) };
}

view("watch", "watch", el => {
  const host = h("div");
  const runs = {
    clean: { label: "sonnet · user-message tail", note: "two regions, six leaves, correct totals · " },
    runaway: { label: "sonnet · tool-result tail", note: "every region re-launched both regions; stopped at its $0.15 cap · " },
  };
  const seg = h("div", { class: "seg" });
  const note = h("span", { class: "tape-label mono", style: { color: "var(--mute)", fontSize: "12px" } });
  el.append(h("h1", {}, "Watch it ", h("em", {}, "run")),
    h("p", { class: "lede" }, "Every bar is one request: its width is how long the provider took, its colour is how much of the prompt came from cache. Click any bar for the exact messages sent."),
    h("div", { class: "watch-top" }, seg, note),
    h("div", { class: "legend", style: { margin: "6px 0 10px" } },
      h("span", {}, h("i", { class: "swatch", style: { background: "var(--ice)" } }), "prompt read from cache"),
      h("span", {}, h("i", { class: "swatch", style: { background: "var(--ember)" } }), "prompt paid in full"),
      h("span", {}, h("i", { class: "swatch", style: { background: "var(--tool)", transform: "rotate(45deg)" } }), "tool call"),
      h("span", {}, h("i", { class: "swatch", style: { background: "var(--ice)", borderRadius: "50%" } }), "task call → fork"),
      h("span", {}, h("i", { class: "swatch", style: { background: "var(--gold)" } }), "report back")),
    host);
  let tl, t = 0, playing = false, last = 0;
  const btn = h("button", { class: "play" }, "▶");
  const clock = h("span", { class: "clock" });
  const cost = h("span", { class: "money" });
  const scrub = h("input", { type: "range", min: 0, step: 0.05, style: { flex: 1, accentColor: "var(--gold)" } });
  const frame = now => {
    if (playing) { t = Math.max(0, Math.min(tl.T, t + (Math.max(0, now - last) / 1000) * 4)); last = now; if (t >= tl.T) playing = false; }
    tl.draw(t);
    clock.textContent = t.toFixed(1) + "s";
    cost.textContent = fmt.usd(tl.spentAt(t));
    scrub.value = t;
    btn.textContent = playing ? "❚❚" : t >= tl.T ? "↺" : "▶";
    if (playing) requestAnimationFrame(frame);
  };
  const play = () => { if (t >= tl.T) t = 0; playing = true; last = performance.now(); requestAnimationFrame(frame); };
  btn.onclick = () => (playing ? (playing = false) : play());
  scrub.oninput = () => { playing = false; t = +scrub.value; frame(0); };
  const pick = which => {
    seg.querySelectorAll("button").forEach(b => b.classList.toggle("on", b.dataset.w === which));
    const trial = trialById(DATA.featured[which]);
    note.replaceChildren(runs[which].note, h("span", { class: "link", onclick: () => openTrial(trial) }, "the tree →"));
    tl = timeline(which, Math.max(900, el.clientWidth - 90));
    scrub.max = tl.T;
    host.replaceChildren(h("div", { class: "watch-top" }, btn, clock, cost, scrub, h("span", { class: "tape-label mono", style: { color: "var(--mute)", fontSize: "11px" } }, "real time ÷ 4")), tl.svg);
    t = tl.T; frame(0);
  };
  for (const [which, r] of Object.entries(runs)) seg.append(h("button", { "data-w": which, onclick: () => pick(which) }, r.label));
  WATCH.pick = pick;
  pick("clean");
});
