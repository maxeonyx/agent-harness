// Part 6: how data moves through one run, and through the benchmark. An arrow means data moves that way.

import { graph, type Node, type Edge } from "underview/graph"
import { excerpt } from "underview/excerpt" with { type: "macro" }
import { h, code, part, cameTo } from "./ui"

type Datum = { id: string; t: string; s?: string; ext?: boolean; at?: ReturnType<typeof excerpt> }

// A loop drawn as a loop makes the layout reverse an edge, so an agent's messages appear twice: before a turn, and after it.
const RUN: Datum[] = [
  { id: "argv", t: "the task, and flags", s: "forks run --dir … \"…\"", ext: true },
  { id: "config", t: "Config", s: "model, backend, cut, identity, mode, caps, handover point", at: excerpt("../src/agent.rs:/pub struct Config {/") },
  { id: "root", t: "the root's first messages", s: "system ×2 on Claude, then user: the task", at: excerpt("../src/wire.rs:/pub fn system(&self, prompt: &str)/") },
  { id: "before", t: "an agent's messages, turn n", s: "Vec<Message>, only ever appended to", at: excerpt("../src/wire.rs:/pub struct Message {/") },
  { id: "request", t: "request body", s: "envelope + messages + 4 tools, per backend", at: excerpt("../src/wire.rs:/pub fn body(/") },
  { id: "provider", t: "Anthropic on your subscription, or OpenRouter", s: "cache read, then generation", ext: true },
  { id: "reply", t: "reply + usage", s: "assistant message, tokens, cost", at: excerpt("../src/anthropic.rs:/pub fn reply(/") },
  { id: "spent", t: "spent, in flight", s: "checked before every attempt", at: excerpt("../src/agent.rs:/let committed = state.spent/") },
  { id: "wire", t: "wire.jsonl", s: "every body sent and received", at: excerpt("../src/record.rs:/let wire = std::fs::File::create/") },
  { id: "files", t: "files under --dir", s: "read by list_dir / read_file", ext: true },
  { id: "results", t: "tool results", s: "one message per local call", at: excerpt("../src/agent.rs:/The local tools run first/") },
  { id: "spec", t: "ChildSpec per child", s: "from the task call's arguments", at: excerpt("../src/agent.rs:/struct ChildSpec {/") },
  { id: "turn", t: "ParentTurn", s: "turn n's messages + the task call", at: excerpt("../src/agent.rs:/struct ParentTurn {/") },
  { id: "down", t: "the down handoff", s: "shared part, attachments, own part", at: excerpt("../src/agent.rs:/let down = Handoff {/") },
  { id: "child", t: "the child's first messages", s: "the down handoff, delivered onto the cut", at: excerpt("../src/agent.rs:/fn child_context(/") },
  { id: "turns", t: "the child's own turns", s: "this same flow, one level down", ext: true },
  { id: "report", t: "ChildReport", s: "outcome + its last message", at: excerpt("../src/agent.rs:/struct ChildReport {/") },
  { id: "up", t: "the up handoff", s: "every report, as the task call's answer", at: excerpt("../src/agent.rs:/let up = Handoff {/") },
  { id: "across", t: "the across handoff", s: "onto the system prompt alone", at: excerpt("../src/agent.rs:/let fresh = Handoff {/") },
  { id: "after", t: "an agent's messages, turn n + 1", s: "+ reply + every result, or a fresh context", at: excerpt("../src/agent.rs:/for result in results {/") },
  { id: "summary", t: "summary.json, agents/*.md", s: "each agent's record and final context", at: excerpt("../src/session.rs:/let file = format!(\"agents/{}.md\"/") },
]

