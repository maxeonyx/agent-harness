// Drill-downs: a trial, an agent, a request, a message. Everything here is read straight off the recorded runs.

const TRIALS = DATA.trials;
const trialById = id => TRIALS.find(t => t.id === id);
const valid = TRIALS.filter(t => t.valid);
const rowOf = t => (t.mode === "fresh" ? "fresh" : t.cut);

// What region agents said instead of reporting numbers: their own account of who they thought they were.
function voices(filter) {
  const out = [];
  for (const t of valid.filter(filter)) {
    for (const a of t.agents) {
      if (a.path.split(" › ").length === 2 && a.handoff && !/\d+\.\d\d/.test(a.handoff)) out.push({ trial: t, agent: a, text: a.handoff.trim() });
    }
  }
  return out;
}
function quote(v, opts = {}) {
  return h("div", { class: "quote", onclick: () => openTrial(v.trial, v.agent.path) },
    h("div", { class: "q" }, "“" + v.text + "”"),
    h("div", { class: "who" }, `${short(v.trial.model)} · ${v.agent.path.split(" › ").pop()} · ${v.trial.cut} cut · ${v.trial.words} words`));
}

function snippet(m) {
  if (m.tool_calls && m.tool_calls.length) {
    return m.tool_calls.map(c => {
      let args = c.function.arguments;
      try {
        const a = JSON.parse(args);
        if (c.function.name === "task") args = "agents: [" + (a.agents || []).map(x => x.name).join(", ") + "]";
        else args = Object.values(a).join(", ");
      } catch {}
      return `${c.function.name}(${args})`;
    }).join("  ");
  }
  return String(m.content ?? "").replace(/\s+/g, " ").trim();
}
const msgLength = m => String(m.content ?? "").length + (m.tool_calls ? JSON.stringify(m.tool_calls).length : 0);

function messageText(m) {
  let text = String(m.content ?? "");
  if (m.tool_calls) {
    for (const c of m.tool_calls) {
      let args = c.function.arguments;
      try { args = JSON.stringify(JSON.parse(args), null, 2); } catch {}
      text += (text ? "\n\n" : "") + `→ ${c.function.name}(${args})`;
    }
  }
  return text;
}

function tailRows(messages, opts = {}) {
  return h("div", { class: "tail" }, messages.map(m => h("div", { class: "msg" + (opts.ghost && opts.ghost(m) ? " ghost" : "") },
    h("span", {}, h("span", { class: "role " + m.role }, m.role)),
    h("span", { class: "txt", title: snippet(m) }, snippet(m)))));
}

function kv(pairs) {
  return h("div", { class: "kv" }, pairs.filter(Boolean).map(([k, v]) => h("span", {}, k + " ", h("b", {}, v))));
}

function collapsible(label, text, open) {
  const pre = h("pre", { class: "text", style: { display: open ? "block" : "none" } }, text);
  const head = h("div", { class: "link mono", style: { fontSize: "12px", margin: "6px 0" }, onclick: () => { pre.style.display = pre.style.display === "none" ? "block" : "none"; } }, label);
  return [head, pre];
}

function openTrial(trial, focus) {
  const cards = [];
  const walk = node => { cards.push(agentCard(trial, node)); node.kids.forEach(walk); };
  const { root } = treeOf(trial);
  walk(root);
  const big = drawTree(trial, 860, 230, { r: 7, labels: 11, pad: 28, edge: 1.6, onNode: node => document.getElementById("card-" + cssId(node.path))?.scrollIntoView({ behavior: "smooth", block: "start" }) });
  const featured = Object.entries(DATA.featured).find(([, id]) => id === trial.id);
  openDrawer(`${short(trial.model)} · ${rowOf(trial)}${trial.mode === "fresh" ? "" : " cut"} · ${trial.words} words`,
    kv([
      ["outcome", trial.valid ? trial.outcome : "INVALID — " + (trial.fault_kind || "") + " fault"],
      ["tree as asked", trial.structure_ok ? "yes" : "no"],
      ["over-reached", `${trial.overreached.length}`],
      ["totals", trial.correct ? "correct" : "wrong"],
      ["cost", fmt.usd(trial.cost)],
      ["wall", fmt.sec(trial.millis)],
    ]),
    trial.detail && trial.outcome !== "completed" ? h("div", { class: "mono", style: { fontSize: "12px", color: "var(--ember)", margin: "8px 0" } }, trial.detail) : null,
    featured ? h("div", { style: { margin: "10px 0" } }, h("span", { class: "chip click", onclick: () => { closeDrawer(); show("watch"); WATCH.pick(featured[0]); } }, "▶ replay this run, request by request")) : null,
    h("div", { style: { margin: "12px 0", background: "#0a0d13", borderRadius: "12px" } }, big),
    h("div", { class: "legend" }, legendTree()),
    cards,
    h("h4", {}, "root's final answer"), h("pre", { class: "text" }, trial.root_handoff || "(none — the root never resumed)"),
    h("div", { class: "mono", style: { fontSize: "11px", color: "var(--faint)", marginTop: "14px" } }, `runs.ignore/${trial.bench}/${trial.id}`));
  if (focus) setTimeout(() => document.getElementById("card-" + cssId(focus))?.scrollIntoView({ block: "start" }), 50);
}
const cssId = path => path.replace(/[^a-z0-9]+/gi, "-");

