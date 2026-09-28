// Part 8: the concerns that cut across every agent — which are handled, where, and what proves it; and which are not.

import { excerpt } from "underview/excerpt" with { type: "macro" }
import { h, code, toggle, part, cameTo } from "./ui"
import { SOURCES } from "./code"

type Handled = { concern: string; how: string; at: ReturnType<typeof excerpt>; tests: string[] }

const HANDLED: Handled[] = [
  { concern: "One cached prefix", how: "every agent gets the same system prompt and tool list; the depth limit refuses a call rather than removing the tool", at: excerpt("../src/framing.rs:/pub const SYSTEM_PROMPT/"), tests: ["prefix_is_identical", "an_over_deep_task_call_is_an_error_result_not_a_missing_tool"] },
  { concern: "One cached shared part", how: "a breakpoint after the shared part; the first sibling that waits on no other starts first, the rest when its first request has come back", at: excerpt("../src/agent.rs:/The first sibling that waits on no other goes first/"), tests: ["cut_before_ends_at_the_message_before_the_task_call_then_the_cached_shared_part", "the_benchmark_runs_on_claude_and_counts_shared_part_cache_hits"] },
  { concern: "Structured scopes", how: "a task call spawns every child and awaits all of them before it returns; nothing outlives its scope", at: excerpt("../src/agent.rs:/let mut handles = Vec::new();/"), tests: ["scope_suspends_the_parent_while_its_children_run_at_the_same_time", "after_makes_a_sibling_wait_and_hands_it_the_report", "nested_scopes_resume_bottom_up"] },
  { concern: "Context length", how: "past --handover-at, an agent is told once per context to hand over; a handover inside a scope is still one child to its parent", at: excerpt("../src/agent.rs:/if let Some(limit) = run.config.handover_at/"), tests: ["past_the_handover_point_the_agent_is_told_to_hand_over", "a_context_born_past_the_handover_point_is_not_told_again", "a_child_that_hands_over_is_still_one_child_to_its_parent", "a_handover_with_other_calls_in_its_turn_is_refused"] },
  { concern: "The user's obligations", how: "only an agent talking with the user starts a child that talks with the user; when the user leaves, the child waiting for them ends", at: excerpt("../src/agent.rs:/if user_facing == Some(true) && !talking {/"), tests: ["in_chat_a_split_also_starts_a_child_that_talks_with_the_user_until_done", "an_autonomous_agent_cannot_start_a_user_facing_child", "when_the_user_leaves_the_agent_waiting_for_them_ends"] },
  { concern: "In-band vs out-of-band failure", how: "a child that can't do its task reports so and completes; a provider fault, panic or cap faults it and leaves every ancestor suspended", at: excerpt("../src/agent.rs:/fn blocks_the_parent(/"), tests: ["a_rejected_request_faults_the_agent_and_leaves_its_ancestors_suspended", "a_panicked_child_ends_with_a_recorded_outcome", "a_panicked_root_still_writes_its_summary"] },
  { concern: "Cancellation", how: "one token for the run; nothing new starts, requests in flight are awaited and kept, every agent ends with an outcome, then the process closes; a second Ctrl-C force-cancels, abandoning what is in flight and still writing the record", at: excerpt("../src/agent.rs:/if run.cancel.is_cancelled() {/"), tests: ["cancel_waits_for_what_is_in_flight_then_closes", "cancelling_stops_the_retries", "a_second_interrupt_force_cancels"] },
  { concern: "Spend", how: "checked before every HTTP attempt, charging each request in flight the most any request has cost so far", at: excerpt("../src/agent.rs:/let committed = state.spent/"), tests: ["the_spend_cap_is_a_fault_before_the_request_that_would_break_it", "the_cap_counts_the_requests_already_in_flight"] },
  { concern: "Retries, timeouts, rate limits", how: "transient failures retried four times; a 429 — which OpenRouter sends as a 200 with an error body — waited out, honouring Retry-After; Anthropic's identity rejection, a 429 whose message is \"Error\", faults at once", at: excerpt("../src/wire.rs:/pub async fn send_once(/"), tests: ["a_silent_provider_times_out_and_the_retry_succeeds", "a_rate_limit_arriving_as_a_200_is_waited_out_not_treated_as_fatal", "retry_after_is_honoured", "a_rate_limit_that_never_lifts_ends_as_a_provider_fault", "an_identity_rejection_is_a_fault_not_a_rate_limit"] },
  { concern: "Shared state", how: "one Mutex around the run's records, poison-tolerant; after-dependencies are watch channels", at: excerpt("../src/agent.rs:/fn state(&self)/"), tests: [] },
  { concern: "Evidence", how: "every body sent and received is written as it happens; each agent's record and every context it had at the end", at: excerpt("../src/record.rs:/let wire = std::fs::File::create/"), tests: ["rescoring_a_recorded_benchmark_reproduces_its_scores", "the_face_shows_every_message_exactly_as_it_enters_a_context"] },
  { concern: "The limb's boundary", how: "paths are canonicalised and must stay under --dir", at: excerpt("../src/limb.rs:/fn resolve(&self, path: &str)/"), tests: [] },
  { concern: "Credentials", how: "on the Claude backend, the subscription token opencode keeps in its database, re-read before every request and never refreshed here — refreshing would rotate opencode's own token; on OpenRouter, a key from the environment or keys.ignore.env. Either goes only into a request header", at: excerpt("../src/anthropic.rs:/pub fn access_token(/"), tests: ["an_expired_subscription_token_stops_the_run_before_any_request"] },
  { concern: "Tests that can't flake", how: "the fake provider holds requests at barriers and releases them on command; no test passes because an interval elapsed", at: excerpt("../src/bin/fake_provider.rs:/fn barrier(/"), tests: ["scope_suspends_the_parent_while_its_children_run_at_the_same_time"] },
]

