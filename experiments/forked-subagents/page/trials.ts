// The benchmark trials, and the few ways every part slices them.

import { trials } from "./data" with { type: "macro" }

export const TRIALS = trials()
export const VALID = TRIALS.filter((t) => t.valid)
export const CUTS = ["full", "own", "before", "fresh"] as const
export const short = (model: string) => (model.includes("sonnet") ? "sonnet" : "luna")
export const rowOf = (t: { mode: string; cut: string }) => (t.mode === "fresh" ? "fresh" : t.cut)

const sum = (list: any[], key: string) => list.reduce((a, t) => a + t[key], 0)
export const leafRate = (list: any[]) => [sum(list, "leaf_overreach"), sum(list, "leaves")] as const
export const firstShare = (list: any[]) => {
  const c = sum(list, "child_first_cached_in"), u = sum(list, "child_first_uncached_in")
  return c + u ? c / (c + u) : 0
}

/** What region agents wrote instead of numbers: their own account of what they thought had happened. */
export function voices(filter: (t: any) => boolean) {
  return VALID.filter(filter).flatMap((t) =>
    t.agents.filter((a: any) => a.depth === 1 && a.handoff && !/\d+\.\d\d/.test(a.handoff)).map((a: any) => ({ trial: t, agent: a.path, text: a.handoff.trim() })))
}