function legendTree() {
  return [
    h("span", {}, h("i", { class: "swatch", style: { background: "var(--maunga)", borderRadius: "50%" } }), "maunga (mountain) and its branches"),
    h("span", {}, h("i", { class: "swatch", style: { background: "var(--awa)", borderRadius: "50%" } }), "awa (river) and its branches"),
    h("span", {}, h("i", { class: "swatch", style: { background: "var(--bad)", borderRadius: "50%" } }), "over-reached"),
    h("span", {}, h("i", { class: "swatch", style: { border: "1.5px dashed var(--mute)", borderRadius: "50%" } }), "never resumed"),
  ];
}

function agentCard(trial, node) {
  const a = node.agent;
  const reads = a.tools.filter(t => t.name === "read_file");
  return h("div", { id: "card-" + cssId(node.path), style: { marginTop: "18px", padding: "12px 14px", borderRadius: "12px", background: "#121622", boxShadow: node.over ? "0 0 0 1px #7a2440 inset" : "0 0 0 1px var(--line) inset", marginLeft: node.depth * 18 + "px" } },
    h("div", { style: { display: "flex", gap: "10px", alignItems: "center" } },
      h("i", { class: "swatch", style: { background: HUE[lineage(node.name)], borderRadius: "50%" } }),
      h("b", { class: "mono", style: { color: "var(--bright)" } }, node.name),
      h("span", { class: "chip" }, a.state),
      node.over ? h("span", { class: "chip", style: { background: "#3a1020", color: "#ff9db5" } }, "over-reached") : null,
      node.unscored ? h("span", { class: "chip" }, "unscoreable") : null,
      a.fresh ? h("span", { class: "chip" }, "fresh") : h("span", { class: "chip", style: { color: "var(--ice)" } }, "forked")),
    h("div", { style: { margin: "8px 0" } }, kv([["requests", a.requests], ["cached", fmt.int(a.cached)], ["new", fmt.int(a.uncached)], ["out", fmt.int(a.out)], ["cost", fmt.usd(a.cost)], ["time", fmt.sec(a.millis)]])),
    a.task ? collapsible("what its parent asked ▾", a.task, false) : null,
    a.tools.length ? h("div", { style: { margin: "6px 0" } }, a.tools.map(t => {
      let arg = t.args;
      try { const p = JSON.parse(t.args); arg = t.name === "task" ? (p.agents || []).map(x => x.name).join(", ") : Object.values(p).join(", "); } catch {}
      return tipOn(h("div", { class: "mono", style: { fontSize: "12px", color: t.name === "task" ? "var(--ice)" : t.name === "read_file" ? "var(--tool)" : "#c9cedb", padding: "1px 0" } }, `→ ${t.name}(${arg})`), (t.result || "").slice(0, 900));
    })) : null,
    collapsible("its report ▾", a.handoff || "(none)", node.depth > 0 && (a.handoff || "").length < 600));
}

