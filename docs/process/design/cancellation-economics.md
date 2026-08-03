# Cancellation economics — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why, what (agent-drafted, unreviewed) · interactions, summary — not yet done.** A targeted question rather than a broad design; expected to stop at L2 depth. Derives from `source-notes/analytics.md`.

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

## What

This is a measurement, so the "what" is a method rather than a design. It stops at L2 deliberately: there is nothing here to architect, and the only thing that would make it a bigger piece of work is if the answer turned out to be provider-specific in an awkward way, which is itself one of the things being measured.

### The quantity being measured

The question is what a provider bills for a request we stopped wanting. So the measurable is billed tokens — and billed cost, which is not the same thing once cache reads and reasoning tokens are priced differently — as a function of the point at which we stopped.

Two things that sound like one need separating before any measurement is meaningful. Closing the HTTP connection is not the same as telling the provider to stop. Whether a provider even offers an explicit cancellation call, and whether using it changes the bill compared with just hanging up, is part of what the experiment establishes rather than something to assume. It is entirely plausible that hanging up is invisible to the biller and an explicit cancel is not, or that neither is.

### Stimulus and baseline

The stimulus has to reliably produce a long generation phase, because a short one leaves no room between the cancel points. High reasoning effort plus a request for a long, mechanically-produced output — an enumeration, a count, a repetition — with a generous token ceiling gets there without needing anything clever.

The baseline matters more than it looks. Every result here is a *difference* between a cancelled run and an uncancelled one, and generation length varies run to run. So the baseline is not one uncancelled request; it is enough uncancelled requests to know the variance of billed tokens for that stimulus. Without that error bar there is no way to tell a partial charge from noise, and the experiment produces a number that feels like an answer and is not one.

### The cancel points

Five conditions, plus the baseline. Cancel before the request is sent, which is the trivial control and should cost nothing — if it does not, something is wrong with the harness rather than the provider. Cancel after sending but before the first byte. Cancel just after the first byte, while the response is presumably still thinking. Cancel mid-stream, some way into the output. And cancel very near the end, which is the case where the tentative "let it finish and discard" policy is most obviously right and is worth confirming.

Each condition repeated enough times to clear the granularity of the measuring instrument, which is the real constraint — see below.

### The instrument problem

Usage arrives in provider response metadata, at or near the end of a stream. A cancelled stream may never deliver it. So the in-band reporting the harness normally relies on is unavailable in precisely the case of interest, and this is the practical difficulty to plan for rather than discover.

That forces an external instrument: account-level usage — a dashboard, a usage API, or a per-generation lookup where a gateway offers one after the fact. Whether each provider exposes something queryable by generation id is part of the survey.

Using account-level usage as the instrument has consequences for the method that are easy to get wrong. Each condition needs an isolated window with no other traffic on the account, so a delta is attributable. A distinct API key per condition helps where usage is reported per key. Account figures often lag and round, so repetitions per condition have to be enough to lift the signal above that granularity — and if the granularity is coarse (whole dollars, hourly buckets), the number of repetitions is set by the instrument, not by statistical taste.

### Per provider

The answer is probably not one fact. Anthropic direct, an OpenAI-compatible endpoint direct, and OpenRouter in front of others, at minimum. A gateway adds a distinct hypothesis worth naming: cancellation may be absorbed at the gateway while the upstream keeps generating, in which case the gateway bills for the whole thing regardless of what the upstream did, and the answer differs from the same model reached directly.

One factor to hold constant rather than test: whether a subscription-backed connection bills differently from an API key. That interacts with the OAuth question and is not known here; the safe move is to run the measurement on API keys and note the gap explicitly.

### What the experiment must produce

Three outputs, and only the third is a product change.

A per-provider table of cancel point against billed fraction of baseline, with error bars, and a plain statement of whether an explicit cancel call exists and whether it made a difference.

A ruling on the harness's default: keep the conservative "let the future complete and discard the results" behaviour, or cancel after some point. The user's own tentative position is the null hypothesis, and it wins unless the evidence is clear.

And a **recording rule**, which is the part persistence-analytics needs and the part that must be right whatever the billing answer turns out to be. A cancelled request's cost may be genuinely unknown to the harness, because the metadata never arrived. That must be representable as unknown-with-a-reason, not as zero — a zero makes every cost query silently wrong in exactly this case. So cost queries report coverage alongside their totals, and invariant 9's `cancelled` outcome gets a nullable cost with an explicit reason rather than an implied one.

### What it costs to run

The user asked roughly how much, so here is the arithmetic rather than a number: five conditions plus a baseline, times three providers, times enough repetitions to clear the instrument's granularity — call it ten if the instrument is a per-generation lookup, considerably more if it is a rounded dashboard figure. That is on the order of 150 to 500 long reasoning responses. At a few cents to twenty cents each, depending on model and length, that is somewhere between roughly five and fifty dollars, with the instrument's granularity being what decides where in that range it lands. Worth confirming before committing, and worth knowing that choosing a cheaper model with long reasoning shifts the whole estimate down without weakening the result, since the question is about billing behaviour rather than about a specific model.

### Thesis, falsification, and invariants

The thesis: **cancelling a request after the first byte does not avoid the charge, so the harness should let in-flight requests complete and discard results it no longer wants** — the user's tentative position, stated as the hypothesis to be tested rather than assumed.

It is falsified if billed usage for a mid-stream cancellation is materially below the uncancelled baseline, beyond the baseline's own variance, on any provider we actually use. It is *also* usefully falsified in the other direction if a before-first-byte cancellation still bills in full, which would say something stronger than expected about how requests are priced. A null result — cancelled and uncancelled indistinguishable within error — confirms the current design and lets it stop being tentative, which is a good outcome and not a failed experiment.

Invariants touched: 9 primarily, since it decides what cost attaches to the `cancelled` outcome and whether soft cancellation's deliberate token spend is quantified; and 5, because the recording rule is a storage requirement — unknown cost must be storable as unknown.

## Parked for later stages

**Interactions flagged for stage 3:** persistence-analytics (records the answer; needs a cost value for cancelled requests); forked-subagents (soft cancellation deliberately spends tokens — this quantifies what that costs); operator-lifecycle (shutdown currently waits rather than cancels, on this assumption); compaction-handover (only indirectly — its economics are about cache reads, not cancellation).

## Questions for review

- Is this worth doing early, given it changes decisions already made, or is it fine to keep the conservative "let it finish" behaviour indefinitely and never need the answer?
- Are you willing to spend real money on provoking long thinking responses for this, and roughly how much? The arithmetic above lands somewhere between roughly five and fifty dollars depending on how coarse the usage instrument turns out to be; the estimate needs your ceiling, not the other way round.
- The measurement is proposed on **API keys only**, with subscription-backed billing held constant rather than tested, because whether a subscription bills differently is unknown here and entangled with the OAuth work. Is that gap acceptable, given a subscription is what you actually use day to day?
- The recording rule above says a cancelled request's cost may be stored as **unknown-with-a-reason**, and that cost queries therefore report coverage alongside totals. That is a small permanent complication in the analytics surface, bought to avoid silently wrong numbers. Confirm you want it that way rather than treating unmeasurable cost as zero.