const RUN_EDGES: [string, string][] = [
  ["argv", "config"], ["argv", "root"], ["root", "before"], ["before", "request"], ["config", "request"], ["request", "provider"], ["provider", "reply"],
  ["request", "wire"], ["reply", "wire"], ["reply", "spent"],
  ["reply", "results"], ["files", "results"], ["reply", "spec"], ["before", "turn"], ["reply", "turn"],
  ["spec", "down"], ["turn", "child"], ["down", "child"], ["child", "turns"], ["turns", "report"], ["report", "up"],
  ["reply", "after"], ["results", "after"], ["up", "after"], ["reply", "across"], ["across", "after"], ["after", "summary"],
]

const BENCH: Datum[] = [
  { id: "gen", t: "the fixture generators", s: "ledgers.rs or projects.rs, by --task", at: excerpt("../src/bench.rs:/fn write(task: Task/") },
  { id: "fixture", t: "fixture/", s: "ledgers/ or work/, and the right answer", ext: true },
  { id: "run", t: "one run per trial", s: "the flow above, with that task's ROOT_TASK", ext: true },
  { id: "dir", t: "the trial's run directory", s: "wire.jsonl, summary.json", ext: true },
  { id: "observed", t: "Observed per agent", s: "answered reads, task calls, report", at: excerpt("../src/bench.rs:/pub struct Observed {/") },
  { id: "score", t: "Score", s: "over-reach, shape, answer, per task", at: excerpt("../src/bench.rs:/pub fn score(&self/") },
  { id: "row", t: "one row per trial", s: "trials.json, summary.md", at: excerpt("../src/bench.rs:/pub fn trial_row(/") },
]
const BENCH_EDGES: [string, string][] = [["gen", "fixture"], ["fixture", "run"], ["run", "dir"], ["dir", "observed"], ["fixture", "score"], ["observed", "score"], ["score", "row"]]

function box(d: Datum): HTMLElement {
  const el = h("div", { class: `node${d.ext ? " ext" : ""}${d.at ? " click" : ""}` }, h("div", { class: "t" }, d.t), d.s && h("div", { class: "s mono" }, d.s))
  if (d.at) {
    let shown: HTMLElement | null = null
    el.addEventListener("click", () => {
      el.classList.toggle("open")
      if (shown) (shown.remove(), (shown = null))
      else el.append((shown = h("div", { class: "detail" }, code(d.at!))))
    })
  }
  return el
}

function flowGraph(data: Datum[], edges: [string, string][]): HTMLElement {
  const host = h("div", { class: "graph" })
  const nodes: Node[] = data.map((d) => ({ id: d.id, el: box(d) }))
  const es: Edge[] = edges.map(([from, to]) => ({ from, to, class: "data" }))
  queueMicrotask(() => graph(host, nodes, es, { direction: "DOWN", gap: 18, elk: { "elk.layered.considerModelOrder.strategy": "NODES_AND_EDGES" } }))
  return host
}

export function dataflow(): HTMLElement {
  return part(6, "dataflow", "How data moves",
    "First through one run, then through the benchmark around it. An arrow only ever means data moves that way; dashed boxes are outside the code. Click a box for the lines that make that data.",
    h("h3", {}, "6.1 One run"),
    h("p", {}, "The loop is the cycle through ", h("i", {}, "an agent's messages"), ": request, reply, results, appended, again. A task call leaves that cycle through ", h("code", {}, "ChildSpec"), " and ", h("code", {}, "ParentTurn"), ", becomes each child's messages through the down handoff — which run this same flow — and comes back through the up handoff. A handover call leaves it through the across handoff, and the agent's messages start again."),
    flowGraph(RUN, RUN_EDGES),
    h("h3", {}, "6.2 The benchmark"),
    flowGraph(BENCH, BENCH_EDGES),
    cameTo("Everything an agent sees is built in three places: ", h("code", {}, "session.rs"), " for the root's first messages, ", h("code", {}, "Handoff::deliver"), " for every context after that, and ", h("code", {}, "framing.rs"), " for every word the harness itself says. Everything the evidence says is read back from what was recorded on the wire, by the same scorer whether live or rescored."))
}
