// Path: where this experiment sits — in the harness's process, in the pool of experiments, and in time — and what now waits on Max.

const PR = n => `${REPO}/pull/${n}`;
const WAITING = [
  { t: "Watch it, then drive it yourself", d: "The brief's exit condition: you run a task and watch the tree suspend, fan out and resume. Sonnet, pinned to Bedrock, about $0.10–0.40 a task.",
    code: "cd ~/at-workspace/at-forked-subagents/tools/agent-harness/experiments/forked-subagents\ncargo run --release --bin forks -- chat --dir ../walking-skeleton/src" },
  { t: "Gate 1 — accept, redo or discard", d: "The outcome and this overview are the evidence. Draft PR, stacked on #12.", link: [PR(13), "#13 forked-subagents"] },
  { t: "Review #12 first", d: "The design docs the brief quotes — your statements, verbatim, and the open questions. #13 is stacked on it.", link: [PR(12), "#12 design docs"] },
];

view("path", "path", el => {
  const STAGES = [
    { k: "your notes", d: "22 source notes", st: "done", open: () => openDoc("docs/source-notes/agent-hierarchy.md") },
    { k: "design aspects", d: "2 of 8 elicited", st: "part", open: () => openDoc("docs/design/lifecycle.md"), pr: 12 },
    { k: "brief", d: "the scope-down", st: "done", open: () => openDoc("docs/process/experiments/forked-subagents-brief.md") },
    { k: "probe", d: "on the wire", st: "done", open: () => openDoc("probe/runs.md") },
    { k: "build", d: "35 scenarios", st: "done", open: () => show("machine") },
    { k: "review", d: "fresh context", st: "done", open: () => show("proof") },
    { k: "grid", d: `${TRIALS.length} trials`, st: "done", open: () => show("trials") },
    { k: "outcome", d: "the write-up", st: "done", open: () => openDoc("docs/process/experiments/forked-subagents-outcome.md"), pr: 13 },
    { k: "Gate 1", d: "you", st: "wait", open: () => document.getElementById("waits").scrollIntoView({ behavior: "smooth", block: "center" }) },
    { k: "core", d: "from evidence", st: "later" },
  ];
  const W = 1240, H = 170, gap = (W - 120) / (STAGES.length - 1);
  const svg = s("svg", { width: W, height: H, viewBox: `0 0 ${W} ${H}` });
  svg.append(s("line", { x1: 60, x2: 60 + gap * (STAGES.length - 2), y1: 60, y2: 60, stroke: "#394054", "stroke-width": 2 }));
  svg.append(s("line", { x1: 60 + gap * (STAGES.length - 2), x2: W - 60, y1: 60, y2: 60, stroke: "#394054", "stroke-width": 2, "stroke-dasharray": "4 5" }));
  STAGES.forEach((st, i) => {
    const x = 60 + i * gap;
    const colour = st.st === "wait" ? "var(--wait)" : st.st === "later" ? "#394054" : st.st === "part" ? "var(--gold)" : "var(--ice)";
    const g = s("g", { class: "stage", style: { cursor: st.open ? "pointer" : "default" } });
    if (st.st === "wait") g.append(s("circle", { cx: x, cy: 60, r: 16, fill: "none", stroke: "var(--wait)", opacity: 0.5 }, s("animate", { attributeName: "r", values: "11;22;11", dur: "1.8s", repeatCount: "indefinite" }), s("animate", { attributeName: "opacity", values: ".7;0;.7", dur: "1.8s", repeatCount: "indefinite" })));
    g.append(s("circle", { cx: x, cy: 60, r: st.st === "wait" ? 10 : 8, fill: st.st === "later" ? "#0a0c11" : colour, stroke: colour, "stroke-width": 2 }));
    g.append(s("text", { x, y: 100, "text-anchor": "middle", fill: st.st === "wait" ? "#ffd4ea" : "var(--bright)", "font-size": 13, "font-weight": 500 }, st.k));
    g.append(s("text", { x, y: 118, "text-anchor": "middle", class: "tape-label" }, st.d));
    if (st.pr) {
      const pr = s("text", { x, y: 136, "text-anchor": "middle", class: "tape-label", fill: "var(--gold)", style: { cursor: "pointer", textDecoration: "underline" } }, `draft #${st.pr} ↗`);
      pr.addEventListener("click", e => { e.stopPropagation(); open(PR(st.pr), "_blank"); });
      g.append(pr);
    }
    if (st.open) g.addEventListener("click", st.open);
    svg.append(g);
  });

  // time: every commit on this branch
  const commits = DATA.commits;
  const days = [...new Set(commits.map(c => c.date.slice(0, 10)))];
  const strip = s("svg", { width: W, height: 90, viewBox: `0 0 ${W} 90` });
  const perDay = (W - 160) / days.length;
  days.forEach((day, i) => {
    const x = 80 + i * perDay;
    strip.append(s("text", { x, y: 78, class: "tape-label" }, new Date(day + "T12:00").toLocaleDateString("en-NZ", { weekday: "short", day: "numeric", month: "short" })));
    strip.append(s("line", { x1: x, x2: x + perDay - 30, y1: 40, y2: 40, stroke: "#1f2533" }));
    const today = commits.filter(c => c.date.startsWith(day));
    today.forEach((c, k) => {
      const cx = x + (k + 0.5) * ((perDay - 30) / today.length);
      const design = /design|elicit|statement|context-updates|AGENTS/i.test(c.subject) && day < "2026-09-20";
      const dot = s("circle", { cx, cy: 40, r: 6, fill: design ? "var(--user)" : "var(--ice)", style: { cursor: "pointer" } });
      tipOn(dot, `${new Date(c.date).toLocaleString("en-NZ", { hour: "2-digit", minute: "2-digit", day: "numeric", month: "short" })}\n${c.subject}\nclick: the commit on GitHub`);
      dot.addEventListener("click", () => open(`${REPO}/commit/${c.sha}`, "_blank"));
      strip.append(dot);
    });
  });

  const POOL = [
    ["walking-skeleton", "done", "the substrate"], ["forked-subagents", "here", "this"], ["provider-cache-probe", "built", "draft #10, never run"],
    ["user-turn", "pool", "soul"], ["limb-model", "pool", "soul"], ["compaction-handover", "pool", "soul"],
    ["context-updates", "pool", "taste"], ["topology", "pool", "taste"], ["persistence-analytics", "pool", "taste"], ["self-modification", "pool", "taste"],
    ["modular-components", "pool", "taste"], ["multi-client-ui", "pool", "taste"], ["operator-lifecycle", "pool", "taste"],
  ];
  const pool = h("div", { style: { display: "flex", flexWrap: "wrap", gap: "8px" } }, POOL.map(([name, st, note]) => h("span", {
    class: "chip click",
    onclick: st === "built" ? () => open(PR(10), "_blank") : () => openDoc("docs/process/PLAN.md", name),
    style: {
      background: st === "here" ? "#1c2a33" : st === "done" ? "#16241c" : "#151925",
      color: st === "here" ? "var(--ice)" : st === "done" ? "var(--tool)" : st === "built" ? "var(--gold)" : "var(--mute)",
      boxShadow: st === "here" ? "0 0 0 1px var(--ice) inset" : "none", padding: "5px 12px",
    },
  }, name, h("small", { style: { opacity: 0.6 } }, " " + note))));

  el.append(h("h1", {}, "Where this ", h("em", {}, "fits")),
    h("p", { class: "lede" }, "Your process: statements on every design aspect, an explicit scope-down, then the experiment — and core built later, fresh, from the evidence. This is the first experiment to walk that path. Click any stage."),
    svg,
    h("h2", {}, "waiting on you"),
    h("div", { class: "waits", id: "waits" }, WAITING.map((w, i) => h("div", { class: "wait" },
      h("div", { class: "n" }, String(i + 1).padStart(2, "0")),
      h("div", { class: "t" }, w.t), h("div", { class: "d" }, w.d),
      w.code ? h("code", {}, w.code) : null,
      w.link ? h("div", { style: { marginTop: "8px" } }, h("a", { class: "link", href: w.link[0], target: "_blank" }, w.link[1] + " ↗")) : null))),
    h("p", { class: "lede", style: { marginTop: "14px", fontSize: "12.5px" } }, "Spent on your OpenRouter harness key: about $5.10 — over the $3 I set myself. $4.83 is left on it."),
    h("h2", {}, "your open questions this bears on"), questions(),
    h("h2", {}, "choices I made without exploring the alternatives"),
    h("div", { class: "claims", style: { columns: "2 460px", columnGap: "28px" } }, UNEXPLORED.map(t => h("div", { class: "claim", style: { cursor: "default" } }, h("i", { class: "unknown" }), h("span", {}, t)))),
    h("h2", {}, "the pool of experiments"), pool,
    h("h2", {}, "every commit on this branch"),
    h("div", { class: "legend", style: { marginBottom: "4px" } }, h("span", {}, h("i", { class: "swatch", style: { background: "var(--user)", borderRadius: "50%" } }), "design docs, #12"), h("span", {}, h("i", { class: "swatch", style: { background: "var(--ice)", borderRadius: "50%" } }), "this experiment, #13")),
    strip);
});

