// Part 3: every run the evidence comes from, and one of them request by request.

import { h, json, part, cameTo, toggle, pct, int } from "./ui"
import { flow } from "./data" with { type: "macro" }
import { TRIALS } from "./trials"

const CLEAN = flow("20260928-105142-bench/20260928-105901-trial012-before-agent-fork-rep2")

// Every benchmark as launched. `forks` is target/release/forks; `first` is the first harness, built from 16072b6^.
type Launch = [dir: string, command: string]

const OPENROUTER: Launch[] = [
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

const SUBSCRIPTION: Launch[] = [
  ["20260928-105142-bench", "forks bench --cut result,before --identity agent,task --mode fork --reps 5"],
  ["20260928-105145-bench", "forks bench --cut before --identity agent --mode fresh --reps 5"],
  ["20260928-112536-bench", "a smoke run of the projects benchmark"],
  ["20260928-112917-bench", "forks bench --task projects --cut result --identity agent,task --mode fork --reps 5"],
  ["20260928-112920-bench", "forks bench --task projects --cut before --identity agent,task --mode fork --reps 5"],
  ["20260928-112923-bench", "forks bench --task projects --cut call --identity agent,task --mode fork --reps 5"],
  ["20260928-112926-bench", "forks bench --task projects --cut before --identity agent --mode fresh --reps 5"],
]

const FIRST_AGAIN: Launch[] = [
  ["20260928-111113-bench", "first bench --cut full,before --words stop,explained --mode fork --reps 5\n  (stopped once full · stop was done)"],
  ["20260928-113454-bench", "first bench --cut full --words explained --mode fork --reps 5\n  (stopped at its third trial)"],
  ["20260928-113457-bench", "first bench --cut before --words stop --mode fork --reps 5"],
  ["20260928-113500-bench", "first bench --cut before --words explained --mode fork --reps 5"],
  ["20260928-115121-bench", "first bench --cut full --words explained --mode fork --reps 2"],
  ["20260928-115124-bench", "first bench --cut full --words explained --mode fork --reps 1"],
  ["20260928-121424-bench", "first bench --cut full --words stop --mode fork --reps 1"],
]

const RUNS: Launch[] = [
  ["20260928-104824-run", "forks run --cut result … \"Use the task tool to launch two agents at once: one reads face.rs and one reads limb.rs …\""],
  ["20260928-104906-run", "forks run --cut before … the same task"],
  ["20260928-125434-run", "forks run --dir <ledgers fixture> --handover-at 5000 --mode fork \"<the ledgers task>\""],
  ["20260928-125859-run", "forks run --dir <ledgers fixture> --handover-at 4000 --mode fork \"<the ledgers task>\""],
  ["20260928-125453-chat", "forks chat --dir src, typed into by handoffs.ignore/chat-drive.py"],
]

function launches(list: Launch[]) {
  const trials = (dir: string) => TRIALS.filter((t) => t.launch === dir)
  return h("td", {}, list.map(([dir, command]) => {
    const n = trials(dir).length
    return h("div", { class: "launch" }, h("code", {}, command), h("span", { class: "tok" }, `  → runs.ignore/${dir}${dir.endsWith("-bench") ? `, ${n} trial${n === 1 ? "" : "s"}` : ""}`))
  }))
}

function kinds() {
  const row = (what: string, how: HTMLElement, where: string) => h("tr", {}, h("td", {}, what), how, h("td", {}, where))
  return [
    h("h3", {}, "3.1 Every run"),
    h("p", {}, "Newest first. The two harnesses differ: ", h("i", {}, "the first harness"), " (PR #13) gave a child one assignment and said ", h("code", {}, "--words stop|explained"), "; this branch gives it a shared part and an own part and says ", h("code", {}, "--identity agent|task"), ". Every command below is what was run."),
    h("table", { class: "grid runs" },
      h("tr", {}, ["what", "exactly", "on"].map((c) => h("th", {}, c))),
      row("plain runs and a chat", launches(RUNS), "your subscription"),
      row("this branch's benchmarks", launches(SUBSCRIPTION), "your subscription"),
      row("the first harness, again", launches(FIRST_AGAIN), "your subscription"),
      row("probes", h("td", {}, h("code", {}, "probe/breakpoints.py"), ", ", h("code", {}, "shared-context.py"), ", ", h("code", {}, "tool-use-breakpoint.py"), ", ", h("code", {}, "attachments.py"), h("span", { class: "tok" }, "  where Anthropic accepts a breakpoint, how many, and whether a made-up read_file turn is accepted")), "your subscription"),
      row("the first round's benchmarks", launches(OPENROUTER), "OpenRouter"),
      row("the first round's smoke runs", h("td", {}, h("code", {}, "forks run …"), " ×5, ", h("code", {}, "forks bench --reps 1"), " ×2", h("span", { class: "tok" }, "  one bench was killed mid-run by a shell timeout; $1.38")), "OpenRouter"),
      row("the first round's probe", h("td", {}, h("code", {}, "python3 probe/probe.py [amazon-bedrock | google-vertex]"), h("span", { class: "tok" }, "  8 runs × 3 requests, ≈ $0.10")), "OpenRouter"),
      row("rescoring, offline", h("td", {}, h("code", {}, "forks rescore runs.ignore/<bench> --json …"), h("span", { class: "tok" }, "  every trial above, from its recorded wire; this page's numbers")), "nothing")),
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
    const chips = r.messages.map((id: string) => {
      const m = CLEAN.messages[id]
      const chip = h("span", { class: `chip ${m.role}${firstSeen.get(id) === i ? " first" : ""}`, title: m.role }, id.slice(1))
      chip.addEventListener("click", () => show(chip, id, m))
      return chip
    })
    const next = replyId(r)
    const reply = h("span", { class: "chip reply", title: next ? "the reply, sent back as this message" : "the reply: this agent's report" }, next ? "→ " + next.slice(1) : "→ report")
    reply.addEventListener("click", () => show(reply, next ?? "report", next ? CLEAN.messages[next] : r.reply))
    const prompt = r.cached + r.written + r.uncached
    row.append(
      h("span", { class: "tok" }, i + 1),
      h("span", { class: "tok" }, "+" + r.sent.toFixed(1) + "s"),
      h("span", { class: "agent", title: r.agent }, r.agent.replace(/^root › /, "  ")),
      h("span", { class: "chips" }, chips, reply),
      h("span", {},
        h("span", { class: "tok" }, h("b", {}, int(prompt)), " in: ", h("span", { class: "c" }, int(r.cached) + " cached"), " · ", h("span", { class: "w" }, int(r.written) + " written"), r.uncached > 20 ? ` · ${int(r.uncached)} plain` : "", " · ", h("span", { class: "o" }, int(r.out) + " out")),
        h("div", { class: "bar" }, h("i", { style: { width: pct(r.cached / prompt), background: "var(--cached)" } }), h("i", { style: { width: pct(r.written / prompt), background: "var(--new)" } }))),
      opened)
    return row
  })
  return [
    h("h3", {}, "3.2 One run, request by request"),
    h("p", {}, `The ledgers trial part 1 quotes: `, h("code", {}, "before · agent"), `, a tree built as asked with every total right. ${CLEAN.requests.length} requests. Every request has this envelope; only `, h("code", {}, "messages"), ` changes:`),
    json(CLEAN.envelope),
    h("p", {}, toggle("the tool schemas, exactly as sent", () => json(CLEAN.tools)), " — byte-identical in every request of every agent, like the system prompt, because the provider caches tools, then system, then messages, and one difference moves the start of the cache. These benchmarks ran before ", h("code", {}, "handover"), " was added, so they have three tools; the runs in 1.4 and 1.5 have four."),
    h("p", {}, "Each row is one request. Each numbered box is one message, and the same number is the same bytes wherever it appears. So a child's row starting with its parent's numbers ", h("i", {}, "is"), " the fork. A box with a dark outline is sent for the first time in that row. Click any box for its JSON; the dashed box is the reply, which comes back as a message in that agent's next request."),
    h("div", { class: "legend" }, ["user", "assistant"].map((r) => h("span", {}, h("span", { class: `chip ${r}` }, "n"), r)), h("span", {}, h("i", { class: "sw", style: { background: "var(--cached)" } }), "prompt tokens read from cache"), h("span", {}, h("i", { class: "sw", style: { background: "var(--new)" } }), "written to cache")),
    h("div", { class: "flow" }, h("div", { class: "row head" }, h("span", {}, "#"), h("span", {}, "sent"), h("span", {}, "agent"), h("span", {}, "messages sent → reply"), h("span", {}, "tokens, as the provider counted them")), rows),
  ]
}

export function runs(): HTMLElement {
  return part(3, "runs", "What was run",
    "Every run the evidence comes from, with the command that made it; then one benchmark trial, request by request.",
    kinds(), exactFlow(),
    cameTo("Everything on this page since the first round ran on your subscription, on Claude Sonnet 5. A forked child really is sent its parent's messages byte for byte, and the provider really serves them from the cache: in the trial above, each child's first row starts with its parent's numbers, and most of its tokens are blue."))
}