const missing = HANDLED.flatMap((c) => c.tests).filter((t) => !SOURCES["tests/scenario.rs"].includes(`fn ${t}(`))
if (missing.length > 0) throw new Error(`concerns name scenarios that do not exist: ${missing.join(", ")}`)

const ABSENT: [string, string][] = [
  ["Persistence, restart and resume", "a run lives in memory; the run directory is a record, not state it resumes from"],
  ["Siblings launching siblings, re-wiring dependencies after launch", "out of scope in the brief"],
  ["Two-part launch, for task and for handover", "out of scope in the brief"],
  ["Handing over a suspended parent, or when the cache expires", "a suspended parent is never told to hand over, and nothing hands over on a clock"],
  ["Writes, and siblings racing on one filesystem", "the limb is read-only on purpose"],
  ["More than one limb, fresh-across-limbs", "one local directory"],
  ["More than one task call per turn", "a second in the same turn gets an error result"],
  ["Streaming", "whole responses only"],
  ["A provider abstraction", "two backends — Anthropic's Messages API on your subscription, and OpenRouter's chat-completions API — each one plain function from messages to a request body and back"],
  ["Configuration layering", "flags only"],
  ["The Deno half of your language split", "Rust only"],
  ["Tracing and metrics", "the face's lines and the run directory are all there is"],
]

export function concerns(): HTMLElement {
  return part(8, "concerns", "Cross-cutting concerns",
    "What every agent in a run depends on regardless of what it's doing: first the concerns the harness handles — where, and which scenario proves it — then the ones it deliberately doesn't.",
    h("h3", {}, "8.1 Handled"),
    h("table", { class: "grid concerns" },
      h("tr", {}, ["concern", "how", "where", "proved by"].map((c) => h("th", {}, c))),
      HANDLED.map((c) => {
        const row = h("tr", {})
        const cell = h("td", {})
        const rel = c.at.path.slice(c.at.path.indexOf("experiments/forked-subagents/") + "experiments/forked-subagents/".length)
        cell.append(toggle(`${rel}:${c.at.from}`, () => code(c.at)))
        row.append(h("td", {}, c.concern), h("td", {}, c.how), cell, h("td", { class: "mono", style: { fontSize: "11.5px" } }, c.tests.length ? c.tests.map((t) => h("div", {}, t.replace(/_/g, " "))) : h("span", { style: { color: "var(--mute)" } }, "no scenario")))
        return row
      })),
    h("h3", {}, "8.2 Not handled, on purpose"),
    h("table", { class: "grid concerns" }, h("tr", {}, ["concern", "why not"].map((c) => h("th", {}, c))), ABSENT.map(([c, why]) => h("tr", { class: "no" }, h("td", {}, c), h("td", {}, why)))),
    cameTo("Everything that makes a scope trustworthy — ordering, failure, cancellation, spend, context length, and who may ask the user for their time — is handled and proved by a scenario. Two handled concerns have no scenario of their own: shared state and the limb’s boundary. Most of what is absent the brief put out of scope. Streaming, configuration layering, the Deno half and tracing were my omissions, not decided with you."))
}
