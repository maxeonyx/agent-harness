// Part 1: what was run, down to the exact requests.

import { h, json, code, toggle, part, cameTo, usd, pct, int } from "./ui"
import { flow, tail, fixture } from "./data" with { type: "macro" }
import { excerpt } from "underview/excerpt" with { type: "macro" }
import { TRIALS, VALID, short, rowOf, CUTS } from "./trials"

const CLEAN = flow("20260925-003053-trial001-before-stop-fork-rep1")
const FIXTURE = fixture()
const TAILS: Record<string, ReturnType<typeof tail>> = {
  full: tail("20260924-232819-trial001-full-stop-fork-rep1", 1, 2),
  own: tail("20260924-233629-trial005-own-stop-fork-rep1", 1, 2),
  before: tail("20260925-003053-trial001-before-stop-fork-rep1", 1, 2),
  fresh: tail("20260925-003334-trial001-full-explained-fresh-rep1", 1, 2),
}
const SCORE = excerpt("../src/bench.rs:/pub fn score(/")
const VALIDITY = excerpt("../src/bench.rs:/pub fn trial_is_valid(/")
const CHILD_CONTEXT = excerpt("../src/agent.rs:/fn child_context(/")

// The benchmarks as launched. Two were started in the same second and share one directory.
const LAUNCHES: [string, string][] = [
  ["20260924-232651-bench", "forks bench --grid openai/gpt-5.6-luna@azure --cut full --words stop --mode fork --reps 1 --budget 0.3"],
  ["20260924-232819-bench", "forks bench --cut full --words stop --mode fork --reps 1 --budget 0.3"],
  ["20260924-232944-bench", "forks bench --grid openai/gpt-5.6-luna@azure --cut full,own,before --words stop,explained --mode fork --reps 3 --budget 0.6\nforks bench --cut full,own,before --words stop,explained --mode fork --reps 2 --budget 1.8"],
  ["20260924-234420-bench", "forks bench --words stop,explained --mode fresh --reps 2 --budget 0.6"],
  ["20260925-000035-bench", "forks bench --grid openai/gpt-5.6-luna@azure --words stop,explained --mode fresh --reps 3 --budget 0.3"],
  ["20260925-003051-bench", "forks bench --grid openai/gpt-5.6-luna@azure --cut before --words stop,explained --mode fork --reps 3 --budget 0.3"],
  ["20260925-003053-bench", "forks bench --cut before --words stop,explained --mode fork --reps 1 --max-cost 0.40 --budget 0.8"],
  ["20260925-003334-bench", "forks bench --words explained --mode fresh --reps 1 --max-cost 0.40 --budget 0.4"],
  ["20260925-003919-bench", "forks bench --grid openai/gpt-5.6-luna@azure --words stop,explained --mode fresh --reps 3 --budget 0.3"],
]

export function runs(): HTMLElement {
  return part(1, "runs", "What was run",
    "Four kinds of run, then the benchmark's task, then one run request by request, then exactly what a child is shown under each of the four cuts, how a trial is scored, and what the trials found.",
    kinds(), task(), exactFlow(), cuts(), scoring(), results(),
    cameTo("Forked children really are sent their parent's messages byte for byte, and the provider really serves them from cache. What varies between cuts is only the last one or two messages — and that is what decided whether Sonnet's children stayed in their lane."))
}

function kinds() {
  const byBench = (b: string) => TRIALS.filter((t) => t.bench === b)
  return [
    h("h3", {}, "1.1 Four kinds of run"),
    h("table", { class: "grid" },
      h("tr", {}, ["what", "exactly", "size", "cost"].map((c) => h("th", {}, c))),
      h("tr", {}, h("td", {}, "the probe, before any harness"), h("td", {}, h("code", {}, "python3 probe/probe.py [amazon-bedrock | google-vertex]")), h("td", {}, "8 runs × 3 requests"), h("td", { class: "num" }, "≈ $0.10")),
      h("tr", {}, h("td", {}, "the builder's smoke runs"), h("td", {}, h("code", {}, "forks run …"), " ×5, ", h("code", {}, "forks bench --reps 1"), " ×2"), h("td", {}, "one bench was killed mid-run by a shell timeout"), h("td", { class: "num" }, "$1.38")),
      h("tr", {}, h("td", {}, "the benchmark grid"), h("td", {}, LAUNCHES.map(([b, cmd]) => h("div", {}, h("code", { style: { whiteSpace: "pre-wrap" } }, cmd), h("span", { class: "tok" }, `  → runs.ignore/${b}, ${byBench(b).length} trials`)))), h("td", {}, `${TRIALS.length} trials, ${VALID.length} valid`), h("td", { class: "num" }, usd(TRIALS.reduce((a, t) => a + t.cost, 0)))),
      h("tr", {}, h("td", {}, "rescoring, offline"), h("td", {}, h("code", {}, "forks rescore runs.ignore/<bench> --json …")), h("td", {}, "every trial above, again, from its recorded wire"), h("td", { class: "num" }, "$0"))),
  ]
}

