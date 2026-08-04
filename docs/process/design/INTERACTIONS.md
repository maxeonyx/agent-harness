# Interactions — the portfolio view

**Stages: this is the stage-3 output (agent-drafted, unreviewed).** Produced after every design doc reached stage 2, because interactions cannot be judged before the things interacting exist.

This is the cross-cutting half of stage 3. Each design doc has its own Interactions section for connections local to it; this file holds what belongs to no single doc — machinery several experiments need and none owns, places where two designs genuinely disagree, the dependencies between them, and the parts of the matrix that turned out to be empty. A conflict between two designs is written up here once rather than in both docs. Dependencies are recorded here; turning them into an order is `PLAN.md`'s job, not this file's.

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

The probe's shopping list is wider than read semantics. It must also verify the **cache-write multiplier** (assumed ~1.25× base input price from general provider knowledge, unverified here), because compaction's payback arithmetic — one-off write cost against per-turn read savings — is most exposed to it, alongside the read discount, TTLs, what counts as a prefix, and what a fork inherits.

**Self-modification is deliberately *not* in this cluster**, which is worth recording because it has a cache interaction and could be mistaken for a cache design. It is about iteration speed on the harness itself; its single cache interaction is that a reload **must not force cache expiry**. That needs the cache state *observed*, not *predicted* — the claim is that a reload leaves the system prompt byte-identical, and whether it did is answered directly by the provider's reported cached-input tokens on the next request. No bet is placed, so there is nothing to predict, and **self-modification does not depend on the cache probe** — a useful independence given how much else does.

### One finish procedure, several reasons

Forked-subagents concluded that cleanup is not a cancellation feature but what ending a task *means*, and that there should be a single finish procedure parameterised by reason. That conclusion has reach beyond its own doc.

Compaction's two-stage flow contains a tidy-up step for exactly this reason: the middle step exists so the agent cleans up while it still has the context to know what it owns. Layered-shutdown hits the same procedure from the other end and proposes *not* running it during shutdown, recording cleanup-outstanding instead. Operator-lifecycle needs it because a relaunch can land mid-scope.

So four designs describe pieces of one mechanism. If they are built separately there will be three or four cleanup paths with different semantics, which is precisely the failure the single-procedure conclusion was meant to avoid. Worth noting that the reasons genuinely differ in one respect — cancellation and compaction give the agent a turn, shutdown may not — so the parameterisation has to include "do you get to act at all".

### Ordered durable events, causal sends, and catch-up

Topology needs reconnect and catch-up without duplicates, and multiple faces seeing coherent state. Multi-client-ui needs stale sends to be representable rather than silently applied, and reconnect without duplicates. Persistence-analytics is where the ordered durable event stream actually lives.

These three are close to describing one substrate. The distinction that keeps them separate designs is real, though: topology asks whether the substrate can be *sequenced* across a process boundary at all, multi-client-ui asks what converges when it cannot, and persistence asks what is durably recorded either way. The unresolved same-machine-IPC question sits underneath topology and multi-client-ui equally.

Persistence now also names what a resume point is *over*: the per-store arrival sequence (`origin_brain` + `arrival_seq`) is the cursor that topology's rejoin, multi-client-ui's reconnect, and federation's sync watermarks all resume from — an honest fact about the store that wrote it, not a cross-emitter causal order.

**Snapshots and roll-ups are part of this cluster, by user ruling, 2026-08-04.** Added wording preserved: "while we have event streaming, we should also have roll ups, and we should deliver snapshots, not just event streams. I would ideally like that baked into the model from the very start... every thing that implements event streaming should ideally implement snapshotting." The reason given is cost — "otherwise it gets really, like, really expensive." Note the hedging: "ideally" in both halves.

