# Interactions — the portfolio view

**Stages: this is the stage-3 output (agent-drafted, unreviewed).** Produced after every design doc reached stage 2, because interactions cannot be judged before the things interacting exist.

This is the cross-cutting half of stage 3. Each design doc has its own Interactions section for connections local to it; this file holds what belongs to no single doc — machinery several experiments need and none owns, places where two designs genuinely disagree, and the parts of the matrix that turned out to be empty. Sequencing consequences are not here; they are in `PLAN.md`.

## How the matrix was examined

Fourteen designs give ninety-one pairs. Examining all of them at the level of *aspects* (the shared vocabulary at the end of `README.md`) rather than at the level of whole designs is what makes it tractable, and it is also what makes the empty cells visible — two designs can both be about lifecycle and still have nothing to say to each other.

The honest summary of density: the portfolio is not evenly connected. There is a tight cluster around context and cache that is almost one design wearing four hats, a second cluster around events and transport, and then a periphery of designs that connect to the middle but barely to each other. That shape is the most useful thing this stage produced, because it says which experiments can proceed independently.

## Shared machinery: needed by several, owned by none

These are the real finding. Each is something more than one experiment requires, which means each is something that will otherwise be invented several times, differently.

### Cache-state prediction

The largest one. Three designs cannot function without an answer to "is the cache warm right now, and will it be in a moment":

Compaction's why #4 — the agent picking the moment — is *specifically* about firing before the cache lapses, and its why #3 depends on compacting earlier than a naive trigger would. Context-updates' entire structure follows from choosing append mode or rebuild mode, which is the same prediction. Forked-subagents' fork-versus-fresh routing turns on whether the parent's prefix is still warm, and its most awkward open case — a fresh child wanting forked siblings when parent and forks are all stale — is nothing but this prediction in a hard instance.

Underneath all three sits the same unknown, which none of them can settle alone: **the provider's actual cache semantics.** What counts as a prefix, whether append-only means append-only with respect to the whole context or some smaller unit, what a fork inherits, and how the OpenAI responses API and the Anthropic messages API differ. The notes are explicit that getting this right is a precondition rather than a detail.

Two consequences. First, this wants to be probed *before* the three designs that depend on it commit to mechanisms — it is small, cheap and unblocks a lot. Second, prediction needs durable cache metadata, which is persistence-analytics' territory, so persistence is upstream of it in a way that is easy to miss.

### One finish procedure, several reasons

Forked-subagents concluded that cleanup is not a cancellation feature but what ending a task *means*, and that there should be a single finish procedure parameterised by reason. That conclusion has reach beyond its own doc.

Compaction's two-stage flow contains a tidy-up step for exactly this reason: the middle step exists so the agent cleans up while it still has the context to know what it owns. Layered-shutdown hits the same procedure from the other end and proposes *not* running it during shutdown, recording cleanup-outstanding instead. Operator-lifecycle needs it because a relaunch can land mid-scope.

So four designs describe pieces of one mechanism. If they are built separately there will be three or four cleanup paths with different semantics, which is precisely the failure the single-procedure conclusion was meant to avoid. Worth noting that the reasons genuinely differ in one respect — cancellation and compaction give the agent a turn, shutdown may not — so the parameterisation has to include "do you get to act at all".

### Ordered durable events, causal sends, and catch-up

Topology needs reconnect and catch-up without duplicates, and multiple faces seeing coherent state. Multi-client-ui needs stale sends to be representable rather than silently applied, and reconnect without duplicates. Persistence-analytics is where the ordered durable event stream actually lives.

These three are close to describing one substrate. The distinction that keeps them separate designs is real, though: topology asks whether the substrate can be *sequenced* across a process boundary at all, multi-client-ui asks what converges when it cannot, and persistence asks what is durably recorded either way. The unresolved same-machine-IPC question sits underneath topology and multi-client-ui equally.

### The single context projection

Already ruled from walking-skeleton evidence: the request builder and the `/dump` introspection share one projection so they cannot diverge. Stage 2 found three more designs leaning on it.

Compaction needs to describe the difference between the current context and the successor's, which means building the successor's context *before* the successor exists — a dry-run rebuild, which only works if the projection is deterministic and re-runnable. Context-updates needs the same rebuild to be canonical and to not replay obsolete notices. User-turn adds a projection per user tool. Multi-client-ui adds a negative requirement — shared-live state must have no path into it — with the subtlety that appearing in `/dump` is not the violation, since `/dump` deliberately shows what the model cannot see; appearing *unmarked*, or on the wire, is.

### Data lifecycle classification

Persistence-analytics owns this and proposed splitting invariant 5's four classes into two independent axes, a retention rule and a projection rule, because none of the four names can express "durable but must never reach the model". That refinement is load-bearing for others: oauth's refresh tokens are exactly durable-but-never-projected, and multi-client-ui's shared-live state is the other instance. Context-updates contributes notices, which are durable only for the lifetime of the current context.

If the two-axis refinement is rejected, three designs need a different answer to the same question.

### Construction and injection

Modular-components owns this, and it is upstream of every other experiment's *testing* rather than of their designs. Topology is the exception where it is upstream of the design too, since in-process composition is both designs' claim from different directions — which is why the two are a standing merge candidate.

### The limb as the place work happens