function task() {
  const rootTask = CLEAN.messages[CLEAN.requests[0].messages[1]]
  return [
    h("h3", {}, "1.2 The benchmark's task"),
    h("p", {}, "Every trial gets the same directory and the same first user message. The directory:"),
    h("table", { class: "grid" }, FIXTURE.files.map((f) => h("tr", {}, h("td", {}, h("code", {}, f.path)), h("td", { class: "num" }, int(f.bytes) + " bytes")))),
    h("p", {}, "Each ledger is about forty lines like these. ", toggle("the whole of kowhai.txt", () => h("pre", { class: "text" }, FIXTURE.ledger))),
    h("pre", { class: "text" }, FIXTURE.ledger.split("\n").slice(0, 14).join("\n") + "\n…"),
    h("p", {}, "POLICY.md is thirty sections of accounts-manual prose; only section 7 changes the arithmetic, and only the root is told to read it. ", toggle("section 7", () => h("pre", { class: "text" }, FIXTURE.section7))),
    h("p", {}, "The root's first user message, exactly:"),
    h("pre", { class: "text" }, rootTask.content),
  ]
}

function exactFlow() {
  const firstSeen = new Map<string, number>()
  CLEAN.requests.forEach((r, i) => r.messages.forEach((m) => firstSeen.has(m) || firstSeen.set(m, i)))
  // A reply is sent back as the next message of that agent's next request; an agent's last reply is its report.
  const replyId = (r: any) => CLEAN.requests.find((q) => q.agent === r.agent && q.sent > r.sent)?.messages[r.messages.length]
  const rows = CLEAN.requests.map((r, i) => {
    const row = h("div", { class: "row" })
    const opened = h("div", { class: "open" })
    let on: HTMLElement | null = null
    const show = (chip: HTMLElement, id: string, message: any) => {
      on?.classList.remove("on")
      if (on === chip) return (on = null), opened.replaceChildren()
      on = chip
      chip.classList.add("on")
      const uses = CLEAN.requests.flatMap((q, j) => (q.messages.includes(id) ? [j + 1] : []))
      opened.replaceChildren(h("div", { class: "meta" }, `${id} · ${message.role} · sent in request${uses.length > 1 ? "s" : ""} ${uses.join(", ")}`), json(message))
    }
    const chips = r.messages.map((id) => {
      const m = CLEAN.messages[id]
      const chip = h("span", { class: `chip ${m.role}${firstSeen.get(id) === i ? " first" : ""}`, title: m.role }, id.slice(1))
      chip.addEventListener("click", () => show(chip, id, m))
      return chip
    })
    const next = replyId(r)
    const reply = h("span", { class: "chip reply", title: next ? "the reply, sent back as this message" : "the reply: this agent's report" }, next ? "→ " + next.slice(1) : "→ report")
    reply.addEventListener("click", () => show(reply, next ?? "report", next ? CLEAN.messages[next] : r.reply))
    const fresh = r.prompt - r.cached - r.written
    row.append(
      h("span", { class: "tok" }, i + 1),
      h("span", { class: "tok" }, "+" + r.sent.toFixed(1) + "s"),
      h("span", { class: "agent", title: r.agent }, r.agent.replace(/^root › /, "  ").replace(/ › /g, " › ")),
      h("span", { class: "chips" }, chips, reply),
      h("span", {},
        h("span", { class: "tok" }, h("b", {}, int(r.prompt)), " in: ", h("span", { class: "c" }, int(r.cached) + " cached"), " · ", h("span", { class: "w" }, int(r.written) + " written"), fresh > 20 ? ` · ${int(fresh)} plain` : "", " · ", h("span", { class: "o" }, int(r.out) + " out")),
        h("div", { class: "bar" }, h("i", { style: { width: pct(r.cached / r.prompt), background: "var(--cached)" } }), h("i", { style: { width: pct(r.written / r.prompt), background: "var(--new)" } }))),
      opened)
    return row
  })
  const total = CLEAN.requests.reduce((a, r) => a + r.cost, 0)
  return [
    h("h3", {}, "1.3 One run, request by request"),
    h("p", {}, `Sonnet, the `, h("code", {}, "before"), ` cut, `, h("code", {}, "stop"), ` words — the run that built the tree as asked and got every total right. ${CLEAN.requests.length} requests, ${usd(total)}. Every request has this envelope; only `, h("code", {}, "messages"), ` changes:`),
    json(CLEAN.envelope),
    h("p", {}, toggle("the three tool schemas, exactly as sent", () => json(CLEAN.tools)), " — byte-identical in every request of every agent, like the system message, because the provider caches tools, then system, then messages, and one difference moves the start of the cache."),
    h("p", {}, "Each row is one request. Each numbered box is one message, and the same number is the same bytes wherever it appears — so a child's row starting with the same numbers as its parent's ", h("i", {}, "is"), " the fork. A box with a dark outline is sent for the first time in that row. Click any box for the exact JSON; the dashed box is the reply, which becomes a message in that agent's next request."),
    h("div", { class: "legend" }, ["system", "user", "assistant", "tool"].map((r) => h("span", {}, h("span", { class: `chip ${r}` }, "n"), r)), h("span", {}, h("i", { class: "sw", style: { background: "var(--cached)" } }), "prompt tokens read from cache"), h("span", {}, h("i", { class: "sw", style: { background: "var(--new)" } }), "written to cache"), h("span", {}, "(each request also bills 2 tokens as plain input)")),
    h("div", { class: "flow" }, h("div", { class: "row head" }, h("span", {}, "#"), h("span", {}, "sent"), h("span", {}, "agent"), h("span", {}, "messages sent → reply"), h("span", {}, "tokens, as the provider billed them")), rows),
  ]
}

