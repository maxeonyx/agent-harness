// Who am I: the four ways a child can be shown its assignment, what each looked like on the wire, and what every leaf then did.

const CUTS = [
  { id: "full", title: "full", how: "the parent's task call, then a tool result addressed to this child", voice: "tool" },
  { id: "own", title: "own", how: "the same, but its copy of the call lists only its own entry", voice: "tool" },
  { id: "before", title: "before", how: "the parent's context before the call, then a user message", voice: "user" },
  { id: "fresh", title: "fresh", how: "no parent context at all: the system prompt, then a user message", voice: "user" },
];

function leafDots(trials) {
  const box = h("div", { style: { display: "flex", flexWrap: "wrap", gap: "8px" } });
  for (const t of trials) {
    const { nodes } = treeOf(t);
    const leaves = [...nodes.values()].filter(n => n.depth === 2);
    const cluster = h("div", { style: { display: "flex", gap: "3px", padding: "5px 6px", borderRadius: "7px", background: "#0d1017", cursor: "pointer", boxShadow: t.structure_ok ? "0 0 0 1px #2f5a41 inset" : "0 0 0 1px #4a2030 inset" }, onclick: () => openTrial(t) });
    if (!leaves.length) cluster.append(h("span", { class: "mono", style: { fontSize: "10px", color: "var(--faint)" } }, "no leaves"));
    for (const n of leaves) cluster.append(h("i", { style: { width: "9px", height: "9px", borderRadius: "50%", background: n.over ? "var(--bad)" : n.unscored ? "transparent" : HUE[lineage(n.name)], boxShadow: n.unscored ? "0 0 0 1px var(--mute) inset" : n.over ? "0 0 6px var(--bad)" : "none" } }));
    tipOn(cluster, `${t.words} words · ${t.outcome}\n${t.leaf_overreach} of ${t.leaves} leaves over-reached\ntree as asked: ${t.structure_ok ? "yes" : "no"} · totals ${t.correct ? "correct" : "wrong"} · ${fmt.usd(t.cost)}\nclick: open the tree`);
    box.append(cluster);
  }
  return box;
}

view("who", "who am I", el => {
  el.append(h("h1", {}, "Who does the child ", h("em", {}, "think it is?")),
    h("p", { class: "lede" }, "Max's note: \"it is given task A, then told to only focus on A.1, then told to only focus on A.1.3 … It must end its turn after A.1.3.\" The benchmark builds exactly that — root → region → leaf — and varies only how the assignment reaches the child. Each cluster is one trial; each dot one leaf."));
  const grid = h("div", { style: { display: "grid", gridTemplateColumns: "repeat(4, minmax(250px, 1fr))", gap: "18px" } });
  for (const cut of CUTS) {
    const sample = DATA.samples[cut.id];
    const trialsFor = model => valid.filter(t => short(t.model) === model && rowOf(t) === cut.id);
    const agg = list => [list.reduce((a, t) => a + t.leaf_overreach, 0), list.reduce((a, t) => a + t.leaves, 0), list.filter(t => t.structure_ok).length, list.length];
    const block = model => {
      const list = trialsFor(model);
      const [o, n, ok, total] = agg(list);
      return h("div", { style: { marginTop: "14px" } },
        h("div", { style: { display: "flex", alignItems: "baseline", gap: "10px", marginBottom: "6px" } },
          h("span", { class: "mono", style: { color: "var(--bright)", fontSize: "12px" } }, model),
          h("span", { class: "mono", style: { fontSize: "12px", color: o ? "#ff9db5" : "var(--tool)" } }, `${o}/${n} over-reached`),
          h("span", { class: "mono", style: { fontSize: "11px", color: "var(--mute)" } }, `· ${ok}/${total} trees as asked`)),
        leafDots(list));
    };
    const shown = sample.tail.slice(-2);
    grid.append(h("div", { style: { background: "var(--panel)", borderRadius: "14px", padding: "16px", boxShadow: cut.voice === "user" ? "0 0 0 1px #3d2f63 inset" : "0 0 0 1px #3b4130 inset" } },
      h("div", { style: { display: "flex", alignItems: "center", gap: "10px" } },
        h("span", { class: "mono", style: { font: "600 18px var(--mono)", color: "var(--bright)" } }, cut.title),
        h("span", { class: "role " + cut.voice }, cut.voice + " says who it is")),
      h("div", { style: { color: "var(--mute)", fontSize: "12.5px", margin: "6px 0 12px", minHeight: "36px" } }, cut.how),
      h("div", { class: "mono", style: { fontSize: "10.5px", color: "var(--faint)", marginBottom: "4px" } }, `the last ${shown.length} of ${sample.count} messages ${sample.agent.split(" › ").pop()} was sent:`),
      h("div", { style: { cursor: "pointer" }, onclick: () => openDrawer(`${sample.agent} — first request, ${cut.title}`, shown.map(m => [h("div", { style: { margin: "12px 0 6px" } }, h("span", { class: "role " + m.role }, m.role)), h("pre", { class: "text" }, messageText(m))])) }, tailRows(shown)),
      block("sonnet"), block("luna")));
  }
  el.append(grid,
    h("div", { class: "legend", style: { marginTop: "18px" } },
      h("span", {}, h("i", { class: "swatch", style: { background: "var(--bad)", borderRadius: "50%" } }), "leaf read a sibling's ledger, claimed its total, or split itself again"),
      h("span", {}, h("i", { class: "swatch", style: { background: "var(--maunga)", borderRadius: "50%" } }), h("i", { class: "swatch", style: { background: "var(--awa)", borderRadius: "50%" } }), "stayed in its lane"),
      h("span", {}, h("i", { class: "swatch", style: { boxShadow: "0 0 0 1px #2f5a41 inset" } }), "tree as asked"),
      h("span", {}, "invalid trials (provider faults) are left out")),
    h("h2", {}, "in their own words"),
    h("p", { class: "lede" }, "When a region's tail is a tool result, sonnet takes that result as its own task call returning — and returning wrong. Under ", h("span", { class: "mono" }, "own"), ", which was meant to help by hiding the siblings, the rewritten call lists only itself, so it decides it made a mistake launching just one region. Every non-numeric report a region wrote:"),
    h("div", { class: "quotes wide" }, voices(t => t.mode === "fork" && t.cut !== "before").map(v => quote(v))));
});