const UNEXPLORED = [
  "The scope-down in the brief is mine, written from your statements; you never saw it before the build.",
  "The benchmark tells the root the tree to build. Real tasks won't, so it measures discipline, not routing.",
  "The two word variants were written once and never iterated.",
  "The depth-limit refusal says \"Do this work yourself.\" — chosen once. All three leaves that hit it then read their whole region's ledgers.",
  "OpenRouter only, pinned to Bedrock and Azure because the account is zero-data-retention.",
  "Rust and tokio, following the walking skeleton and your tech note; no Deno half.",
  "The $0.15 per-trial cap was a guess. It cut all eight first-grid Sonnet before-and-fresh trees short.",
];

function questions() {
  const doc = path => DATA.sources[path];
  const q = (path, n) => {
    const m = doc(path).match(new RegExp("^" + n + "\\. \\*\\*(.+?)\\*\\*", "m"));
    return m ? m[1] : "?";
  };
  const MF = "docs/design/model-framing.md", LC = "docs/design/lifecycle.md";
  const ITEMS = [
    [MF, 5, "the cut: a user-message tail kept Sonnet's leaves in lane (3 of 33); a tool-result tail didn't (19 of 26). Luna was clean either way.", "who"],
    [MF, 1, "of the mechanisms your notes name, only the task text and the child's tail were varied; the tail mattered more than the words.", "who"],
    [MF, 4, "not tested: every variant told the child its name; none left it to work out that it was a child.", null],
    [MF, 8, "an in-band failure is the child's own report; an out-of-band one faults the agent and never becomes a tool result.", "proof"],
    [LC, 1, "built your L1 distinction: in-band completes, out-of-band leaves every ancestor suspended.", "proof"],
    [LC, 6, "built the settled case only: `after` fixed at launch. Re-wiring after launch is not built.", "proof"],
  ];
  return h("div", { class: "waits" }, ITEMS.map(([path, n, evidence, go]) => h("div", { class: "word", style: { marginBottom: 0 } },
    h("div", { class: "meta" }, (path === MF ? "model framing" : "lifecycle") + " · open question " + n),
    h("div", { style: { color: "var(--bright)", fontSize: "14px", marginBottom: "8px", cursor: "pointer" }, onclick: () => openDoc(path) }, q(path, n).replace(/`/g, "")),
    h("div", { style: { fontSize: "13px", color: "#aeb5c6" } }, evidence.replace(/`/g, ""), " ", go ? h("span", { class: "link", onclick: () => show(go) }, "→") : null))));
}