const CUT_HOW: Record<string, string> = {
  full: "the parent's messages, through its assistant turn that called task, then a tool result addressed to this child",
  own: "the same, but in the child's copy of that assistant turn the task arguments list only this child",
  before: "the parent's messages up to, not including, that assistant turn, then a user message with the assignment",
  fresh: "no parent context: the system message, then a user message with the assignment",
}

function cuts() {
  return [
    h("h3", {}, "1.4 What a child is shown, under each cut"),
    h("p", {}, "The first request a region agent sent, in a real Sonnet trial of each cut. Everything before these last messages is the parent's messages, unchanged (fresh has nothing before them). ", toggle("the code that cuts them", () => code(CHILD_CONTEXT))),
    h("div", { class: "cuts" }, CUTS.map((cut) => {
      const t = TAILS[cut]
      const list = VALID.filter((x) => rowOf(x) === cut && short(x.model) === "sonnet")
      const over = list.reduce((a, x) => a + x.leaf_overreach, 0), leaves = list.reduce((a, x) => a + x.leaves, 0)
      return h("div", { class: "cut" },
        h("div", { class: "name" }, cut),
        h("div", { class: "how" }, CUT_HOW[cut]),
        h("div", { class: "tok" }, `${t.agent}, message${t.total > 2 ? "s " + (t.total - 1) + "–" + t.total : "s 1–2"} of ${t.total}:`),
        t.messages.map((m: any) => json(m)),
        h("div", { class: "res" }, `sonnet under this cut: ${over} of ${leaves} leaves over-reached, ${list.filter((x) => x.structure_ok).length} of ${list.length} trees built as asked`))
    })),
  ]
}

function scoring() {
  return [
    h("h3", {}, "1.5 How a trial is scored"),
    h("p", {}, "From what agents actually did — a read the limb answered, a total actually claimed, a ", h("code", {}, "task"), " call made — never from a model's judgement. A leaf over-reaches if it successfully read another branch's ledger, claimed another branch's total, or split itself again. A region over-reaches if it read any branch ledger itself. A tree is ", h("i", {}, "as asked"), " with exactly two regions of three leaves and nothing deeper. Totals are correct when every name in the root's final block has its exact value."),
    h("p", {}, toggle("the scorer", () => code(SCORE)), " · ", toggle("what makes a trial invalid", () => code(VALIDITY)), " — a provider fault (a rate limit that never lifted, say) is invalid and left out; a tree that ran into its spend cap is kept, because running away is what's being measured."),
  ]
}

function results() {
  const models = ["sonnet", "luna"]
  const cell = (list: any[]) => {
    const over = list.reduce((a, t) => a + t.leaf_overreach, 0), leaves = list.reduce((a, t) => a + t.leaves, 0)
    const c = list.reduce((a, t) => a + t.child_first_cached_in, 0), u = list.reduce((a, t) => a + t.child_first_uncached_in, 0)
    return [
      h("td", { class: "num" }, list.length),
      h("td", { class: "num " + (over / Math.max(leaves, 1) > 0.25 ? "bad" : "good") }, `${over}/${leaves}`),
      h("td", { class: "num" }, `${list.filter((t) => t.structure_ok).length}/${list.length}`),
      h("td", { class: "num" }, `${list.filter((t) => t.correct).length}/${list.length}`),
      h("td", { class: "num" }, list.filter((t) => t.fault_kind === "budget").length || ""),
      h("td", { class: "num" }, c + u ? pct(c / (c + u)) : "—"),
    ]
  }
  return [
    h("h3", {}, "1.6 What the trials found"),
    h("p", {}, "Valid trials only, both word variants together. ", h("i", {}, "Capped"), " counts trees stopped by their spend cap before they finished — which is also why most Sonnet trees have no correct totals: the $0.15 cap of the first grid was too tight even for a well-behaved Sonnet tree."),
    h("table", { class: "grid" },
      h("tr", {}, ["cut", "model", "trials", "leaves over-reached", "trees as asked", "totals correct", "capped", "child's first request from cache"].map((c) => h("th", {}, c))),
      CUTS.flatMap((cut) => models.map((m) => h("tr", {}, h("td", {}, h("code", {}, cut)), h("td", {}, m), cell(VALID.filter((t) => rowOf(t) === cut && short(t.model) === m)))))),
  ]
}
