// Part 5: every type the experiment introduced, nested in its file. An arrow from A to B means A holds a B.

import { graph, type Node, type Edge } from "underview/graph"
import { h, code, part, cameTo } from "./ui"
import { DEPS, SOURCES, excerptOf } from "./code"

type Item = (typeof DEPS.items)[number]

// What each type is for, where its doc comment doesn't already say.
const FOR: Record<string, string> = {
  Run: "the one record every agent in a run shares: config, limb, recorder, cancel token, and its state behind a mutex",
  RunState: "spend and requests in flight, for the cap; the first fault; every agent's record",
  AgentRecord: "one agent, as summary.json reports it",
  Outcome: "how an agent ended; Faulted, Suspended and Panicked keep its parent suspended",
  Fault: "an out-of-band failure, and why",
  RequestEnd: "why a request produced no reply",
  ParentTurn: "",
  ChildSpec: "one entry of a task call, after validation: names unique, after-names present, no cycles",
  ChildReport: "what a child hands back: its outcome and its last message",
  AgentEnd: "an agent's outcome, report and final messages",
  Config: "everything fixed for a run",
  Mode: "fork, fresh, or whatever each task entry declared",
  Framing: "",
  Cut: "what a forked child inherits: full, own, or before",
  Words: "what its assignment says: stop, or explained",
  Local: "the two tools the limb runs",
  Tally: "a row of the tree snapshot",
  ToolCallRecord: "one call and its result, for summary.json",
  Message: "stored exactly as sent, so a cloned context serialises to the same bytes",
  ChatRequest: "the body sent to the provider",
  ChatResponse: "the body that came back",
  Sent: "a response and the exact body that asked for it",
  Retryable: "a transient failure, and how long to wait",
  Session: "one root agent and its run directory",
  Ending: "what a finished run reports",
  Recorder: "the run directory; wire.jsonl is written as requests happen",
  Face: "the append-only lines on stdout",
  Limb: "one directory, read-only",
  Args: "the command line",
  Fixture: "the benchmark's directory and its expected totals",
  TrialFacts: "one trial's configuration and outcome",
  Score: "what the scorer concluded about one trial",
  Seen: "what rescoring reads off one agent's wire",
  Fake: "the fake provider: scripted replies, holds and barriers, and a request log",
  Gate: "how many requests wait at a barrier, and whether they're released",
}

const types = DEPS.items.filter((i: Item) => (i.kind === "struct" || i.kind === "enum") && !i.file.startsWith("tests/"))
const names = new Set(types.map((t: Item) => t.name))

// A type's doc comment, whole: the `///` lines above its declaration.
function docOf(t: Item): string {
  const lines = SOURCES[t.file].split("\n").slice(t.start - 1, t.end)
  const head = lines.findIndex((l) => /\b(struct|enum)\s/.test(l))
  return lines.slice(0, head).filter((l) => l.trim().startsWith("///")).map((l) => l.trim().slice(3).trim()).join(" ")
}

const SYSTEMS: [string, string[]][] = [
  ["The run", ["src/agent.rs", "src/framing.rs", "src/session.rs", "src/limb.rs", "src/record.rs", "src/face.rs", "src/main.rs"]],
  ["The wire", ["src/wire.rs"]],
  ["The benchmark", ["src/bench.rs", "src/rescore.rs"]],
  ["The fake provider", ["src/bin/fake_provider.rs"]],
]
const systemOf = (file: string) => SYSTEMS.find(([, fs]) => fs.includes(file))![0]
const byName = new Map(types.map((t: Item) => [t.name, t]))
const holds = (t: Item) => [...new Set((t.fields ?? []).flatMap((f: any) => (f.type.match(/\w+/g) ?? []).filter((w: string) => names.has(w) && w !== t.name)))].map((n) => byName.get(n)!)

function box(t: Item): HTMLElement {
  const members = t.kind === "enum" ? (t.variants ?? []).map((v: string) => h("div", {}, v)) : (t.fields ?? []).map((f: any) => h("div", {}, f.name, ": ", h("span", { style: { color: "var(--mute)" } }, f.type)))
  const purpose = docOf(t) || FOR[t.name]
  const elsewhere = holds(t).filter((o) => systemOf(o.file) !== systemOf(t.file))
  const el = h("div", { class: "node type click", style: { maxWidth: "320px" } },
    h("div", { class: "t mono" }, t.name, h("span", { class: "s", style: { marginLeft: "6px" } }, t.kind)),
    purpose && h("div", { class: "s", style: { margin: "2px 0 4px" } }, purpose),
    h("div", { class: "mono", style: { fontSize: "11px", lineHeight: "1.45" } }, members),
    elsewhere.length > 0 && h("div", { class: "s", style: { marginTop: "4px" } }, "holds, from elsewhere: ", elsewhere.map((o) => `${o.name} (${o.file.split("/").pop()})`).join(", ")))
  let shown: HTMLElement | null = null
  el.addEventListener("click", () => {
    el.classList.toggle("open")
    if (shown) (shown.remove(), (shown = null))
    else el.append((shown = h("div", { class: "detail" }, code(excerptOf(t)))))
  })
  return el
}

function system(files: string[]): HTMLElement {
  const host = h("div", { class: "graph" })
  const mine = types.filter((t: Item) => files.includes(t.file))
  const nodes: Node[] = files.filter((f) => mine.some((t: Item) => t.file === f)).map((f) => ({
    id: "f:" + f,
    el: h("div", { class: "node group" }, h("div", { class: "gh" }, f)),
    children: mine.filter((t: Item) => t.file === f).map((t: Item) => ({ id: t.id, el: box(t) })),
  }))
  const edges: Edge[] = mine.flatMap((t: Item) => holds(t).filter((o) => files.includes(o.file)).map((o) => ({ from: t.id, to: o.id })))
  queueMicrotask(() => graph(host, nodes, edges, { direction: "DOWN", gap: 16 }))
  return host
}

export function typesPart(): HTMLElement {
  return part(5, "types", "The abstractions",
    `The ${types.length} types the experiment introduced, in four groups: the run, the wire, the benchmark and the fake provider. Each sits in the file that defines it, with its fields or variants as written and what it is for. An arrow from A to B means A holds a B. Click a type for its source.`,
    SYSTEMS.map(([title, files], i) => [h("h3", {}, `5.${i + 1} ${title}`), system(files)]),
    cameTo(h("code", {}, "Run"), " is the only thing agents share, and all its mutable state sits behind one mutex. A scope is nothing more than ", h("code", {}, "ParentTurn"), " in, ", h("code", {}, "ChildSpec"), "s across, ", h("code", {}, "ChildReport"), "s back. Everything a model is shown is a ", h("code", {}, "Message"), ", kept byte-exact so a fork is a clone. The benchmark adds its own small world — ", h("code", {}, "Fixture"), ", ", h("code", {}, "Observed"), ", ", h("code", {}, "Score"), " — which reads the run's records and nothing else."))
}
