// What the page shows, read from the recorded runs and the source when the page is built.
// Every export is a Bun macro: it runs at build time and its result is baked into the page.

import { readFileSync, readdirSync, existsSync, mkdtempSync, rmSync } from "node:fs"
import { join, resolve } from "node:path"
import { tmpdir } from "node:os"
import { execSync } from "node:child_process"

const EXPERIMENT = resolve(import.meta.dir, "..")
const RUNS = join(EXPERIMENT, "runs.ignore")

const benches = () => readdirSync(RUNS).filter((d) => d.endsWith("-bench")).sort()
const trialDir = (id: string) => join(RUNS, benches().find((b) => existsSync(join(RUNS, b, id)))!, id)
const wireOf = (id: string) => readFileSync(join(trialDir(id), "wire.jsonl"), "utf8").trim().split("\n").map((l) => JSON.parse(l))

/** Every benchmark trial, scored by `forks rescore` — the scorer the outcome doc used — with each agent's record. */
export function trials() {
  const scratch = mkdtempSync(join(tmpdir(), "forks-rows-"))
  const rows: any[] = []
  for (const bench of benches()) {
    const out = join(scratch, bench + ".json")
    try {
      execSync(`target/release/forks rescore runs.ignore/${bench} --json ${out}`, { cwd: EXPERIMENT, stdio: "ignore" })
    } catch {
      continue // the two earliest smoke benches predate the policy fixture and cannot be rescored
    }
    for (const row of JSON.parse(readFileSync(out, "utf8"))) {
      const summary = JSON.parse(readFileSync(join(EXPERIMENT, row.dir, "summary.json"), "utf8"))
      rows.push({
        ...row,
        id: row.dir.split("/").pop(),
        bench,
        root_handoff: summary.root_handoff ?? "",
        agents: summary.agents.map((a: any) => ({
          path: a.path, depth: a.depth, state: a.state, fresh: a.fresh, requests: a.requests,
          cached: a.cached_in, written: a.written_in, uncached: a.uncached_in, out: a.out, cost: a.cost,
          task: a.task, handoff: a.handoff,
          tools: a.tool_calls.map((c: any) => ({ name: c.name, args: c.arguments, ok: !String(c.result ?? "").startsWith("Error") })),
        })),
      })
    }
  }
  rmSync(scratch, { recursive: true })
  return rows
}

/** One run's every request and response, in order, with identical messages stored once. */
export function flow(id: string) {
  const messages: Record<string, any> = {}
  const order: string[] = []
  const key = (m: any) => {
    const text = JSON.stringify(m)
    let found = order.find((k) => JSON.stringify(messages[k]) === text)
    if (!found) {
      found = `m${order.length + 1}`
      messages[found] = m
      order.push(found)
    }
    return found
  }
  const requests: any[] = []
  const open = new Map<string, any>()
  let envelope: any = null
  for (const entry of wireOf(id)) {
    const at = Date.parse(entry.at) / 1000
    if (entry.kind === "request") {
      const { messages: sent, tools, ...rest } = entry.body
      envelope ??= { ...rest, tools: tools.map((t: any) => t.function.name), messages: "…" }
      open.set(entry.agent, { agent: entry.agent, sent: at, messages: sent.map(key) })
    } else if (entry.kind === "response") {
      const request = open.get(entry.agent)
      open.delete(entry.agent)
      const usage = entry.body.usage
      const message = entry.body.choices[0].message
      requests.push({
        ...request,
        returned: at,
        prompt: usage.prompt_tokens,
        cached: usage.prompt_tokens_details.cached_tokens,
        written: usage.prompt_tokens_details.cache_write_tokens,
        out: usage.completion_tokens,
        cost: usage.cost,
        reply: { role: "assistant", content: message.content ?? null, tool_calls: message.tool_calls ?? undefined },
      })
    }
  }
  const start = Math.min(...requests.map((r) => r.sent))
  for (const r of requests) (r.sent -= start), (r.returned -= start)
  const tools = wireOf(id)[0].body.tools
  return { id, envelope, tools, messages, requests }
}

/** The last messages of the first request a child at `depth` sent: exactly what it was told, under one trial's cut. */
export function tail(id: string, depth: number, count: number) {
  const entry = wireOf(id).find((e) => e.kind === "request" && e.agent.split(" › ").length === depth + 1)
  const sent = entry.body.messages
  return { id, agent: entry.agent, total: sent.length, messages: sent.slice(-count) }
}

/** The benchmark's fixture, as a trial saw it. */
export function fixture() {
  const bench = benches().find((b) => existsSync(join(RUNS, b, "fixture", "ledgers", "POLICY.md")))!
  const root = join(RUNS, bench, "fixture", "ledgers")
  const policy = readFileSync(join(root, "POLICY.md"), "utf8")
  const files = ["POLICY.md", ...["maunga", "awa"].flatMap((r) => readdirSync(join(root, r)).sort().map((f) => `${r}/${f}`))]
  return {
    files: files.map((f) => ({ path: `ledgers/${f}`, bytes: readFileSync(join(root, f)).length })),
    ledger: readFileSync(join(root, "maunga", "kowhai.txt"), "utf8"),
    section7: policy.slice(policy.indexOf("## 7"), policy.indexOf("## 8")).trim(),
  }
}

/** What every recorded run cost, summed from the provider's own `usage.cost` on each response. The probe is not recorded here. */
export function spend() {
  const walk = (dir: string): string[] =>
    readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(join(dir, e.name)) : e.name === "wire.jsonl" ? [join(dir, e.name)] : []))
  let usd = 0, responses = 0
  for (const file of walk(RUNS)) {
    for (const line of readFileSync(file, "utf8").split("\n")) {
      if (!line.startsWith("{")) continue
      try {
        const entry = JSON.parse(line)
        if (entry.kind === "response") (usd += entry.body.usage?.cost ?? 0), responses++
      } catch {} // the one trial two benchmarks wrote into at once has torn lines
    }
  }
  return { usd, responses }
}

/** The dependency graph: "A depends on B" means deleting B breaks A, as measured by the compiler. */
export function deps() {
  return JSON.parse(readFileSync(join(EXPERIMENT, "page", "deps.json"), "utf8"))
}

/** A whole source file, for the code viewer. */
export function source(path: string) {
  return readFileSync(join(EXPERIMENT, path), "utf8")
}
