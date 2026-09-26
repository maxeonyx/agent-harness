// The opening: what the page covers, in what order, and what is waiting on Max.

import { h } from "./ui"
import { spend } from "./data" with { type: "macro" }
import { TRIALS, VALID } from "./trials"

const PARTS: [string, string, string][] = [
  ["runs", "What was run", "The four kinds of run, the benchmark's task, one run request by request, exactly what a child is shown under each cut, how trials are scored, what they found."],
  ["claims", "What it claims", "The thesis, the three things that would falsify it, and the evidence and caveats each rests on."],
  ["code", "The code", "Every file as a box nested in its folder; open one to see its functions and types. An arrow means: delete the thing it points at, and this breaks."],
  ["dataflow", "How data moves", "Through one run and through the benchmark. An arrow means data moves that way."],
  ["types", "The abstractions", "Every type the experiment introduced: what it holds, and what holds it."],
  ["concerns", "Cross-cutting concerns", "What every agent depends on — handled, where, proved by which scenario — and what was deliberately left out."],
]

export function open(): HTMLElement {
  const cost = spend()
  return h("header", {},
    h("div", { class: "kicker" }, "agent-harness · experiment · forked-subagents"),
    h("h1", {}, "Agents as structured concurrency"),
    h("p", {}, "A ", h("code", {}, "task"), " call suspends the agent that makes it, runs its children at once — each child starting as the parent's own messages plus one saying who it is — and returns every child's report as one tool result. This page is the experiment that built that and tested it on real models: first what was run and what it claims, then the code, its dataflow, its abstractions and its cross-cutting concerns."),
    h("div", { class: "waiting" },
      h("div", { class: "kicker" }, "Waiting on you"),
      h("ol", {},
        h("li", {}, "Run a task yourself and watch the tree — the brief's exit condition. About $0.10–0.40 a task on Sonnet.", h("pre", {}, "cd ~/at-workspace/at-forked-subagents/tools/agent-harness/experiments/forked-subagents\ncargo run --release --bin forks -- chat --dir ../walking-skeleton/src")),
        h("li", {}, "Gate 1: accept, redo or discard — ", h("a", { href: "https://github.com/maxeonyx/agent-harness/pull/13" }, "#13"), ", a draft stacked on ", h("a", { href: "https://github.com/maxeonyx/agent-harness/pull/12" }, "#12"), ", the design docs it quotes, which wants your review first."))),
    h("nav", { class: "map" }, PARTS.map(([id, t, q], i) => h("a", { href: "#" + id }, h("div", { class: "n" }, i + 1), h("div", { class: "t" }, t), h("div", { class: "q" }, q)))),
    h("p", { class: "tok", style: { marginTop: "14px" } }, `Every number below is read from the recorded runs when this page is built. Those runs cost $${cost.usd.toFixed(2)} across ${cost.responses.toLocaleString("en-NZ")} responses, on your OpenRouter harness key.`))
}

export function close(): HTMLElement {
  return h("section", { class: "part", id: "close" },
    h("header", {}, h("span", { class: "n" }, "·"), h("h2", {}, "What it all came to")),
    h("ol", {},
      h("li", {}, h("b", {}, "What was run: "), `${TRIALS.length} benchmark trials, ${VALID.length} valid, on Sonnet and Luna, varying how a forked child is told its assignment.`),
      h("li", {}, h("b", {}, "What it claims: "), "forking is cheap with the provider pinned; stopping at A.1.3 depends on the tail, and isn't reliable on Sonnet yet; understandability is yours to judge."),
      h("li", {}, h("b", {}, "The code: "), "one loop that a task call re-enters one level down; ", h("code", {}, "agent.rs"), " holds it, ", h("code", {}, "framing.rs"), " holds every word said to a model."),
      h("li", {}, h("b", {}, "How data moves: "), "a child's messages are cut from its parent's in one function; everything the evidence says is read back from the wire."),
      h("li", {}, h("b", {}, "The abstractions: "), "a run's shared record, an agent's outcome, a child's spec and report, and the three framing knobs."),
      h("li", {}, h("b", {}, "Cross-cutting concerns: "), "ordering, failure, cancellation and spend are handled and proved; persistence, writes, UX and the rest are out, as the brief said.")))
}
