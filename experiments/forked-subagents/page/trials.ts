// The benchmark trials, and the few ways every part slices them.

import { trials } from "./data" with { type: "macro" }

export const TRIALS = trials()
export const VALID = TRIALS.filter((t) => t.valid)

/** OpenRouter rows name the provider they were pinned to; on the subscription there is none. */
export const onClaude = (t: { provider: string }) => t.provider === ""
/** The first experiment (PR #13) said `--words stop|explained` where this branch says `--identity agent|task`. */
export const firstHarness = (t: { identity: string }) => t.identity === "stop" || t.identity === "explained"
export const variant = (t: { mode: string; cut: string; identity: string }) => (t.mode === "fresh" ? "fresh" : `${t.cut} · ${t.identity}`)

const sum = (list: any[], key: string) => list.reduce((a, t) => a + (t[key] ?? 0), 0)
export const ratio = (list: any[], part: string, whole: string) => `${sum(list, part)}/${sum(list, whole)}`
export const count = (list: any[], pass: (t: any) => boolean) => `${list.filter(pass).length}/${list.length}`
export const firstShare = (list: any[]) => {
  const c = sum(list, "child_first_cached_in"), u = sum(list, "child_first_uncached_in")
  return c + u > 0 ? c / (c + u) : undefined
}

/** A forked child's last words when its own part arrived in a tool result: it took the part for its own call's broken answer. */
export function voices(filter: (t: any) => boolean) {
  return VALID.filter(filter).flatMap((t) =>
    t.agents.filter((a: any) => a.depth > 0 && /echo|didn.t return|did not return|malformed/i.test(a.handoff ?? "")).map((a: any) => ({ trial: t, agent: a.path, text: a.handoff.trim() })))
}
