// Part 2: how a split child is told what is its own — the question, the variants, the benchmarks, the results.

import { excerpt } from "underview/excerpt" with { type: "macro" }
import { requests, ledgers, projects } from "./data" with { type: "macro" }
import type { Request } from "./data"
import { h, code, toggle, part, cameTo, pct, int } from "./ui"
import { context } from "./context"
import { VALID, TRIALS, onClaude, firstHarness, variant, ratio, count, firstShare, voices } from "./trials"

const LEDGERS = ledgers("20260928-105142-bench")
const PROJECTS = projects("20260928-112920-bench")
// Each variant's child is a region or project agent's first request; its parent is the root's request whose reply was the task call.
const R = requests({
  ledgersTask: ["20260928-105142-bench/20260928-105901-trial012-before-agent-fork-rep2", "root", 0],
  projectsTask: ["20260928-112920-bench/20260928-112920-trial001-before-agent-fork-rep1", "root", 0],
  result: ["20260928-105142-bench/20260928-105458-trial006-result-task-fork-rep1", "root › maunga_region", 0],
  resultParent: ["20260928-105142-bench/20260928-105458-trial006-result-task-fork-rep1", "root", 2],
  before: ["20260928-105142-bench/20260928-110201-trial016-before-task-fork-rep1", "root › maunga", 0],
  beforeParent: ["20260928-105142-bench/20260928-110201-trial016-before-task-fork-rep1", "root", 2],
  call: ["20260928-112923-bench/20260928-112923-trial001-call-agent-fork-rep1", "root › datacentral", 0],
  callParent: ["20260928-112923-bench/20260928-112923-trial001-call-agent-fork-rep1", "root", 2],
  fresh: ["20260928-105145-bench/20260928-105145-trial001-before-agent-fresh-rep1", "root › maunga_region", 0],
  full: ["20260928-111113-bench/20260928-111113-trial001-full-stop-fork-rep1", "root › maunga_region", 0],
  fullParent: ["20260928-111113-bench/20260928-111113-trial001-full-stop-fork-rep1", "root", 2],
  firstBefore: ["20260928-113457-bench/20260928-113457-trial001-before-stop-fork-rep1", "root › maunga", 0],
  firstBeforeParent: ["20260928-113457-bench/20260928-113457-trial001-before-stop-fork-rep1", "root", 2],
})
const SCORE_LEDGERS = excerpt("../src/ledgers.rs:/pub fn score(/")
const SCORE_PROJECTS = excerpt("../src/projects.rs:/pub fn score(/")
const VALIDITY = excerpt("../src/bench.rs:/pub fn trial_is_valid(/")
const WORDS = excerpt("../src/framing.rs:/const AGENT_SPLIT/")

type Variant = {
  name: string
  how: string
  child: Request
  /** The parent's request whose reply was the task call: what the child shares with it is folded. */
  parent?: Request
  task: "ledgers" | "projects"
  rows: (t: any) => boolean
}

const claude = (pick: (t: any) => boolean) => (t: any) => onClaude(t) && pick(t)
const is = (v: string, first: boolean) => claude((t) => variant(t) === v && firstHarness(t) === first)

const THIS: Variant[] = [
  {
    name: "result · task", how: "The parent's task turn, then the shared part as that call's answer, with the breakpoint, then the own part as text.",
    child: R.result, parent: R.resultParent,
    task: "ledgers", rows: claude((t) => t.cut === "result" && t.mode === "fork"),
  },
  {
    name: "before · task", how: "The parent's messages before its task turn, then the shared part as user text, with the breakpoint, then the own part.",
    child: R.before, parent: R.beforeParent,
    task: "ledgers", rows: claude((t) => t.cut === "before" && t.mode === "fork" && !firstHarness(t)),
  },
  {
    name: "call, dropped", how: "The parent's task turn, whose arguments hold the shared text, with the breakpoint on it. Then the own part alone, as that call's answer.",
    child: R.call, parent: R.callParent,
    task: "projects", rows: claude((t) => t.cut === "call"),
  },
  {
    name: "fresh", how: "No parent context: the system prompt, then the shared part, with the breakpoint, then the own part.",
    child: R.fresh,
    task: "ledgers", rows: claude((t) => t.mode === "fresh"),
  },
]

