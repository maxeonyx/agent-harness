// Part 2: what the experiment claims, and what each claim stands on.

import { graph, type Node, type Edge } from "underview/graph"
import { excerpt } from "underview/excerpt" with { type: "macro" }
import { h, code, part, cameTo, pct } from "./ui"
import { VALID, short, rowOf, leafRate, firstShare, voices } from "./trials"

const PROBE = excerpt("../probe/runs.md")
const BRIEF = excerpt("../../../docs/process/experiments/forked-subagents-brief.md:/Thesis:/")

type Claim = { id: string; title: string; verdict: [string, string]; kids: Leaf[] }
type Leaf = { id: string; title: string; note?: string; caveat?: boolean; open?: () => HTMLElement }

const sonnetFork = VALID.filter((t) => short(t.model) === "sonnet" && t.mode === "fork")
const toolTail = sonnetFork.filter((t) => t.cut !== "before")
const userTail = sonnetFork.filter((t) => t.cut === "before")
const ratio = ([a, b]: readonly [number, number]) => `${a} of ${b}`
const trees = (list: any[]) => `${list.filter((t) => t.structure_ok).length} of ${list.length}`
const luna = (cut: string) => VALID.filter((t) => short(t.model) === "luna" && t.mode === "fork" && t.cut === cut)
const said = voices((t) => short(t.model) === "sonnet" && t.mode === "fork" && t.cut !== "before")

const CLAIMS: Claim[] = [
  {
    id: "cache", title: "A forked child reads its parent's context from the provider's cache", verdict: ["held", "held, on two conditions"],
    kids: [
      { id: "cache-share", title: `First request of a forked child: ${pct(firstShare(VALID.filter((t) => t.mode === "fork")))} from cache`, note: `fresh children: ${pct(firstShare(VALID.filter((t) => t.mode === "fresh")))} — every valid trial, both models`, open: () => h("p", {}, "Summed over each child's first request: tokens the provider reported as cached, over all prompt tokens. See ", h("a", { href: "#runs" }, "1.3"), " for one run request by request.") },
      { id: "cache-pin", caveat: true, title: "Only with the provider pinned", note: "unrouted, 1 of 2 probe runs sent both children to a cold backend", open: () => code(PROBE, "probe/runs.md") },
      { id: "cache-small", caveat: true, title: "Not at small prefixes, for a reason not found", note: "at ~1.2k tokens, children read 0 from cache in 4 of 4 of the builder's smoke runs — not in the recorded grid" },
    ],
  },
  {
    id: "stop", title: "A forked child can be made to stop at its own assignment — A.1.3", verdict: ["open", "not settled"],
    kids: [
      { id: "stop-tool", title: `Tool-result tail (full, own): sonnet ${ratio(leafRate(toolTail))} leaves over-reached`, note: `trees built as asked: ${trees(toolTail)}`, open: () => h("p", {}, "The last message a child sees is a tool result answering the parent's own task call. ", h("a", { href: "#runs" }, "1.4"), " shows it exactly.") },
      { id: "stop-user", title: `User-message tail (before): sonnet ${ratio(leafRate(userTail))}`, note: `trees built as asked: ${trees(userTail)}` },
      { id: "stop-words", title: `In their own words: ${said.length} sonnet region reports, none with a number in them, read the assignment as their own call returning wrong`, open: () => h("div", {}, said.map((v) => h("p", {}, "“" + v.text + "” ", h("span", { class: "tok" }, `${v.agent.split(" › ").pop()}, ${v.trial.cut} cut`)))) },
      { id: "stop-luna", title: `Luna: full ${ratio(leafRate(luna("full")))}, own ${ratio(leafRate(luna("own")))}, before ${ratio(leafRate(luna("before")))}` },
      { id: "stop-n", caveat: true, title: "One to five valid trials per cell" },
      { id: "stop-refusal", caveat: true, title: "The three before-cut leaves that over-reached had just been refused by the depth limit", note: "its wording, “Do this work yourself.”, was never varied" },
      { id: "stop-cap", caveat: true, title: "The first grid's $0.15 cap stopped all 8 Sonnet before-cut and fresh trees early", note: "so their shape is scored, their totals are not" },
    ],
  },
  {
    id: "watch", title: "The scope model is understandable when you watch it run", verdict: ["wait", "waiting on you"],
    kids: [{ id: "watch-you", title: "Nothing mechanical measures this: it is you running a task and watching the tree", note: "the command is at the top of the page" }],
  },
]

function card(title: string, note?: string, badge?: [string, string], open?: () => HTMLElement, extra = ""): HTMLElement {
  const el = h("div", { class: `node${open ? " click" : ""}${extra}` }, h("div", { class: "t" }, title, badge && h("span", { class: `verdict ${badge[0]}` }, badge[1])), note && h("div", { class: "s" }, note))
  if (open) {
    let shown: HTMLElement | null = null
    el.addEventListener("click", () => {
      el.classList.toggle("open")
      if (shown) (shown.remove(), (shown = null))
      else el.append((shown = h("div", { class: "detail" }, open())))
    })
  }
  return el
}

export function claims(): HTMLElement {
  const host = h("div", { class: "graph" })
  const nodes: Node[] = [{ id: "thesis", el: card("Thesis: agents can run as structured concurrency", "a task call is a scope; forks are cheap; each child stops at its own part", undefined, () => code(BRIEF, "the brief"), " click") }]
  const edges: Edge[] = []
  for (const c of CLAIMS) {
    nodes.push({ id: c.id, el: card(c.title, undefined, c.verdict) })
    edges.push({ from: "thesis", to: c.id })
    for (const k of c.kids) {
      nodes.push({ id: k.id, el: card(k.title, k.note, undefined, k.open, k.caveat ? " ext" : "") })
      edges.push({ from: c.id, to: k.id, class: k.caveat ? "dashed" : undefined })
    }
  }
  queueMicrotask(() => graph(host, nodes, edges, { direction: "RIGHT", gap: 14, elk: { "elk.layered.considerModelOrder.strategy": "NODES_AND_EDGES" } }))
  return part(2, "claims", "What it claims",
    "The thesis from the brief, the three things that would have falsified it, and what each rests on. Solid boxes are evidence; dashed boxes are caveats that weaken it. Click a box with more behind it.",
    h("div", { class: "legend" }, h("span", {}, h("i", { class: "sw" }), "evidence"), h("span", {}, h("i", { class: "sw", style: { borderStyle: "dashed" } }), "caveat"), h("span", {}, "an arrow means “rests on”")),
    host,
    cameTo("Forking is cheap, with the provider pinned. Whether a child stops at A.1.3 depends on how its assignment arrives, and no framing tried makes Sonnet reliable yet. Whether the model is understandable is for you to say."))
}
