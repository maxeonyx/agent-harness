// Proof: every claim the scenario tests make, in their own words, with the state from the last run of the suite.

const GROUPS = [
  ["the scope", "var(--ice)", /^scope|^after_|^nested|^cut_|^fresh_children/],
  ["when things go wrong", "var(--ember)", /fault|rejected|spend_cap|cap_counts|silent|rate_limit|retry_after|over_deep|invalid_task|panicked/],
  ["stopping", "var(--user)", /cancel|interrupt|chat_does/],
  ["the benchmark's honesty", "var(--gold)", /benchmark|leaf|totals|name_given|failed_read|policy|rescoring/],
];

view("proof", "proof", el => {
  const tests = DATA.tests;
  const passed = tests.filter(t => t.state === "passed").length;
  el.append(h("h1", {}, `${passed} of ${tests.length} claims `, h("em", {}, "hold")),
    h("p", { class: "lede" }, "Each is a scenario test that drives the real ", h("span", { class: "mono" }, "forks"), " binary against a fake provider and checks only what it printed and what the provider received. Concurrency is proved with barriers, ordering with message content — nothing waits on a clock. The whole suite runs in under two seconds."));
  const used = new Set();
  const grid = h("div", { class: "proof" });
  for (const [title, colour, re] of GROUPS) {
    const list = tests.filter(t => !used.has(t.name) && re.test(t.name));
    list.forEach(t => used.add(t.name));
    grid.append(h("div", { class: "claims" }, h("h3", { style: { color: colour } }, title),
      list.map(t => h("div", { class: "claim", onclick: () => openSource("tests/scenario.rs", t.line, t.line + 40) }, h("i", { class: t.state }), h("span", {}, t.name.replace(/_/g, " "))))));
  }
  const rest = tests.filter(t => !used.has(t.name));
  if (rest.length) grid.append(h("div", { class: "claims" }, h("h3", {}, "other"), rest.map(t => h("div", { class: "claim", onclick: () => openSource("tests/scenario.rs", t.line, t.line + 40) }, h("i", { class: t.state }), h("span", {}, t.name.replace(/_/g, " "))))));
  el.append(grid,
    h("h2", {}, "made to fail first"),
    h("p", { class: "lede" }, "Every claim was checked by breaking the harness on purpose and watching its test go red — children run one after another, a faulted child releases its parent, a fork drops the parent's turn, the cap forgets requests in flight. A fresh-context review then found four ways to fool the scorer; each became a test here before it was fixed."));
});