const FIRST: Variant[] = [
  {
    name: "full", how: "The parent's task turn, then a tool result addressed to this child, holding its assignment. No shared part.",
    child: R.full, parent: R.fullParent,
    task: "ledgers", rows: claude((t) => t.cut === "full"),
  },
  {
    name: "before, first harness", how: "The parent's messages before its task turn, then one user message with the assignment. No shared part.",
    child: R.firstBefore, parent: R.firstBeforeParent,
    task: "ledgers", rows: claude((t) => t.cut === "before" && firstHarness(t)),
  },
]

function card(v: Variant): HTMLElement {
  if (v.parent !== undefined && !v.parent.reply!.some((b: any) => b.name === "task")) throw new Error(`${v.name}: ${v.parent.agent}'s request ${v.parent.n + 1} did not end in a task call`)
  return h("div", { class: "cut" },
    h("div", { class: "name" }, v.name),
    h("div", { class: "how" }, v.how),
    context(v.child, v.parent === undefined ? {} : { against: v.parent, as: `${v.parent.agent}'s context` }))
}

function question() {
  return [
    h("h3", {}, "2.1 The question"),
    h("p", {}, "You, 26 September:"),
    h("blockquote", {}, "I also want to bikeshed the exact framing of an agent that gets split up... is it a \"new agent\"? Maybe? Maybe it's a \"task\"? Maybe the subagent tool with fork type shows a ", h("i", {}, "different"), " tool call result to the child and the parent? Maybe the parent must produce a shared explanation, then N shorter specific explanations? And the cache point is after the shared one. Also see if it can be system part or user part or what."),
    h("p", {}, "And 28 September, on a child's context ending with the shared part as message 5 and its own part as message 6:"),
    h("blockquote", {}, "5 & 6 should be separate parts of the parent's single tool call, I think.", h("br"), h("br"), "I feel like it's elegant if 5 & 6 are tool result - we can try it. I'm not sure that an agent will sufficiently trust a tool result to narrow its task, but it might."),
    h("p", {}, "Each maybe became something the harness does or a knob it sweeps:"),
    h("ul", {},
      h("li", {}, h("b", {}, "A new agent, or a task: "), h("code", {}, "--identity agent|task"), ". ", toggle("the two wordings", () => code(WORDS))),
      h("li", {}, h("b", {}, "A shared explanation, then N shorter ones, with the cache point after the shared one: "), "the task call's required ", h("code", {}, "shared"), " and each child's own ", h("code", {}, "task"), ". The breakpoint goes after the shared part in every variant below."),
      h("li", {}, h("b", {}, "A different tool result for the child and the parent: "), h("code", {}, "--cut result"), ". The child's answer to the task call is the shared part; the parent's is the reports."),
      h("li", {}, h("b", {}, "Both parts in the tool result: "), "the variants ", h("code", {}, "call"), " and the first harness's ", h("code", {}, "full"), " put the own part there."),
      h("li", {}, h("b", {}, "A system part: "), "ruled out. Every agent has the identical system prompt, because Anthropic caches tools, then system, then messages, and any difference breaks the child's inherited prefix.")),
  ]
}

function variants() {
  return [
    h("h3", {}, "2.2 The variants"),
    h("p", {}, "The first request a child sent, in a real trial of each. Messages it shares with its parent are folded. ◆ marks a cache breakpoint. Click any line for its JSON as sent."),
    h("h4", {}, "This branch: a shared part, then an own part"),
    h("div", { class: "cuts" }, THIS.map(card)),
    h("p", { class: "s" }, "1.2 showed result and before with ", h("code", {}, "--identity agent"), "; here they are with ", h("code", {}, "task"), ". ", h("code", {}, "call"), " ran on the projects benchmark only, and was deleted from the harness after it; its trials are on disk. A fresh child is always an agent."),
    h("h4", {}, "The first harness (PR #13): no shared part, run again on your subscription"),
    h("div", { class: "cuts" }, FIRST.map(card)),
    h("p", { class: "s" }, "The first harness's ", h("code", {}, "--words stop|explained"), " varied only the stop instruction. Its ", h("code", {}, "own"), " cut, like ", h("code", {}, "full"), " but with the child's copy of the task call listing only itself, ran on OpenRouter only (2.6)."),
  ]
}

