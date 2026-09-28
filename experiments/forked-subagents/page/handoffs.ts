// Part 1: the one handoff type, and the exact context each receiver of one got.

import { excerpt } from "underview/excerpt" with { type: "macro" }
import { requests } from "./data" with { type: "macro" }
import { h, code, toggle, part, cameTo, int } from "./ui"
import { context } from "./context"
import type { Request } from "./data"

// Two ledgers trials, one per cut; a run that handed over; a chat with a child that talked with the user.
const R = requests({
  beforeChild: ["20260928-105142-bench/20260928-105901-trial012-before-agent-fork-rep2", "root › maunga_region", 0],
  beforeSibling: ["20260928-105142-bench/20260928-105901-trial012-before-agent-fork-rep2", "root › awa_region", 0],
  beforeTaskCall: ["20260928-105142-bench/20260928-105901-trial012-before-agent-fork-rep2", "root", 2],
  beforeResumed: ["20260928-105142-bench/20260928-105901-trial012-before-agent-fork-rep2", "root", 3],
  resultChild: ["20260928-105142-bench/20260928-105337-trial004-result-agent-fork-rep4", "root › maunga", 0],
  resultSibling: ["20260928-105142-bench/20260928-105337-trial004-result-agent-fork-rep4", "root › awa", 0],
  resultResumed: ["20260928-105142-bench/20260928-105337-trial004-result-agent-fork-rep4", "root", 3],
  leaf: ["20260928-105142-bench/20260928-105337-trial004-result-agent-fork-rep4", "root › maunga › rimu", 0],
  leafParent: ["20260928-105142-bench/20260928-105337-trial004-result-agent-fork-rep4", "root › maunga", 1],
  handoverLast: ["20260928-125859-run", "root", 2],
  handoverNext: ["20260928-125859-run", "root", 3],
  talkingFirst: ["20260928-125453-chat", "root › root'", 0],
  talkingDone: ["20260928-125453-chat", "root › root'", 3],
  talkingResumed: ["20260928-125453-chat", "root", 2],
  talkingSibling: ["20260928-125453-chat", "root › face-reader", 0],
})

const HANDOFF = excerpt("../src/handoff.rs:/pub struct Handoff {/")
const DELIVER = excerpt("../src/handoff.rs:/pub fn deliver(/")
const REFUSED = excerpt("../src/framing.rs:/pub const USER_FACING_REFUSED/")

type Direction = { name: string; made: string; shared: string; attachments: string; own: string; breakpoint: string; onto: string; at: ReturnType<typeof excerpt> }

const DIRECTIONS: Direction[] = [
  {
    name: "down", made: "a task call, once per child",
    shared: "who the work went to, then the parent's shared text", attachments: "the call's attachments",
    own: "the child's name, its task, the stop instruction, and reports it waits on", breakpoint: "yes",
    onto: "forked: the parent's messages, cut before or after its task turn; fresh: the system prompt alone",
    at: excerpt("../src/agent.rs:/let down = Handoff {/"),
  },
  {
    name: "down, to the child that talks with you", made: "the same call, in chat",
    shared: "the same bytes as its siblings", attachments: "the same",
    own: "you carry on this conversation with the user", breakpoint: "yes",
    onto: "the parent's messages; always forked",
    at: excerpt("../src/framing.rs:/pub fn conversation_part(/"),
  },
  {
    name: "up", made: "the scope, once every child has ended",
    shared: "every child's report, in the order the parent declared them", attachments: "none",
    own: "none: the parent's turn carries on", breakpoint: "no",
    onto: "the parent's own messages, as the answer to its task call",
    at: excerpt("../src/agent.rs:/let up = Handoff {/"),
  },
  {
    name: "across", made: "a handover call",
    shared: "what the agent wrote for itself", attachments: "the call's attachments",
    own: "its next step", breakpoint: "no: nothing else shares it",
    onto: "the system prompt alone",
    at: excerpt("../src/agent.rs:/let fresh = Handoff {/"),
  },
]

function table() {
  const cols: (keyof Direction)[] = ["made", "shared", "attachments", "own", "breakpoint", "onto"]
  const head = ["", "made by", "shared", "attachments", "own", "breakpoint after shared", "delivered onto"]
  return h("table", { class: "grid handoffs" },
    h("tr", {}, head.map((c) => h("th", {}, c))),
    DIRECTIONS.map((d) => {
      const row = h("tr", {}, h("td", {}, h("b", {}, d.name)), cols.map((c) => h("td", {}, d[c] as string)))
      const where = h("td", { colspan: 7 }, code(d.at))
      let shown: HTMLElement | null = null
      row.classList.add("click")
      row.addEventListener("click", () => {
        if (shown !== null) (shown.remove(), (shown = null))
        else row.after((shown = h("tr", {}, where)))
      })
      return row
    }))
}

const reads = (r: Request) => `read ${int(r.usage!.cached)} from the cache and wrote ${int(r.usage!.written)}`