This is a genuine addition rather than a detail, and it changes the cluster in three ways. It makes **snapshotting a companion obligation of event streaming** rather than an optimisation added later, which is a constraint on every design that emits events — so it belongs in the shared-machinery list, not in one doc. It makes **catch-up cheap**, which is what multi-client-ui's reconnect and topology's rejoin both need; replaying a long session's entire event history to a phone that just woke up is exactly the expense being avoided. And it interacts with persistence's retention design, because a snapshot is a **roll-up of facts that may themselves be collectable** — which is a cleaner answer to the tension persistence recorded between keeping every context epoch forever and collecting superseded rows: once a snapshot exists, the events behind it may be able to go.

A gap this made sharper closed the same day. The credential store had been flagged (in operator-lifecycle's questions) as a fourth durable store with no version, snapshot or migration story and no owner. Ruled 2026-08-04: credentials live *inside* the session database, as a durable-never-projected row class whose replication is scoped by brain profile — "credentials should be treated like everything else we treated" — so the database's own snapshot and migration ceremony covers them and no fourth store exists. The OS keychain remains available as a security root (a key encrypting those rows at rest), which is decoupled from the home of record. Detail in `oauth-credentials.md`.

### The single context projection

Already ruled from walking-skeleton evidence: the request builder and the `/dump` introspection share one projection so they cannot diverge. Stage 2 found three more designs leaning on it.

Compaction needs to describe the difference between the current context and the successor's, and does it by reading the **content-version record** — which elements this context contains, at what version — against the world; that is the same record context-updates' notice diff reads at flush time, one computation at two boundaries. (An earlier framing demanded a deterministic, re-runnable dry-run rebuild; determinism is unachievable — a rebuild states the current time and reads current files — and not needed, since the diff is at the level of which elements change. What survives is that the projection has **no hidden state**.) Context-updates needs the rebuild to be canonical and to not replay obsolete notices. User-turn adds a projection per user tool. Multi-client-ui adds a negative requirement — shared-live state must have no path into it — with the subtlety that appearing in `/dump` is not the violation, since `/dump` deliberately shows what the model cannot see; appearing *unmarked*, or on the wire, is.

### Data lifecycle classification

Persistence-analytics owns this and proposed splitting invariant 5's four classes into two independent axes, a retention rule and a projection rule, because none of the four names can express "durable but must never reach the model". That refinement is load-bearing for others: oauth's refresh tokens are exactly durable-but-never-projected, and multi-client-ui's shared-live state becomes the other instance if persistence's draft-durability proposal is accepted — multi-client-ui itself classifies it as live rather than durable. Context-updates contributes notices, which are durable only for the lifetime of the current context.

If the two-axis refinement is rejected, three designs need a different answer to the same question.

### Construction and injection

Modular-components owns this, and it is upstream of every other experiment's *testing*. It is upstream of four experiments' *designs* as well, which is more than it first looks: **topology**, because in-process composition is both designs' claim from different directions; **self-modification**, because a plugin that receives everything at construction is a plugin whose fault has a name, which is what auto-rollback's attribution needs; **oauth-credentials**, because a destination-bound fetch is worth exactly as much as the rule that a plugin cannot construct network access for itself; and **operator-lifecycle**, because its pre-activation self-check needs config validation to be a pure side-effect-free mode rather than a consequence of starting up.

**The standing merge question with topology, answered here rather than in both docs: do not merge; make modular-components a precondition of topology.** Their falsification surfaces differ — modular-components fails if an in-process test cannot reach an assertion surface without naming a private type, or if config forces a component to know its construction context; topology fails if a scenario behaves differently co-located and split. A merged experiment that failed would not say which claim failed, which matters most for "in-process composition is not a test-only path", where the temptation to fudge is real. The genuine overlap is one mechanism rather than one design, and it splits: **modular-components owns the builder and the rule that each participant is handed distinct port instances; topology's deliberately hostile co-located configuration is the check that the rule held.** Only the builder can enforce distinctness; only a hostile deployment can prove it was not quietly bypassed. Both docs raised this, so it wants the user's ruling.

### Configuration merge machinery

Two designs need one recursive merge, and the split between them is the finding. Modular-components ports `deconfuse`: a typed schema declared once, explicitly ordered sources, precedence as a separate axis from load order, recursive field-by-field merge. Limb-model needs the same machinery for context-layer composition and deliberately declined to design it, bounding the problem instead — composition is a deterministic function evaluated at known rebuild boundaries — and leaving the *precedence order itself* open.

So: **modular-components owns the merge machinery; limb-model owns the precedence policy.** The consequence worth acting on is that the machinery must take precedence as an explicit input rather than baking config's own answer into it. Deconfuse already separates precedence from load order, so this is a feature to *preserve* in the port rather than a generality to add — and it is not unused, since it is what lets limb-model reuse the machinery once someone rules on layer precedence. `PLAN.md` flags the two as merge candidates; this is the sharper boundary.

### The limb as the place work happens

Limb-model owns it, and it turns out to be load-bearing in more places than "tools live here". Self-modification's fast workflow — editing plugins in memory — happens in the meta limb, because that is the only limb where the live plugin set is reachable; its slow workflow happens in an ordinary project limb pointed at the harness's own repository. So it is the **shell / soft-middle boundary** that divides the two workflows, and the meta limb hosts one side of it rather than being the divider. Persistence-analytics exposes cross-session search as a limb rather than as a special feature. Context-updates needs the limb to *report* change while the brain decides what to do about it. User-turn's tools reach the filesystem through the limb even when face and limb share a machine.

## Where two designs actually disagree

Discovering these was the point of doing this stage after stage 2 rather than before. Two have since been settled by the user and are marked as such; the rest need a ruling and are collected under Questions for review at the end. A closing subsection records three that were listed here and turned out not to be conflicts at all.

**Topology's own two whys — RESOLVED by user ruling, 2026-08-04.** This was recorded as a conflict: why #4 says rate limits, billing, session management and provider connection should exist exactly once, while why #7's federation puts a brain on every machine that hosts limbs. The reading was too coarse. "Exactly one" is **per provider/billing/data-access domain**, not globally, and the user's domains must stay separate for reasons that have nothing to do with topology — "home data access, work data access, home billing, work billing should be separate." Federation is what keeps separate things separate while still being reachable from one place, so it does not violate #4. Within a domain #4 holds undiluted: one home brain is enough. Also ruled: acting as your own brain and connecting to another brain are both ordinary configurations, neither privileged. The full statement is in `topology.md` under why #4.

**Fork-for-cache versus safe parallel work — reframed as a decoupling, one empirical question remaining.** This was recorded as a forced trade: forked siblings share one limb's filesystem (which the notes are blunt is a poor experience), and the safe answer — a fresh clone per child via the limb factory — seemed to force a fresh context, costing exactly the cache saving that motivated forking. The trade dissolved on audit because two pairs were fused: *safety* with *separate limbs* (safety needs disjoint write regions, which per-child worktrees inside one limb provide), and a limb's *context* with its *location* ("different limb ⇒ always fresh" exists because a different limb carries different load-bearing instructions — which a clone of the same repo does not, so a same-context clone costs a bounded prefix delta, not a fresh context). What remains open is empirical, not structural: whether *instructed* ownership of a write region holds in practice, or isolation must be enforced. Detail in `forked-subagents.md`.

**"Input is cheap" versus the permanence arithmetic — REFRAMED by user feedback, 2026-08-04: not a conflict, a quantity.** This was recorded as two positions that might be incompatible: user-turn's why #4 holds that input is cheap so attaching what the user looked at is affordable, while context-updates' why #3 establishes that anything appended to a warm context is re-read on every subsequent request for the rest of the session.

The user's feedback, wording preserved: **"this is not about a versus b. It's about how much a versus how much b."** So the framing as a contradiction was wrong; it is a sizing question, and the design work is finding the ratio rather than picking a side.

Two clarifications came with it. First, what "input is cheap" is cheap *relative to*: "Input is cheap compared to output. Right? And compared to repeated tool calling." Both comparators matter — the second is the one that justifies attaching context at all, since the alternative to carrying what the user looked at is an agent making repeated tool calls to rediscover it.

Second, on whether the two arithmetics even meet, the user worked through it and did not fully settle it, so his uncertainty is preserved rather than tidied away: "The context updates arithmetic has nothing to do with output. It's all about input, I think. Not not clear. Well, actually, no. I guess it is in intention because the context additions... well, they're not... I think it's they're not additional terms. They piggyback on additional terms, so they're not really in contest still. But but... yes." The load-bearing observation in there is the **piggyback** one: user activity does not create additional turns, it rides on turns that were going to happen anyway (invariant 2 and `source-notes/context-and-agent-loop.md`), which is why it is not straightforwardly in contention with the permanence cost. It still accumulates, so the quantity question stands.

**Invariant 7 reaches multi-client-ui's case with one clause, not two.** Its first clause — "stale clients cannot silently overwrite newer state" — already decides the stale-phone-versus-newer-desktop case in one direction. What it does not cover is the other direction: silently *discarding* the phone's text is the same failure. Multi-client-ui proposes one added clause rather than a replacement — no branch of the user's own text is discarded silently; a stale send may be rejected, merged or superseded provided the losing text stays recoverable and the divergence visible.

**Streaming is deferred, but topology wants a streaming fast path.** Streaming responses are deferred by explicit ruling. That means the direct face-to-limb fast path can only be exercised for tool stdout, not model tokens, which is a weaker test of the capability than the design implies.

**Two cache figures — not a conflict; different quantities, now labelled.** Compaction's ~10× read discount (and ~1.25× write multiplier) are general provider assumptions the cache probe must verify — the earlier "the discount he observes" framing had no root in the source notes and is withdrawn. The user's "conservative assumption of 5x" (`source-notes/compaction.md`) is his floor for a different question: whether the append-based handover tool pays given it takes a cache hit. Compaction designs to the conservative figure and labels both.

**Oauth and modular-components — resolved in modular-components' favour.** The two docs disagreed about whether they interact at all; modular-components had the better of the argument (a destination-bound fetch is worth exactly as much as the rule that a plugin cannot construct network access for itself), and oauth's doc now records the connection: one-way, into oauth, which assumes the construction rule holds for provider plugins and owes nothing back.

### Three that were recorded here and are not conflicts

Recorded because each was written up as needing a ruling, and a reader who re-derives them will spend the same effort twice. Each dissolved on a closer read of the source it cited, not on a judgement call.

**Invariant 9 does not fight soft cancellation.** The invariant's own sequence is *request → drain → finalize*, which already supplies the distinction: the agent's cleanup turn sits in the **request** phase, and the drain is the phase that follows it. "A drain structurally cannot start new work" is true of the drain and says nothing about the cleanup turn. No new vocabulary is needed and the invariant's wording stands.

**Permission versus invitation is two different subjects.** `REQUIREMENTS.md` rejects permission prompts as a model of *tool-execution* authority — "personal limbs may run in YOLO mode". `source-notes/agent-hierarchy.md` asks for the user's consent before a session is launched that will *block on him*. Those do not overlap, so there is nothing to reconcile.

**An autonomous session needs no `done` tool.** `source-notes/agent-hierarchy.md` says an autonomous session "Completes automatically at end of turn", so cleaning up and then ending the turn *is* completion. The `/done` in the notes is a **user** slash-command in user-facing sessions, not an agent tool. The apparent conflict came from design-side wording for the cancellation message rather than from anything in the notes, so it is a wording choice we own and not a question for the user.

## The cells that turned out to be empty

Recording these because the method asks for connections to be *discarded* as well as developed, and because an empty cell is information — it means two experiments can proceed without coordinating.

Oauth-credentials connects to self-modification (auth outside the plugin), persistence (credential rows as a durable-never-projected class, ruled 2026-08-04), and topology (credentials stay brain-owned; replication scoped by profile), plus the modular-components question recorded among the disagreements above and one line to cancellation-economics below. It has no relationship with forked-subagents, user-turn, multi-client-ui or context-updates beyond the trivial.

Cancellation-economics is peripheral by design: it touches persistence (where its answer is recorded), forked-subagents and operator-lifecycle (whose assumptions it tests). It has nothing to say to limb-model, multi-client-ui or modular-components — the latter because that experiment measures billed tokens against an external account-level instrument on real providers, so the fast in-process suite is irrelevant to it in both directions.

**Oauth and cancellation-economics have one real line between them**, running from cancellation outward: the measurement is proposed on API keys only, holding subscription-backed billing constant rather than testing it, because whether a subscription bills differently is unknown and entangled with the OAuth work. That gap matters because a subscription is what the user actually uses day to day, and closing it is oauth's if it matters.

The TUI styling and throbber material inside multi-client-ui connects only to self-modification's soft-middle boundary, and to multi-client-ui's own state model via the throbber-as-test argument. It is otherwise inert — which is why it can be deferred without blocking anything.

Layered-shutdown's connections are almost entirely to operator-lifecycle and topology, plus the finish-procedure cluster — and the operator-lifecycle link gained a real dependency: the shutdown escalation ladder's out-of-band rung (SSH in and kill) exists exactly where operator-lifecycle classifies a boundary as *repair*-able, so the two share that classification. It has no meaningful relationship with user-turn, context-updates or multi-client-ui, which supports the conclusion in its own doc that it may be a pattern note rather than an experiment.

Limb-model and multi-client-ui barely interact. Both are about where things live, but one is about environments for agents and the other about views for humans, and they meet only through topology.

## The dependencies this implies

Dependencies, not an order: what cannot be interpreted before what. `PLAN.md` turns these into a sequence.

The tight cluster is one design conversation even though it is four experiments: compaction, context-updates and forked-subagents, plus the cache probe underneath them. All three designs depend on the probe and none can settle it alone, so results obtained before it lands are not interpretable.

The events-and-transport cluster — topology, multi-client-ui, persistence — is a second conversation, coupled to the first only through persistence.

The periphery — oauth, cancellation-economics, layered-shutdown, modular-components — depends on nothing in either cluster. Modular-components is the one to watch, because being depended on is not the same as depending: it is upstream of everyone's testing and of four designs, while needing nothing itself.

## Questions for review

Portfolio-level calls that belong to no single design. Each is stated where the reasoning is, above.

- **Should modular-components and topology merge?** Answered above as *no — make modular-components a precondition of topology*, on the grounds that their falsification surfaces differ and a merged failure would be unattributable, with the real overlap being the builder plus the distinct-port rule. Both docs raised it, so it wants one ruling.
- **Is the two-axis data-lifecycle refinement accepted?** Persistence proposes splitting invariant 5's four classes into a retention rule and a projection rule, because none of the four names can express "durable but must never reach the model". Oauth's refresh tokens, multi-client-ui's shared-live state and context-updates' notices all lean on it. If it is rejected, three designs need a different answer to the same question.
- ~~Cache figures: accept the labelling?~~ **Accepted 2026-08-04**: "These are assumptions, which is fine. I would not want to lock them down, since timelines are uncertain and model pricing will likely change." And on the entry itself: "Interactions require the two cache figures as conflict, and they are not one. That is fine."
- **Does invariant 7 gain one clause?** Its first clause already forbids a stale client silently overwriting; multi-client-ui proposes adding the symmetric half — no branch of your own text is discarded silently either.
- **Fork's parallel case: is instructed ownership acceptable?** The fork-versus-safety trade dissolved into per-child worktrees within one limb plus instructed write-region ownership, with enforced isolation (fresh manufactured limb) as the fallback. The remaining question is whether instructing ownership is acceptable or isolation must be enforced — and whether the worktree/location-refinement reading matches your intent.
- **Is the streaming fast path worth building against a deferred capability?** With streaming responses deferred, topology's direct face-to-limb path can only be exercised for tool stdout, which is a weaker test than the design implies.
