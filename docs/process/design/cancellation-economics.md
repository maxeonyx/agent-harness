# Cancellation economics — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why (agent-drafted, unreviewed) · what, interactions, summary — not yet done.** A targeted question rather than a broad design; expected to stop at L2 depth. Derives from `source-notes/analytics.md`.

The question, in the user's words: "does long thinking process actually get interrupted on the providers server or do they charge for the whole thing?" And the tentative consequence — "don't cancel API requests (perhaps only if we already recieved first byte) as I think providers still charge for cancelled requests... Rather, we can keep around the request future and finish it but discard the results. probably worth experiment."

## Why

### 1. A design decision already rests on the answer — *correctness of a decision, not of code*

This is a small experiment whose value is disproportionate, because several parts of the design already assume something about it.

Cancellation in this harness is deliberately **soft**: it is a message — "your task has been cancelled, please clean up then call done" — not a kill, and it keeps burning tokens on purpose because cleanup is the point. Invariant 9 goes further: completed work that ties with a cancel is *kept and recorded*, because "it cost money and is probably good".

Both of those positions are reasoning about money. If a cancelled request is billed in full regardless, then discarding a response is pure waste and the harness should always let it finish and keep the result. If cancellation genuinely stops generation and stops billing, then the calculus flips for long thinking responses, and there is a real saving available at the cost of losing partial work.

So the root is: **we are making cost-shaped decisions on an unverified belief about provider behaviour, and the belief is cheap to test.**

### 2. It is also a correctness question about what gets recorded — *correctness*

Whatever the billing answer, the harness must record honestly what happened. Persistence-analytics exists partly to make the design's economic claims checkable rather than believed, and cancelled requests are the case where "what did this cost" is least obvious. A cancelled-but-billed request that records nothing makes cost queries silently wrong.

### 3. Operator lifecycle wants the same answer — *shared dependency*

Graceful shutdown waits for in-flight API requests to complete rather than cancelling them, for exactly this reason. That is currently a reasonable guess. If the guess is wrong, shutdown could be faster; if it is right, the current design is correct and can stop being tentative.

## Forward: what this forces

- **A measurement, not a design.** The deliverable is evidence: issue requests that provoke long thinking, cancel at several points (before first byte, after first byte, mid-stream), and compare provider-reported usage against an uncancelled baseline.
- **Per-provider answers.** Anthropic and OpenAI-compatible endpoints may differ, and OpenRouter sits in front of others. The answer is probably not one fact.
- **A recorded outcome the schema can hold**, so persistence-analytics can represent cancelled requests with their true cost.
- **A ruling on the four-valued outcome's cost field.** Invariant 9 already distinguishes ok / error / cancelled / panicked; this decides what cost attaches to `cancelled`.

## Parked for later stages

**Method notes:** usage is reported in provider response metadata, which a cancelled stream may never deliver — so the experiment may need to compare against account-level or dashboard-level usage rather than in-band reporting. That is the likely practical difficulty and should be planned for rather than discovered.

**Interactions flagged for stage 3:** persistence-analytics (records the answer; needs a cost value for cancelled requests); forked-subagents (soft cancellation deliberately spends tokens — this quantifies what that costs); operator-lifecycle (shutdown currently waits rather than cancels, on this assumption); compaction-handover (only indirectly — its economics are about cache reads, not cancellation).

## Questions for review

- Is this worth doing early, given it changes decisions already made, or is it fine to keep the conservative "let it finish" behaviour indefinitely and never need the answer?
- Are you willing to spend real money on provoking long thinking responses for this, and roughly how much?
