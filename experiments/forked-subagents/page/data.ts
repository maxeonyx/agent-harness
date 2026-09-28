// What the page shows, read from the recorded runs and the source when the page is built.
// Every export is a Bun macro: it runs at build time and its result is baked into the page.

import { readFileSync, readdirSync, existsSync, statSync, mkdtempSync, rmSync } from "node:fs"
import { join, resolve } from "node:path"
import { tmpdir } from "node:os"
import { execSync } from "node:child_process"

const EXPERIMENT = resolve(import.meta.dir, "..")
const RUNS = join(EXPERIMENT, "runs.ignore")

const benches = () => readdirSync(RUNS).filter((d) => d.endsWith("-bench")).sort()
const wireOf = (dir: string) => readFileSync(join(RUNS, dir, "wire.jsonl"), "utf8").trim().split("\n").map((l) => JSON.parse(l))

/** Every benchmark trial, scored by `forks rescore`, with each agent's record. */
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
        launch: bench,
        agents: summary.agents.map((a: any) => ({ path: a.path, depth: a.depth, handoff: a.handoff })),
      })
    }
  }
  rmSync(scratch, { recursive: true })
  return rows
}

/** Requests by name, each picked as [a run's directory under runs.ignore/, an agent, which of its requests]. */
export function requests<K extends string>(picks: Record<K, [string, string, number]>) {
  return Object.fromEntries(Object.entries(picks).map(([name, [dir, agent, n]]) => [name, request(dir, agent, n as number)])) as Record<K, Request>
}

export type Request = ReturnType<typeof request>

/** The `n`th request an agent sent, as sent, and what came back. An agent has one request in flight at a time, so its next entry is the answer. */
function request(dir: string, agent: string, n: number) {
  const mine = wireOf(dir).filter((e) => e.agent === agent)
  const sent = mine.filter((e) => e.kind === "request")
  const at = mine.indexOf(sent[n])
  if (at < 0) throw new Error(`${dir}: ${agent} sent ${sent.length} requests, not ${n + 1}`)
  const { messages, tools, ...envelope } = sent[n].body
  const answer = mine[at + 1]
  return {
    dir, agent, n, of: sent.length,
    envelope, messages,
    tools: tools.map((t: any) => t.name),
    reply: answer.kind === "response" ? answer.body.content : null,
    usage: answer.kind === "response" ? tokens(answer.body.usage) : null,
  }
}

const tokens = (u: any) => ({ cached: u.cache_read_input_tokens, written: u.cache_creation_input_tokens, uncached: u.input_tokens, out: u.output_tokens })

/** One run's every request and response, in order, with identical messages stored once. */
export function flow(dir: string) {
  const messages: Record<string, any> = {}
  const ids = new Map<string, string>()
  const key = (m: any) => {
    const text = JSON.stringify(m)
    if (!ids.has(text)) {
      ids.set(text, `m${ids.size + 1}`)
      messages[ids.get(text)!] = m
    }
    return ids.get(text)!
  }
  const entries = wireOf(dir)
  const open = new Map<string, any>()
  const requests: any[] = []
  for (const entry of entries) {
    const at = Date.parse(entry.at) / 1000
    if (entry.kind === "request") open.set(entry.agent, { agent: entry.agent, sent: at, messages: entry.body.messages.map(key) })
    else {
      const r = open.get(entry.agent)
      open.delete(entry.agent)
      requests.push({ ...r, returned: at, ...tokens(entry.body.usage), reply: { role: "assistant", content: entry.body.content } })
    }
  }
  const start = Math.min(...requests.map((r) => r.sent))
  for (const r of requests) (r.sent -= start), (r.returned -= start)
  const { messages: _, tools, ...envelope } = entries[0].body
  return { dir, envelope: { ...envelope, tools: tools.map((t: any) => t.name), messages: "…" }, tools, messages, requests }
}

/** The ledgers benchmark's fixture, as a trial saw it. */
export function ledgers(bench: string) {
  const root = join(RUNS, bench, "fixture", "ledgers")
  const policy = readFileSync(join(root, "POLICY.md"), "utf8")
  const files = ["POLICY.md", ...["maunga", "awa"].flatMap((r) => readdirSync(join(root, r)).sort().map((f) => `${r}/${f}`))]
  return {
    files: files.map((f) => ({ path: `ledgers/${f}`, bytes: readFileSync(join(root, f)).length })),
    ledger: readFileSync(join(root, "maunga", "kowhai.txt"), "utf8"),
    section7: policy.slice(policy.indexOf("## 7"), policy.indexOf("## 8")).trim(),
  }
}

/** The projects benchmark's fixture: every directory under work/<project>/, and what makes it a checkout or not. */
export function projects(bench: string) {
  const work = join(RUNS, bench, "fixture", "work")
  const branchIn = (head: string) => readFileSync(head, "utf8").trim().replace("ref: refs/heads/", "")
  return readdirSync(work).filter((p) => statSync(join(work, p)).isDirectory()).sort().map((project) => ({
    project,
    entries: readdirSync(join(work, project)).sort().map((name) => {
      const git = join(work, project, name, ".git")
      if (!existsSync(git)) return { name, kind: "look-alike", has: readdirSync(join(work, project, name)).join(" ") }
      if (statSync(git).isDirectory()) return { name, kind: "clone", branch: branchIn(join(git, "HEAD")) }
      const gitdir = readFileSync(git, "utf8").trim().replace("gitdir: ", "")
      return { name, kind: "worktree", branch: branchIn(join(work, project, name, gitdir, "HEAD")), gitdir }
    }),
  }))
}

/** What every recorded run used: dollars on OpenRouter, from the provider's own `usage.cost`; requests on the subscription, which bills none. */
export function spend() {
  const walk = (dir: string): string[] =>
    readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(join(dir, e.name)) : e.name === "wire.jsonl" ? [join(dir, e.name)] : []))
  const total = { usd: 0, openrouter: 0, claude: 0 }
  for (const file of walk(RUNS)) {
    for (const line of readFileSync(file, "utf8").split("\n")) {
      if (!line.startsWith("{")) continue
      let entry
      try {
        entry = JSON.parse(line)
      } catch {
        continue // the one trial two benchmarks wrote into at once has torn lines
      }
      if (entry.kind !== "response") continue
      if (entry.body.choices !== undefined) (total.usd += entry.body.usage?.cost ?? 0), total.openrouter++
      else total.claude++
    }
  }
  return total
}

/** The dependency graph: "A depends on B" means deleting B breaks A, as measured by the compiler. */
export function deps() {
  return JSON.parse(readFileSync(join(EXPERIMENT, "page", "deps.json"), "utf8"))
}

/** A whole source file, for the code viewer. */
export function source(path: string) {
  return readFileSync(join(EXPERIMENT, path), "utf8")
}