function benchmarks() {
  const kinds = (entries: any[], kind: string) => entries.filter((e) => e.kind === kind)
  return [
    h("h3", {}, "2.3 The two benchmarks"),
    h("p", {}, "Each gives the root a fixed task over a generated directory, and scores what the agents did mechanically, from the recorded tool calls and reports."),
    h("h4", {}, h("code", {}, "--task ledgers"), ": the task forces the tree"),
    h("p", {}, "The root is told to read the policy, fork one agent per region, and have each fork one per branch. The directory:"),
    h("table", { class: "grid" }, LEDGERS.files.map((f) => h("tr", {}, h("td", {}, h("code", {}, f.path)), h("td", { class: "num" }, int(f.bytes) + " bytes")))),
    h("p", {}, "Each ledger is about forty dated amounts. ", toggle("kowhai.txt", () => h("pre", { class: "text" }, LEDGERS.ledger)), ". POLICY.md is thirty sections of manual prose; only section 7 changes the arithmetic, and only the root is told to read it. ", toggle("section 7", () => h("pre", { class: "text" }, LEDGERS.section7)), ". The root's first message, exactly:"),
    h("pre", { class: "text" }, R.ledgersTask.messages[0].content[0].text),
    h("h4", {}, h("code", {}, "--task projects"), ": the task does not say how deep to go"),
    h("p", {}, "Your clone-survey question, over a directory shaped like your work clones. The root's first message, exactly:"),
    h("pre", { class: "text" }, R.projectsTask.messages[0].content[0].text),
    h("table", { class: "grid" },
      h("tr", {}, ["project", "clones", "worktrees", "look-alikes, not checkouts"].map((c) => h("th", {}, c))),
      PROJECTS.map((p) => h("tr", {},
        h("td", {}, h("code", {}, `work/${p.project}`)),
        h("td", { class: "num" }, kinds(p.entries, "clone").length),
        h("td", { class: "num" }, kinds(p.entries, "worktree").length),
        h("td", {}, kinds(p.entries, "look-alike").map((e) => h("div", { class: "mono s" }, `${e.name}/  ${e.has}`)))))),
    h("p", {}, "A worktree's ", h("code", {}, ".git"), " is a file pointing into another clone of the same project, so its branch is read from there. ", toggle("every checkout and its branch", () => h("table", { class: "grid" }, PROJECTS.flatMap((p) => p.entries.filter((e) => e.kind !== "look-alike").map((e) => h("tr", {}, h("td", { class: "mono" }, `${p.project}/${e.name}`), h("td", {}, e.kind), h("td", { class: "mono" }, e.branch))))))),
    h("h4", {}, "Scoring"),
    h("ul", {},
      h("li", {}, h("b", {}, "Over-reach"), " is what an agent did, not what it wrote. A branch agent over-reaches if it read another branch's ledger, called ", h("code", {}, "task"), ", or claimed another branch's total. A project agent over-reaches if it called ", h("code", {}, "task"), ", listed or read another project's directory, or claimed a branch for another project's checkout."),
      h("li", {}, h("b", {}, "A tree as asked"), " is 2 regions of 3 branch agents, or 1 agent per project, and nothing deeper."),
      h("li", {}, h("b", {}, "Correct"), " is every total, or every branch and every count, right in the root's final message."),
      h("li", {}, h("b", {}, "First-request cache"), " is the share of a child's first request read from the cache: the direct measure of whether it inherited its parent's prefix."),
      h("li", {}, h("b", {}, "Shared-part hits"), " counts, over every scope, the siblings after the first whose first request read the shared part from the cache.")),
    h("p", {}, toggle("the ledgers scorer", () => code(SCORE_LEDGERS)), " · ", toggle("the projects scorer", () => code(SCORE_PROJECTS)), " · ", toggle("what makes a trial invalid", () => code(VALIDITY)), ": a provider fault is invalid and left out; a tree that ran into its own cap or turn limit is kept."),
  ]
}