Limb-model owns it, and it turns out to be load-bearing in more places than "tools live here". Self-modification performs its editing in the meta limb, and stage 2 found the meta limb divides self-modification's two workflows more cleanly than expected. Persistence-analytics exposes cross-session search as a limb rather than as a special feature. Context-updates needs the limb to *report* change while the brain decides what to do about it. User-turn's tools reach the filesystem through the limb even when face and limb share a machine.

## Where two designs actually disagree

Discovering these was the point of doing this stage after stage 2 rather than before. None is fixed here; several need a ruling.

**Topology's own two whys.** Why #4 says rate limits, billing, session management and provider connection should exist exactly once — that is the argument for centralising the agent loop. Why #7's federation puts a brain on every machine that hosts limbs, and a brain is by definition a thing that calls providers. So federation reintroduces the N-of-everything that #4 exists to avoid. Both are the user's; they need reconciling rather than choosing between.

**Fork-for-cache versus safe parallel work.** Forked-subagents' whys #1 and #5 make fork the cache-efficient default. But forked siblings share one limb's filesystem, which the notes are blunt is a poor experience, and the safe answer — a fresh clone per child via limb-model's limb factory — crosses a limb boundary and therefore forces a fresh context, costing exactly the cache saving that motivated forking. If that is right, fork's real home is sequential decomposition and read-mostly parallel work, and the headline parallel case is mostly fresh-plus-attachments. That is a different emphasis than the whys currently carry.

**"Input is cheap" versus the permanence arithmetic.** User-turn's why #4 carries the user's position that input is cheap, so attaching what he looked at is affordable. Context-updates' why #3 establishes that anything appended to a warm context is re-read on *every* subsequent request for the rest of the session — appended material is not paid once, it is paid forever. User-turn appends a lot: file views, terminal output, searches. The two positions are not obviously compatible, and this is the assumption most likely to fail first in practice.

**Invariant 9 versus soft cancellation.** Invariant 9 says a drain structurally cannot start new work; soft cancellation deliberately starts new work in the form of cleanup tool calls. Forked-subagents proposes these are two layers — an agent-level message where the loop keeps running, and a harness-level drain where the prohibition applies. If that reading is right the vocabulary should distinguish them; if not, the invariant's wording needs changing.

**Invariant 7 does not cover multi-client-ui's actual case.** "The user wins on conflicting edits" resolves user-versus-agent. A stale phone draft against a newer desktop draft is user-versus-user, and the invariant is silent. Multi-client-ui proposes a negative rule instead — no branch of the user's own text is discarded silently.

**Streaming is deferred, but topology wants a streaming fast path.** Streaming responses are deferred by explicit ruling. That means the direct face-to-limb fast path can only be exercised for tool stdout, not model tokens, which is a weaker test of the capability than the design implies.

**Two cache figures.** Compaction's why #3 uses roughly 10× for the cache-read discount; `source-notes/compaction.md` says "a conservative assumption of 5x". Read as observed-versus-conservative, the design should be tuned to the conservative one, but the two numbers should not both float around unlabelled.

**Permission versus invitation.** `REQUIREMENTS.md` says permission prompts and approval theatre are explicitly unwanted; the hierarchy notes say launching a user-facing session requires user permission. Forked-subagents reads the latter as a request for the user's *attention* rather than authorisation to act, which makes the expiry idea natural rather than an escape hatch.

**How an autonomous session ends.** The notes say autonomous sessions complete automatically at end of turn, but the cancellation flow says "clean up then call done", implying a `done` tool. These need reconciling, and the answer changes the stopping design.

## The cells that turned out to be empty

Recording these because the method asks for connections to be *discarded* as well as developed, and because an empty cell is information — it means two experiments can proceed without coordinating.

Oauth-credentials connects to self-modification (auth outside the plugin), persistence (token storage), and topology (credentials stay brain-owned) and to essentially nothing else. It has no real relationship with forked-subagents, user-turn, multi-client-ui, context-updates or cancellation-economics beyond the trivial.

Cancellation-economics is similarly peripheral by design: it touches persistence (where its answer is recorded), forked-subagents and operator-lifecycle (whose assumptions it tests), and nothing else. It has nothing to say to limb-model, multi-client-ui, oauth or modular-components.

The TUI styling and throbber material inside multi-client-ui connects only to self-modification's soft-middle boundary, and to multi-client-ui's own state model via the throbber-as-test argument. It is otherwise inert — which is why it can be deferred without blocking anything.

Layered-shutdown's connections are almost entirely to operator-lifecycle and topology, plus the finish-procedure cluster. It has no meaningful relationship with user-turn, context-updates or multi-client-ui, which supports the conclusion in its own doc that it may be a pattern note rather than an experiment.

Limb-model and multi-client-ui barely interact. Both are about where things live, but one is about environments for agents and the other about views for humans, and they meet only through topology.

## What this implies

The tight cluster — compaction, context-updates, forked-subagents, and the cache-state prediction underneath them — should be treated as one design conversation even though it is four experiments, and the provider cache probe should come first because all three depend on it and none can settle it.

The events-and-transport cluster — topology, multi-client-ui, persistence — is a second conversation, coupled to the first only through persistence.

The periphery — oauth, cancellation-economics, layered-shutdown, modular-components — is genuinely independent and can proceed in parallel with either cluster.

Sequencing lives in `PLAN.md`.
