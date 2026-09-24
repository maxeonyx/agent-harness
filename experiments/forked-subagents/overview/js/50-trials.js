// Every trial: all 56, as the trees they actually built. Nothing averaged away.

view("trials", "every trial", el => {
  el.append(h("h1", {}, "Every trial, as the ", h("em", {}, "tree it built")),
    h("p", { class: "lede" }, `${TRIALS.length} trials, ${valid.length} valid. The intended shape is one root, two regions, six leaves. Click any tree for every agent's task, reads and report.`),
    h("div", { class: "legend", style: { marginBottom: "14px" } }, legendTree(),
      h("span", {}, h("b", { class: "mono", style: { color: "var(--gold)" } }, "✓"), "totals correct"),
      h("span", {}, h("b", { class: "mono", style: { color: "var(--ember)" } }, "$"), "stopped at its spend cap"),
      h("span", {}, h("i", { class: "swatch", style: { background: "repeating-linear-gradient(135deg,#1b2030 0 3px,#0e1119 3px 6px)" } }), "invalid — provider fault")));
  const cols = [["sonnet", "stop"], ["sonnet", "explained"], ["luna", "stop"], ["luna", "explained"]];
  const m = h("div", { class: "matrix" }, h("div"), cols.map(([model, words]) => h("div", { class: "hd" }, `${model} · ${words} words`)));
  for (const cut of CUTS) {
    m.append(h("div", { class: "rowh" }, h("div", { class: "cut" }, cut.title), h("div", { class: "how" }, cut.how)));
    for (const [model, words] of cols) {
      const list = TRIALS.filter(t => short(t.model) === model && t.words === words && rowOf(t) === cut.id);
      const cell = h("div", { class: "cell" });
      if (!list.length) cell.append(h("span", { class: "none" }, "not run"));
      for (const t of list) {
        const g = h("div", { class: "glyph" + (t.valid ? "" : " invalid"), onclick: () => openTrial(t) }, drawTree(t, 92, 70, { r: 3.2, pad: 9, edge: 1 }));
        if (t.valid && t.correct) g.append(h("span", { class: "tag ok" }, "✓"));
        if (t.valid && t.fault_kind === "budget") g.append(h("span", { class: "tag cap" }, "$"));
        if (!t.valid) g.append(h("span", { class: "tag x" }, "×"));
        tipOn(g, `${t.valid ? t.outcome : "INVALID: " + t.detail.slice(0, 120)}\n${t.leaf_overreach}/${t.leaves} leaves over-reached · tree as asked: ${t.structure_ok ? "yes" : "no"}\n${fmt.usd(t.cost)} · ${fmt.sec(t.millis)}`);
        cell.append(g);
      }
      m.append(cell);
    }
  }
  el.append(m);
});