function cells(list: any[], task: "ledgers" | "projects"): HTMLElement[] {
  const mine = list.filter((t) => t.bench === task)
  const valid = mine.filter((t) => t.valid)
  if (mine.length === 0) return [h("td", { class: "none", colspan: 6 }, "not run")]
  const [over, of] = task === "ledgers" ? ["leaf_overreach", "leaves"] : ["project_overreach", "projects"]
  const bad = valid.reduce((a, t) => a + t[over], 0) > 0
  const share = firstShare(valid)
  return [
    h("td", { class: "num" }, valid.length === mine.length ? mine.length : `${valid.length} of ${mine.length}`),
    h("td", { class: `num ${bad ? "bad" : "good"}` }, ratio(valid, over, of)),
    h("td", { class: "num" }, count(valid, (t) => t.structure_ok)),
    h("td", { class: "num" }, count(valid, (t) => t.correct)),
    h("td", { class: "num" }, share === undefined ? "—" : pct(share)),
    h("td", { class: "num" }, firstHarness(valid[0] ?? mine[0]) ? "—" : ratio(valid, "shared_hits", "later_siblings")),
  ]
}

const ROWS: [string, (t: any) => boolean][] = [
  ["result · agent", is("result · agent", false)], ["result · task", is("result · task", false)],
  ["before · agent", is("before · agent", false)], ["before · task", is("before · task", false)],
  ["call · agent", is("call · agent", false)], ["call · task", is("call · task", false)],
  ["fresh", claude((t) => t.mode === "fresh")],
  ["full · stop", is("full · stop", true)], ["full · explained", is("full · explained", true)],
  ["before · stop", is("before · stop", true)], ["before · explained", is("before · explained", true)],
]

function results() {
  const mine = VALID.filter(onClaude)
  const sum = (list: any[], key: string) => list.reduce((a, t) => a + t[key], 0)
  const leaves = (pick: (t: any) => boolean) => { const l = mine.filter((t) => t.bench === "ledgers" && pick(t)); return `${sum(l, "leaf_overreach")} of ${sum(l, "leaves")}` }
  const projs = (pick: (t: any) => boolean) => { const l = mine.filter((t) => t.bench === "projects" && pick(t)); return `${sum(l, "project_overreach")} of ${sum(l, "projects")}` }
  const userText = (t: any) => !firstHarness(t) && t.cut !== "call"
  const heads = ["trials", "over-reached", "trees as asked", "correct", "first-request cache", "shared-part hits"]
  return [
    h("h3", {}, "2.4 What the trials found"),
    h("p", {}, `Claude Sonnet 5 on your subscription, 28 September: ${TRIALS.filter(onClaude).length} trials, ${mine.length} valid. Most cells are five trials; the trials column says where not.`),
    h("ol", { class: "findings" },
      h("li", {}, h("b", {}, "An own part in a tool result fails."), ` Under full, ${leaves((t) => t.cut === "full")} branch agents over-reached. Under call · agent, ${projs(is("call · agent", false))} project agents did. The child reads the tool result as its own task call coming back broken, and redoes the split (2.5).`),
      h("li", {}, h("b", {}, "An own part in user text holds."), ` Under result, before and fresh, ${leaves(userText)} branch agents over-reached, and ${projs((t) => userText(t) && t.mode === "fork")} forked project agents. Fresh project agents over-reached ${projs((t) => t.mode === "fresh")}: in three trials the datacentral agent split its 25 checkouts across four agents of its own. The first harness's before, also user text, had ${leaves((t) => firstHarness(t) && t.cut === "before")}.`),
      h("li", {}, h("b", {}, "Identity mattered only under a tool-result own part."), ` call · agent: ${projs(is("call · agent", false))}; call · task: ${projs(is("call · task", false))}. Under result and before, agent and task both scored 0.`),
      h("li", {}, h("b", {}, "result and before were not separated by either benchmark."), " Both built every tree as asked with no over-reach, on both benchmarks, with both identities. The one difference in correctness is one result · agent ledgers trial whose awa region was off by exactly 1,000. The benchmarks are saturated for these two: to choose between them needs a task that tempts a child harder, or a different criterion. One difference is by construction: result sends the shared text twice, once in the task call the child inherits and once as that call's answer.")),
    h("table", { class: "grid results" },
      h("tr", {}, h("th", {}), h("th", { colspan: 6, class: "group" }, "ledgers"), h("th", { colspan: 6, class: "group" }, "projects")),
      h("tr", {}, h("th", {}, "variant"), heads.map((c) => h("th", {}, c)), heads.map((c) => h("th", {}, c))),
      ROWS.map(([name, pick], i) => h("tr", { class: i === 7 ? "split" : undefined }, h("td", {}, h("code", {}, name)), cells(TRIALS.filter(pick), "ledgers"), cells(TRIALS.filter(pick), "projects")))),
    h("p", { class: "s" }, "The last four rows are the first harness, which had no shared part, so they have no shared-part hits. Over-reached counts branch agents on ledgers and project agents on projects."),
  ]
}

