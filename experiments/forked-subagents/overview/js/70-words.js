// Words: every word the harness says to a model, exactly as it arrived on the wire, and the line of code it comes from.

function marked(text, phrase) {
  if (!phrase) return text;
  const at = text.indexOf(phrase);
  if (at < 0) return text;
  return [text.slice(0, at), h("mark", {}, phrase), text.slice(at + phrase.length)];
}

view("words", "words", el => {
  const S = DATA.samples;
  const task = S.tools.find(t => t.function.name === "task").function;
  const params = task.parameters.properties.agents.items.properties;
  const refusalTrial = trialById(S.refusal.trial);
  const policy = S.policy;
  const section7 = policy.slice(policy.indexOf("## 7"), policy.indexOf("## 8"));
  const cards = [
    { title: "The system prompt", voice: "system", meta: "every agent in a run, byte-identical — so every fork's prefix matches", text: S.system.message.content, where: ["framing.rs", "src/framing.rs", lineOf("src/framing.rs", "SYSTEM_PROMPT")] },
    { title: "The task tool", voice: "system", meta: "the only brain-native tool; same list for every agent", text: task.description + "\n\n" + Object.entries(params).map(([k, v]) => `${k}: ${v.description}`).join("\n"), where: ["framing.rs", "src/framing.rs", fnLine("src/framing.rs", "tool_schemas")] },
    { title: "What a child is told — stop", voice: "user", meta: `${S.before.agent}, first request, last message`, text: S.before.tail[S.before.tail.length - 1].content, mark: "Do only this, then stop.", where: ["stop_words()", "src/framing.rs", fnLine("src/framing.rs", "stop_words")] },
    { title: "What a child is told — explained", voice: "user", meta: `${S.explained.agent}, first request, last message`, text: S.explained.message.content, mark: "You are one branch of a split", where: ["explained_words()", "src/framing.rs", fnLine("src/framing.rs", "explained_words")] },
    { title: "What the parent gets back", voice: "tool", meta: "one tool result for the whole scope, reports in declared order", text: S.scope.message.content, where: ["scope_result()", "src/framing.rs", fnLine("src/framing.rs", "scope_result")] },
    { title: "The depth-limit refusal", voice: "tool", flag: true, meta: `${S.refusal.agent} tried to split itself again and got this`, text: S.refusal.message.content, mark: "Do this work yourself.",
      note: h("span", {}, "It then read all three of its region's ledgers and reported the region — as did the two other leaves in that tree that hit it. The wording was never varied, so it may cause the over-reach it is meant to stop. ", h("span", { class: "link", onclick: () => openTrial(refusalTrial, S.refusal.agent) }, "that agent →")),
      where: ["depth_limit_error()", "src/framing.rs", fnLine("src/framing.rs", "depth_limit_error")] },
    { title: "The root's task", voice: "user", meta: "the benchmark's fixed prompt — forces root → region → leaf", text: S.root_task.content, where: ["ROOT_TASK", "src/bench.rs", lineOf("src/bench.rs", "ROOT_TASK")] },
    { title: "POLICY.md — section 7 of 30", voice: "tool", meta: `${fmt.int(policy.length)} bytes, ~7k tokens. Only the root reads it; forks inherit it from cache, fresh children get what their parent copies out.`, text: section7, where: ["the generator", "src/bench.rs", lineOf("src/bench.rs", "POLICY")] },
  ];
  el.append(h("h1", {}, "Every word the harness ", h("em", {}, "says to a model")),
    h("p", { class: "lede" }, "As the model received them, from the recorded wire. Everything else a model sees is the task, the files, and other models."));
  el.append(h("div", { class: "words" }, cards.map(c => h("div", { class: "word" + (c.flag ? " flag" : "") },
    h("div", { class: "top" }, h("span", { class: "role " + c.voice }, c.voice), h("span", { class: "title" }, c.title), h("span", { class: "where" }, srcLink(c.where[0] + " ↗", c.where[1], c.where[2], c.where[2] + 14))),
    h("div", { class: "meta" }, c.meta),
    h("pre", {}, marked(c.text, c.mark)),
    c.note ? h("div", { class: "note" }, c.note) : null))));
});
