// Part 4: what the experiment claims, and what each claim stands on.

import { graph, type Node, type Edge } from "underview/graph"
import { excerpt } from "underview/excerpt" with { type: "macro" }
import { h, code, part, cameTo, pct } from "./ui"
import { VALID, onClaude, firstHarness, firstShare, voices } from "./trials"

const PROBE = excerpt("../probe/runs.md")
const BRIEF = excerpt("../../../docs/process/experiments/forked-subagents-brief.md:/Thesis:/")

type Claim = { id: string; title: string; verdict: [string, string]; kids: Leaf[] }
type Leaf = { id: string; title: string; note?: string; caveat?: boolean; open?: () => HTMLElement }

const now = VALID.filter((t) => onClaude(t) && !firstHarness(t))
const claude = VALID.filter(onClaude)
const sum = (list: any[], key: string) => list.reduce((a, t) => a + t[key], 0)
const of = (list: any[], part: string, whole: string) => `${sum(list, part)} of ${sum(list, whole)}`
const userText = claude.filter((t) => !firstHarness(t) && t.cut !== "call")
const toolResult = claude.filter((t) => t.cut === "full" || t.cut === "call")
const said = voices((t) => onClaude(t) && (t.cut === "full" || t.cut === "call"))
const link = (href: string, text: string) => h("p", {}, h("a", { href }, text))

const CLAIMS: Claim[] = [
  {
    id: "cache", title: "A forked child reads its parent's context from the provider's cache", verdict: ["held", "held"],
    kids: [
      { id: "cache-share", title: `First request of a forked child: ${pct(firstShare(now.filter((t) => t.mode === "fork"))!)} from cache`, note: `fresh children: ${pct(firstShare(now.filter((t) => t.mode === "fresh"))!)} — this branch, on your subscription`, open: () => link("#runs", "3.2 shows one trial request by request") },
      { id: "cache-shared", title: `Shared part: ${of(now, "shared_hits", "later_siblings")} later siblings read it from the cache`, note: "the first sibling starts alone and writes it", open: () => link("#handoffs", "1.2 shows both siblings' numbers") },
      { id: "cache-pin", caveat: true, title: "On OpenRouter, only with the provider pinned", note: "unrouted, 1 of 2 probe runs sent both children to a cold backend", open: () => code(PROBE, "probe/runs.md") },
      { id: "cache-small", caveat: true, title: "On OpenRouter, not at small prefixes, for a reason not found", note: "at ~1.2k tokens, children read 0 from cache in 4 of 4 of the first round's smoke runs" },
    ],
  },
  {
    id: "stop", title: "A forked child can be made to stop at its own part — A.1.3", verdict: ["held", "held, with the own part as user text"],
    kids: [
      { id: "stop-user", title: `Own part as user text: ${of(userText, "leaf_overreach", "leaves")} branch agents over-reached`, note: `project agents: ${of(userText, "project_overreach", "projects")} — result, before and fresh`, open: () => link("#framing", "2.4 has every cell") },
      { id: "stop-tool", title: `Own part in a tool result: ${of(toolResult, "leaf_overreach", "leaves")} branch agents over-reached`, note: `project agents: ${of(toolResult, "project_overreach", "projects")} — full and call`, open: () => h("div", {}, said.slice(0, 3).map((v) => h("p", {}, "“" + v.text.slice(0, 240) + "” ", h("span", { class: "tok" }, v.agent)))) },
      { id: "stop-model", caveat: true, title: "One model: Claude Sonnet 5", note: "Luna ran only in the first round, on OpenRouter" },
      { id: "stop-n", caveat: true, title: "Five trials per cell, and both benchmarks saturated", note: "they do not separate result from before" },
      { id: "stop-plain", caveat: true, title: "Once outside them, under before, a child redid the root's split", note: "runs.ignore/20260928-104906-run: face_reader relaunched both readers, down to the depth limit" },
      { id: "stop-fresh", caveat: true, title: "Fresh, the datacentral agent split its 25 checkouts again in 3 of 5 trials" },
    ],
  },
  {
    id: "one", title: "A compaction, a task and its response can be one handoff", verdict: ["held", "built, and run"],
    kids: [
      { id: "one-code", title: "Down, up and across are one type and one deliver", note: "src/handoff.rs", open: () => link("#handoffs", "part 1 shows each, exactly") },
      { id: "one-run", title: "A run where the root and all six leaves handed over got every total right", note: "runs.ignore/20260928-125859-run" },
      { id: "one-chat", title: "The child that talks with you reported back through the same up handoff", note: "runs.ignore/20260928-125453-chat, driven by a script" },
      { id: "one-attach", caveat: true, title: "No recorded run used attachments", note: "only a scenario test does" },
    ],
  },
  {
    id: "watch", title: "The scope model is understandable when you watch it run", verdict: ["wait", "waiting on you"],
    kids: [
      { id: "watch-26", title: "You ran forks chat on 26 September: “seems good honestly”" },
      { id: "watch-new", title: "Not yet watched by you: the child that talks with you, and handover", note: "the command is at the top of the page" },
    ],
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
  return part(4, "claims", "What it claims",
    "The thesis from the brief, the four things that would falsify it, and what each rests on. Solid boxes are evidence; dashed boxes are caveats that weaken it. Click a box with more behind it.",
    h("div", { class: "legend" }, h("span", {}, h("i", { class: "sw" }), "evidence"), h("span", {}, h("i", { class: "sw", style: { borderStyle: "dashed" } }), "caveat"), h("span", {}, "an arrow means “rests on”")),
    host,
    cameTo("Forking is cheap: a forked child's first request comes mostly from the cache, and so does the shared part for every sibling after the first. A child stops at its own part when that part is user text, on Sonnet 5, on both benchmarks. The handoff model is built and ran end to end. Whether the model is understandable is for you to say."))
}
