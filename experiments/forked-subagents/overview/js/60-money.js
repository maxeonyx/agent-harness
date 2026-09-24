// Money: where the tokens go versus where the dollars go, for every tree that ran to the end.

view("money", "money", el => {
  const done = valid.filter(t => t.outcome === "completed");
  const PARTS = [
    ["read", "var(--ice)", "read from cache"],
    ["write", "var(--ember)", "written to cache"],
    ["plain", "var(--plain)", "uncached input"],
    ["out", "var(--gold)", "output (mostly reasoning)"],
  ];
  const say = h("div", { class: "bigsay" });
  const seg = h("div", { class: "seg" });
  const groups = h("div");
  el.append(h("h1", {}, "Where the ", h("em", {}, "money"), " goes"),
    h("p", { class: "lede" }, "Every tree that ran to the end. The same bars, measured two ways."),
    h("div", { class: "watch-top" }, seg, h("div", { class: "legend" }, PARTS.map(([, c, l]) => h("span", {}, h("i", { class: "swatch", style: { background: c } }), l)))),
    say, groups);
  const bars = [];
  for (const model of ["sonnet", "luna"]) {
    const list = done.filter(t => short(t.model) === model).sort((a, b) => (a.mode + a.cut).localeCompare(b.mode + b.cut));
    if (!list.length) continue;
    const g = h("div", { class: "group" + (list.length > 8 ? " compact" : "") }, h("h2", {}, `${model} — ${list.length} complete trees`));
    for (const t of list) {
      const track = h("div", { class: "track" });
      const segs = PARTS.map(([k, c]) => { const d = h("div", { style: { background: c, width: "0%" } }); track.append(d); return [k, d]; });
      const tot = h("div", { class: "tot" });
      g.append(tipOn(h("div", { class: "moneyrow", onclick: () => openTrial(t) },
        h("div", { class: "who" }, t.mode === "fresh" ? "fresh " : "fork · " + t.cut + " ", h("small", {}, `· ${t.words}`)), track, tot),
        () => PARTS.map(([k, , l]) => `${l}: ${fmt.int(t.tokens[k])} tokens · ${fmt.usd(t.dollars[k])}`).join("\n") + `\nbilled: ${fmt.usd(t.cost)}`));
      bars.push({ t, model, segs, tot });
    }
    groups.append(g);
  }
  const set = unit => {
    seg.querySelectorAll("button").forEach(b => b.classList.toggle("on", b.dataset.u === unit));
    const key = unit === "tokens" ? "tokens" : "dollars";
    const max = {};
    for (const b of bars) max[b.model] = Math.max(max[b.model] || 0, Object.values(b.t[key]).reduce((a, x) => a + x, 0));
    for (const b of bars) {
      const total = Object.values(b.t[key]).reduce((a, x) => a + x, 0);
      for (const [k, d] of b.segs) d.style.width = (b.t[key][k] / max[b.model]) * 100 + "%";
      b.tot.textContent = unit === "tokens" ? fmt.tok(total) : fmt.usd(total);
    }
    const sonnet = bars.filter(b => b.model === "sonnet").map(b => b.t);
    const sum = (list, key, part) => list.reduce((a, t) => a + t[key][part], 0);
    const all = (list, key) => ["read", "write", "plain", "out"].reduce((a, p) => a + sum(list, key, p), 0);
    const readTok = sum(sonnet, "tokens", "read") / all(sonnet, "tokens");
    const readUsd = sum(sonnet, "dollars", "read") / all(sonnet, "dollars");
    const outUsd = sum(sonnet, "dollars", "out") / all(sonnet, "dollars");
    say.replaceChildren(unit === "tokens"
      ? h("span", {}, "On sonnet, ", h("b", { style: { color: "var(--ice)" } }, fmt.pct(readTok) + " of every token"), " was a cache read — children re-reading context they inherited.")
      : h("span", {}, "Those reads were ", h("b", { style: { color: "var(--ice)" } }, fmt.pct(readUsd) + " of the dollars"), ". Output was ", h("b", { style: { color: "var(--gold)" } }, fmt.pct(outUsd)), ". Fork and fresh cost about the same."));
  };
  for (const u of ["tokens", "dollars"]) seg.append(h("button", { "data-u": u, onclick: () => set(u) }, u));
  set("tokens");
  VIEWS.find(v => v.id === "money").onShow = () => { if (STILL) return set("dollars"); set("tokens"); setTimeout(() => set("dollars"), 1600); };
});
