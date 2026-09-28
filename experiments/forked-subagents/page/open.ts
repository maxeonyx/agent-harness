// The opening: what the page covers, in what order, and what is waiting on Max; and the close.

import { h } from "./ui"
import { spend } from "./data" with { type: "macro" }

const PARTS: [string, string, string][] = [
  ["handoffs", "The handoff model", "One type for what a context hands the next: down to a child, up to its parent, across to a fresh context, and to the child that talks with you. Each as the exact context received."],
  ["framing", "The framing experiment", "Your question about how a split child is told its part; the variants, each as a real child's context; the two benchmarks; what they found."],
  ["runs", "What was run", "Every run, with the command that made it; then one benchmark trial, request by request."],
  ["claims", "What it claims", "The thesis, the four things that would falsify it, and the evidence and caveats each rests on."],
  ["code", "The code", "Every file as a box in its folder; open one for its functions and types. An arrow means: delete what it points at, and this breaks."],
  ["dataflow", "How data moves", "Through one run and through the benchmark. An arrow means data moves that way."],
  ["types", "The abstractions", "Every type the experiment introduced: what it holds, and what holds it."],
  ["concerns", "Cross-cutting concerns", "What every agent depends on — handled, where, proved by which scenario — and what was left out."],
]

const HERE = "~/at-workspace/at-forked-subagents/tools/agent-harness/experiments/forked-subagents"

export function open(): HTMLElement {
  const used = spend()
  return h("header", {},
    h("div", { class: "kicker" }, "agent-harness · experiment · forked-subagents · PR #14"),
    h("h1", {}, "Agents as structured concurrency"),
    h("p", {}, "A ", h("code", {}, "task"), " call suspends the agent that makes it, runs its children at once, and returns every child's report as one tool result. A forked child starts as its parent's own messages, then a shared part written once for all its siblings, then its own part. This branch makes that one of three handoffs — down, up, and across into a fresh context — and tests how the child should be told what is its own."),
    h("div", { class: "waiting" },
      h("div", { class: "kicker" }, "Waiting on you"),
      h("ol", {},
        h("li", {}, "Watch a split with a child that talks with you. Say what the root should split, keep talking while it works, then type ", h("code", {}, "/done"), ". Add ", h("code", {}, "--handover-at 4000"), " to watch agents hand over too.", h("pre", {}, `cd ${HERE}\ncargo run --release --bin forks -- chat --dir src`)),
        h("li", {}, h("code", {}, "result"), " or ", h("code", {}, "before"), ": the benchmarks do not separate them (2.4). The default is ", h("code", {}, "before"), ", chosen for being pleasant to watch, not from evidence."),
        h("li", {}, "Review ", h("a", { href: "https://github.com/maxeonyx/agent-harness/pull/14" }, "#14"), ", a draft stacked on ", h("a", { href: "https://github.com/maxeonyx/agent-harness/pull/13" }, "#13"), "."))),
    h("nav", { class: "map" }, PARTS.map(([id, t, q], i) => h("a", { href: "#" + id }, h("div", { class: "n" }, i + 1), h("div", { class: "t" }, t), h("div", { class: "q" }, q)))),
    h("p", { class: "tok", style: { marginTop: "14px" } }, `Every number below is read from the recorded runs when this page is built. Those runs made ${used.claude.toLocaleString("en-NZ")} requests on your Claude subscription, and ${used.openrouter.toLocaleString("en-NZ")} on your OpenRouter harness key, costing $${used.usd.toFixed(2)} there.`))
}

export function close(): HTMLElement {
  return h("section", { class: "part", id: "close" },
    h("header", {}, h("span", { class: "n" }, "·"), h("h2", {}, "What it all came to")),
    h("ol", {},
      h("li", {}, h("b", {}, "The handoff model: "), "a shared part, attachments, an own part and a breakpoint, delivered onto a context. Down, up and across differ only in what they are delivered onto; the child that talks with you differs only in its own part."),
      h("li", {}, h("b", {}, "The framing experiment: "), "the own part must be user text. In a tool result, the child takes it for its own call coming back broken and redoes the split. result and before both hold, and neither benchmark separates them."),
      h("li", {}, h("b", {}, "What was run: "), "this branch's benchmarks and runs on your subscription, on Sonnet 5; the first harness again beside them; the first round on OpenRouter kept as it was."),
      h("li", {}, h("b", {}, "What it claims: "), "forking is cheap; a child stops at its own part when that part is user text; the handoff model works end to end; understandability is yours to judge."),
      h("li", {}, h("b", {}, "The code: "), "one loop that a task call re-enters one level down; ", h("code", {}, "agent.rs"), " holds it, ", h("code", {}, "handoff.rs"), " builds every context after the first, ", h("code", {}, "framing.rs"), " holds every word said to a model."),
      h("li", {}, h("b", {}, "How data moves: "), "out through the down handoff, back through the up handoff, and across through a handover; everything the evidence says is read back from the wire."),
      h("li", {}, h("b", {}, "The abstractions: "), "a run's shared record, an agent's outcome, a child's spec and report, the handoff, and the three framing knobs."),
      h("li", {}, h("b", {}, "Cross-cutting concerns: "), "ordering, failure, cancellation, spend, context length and the user's obligations are handled and proved; persistence, writes and the rest are out.")))
}
