// Part 6: the concerns that cut across every agent — which are handled, where, and what proves it; and which are not.

import { excerpt } from "underview/excerpt" with { type: "macro" }
import { h, code, toggle, part, cameTo } from "./ui"

type Handled = { concern: string; how: string; at: ReturnType<typeof excerpt>; tests: string[] }

const HANDLED: Handled[] = [
  { concern: "One cached prefix", how: "every agent gets the same system message and tool list; the depth limit refuses a call rather than removing the tool", at: excerpt("../src/framing.rs:/pub const SYSTEM_PROMPT/"), tests: ["prefix_is_identical", "an_over_deep_task_call_is_an_error_result_not_a_missing_tool"] },
  { concern: "Structured scopes", how: "a task call spawns every child and awaits all of them before it returns; nothing outlives its scope", at: excerpt("../src/agent.rs:/let mut handles = Vec::new();/"), tests: ["scope_suspends_the_parent_while_its_children_run_at_the_same_time", "after_makes_a_sibling_wait_and_hands_it_the_report", "nested_scopes_resume_bottom_up"] },
  { concern: "In-band vs out-of-band failure", how: "a child that can't do its task reports so and completes; a provider fault, panic or cap faults it and leaves every ancestor suspended", at: excerpt("../src/agent.rs:/fn blocks_the_parent(/"), tests: ["a_rejected_request_faults_the_agent_and_leaves_its_ancestors_suspended", "a_panicked_child_ends_with_a_recorded_outcome", "a_panicked_root_still_writes_its_summary"] },
  { concern: "Cancellation", how: "one token for the run; nothing new starts, requests in flight are awaited and kept, every agent ends with an outcome; a second Ctrl-C exits", at: excerpt("../src/agent.rs:/if run.cancel.is_cancelled() {/"), tests: ["cancel_starts_nothing_new_keeps_what_is_in_flight_and_ends_every_agent", "cancelling_stops_the_retries", "a_second_interrupt_exits_without_waiting"] },
  { concern: "Spend", how: "checked before every HTTP attempt, charging each request in flight the most any request has cost so far", at: excerpt("../src/agent.rs:/let committed = state.spent/"), tests: ["the_spend_cap_is_a_fault_before_the_request_that_would_break_it", "the_cap_counts_the_requests_already_in_flight"] },
  { concern: "Retries, timeouts, rate limits", how: "transient failures retried four times; a 429 — which OpenRouter sends as a 200 with an error body — waited out, honouring Retry-After", at: excerpt("../src/wire.rs:/pub async fn send_once(/"), tests: ["a_silent_provider_times_out_and_the_retry_succeeds", "a_rate_limit_arriving_as_a_200_is_waited_out_not_treated_as_fatal", "retry_after_is_honoured", "a_rate_limit_that_never_lifts_ends_as_a_provider_fault"] },
  { concern: "Shared state", how: "one Mutex around the run's records, poison-tolerant; after-dependencies are watch channels", at: excerpt("../src/agent.rs:/fn state(&self)/"), tests: [] },
  { concern: "Evidence", how: "every body sent and received is written as it happens; each agent's record and final context at the end", at: excerpt("../src/record.rs:/let wire = std::fs::File::create/"), tests: ["rescoring_a_recorded_benchmark_reproduces_its_scores"] },
  { concern: "The limb's boundary", how: "paths are canonicalised and must stay under --dir", at: excerpt("../src/limb.rs:/fn resolve(&self, path: &str)/"), tests: [] },
  { concern: "Credentials", how: "the key comes from the environment or keys.ignore.env and goes only into the Authorization header", at: excerpt("../src/main.rs:/std::env::var(\"OPENROUTER_API_KEY\")/"), tests: [] },
  { concern: "Tests that can't flake", how: "the fake provider holds requests at barriers and releases them on command; nothing waits on a clock", at: excerpt("../src/bin/fake_provider.rs:/fn barrier(/"), tests: ["scope_suspends_the_parent_while_its_children_run_at_the_same_time"] },
]

const ABSENT: [string, string][] = [
  ["Persistence and restart", "a run lives in memory; the run directory is a record, not state it resumes from"],
  ["User-facing children, /done, permission to launch them", "UX & input is not yet elicited"],
  ["Writes, and siblings racing on one filesystem", "the limb is read-only on purpose"],
  ["More than one limb, fresh-across-limbs", "one local directory"],
  ["Compaction inside a scope", "out of scope in the brief"],
  ["Resume, re-wiring dependencies after launch", "out of scope in the brief"],
  ["Attachments, shared seed contexts, two-part launch", "out of scope in the brief"],
  ["Streaming", "whole responses only"],
  ["A provider abstraction", "OpenRouter's chat-completions API only; first-party wire facts belong to provider-cache-probe"],
  ["Configuration layering", "flags only"],
  ["The Deno half of your language split", "Rust only"],
  ["Tracing and metrics", "the face's lines and the run directory are all there is"],
]

export function concerns(): HTMLElement {
  return part(6, "concerns", "Cross-cutting concerns",
    "What every agent in a run depends on regardless of what it's doing: first the concerns the harness handles — where, and which scenario proves it — then the ones it deliberately doesn't.",
    h("h3", {}, "6.1 Handled"),
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
    h("h3", {}, "6.2 Not handled, on purpose"),
    h("table", { class: "grid concerns" }, h("tr", {}, ["concern", "why not"].map((c) => h("th", {}, c))), ABSENT.map(([c, why]) => h("tr", { class: "no" }, h("td", {}, c), h("td", {}, why)))),
    cameTo("Everything that makes a scope trustworthy — ordering, failure, cancellation, spend — is handled and proved by a scenario. Three handled concerns have no scenario of their own: shared state, the limb’s boundary, and credentials. Most of what is absent the brief put out of scope. Streaming, configuration layering, the Deno half and tracing were my omissions, not decided with you."))
}