function downward() {
  const pair = (cut: string, child: Request, sibling: Request, parent: Request, how: string) => {
    return h("div", { class: "cut" },
      h("div", { class: "name" }, h("code", {}, `--cut ${cut}`)),
      h("div", { class: "how" }, how),
      context(child, { against: parent, as: `${parent.agent}'s context` }),
      h("p", { class: "s" }, `${sibling.agent.split(" › ").pop()} started first: its first request ${reads(sibling)}. ${child.agent.split(" › ").pop()} started when that request came back: its first ${reads(child)}.`))
  }
  return [
    h("h3", {}, "1.2 Down: a task call hands each child its part"),
    h("p", {}, "A ledgers benchmark trial on your subscription. The root read the policy, listed the two regions, and called ", h("code", {}, "task"), " for one agent per region. Below is the first request the ", h("code", {}, "maunga"), " region agent sent, under each cut. Messages it shares with the root are folded; what is new is shown whole. Click any line for its JSON as sent."),
    h("div", { class: "cuts" },
      pair("before", R.beforeChild, R.beforeSibling, R.beforeResumed, "The root's messages up to, not including, its task turn. Then the shared part as user text, with the breakpoint, then the own part."),
      pair("result", R.resultChild, R.resultSibling, R.resultResumed, "The root's messages through its task turn. The shared part is the answer to the task call, with the breakpoint on it. The own part follows as text in the same user message.")),
    h("p", {}, "Every level of forking leaves one breakpoint. ", toggle("One level further down, under result: rimu's first request", () => context(R.leaf, { against: R.leafParent, as: "maunga's context" })), " carries the root's breakpoint and maunga's."),
  ]
}

function upward() {
  return [
    h("h3", {}, "1.3 Up: the reports answer the parent's call"),
    h("p", {}, "The same trial under ", h("code", {}, "before"), ". The root was suspended inside its task call. It is resumed with one tool result: every report, in the order it declared the children. The up handoff has no own part and no breakpoint."),
    context(R.beforeResumed, { against: R.beforeTaskCall, as: "its previous request" }),
  ]
}

function acrossward() {
  return [
    h("h3", {}, "1.4 Across: a handover starts a fresh context"),
    h("p", {}, "The ledgers task as a plain run: ", h("code", {}, "forks run --dir <a ledgers fixture> --handover-at 4000 --mode fork \"<the ledgers task>\""), ". The root's third request carried 10,013 tokens, so the harness appended a line telling it to hand over, and the root called ", h("code", {}, "handover"), ". Its first context ends like this:"),
    context(R.handoverLast, { from: 4 }),
    h("p", {}, "Its second context is the system prompt, what it wrote for itself as user text, and its next step as user text. It started with no attachments, because the root asked for none. Every earlier breakpoint ended inside the first context, so this request read nothing from the cache."),
    context(R.handoverNext),
    h("p", {}, "Each of the six leaves in the same run handed over once as well. ", h("code", {}, "agents/<path>.md"), " shows every context an agent had."),
  ]
}

function talking() {
  const shared = R.talkingFirst.messages[2].content[1].text as string
  return [
    h("h3", {}, "1.5 The child that carries on talking with you"),
    h("p", {}, "A ", h("code", {}, "forks chat"), " over this crate's ", h("code", {}, "src/"), ", driven by a script standing in for you. It typed: ", h("q", {}, R.talkingResumed.messages[0].content[0].text), " The root called ", h("code", {}, "task"), " for two fresh readers. Because the root was talking with you, the harness also started ", h("code", {}, "root'"), ": a fork, with the same shared part as its siblings and its own part saying it carries the conversation on."),
    context(R.talkingFirst, { against: R.talkingResumed, as: "the root's context" }),
    h("p", {}, "The fresh readers got the same shared part straight after the system prompt. ", toggle("face-reader's first request", () => context(R.talkingSibling)), " — its shared text is byte-identical to root''s: ", h("code", {}, JSON.stringify(shared.slice(0, 60)) + "…"), "."),
    h("p", {}, "The script asked ", h("code", {}, "root'"), " one question while the readers worked, then typed ", h("code", {}, "/done"), ". The harness asked it for its report:"),
    context(R.talkingDone, { from: 3 }),
    h("p", {}, "The root was resumed once all three had ended, with all three reports in one tool result:"),
    context(R.talkingResumed, { from: 3 }),
    h("p", {}, "The root's reply calls ", h("code", {}, "root'"), " \"another instance of me in that branch\" and says it has no visibility beyond the summary. That is true: the up handoff carries the report, not the conversation."),
    h("p", {}, "The parent sets ", h("code", {}, "user_facing: false"), " to split without one. An agent that is not talking with you is refused one, with this tool result — scenario ", h("code", {}, "an autonomous agent cannot start a user facing child"), ":"),
    code(REFUSED),
  ]
}

export function handoffs(): HTMLElement {
  return part(1, "handoffs", "The handoff model",
    "What one context hands the next. It is one type, delivered three ways: down to a child, up to the parent, and across to a fresh context. The child that talks with you is a down handoff with its own part changed. Each is shown as the exact context the receiver got, from runs on your subscription.",
    h("h3", {}, "1.1 One type"),
    h("p", {}, "A handoff has four fields. ", h("code", {}, "shared"), " is the same bytes for every receiver. ", h("code", {}, "attachments"), " are files, shown after the shared part as ", h("code", {}, "read_file"), " calls the receiver did not make. ", h("code", {}, "own"), " is this receiver's part, always user text. ", h("code", {}, "breakpoint"), " puts a cache breakpoint after everything the receivers share."),
    code(HANDOFF),
    h("p", {}, h("code", {}, "deliver"), " appends a handoff to a context. When a tool call is waiting for its answer, the shared part is that answer. Otherwise it is user text. ", toggle("deliver", () => code(DELIVER))),
    h("p", {}, "Each row is one place the harness makes a handoff. Click a row for the lines that make it."),
    table(),
    downward(), upward(), acrossward(), talking(),
    cameTo("Compaction, a task and its response are one thing: a shared part, attachments, an own part and a breakpoint, appended to a context. Only what they are appended to differs. The breakpoint sits after the shared part wherever there are siblings to share it, so the siblings after the first read it from the cache. No recorded run used attachments; the scenario ", h("code", {}, "attachments are read once and shown to every child as calls it did not make"), " is the only evidence for them."))
}
