# Cancellation economics — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why, what, interactions, summary (agent-drafted, unreviewed).** A targeted question rather than a broad design; expected to stop at L2 depth. Derives from `source-notes/analytics.md`.

The question, in the user's words: "does long thinking process actually get interrupted on the providers server or do they charge for the whole thing?" And the tentative consequence — "don't cancel API requests (perhaps only if we already recieved first byte) as I think providers still charge for cancelled requests... Rather, we can keep around the request future and finish it but discard the results. probably worth experiment."

## Summary

This is a measurement, not a design, and its value is disproportionate to its size because several decisions already rest on the unverified belief. Cancellation in this harness is deliberately soft and deliberately spends tokens, because cleanup is the point; invariant 9 keeps and records work that completed as a cancel arrived, on the grounds that "it cost money and is probably good"; and graceful shutdown waits for in-flight requests rather than cancelling them, for the same reason. All three are reasoning about money. If a cancelled request is billed in full regardless, discarding a response is pure waste and the harness should always let it finish. If cancellation genuinely stops generation and stops billing, the calculus flips for long thinking responses and there is a real saving available at the cost of losing partial work.

The measurable is billed tokens — and billed cost, which is not the same thing once cache reads and reasoning tokens are priced differently — as a function of the point at which we stopped wanting the response. Two things that sound like one need separating first: closing the HTTP connection is not the same as telling the provider to stop, and whether an explicit cancellation call even exists and whether using it changes the bill is part of what the experiment establishes. Five cancel points plus a baseline, against a stimulus that reliably produces a long generation phase. The baseline matters more than it looks, because every result is a *difference* and generation length varies run to run, so it takes enough uncancelled requests to know the variance — without that error bar the experiment produces a number that feels like an answer and is not one. The practical difficulty to plan for rather than discover is the instrument: usage arrives in response metadata at the end of a stream, so a cancelled stream may never deliver it, and the in-band reporting the harness normally relies on is unavailable in precisely the case of interest. That forces account-level usage as the instrument, which imposes isolated windows per condition and sets the repetition count by the instrument's granularity rather than by statistical taste. The answer is probably not one fact — a gateway may absorb the cancellation while the upstream keeps generating — so Anthropic direct, an OpenAI-compatible endpoint direct, and OpenRouter at minimum, on API keys, with subscription-backed billing held constant rather than tested. The arithmetic lands somewhere between roughly five and fifty dollars.

Three things hang on the result and only one of them is a product change. A per-provider table of cancel point against billed fraction of baseline is the evidence. A ruling on the harness's default follows from it, with the user's tentative "let the future complete and discard the results" as the null hypothesis, which wins unless the evidence is clear — and a null result is a good outcome rather than a failed experiment, because it lets the current design stop being tentative. The third is a **recording rule**, and it is the part that must be right whatever the billing answer turns out to be: a cancelled request's cost may be genuinely unknown because the metadata never arrived, so it must be storable as unknown-with-a-reason rather than as zero, and cost queries must report coverage alongside their totals. Notably, that rule does not actually depend on the measurement — its shape is known already — so persistence can adopt it now, which makes this experiment schedulable whenever there is money to spend on it and deferrable indefinitely without blocking anything.

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

## Interactions

This is the most peripheral design in the portfolio and that is a feature, not an oversight. It is a measurement that produces one fact and one recording rule, and it can run in parallel with everything else because it needs nothing from any sibling and gives each of the three it touches a parameter rather than a mechanism.

**What this experiment owns**: the measurement method, the per-provider table of cancel point against billed fraction of baseline, the statement of whether an explicit cancellation call exists and whether it changes the bill, the ruling on the harness's default behaviour, and the recording rule for a cancelled request's cost.

**Persistence-analytics** is where the answer is recorded, and the direction of the dependency is worth being precise about because it is the reverse of what it looks like. Persistence does not need this measurement in order to design its schema; it needs the *shape* of the answer, which is already known — cost may be genuinely unknown because the metadata never arrived, so usage must be nullable with a reason and every cost query must report coverage alongside its total. That shape holds whatever the billing turns out to be, which means the recording rule can be adopted before this measurement ever runs. If the measurement is deferred indefinitely, persistence loses nothing.

**Forked-subagents** is quantified rather than changed. Soft cancellation deliberately spends tokens because cleanup is the point, and this experiment says how much that costs — but it does not decide whether cancellation should be soft, which is settled by forked-subagents' why #4 on correctness grounds rather than economic ones. Its bound on an agent that declines to finish is a deadline, not a bill. So the result informs a number in that design and cannot falsify anything in it.

**Operator-lifecycle** assumes the conservative behaviour — shutdown waits for in-flight requests rather than cancelling them — and does not test it. That assumption is currently well-reasoned and unverified, and this is what verifies it. A null result confirms the design and lets it stop being tentative; a positive result makes shutdown faster without changing its sequence.

One small link to **oauth-credentials** runs the other way. This measurement is proposed on API keys only, with subscription-backed billing held constant rather than tested, because whether a subscription bills differently is unknown here and entangled with the OAuth work. That gap is real and is oauth's to close if it matters, which is worth noting given a subscription is what the user actually uses day to day.

Everything else is empty, and the emptiness is why this can be scheduled whenever there is money to spend on it. Compaction-handover's economics are about cache reads rather than cancellation, so the two measurements share nothing but a units column. There is nothing to say to limb-model, multi-client-ui, modular-components, topology, context-updates, self-modification or user-turn. Layered-shutdown looks adjacent and is not: its problem with soft cancellation is that an agent cleanup turn is unbounded in *time*, which no billing fact changes.

`INTERACTIONS.md` records the one conflict this design decides: whether cost can honestly be stored as unknown depends on this answer, and it is the place where persistence's why #4 promises more confidence than the mechanism can deliver.

## Questions for review

- Is this worth doing early, given it changes decisions already made, or is it fine to keep the conservative "let it finish" behaviour indefinitely and never need the answer?
- Are you willing to spend real money on provoking long thinking responses for this, and roughly how much? The arithmetic above lands somewhere between roughly five and fifty dollars depending on how coarse the usage instrument turns out to be; the estimate needs your ceiling, not the other way round.
- The measurement is proposed on **API keys only**, with subscription-backed billing held constant rather than tested, because whether a subscription bills differently is unknown here and entangled with the OAuth work. Is that gap acceptable, given a subscription is what you actually use day to day?
- The recording rule above says a cancelled request's cost may be stored as **unknown-with-a-reason**, and that cost queries therefore report coverage alongside totals. That is a small permanent complication in the analytics surface, bought to avoid silently wrong numbers. Confirm you want it that way rather than treating unmeasurable cost as zero.
- **The recording rule does not actually depend on the measurement.** Its shape is known whatever the billing answer is, so persistence could adopt it now and this experiment could be deferred indefinitely without blocking anything. Do you want the rule landed independently, which would leave this experiment purely optional?

## Index

| Aspect | L1 | L2 | L3 |
|---|---|---|---|
| Model framing | | | |
| Wire & cache | E | §The cancel points | |
| Tool surface | | | |
| UX & input | | | |
| Ownership & placement | | | |
| Lifecycle | E | §What the experiment must produce | |
| Storage | P | §What the experiment must produce | |
| Economics | E | §The quantity being measured | |
| Security | | | |
| Testing & verification | P | §The instrument problem | |
| Code shape | | | |
| Dev workflow & references | | | |
| Core migration | | | |