function words() {
  const said = voices(claude((t) => t.cut === "full" || t.cut === "call"))
  const shown = said.filter((v) => v.text.length < 200)
  const quote = (v: (typeof said)[number]) => h("p", {}, "“" + v.text + "” ", h("span", { class: "tok" }, `${v.agent}, ${variant(v.trial)}, ${v.trial.bench}`))
  return [
    h("h3", {}, "2.5 In their own words"),
    h("p", {}, `${said.length} agents below the root, under full and call, ended on a message saying the task call had echoed instead of returning. The short ones:`),
    shown.map(quote),
    h("p", {}, toggle(`all ${said.length}`, () => h("div", {}, said.map(quote)))),
  ]
}

function firstRound() {
  const openrouter = VALID.filter((t) => !onClaude(t))
  const model = (t: any) => (t.model.includes("luna") ? "luna" : "sonnet")
  const row = (t: any) => (t.mode === "fresh" ? "fresh" : t.cut)
  const cell = (list: any[]) => {
    const share = firstShare(list)
    return [
      h("td", { class: "num" }, list.length),
      h("td", { class: "num " + (list.reduce((a, t) => a + t.leaf_overreach, 0) > list.reduce((a, t) => a + t.leaves, 0) / 4 ? "bad" : "good") }, ratio(list, "leaf_overreach", "leaves")),
      h("td", { class: "num" }, count(list, (t) => t.structure_ok)),
      h("td", { class: "num" }, count(list, (t) => t.correct)),
      h("td", { class: "num" }, share === undefined ? "—" : pct(share)),
    ]
  }
  return [
    h("h3", {}, "2.6 The first round, on OpenRouter"),
    h("p", {}, `The first harness, on your OpenRouter key, 24–25 September, ledgers only, Sonnet 5 pinned to Bedrock or Vertex and Luna pinned to Azure: ${openrouter.length} valid trials, both word variants together. The first grid's $0.15 cap stopped most Sonnet trees before they finished, which is why few have correct totals. On Sonnet, before had fewer leaves over-reach than full and own.`),
    h("table", { class: "grid" },
      h("tr", {}, ["cut", "model", "trials", "leaves over-reached", "trees as asked", "correct", "first-request cache"].map((c) => h("th", {}, c))),
      ["full", "own", "before", "fresh"].flatMap((cut) => ["sonnet", "luna"].map((m) => h("tr", {}, h("td", {}, h("code", {}, cut)), h("td", {}, m), cell(openrouter.filter((t) => row(t) === cut && model(t) === m)))))),
  ]
}

export function framing(): HTMLElement {
  return part(2, "framing", "The framing experiment",
    "How a split child is told which part of the work is its own. Your question; the variants it became, each as a real child's context; the two benchmarks and how they score; what the trials found; the children's own words where it failed; and the first round, on OpenRouter.",
    question(), variants(), benchmarks(), results(), words(), firstRound(),
    cameTo("The own part must be user text. Put it in a tool result and the child takes it for its own call's broken answer and does the split again, whatever it is called. With the own part as user text, the shared part can go either way — as the call's answer (result) or as user text before the call (before) — and neither benchmark tells them apart. The shared-part breakpoint worked in every scope: every later sibling read it from the cache."))
}