// --- one request, exactly as sent -------------------------------------------
function openRequest(which, index) {
  const w = DATA.wire[which];
  const r = w.requests[index];
  const parentPath = r.agent.split(" › ").slice(0, -1).join(" › ");
  const parentIds = new Set(w.requests.filter(q => q.agent === parentPath).flatMap(q => q.messages));
  const ownEarlier = new Set(w.requests.filter(q => q.agent === r.agent && q.sent < r.sent).flatMap(q => q.messages));
  const nth = w.requests.filter(q => q.agent === r.agent && q.sent <= r.sent).length;
  const rows = [];
  let section = null;
  r.messages.forEach((id, n) => {
    const m = w.messages[id];
    const kind = parentIds.has(id) ? "inherited" : ownEarlier.has(id) ? "earlier" : "new";
    if (kind !== section) {
      section = kind;
      const count = r.messages.slice(n).findIndex((x, k) => (parentIds.has(x) ? "inherited" : ownEarlier.has(x) ? "earlier" : "new") !== kind);
      const len = count < 0 ? r.messages.length - n : count;
      rows.push(h("div", { class: "sep" + (kind === "new" ? " new" : "") },
        kind === "inherited" ? `${len} messages — byte-identical to ${parentPath.split(" › ").pop()}'s own requests` : kind === "earlier" ? `${len} messages — from this agent's earlier requests` : `${len} new in this request`));
    }
    const full = h("div", { class: "full", style: { display: n >= r.messages.length - 2 ? "block" : "none" } }, h("pre", { class: "text" }, messageText(m)));
    rows.push(h("div", { class: "m" + (kind === "inherited" ? " inherited" : ""), onclick: () => { full.style.display = full.style.display === "none" ? "block" : "none"; } },
      h("span", { class: "bar", style: { background: kind === "new" ? "var(--ember)" : "var(--ice)", opacity: kind === "earlier" ? 0.55 : 1 } }),
      h("span", {}, h("span", { class: "role " + m.role }, m.role)),
      h("span", { class: "snip" }, snippet(m) || "…"),
      h("span", { class: "len" }, fmt.int(msgLength(m)) + " ch")), full);
  });
  const share = r.prompt ? r.cached / r.prompt : 0;
  openDrawer(`${r.agent} · request ${nth}`,
    kv([["sent", `+${r.sent.toFixed(1)}s`], ["answered", `+${r.returned.toFixed(1)}s`], ["via", r.via], ["cost", fmt.usd(r.cost)]]),
    h("h4", {}, "what the provider billed"),
    h("div", { style: { display: "flex", height: "18px", borderRadius: "5px", overflow: "hidden", background: "#121620" } },
      h("div", { style: { width: share * 100 + "%", background: "var(--ice)" } }),
      h("div", { style: { width: (r.written / r.prompt) * 100 + "%", background: "var(--ember)" } })),
    h("div", { class: "legend", style: { marginTop: "6px" } },
      h("span", {}, h("i", { class: "swatch", style: { background: "var(--ice)" } }), `${fmt.int(r.cached)} read from cache (0.1×)`),
      h("span", {}, h("i", { class: "swatch", style: { background: "var(--ember)" } }), `${fmt.int(r.written)} written to cache (1.25×)`),
      h("span", {}, h("i", { class: "swatch", style: { background: "var(--gold)" } }), `${fmt.int(r.out)} out`)),
    h("h4", {}, `the context as sent — ${r.messages.length} messages, then the tool list`),
    h("div", { class: "ctx" }, rows),
    h("h4", {}, "what came back"),
    r.reasoning ? collapsible("reasoning ▾", r.reasoning, false) : null,
    r.content ? h("pre", { class: "text" }, r.content) : null,
    r.calls.map(c => { let a = c.args; try { a = JSON.stringify(JSON.parse(a), null, 2); } catch {} return h("pre", { class: "text", style: { marginTop: "6px", color: c.name === "task" ? "var(--ice)" : "var(--tool)" } }, `→ ${c.name}(${a})`); }),
    h("div", { style: { marginTop: "14px" } }, "built by ", srcLink("child_context()", "src/agent.rs", fnLine("src/agent.rs", "child_context"), fnLine("src/agent.rs", "child_context") + 38), " · sent by ", srcLink("send_once()", "src/wire.rs", fnLine("src/wire.rs", "send_once"), fnLine("src/wire.rs", "send_once") + 30)));
}
