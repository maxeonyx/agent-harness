# Persistence and analytics — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why (agent-drafted, unreviewed) · what, interactions, summary — not yet done.** Derives from `source-notes/analytics.md`, `source-notes/tech.md`, the storage sections of `source-notes/agent-harness-design.md`, `source-notes/federated-brain.md`, and the "session storage blast radius" entry in `source-notes/open-questions.md`.

SQLite from the start, one database the brain owns. This design is about what the schema must be able to represent, and what it must not foreclose.

## Why

### 1. Schema mistakes are the expensive kind — *correctness, and the cost of being wrong*

Most of this project's mistakes are cheap: experiment code is disposable, and core is re-designed fresh from evidence. Storage is the exception. Once real sessions exist, a schema error is not a rewrite of code but a migration of data — and the data is the user's actual work history, which cannot be regenerated.

The notes are also honest that the risk was initially underestimated. Storage was claimed not to be deeply entangled with the agent loop, but the hierarchy model makes storage semantics central to concurrency, blocking, resume and compaction — "may be larger work than assumed". So this is not a format choice with a clear boundary; it is the substrate that several soul designs express themselves through. That is the argument for designing it before it calcifies rather than letting it accrete.

### 2. Restart must continue the work, without asking the user — *correctness + desire*

From the notes: the brain works out of SQLite, and on restart "should pick up where it left off without requiring user interaction to resume threads". Graceful shutdown does its part — wait for in-flight API requests to complete, record that tool calls were about to run, then on relaunch run them and carry on.

What makes this more than ordinary crash-safety is self-modification. The whole point there is that an agent edits the harness, rebuilds, and relaunches onto new code while *smoothly continuing*. That loop is impossible unless session state is durable and resumable by design. So persistence is a hard prerequisite for the harness's ability to develop itself — the two designs are coupled far more tightly than a storage layer would normally be to a build process.

The notes also add a judgement call rather than a rule: relaunching within an hour can just continue, but beyond an hour the first client should decide whether to resume other agents; in server mode it should probably continue regardless. Deliberately left optional.

### 3. The user wants to use his own session data as a record of his work — *desire*

Stated plainly: "I want to be able to use my session data for timesheets, for example." This is a human desire, and it is not about the harness at all — it is about the user's working life. It changes the requirement from "store enough to run" to "store enough to answer questions about what I did", which is a materially higher bar and demands classification, timestamps that survive, and queryability by session and project.

It also sets the standard for *how* queries happen: "I'm ok with it just being sql queries." No query API needs designing; the schema being sane is the deliverable.

### 4. Cost decisions are currently guesses, and this is what makes them checkable — *resource*

The design elsewhere makes confident economic claims — compaction earlier beats compaction later, forking beats fresh for shared context, cache reads are roughly ten times cheaper. Those claims are the "secret sauce" of the compaction design, and right now they are reasoning rather than measurement.

So the root here is not "track costs because costs matter" but something sharper: **this is the experiment that lets the rest of the design be verified rather than believed.** Actual token usage across all API requests, by session, per message, with model and provider recorded, is what turns compaction timing from a plausible argument into a tuned parameter.

The cancellation question is a specific instance the notes single out: providers may charge for cancelled requests, so the tentative preference is not to cancel after the first byte but to let the future complete and discard the result — while wanting to know the truth. "Probably worth experiment." That question is separated out as its own targeted experiment, but it is *this* schema that has to record the answer.

### 5. Data has genuinely different lifetimes, and conflating them is how a database rots — *correctness*

The notes distinguish storage levels explicitly: some events durable, some not; some stored only as a projection for analysis, others stored as events; some durable events needed only for the lifetime of the session, some only for the lifetime of the current context, others forever.

Invariant 5 makes this non-negotiable — durable, cache-supporting-transient, shared-UI and disposable-stream data must be explicitly distinguished. The root is that without a declared lifecycle class per datum you get one of two failures: keep everything forever, and queries slow as the database grows without bound; or delete by guesswork, and lose something load-bearing. Cache-supporting data is the awkward middle case — it must survive while useful and be cleaned once the cache it supported has expired.

### 6. Searching his own history is a work tool — *desire*

The notes want session messages and data in an easily searchable format, and a read-only tool surface "or limb" for meta-work. The limb framing matters: this is the meta limb from the limb model, which means cross-session search is not a special feature but an ordinary limb exposing ordinary tools. Ideally queries span all connected brains.

### 7. It gets large fast — *resource*

"It could get very large very fast." Hence indices from the start, message contents and large text or images kept separate from the event tables themselves, and normalisation as far as it goes without degrading performance. This is a mundane root but it constrains the schema shape directly.

## Forward: what these roots force

- **A lifecycle class on every stored datum**, declared rather than implied — the direct expression of invariant 5, and the thing Gate 2 checks.
- **Hierarchy state must be representable**: conversation threads and their parent/child/sibling relationships, a parent blocked on a scope, sibling sets that grew *after* the parent blocked, user-facing session lifecycle, and resume targets complete enough to restart a session as a subagent.
- **Proposed-but-unexecuted tool calls are valid stored state.** Already ruled from walking-skeleton evidence: on cancel, keep the in-flight response but do not execute its proposed calls; they get no fabricated outcome, are omitted from the wire, stay visible to introspection, and may execute on a later resume. The schema has to hold that state honestly.
- **Cache metadata is durable, not ephemeral.** From the notes: "Backend server cache ids etc should not be ephemeral - they should be tracked in the DB by session, so that we can seamlessly continue on relaunch." This is also the store that context-updates and compaction need for cache-state prediction, so it is shared machinery, not local bookkeeping.
- **Provenance and globally unique identity**, so background sync between federated brains cannot duplicate rows or confuse remote data with local.
- **Per-request response metadata as a first-class table** — cost, token usage, cache hit rates, model, provider, tool durations — because #4 is only satisfied if it is queryable, not logged.
- **Blob storage separated from hot tables** from #7, and the query surface treated as product behaviour: per `PROCESS.md`, analytics queries are a public test surface, so they are asserted in black-box tests rather than treated as internals.

## Parked for later stages

**Storage model preference already settled in the notes:** OpenCode's SQLite approach over Pi's JSONL — single indexed database for all sessions, SQL discovery rather than filesystem globbing, branching as a row with a parent FK, real schema migrations. The comparison table in `agent-harness-design.md` is the reasoning.

**Explicitly listed representation demands** (from the same note): threads and relationships, blocked parent state, dynamic sibling sets, user-facing session lifecycle, resume targets, compaction/handover continuity, per-message API response metadata.

**Interactions flagged for stage 3:** self-modification (durable resumable state is what makes relaunch-onto-new-code possible; plugins may live in the DB so an old schema stays addressable while a cache is valid); compaction-handover and context-updates (both need stored cache metadata for cache-state prediction, and both create rebuild boundaries that are durable facts); forked-subagents (the hierarchy state above *is* this schema's hardest requirement); topology (durable ordered events, cross-brain queries, provenance); multi-client-ui (shared-UI state is its own lifecycle class — live but not durable, and explicitly not model context); cancellation-economics (this schema records its answer); operator-lifecycle (migrations are the risky part of any update).

## Questions for review

- Is the "one hour" resume rule worth designing now, or is it a preference to leave until the harness is actually being restarted often?
- Should shared-UI state live in this database at all, or in a separate store with different durability? Invariant 5 names it as a class; it does not say it shares a home.
- Timesheet use (#3) implies a notion of project and of working session that may not map cleanly onto agent sessions and limbs. Worth designing explicitly, or leave it to SQL over what happens to be recorded?
