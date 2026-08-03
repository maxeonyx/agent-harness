# Persistence and analytics — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why, what (agent-drafted, unreviewed) · interactions, summary — not yet done.** Derives from `source-notes/analytics.md`, `source-notes/tech.md`, the storage sections of `source-notes/agent-harness-design.md`, `source-notes/federated-brain.md`, and the "session storage blast radius" entry in `source-notes/open-questions.md`.

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

## What

The deliverable is a schema at the level of tables, relationships and declared lifecycle classes. Not DDL — the exact column types will be decided while writing the migration, and getting them right is not where the risk is. The risk is in what the schema can and cannot *say*.

The starting point is already settled by the notes, so it is not re-argued here: SQLite from the beginning, one database the brain owns, a single indexed database for all sessions rather than a file per session, discovery by SQL query rather than filesystem globbing, branching represented as a row with a parent foreign key, and real schema migrations. That is OpenCode's model in preference to Pi's JSONL, and the comparison table in `agent-harness-design.md` is the reasoning. What the notes leave open — and what the rest of this section is about — is two things: how every stored datum declares its lifetime, and how the hierarchy model's state is actually represented. Those are the two places a wrong decision costs a data migration rather than a code change.

### What a lifecycle class actually decides

Invariant 5 names four classes: durable, cache-supporting-transient, shared-UI, and disposable-stream. Treated as a label those four are easy to satisfy and useless. Treated as a decision procedure they turn out to be answering more than one question at once, and separating those questions is the first real piece of design work here.

A stored datum needs three independent facts attached to it, and only the first is what "lifetime" usually means.

The first is **retention**: when may this row be deleted, and what makes that safe? The second is **survival**: must this row survive a process restart at all? The third is **projectability**: may this row ever reach the model's context, and may it ever leave this machine? Invariant 5's four classes are mostly the first two axes bundled together, and the third axis is missing from them entirely — which matters, because there are at least two things in this system that are durable and must never be projected. Provider refresh tokens are one (invariant 1), and shared UI state is the other (it is explicitly not model context). A classification scheme that cannot express "durable but never visible to the model" will get one of those wrong eventually.

Survival turns out to be the degenerate end of retention rather than a separate axis — "never written at all" is just the shortest retention rule — so two declarations are enough. The proposal is that each table declares a **retention rule** and a **projection rule**, and that invariant 5's four named classes are the common combinations of those two rather than the primitives. This is a refinement of invariant 5, not a contradiction of it, but it is a refinement the user should rule on rather than have assumed — see the questions at the end.

#### The retention rules, and what each one keys off

The notes are more precise than invariant 5 is: "Some durable events are only needed for the lifetime of the session, some only for the lifetime of the current context, others forever. Some not durable at all." That is three durable retention rules, not one, and each needs an anchor in the data that tells a garbage-collection pass when the row's time is up.

**Forever** is the analytics-grade class. Everything a cost question, a cache question or a timesheet question reads lives here, and nothing in this class is ever deleted by the harness. It is small per event and it accumulates without bound, which is fine — it is metadata, not content.

**Session-lifetime** rows are anchored to a session id and become collectable when that session is closed and no longer a resume target. This is where working state lives: which tool calls were proposed but not executed, which scope a parent is blocked on, what the last request's model and settings were.

**Context-lifetime** rows are anchored to a context epoch (see below) and become collectable once a later epoch supersedes them. Compaction is what creates a new epoch, so this is the class for everything that only mattered to the pre-compaction context — expanded tool output that was truncated on handover, the notice-of-change appends that a rebuild made obsolete.

**Cache-supporting-transient** rows are anchored to an observed or assumed expiry time, plus a grace margin, and are collectable after it. This is the awkward class the why calls out, and it is awkward because its *usefulness* has an expiry that the harness only partly knows.

**Disposable-stream** data is never written at all: model token deltas, tool stdout chunks as they arrive, throbber and progress state. The definition that makes this class safe is that the datum is either reconstructible from what *is* stored or expendable by explicit acceptance. Partial streamed text lost to a crash is the expendable case, and the notes already accept it — graceful shutdown waits for in-flight requests to complete, and "crashes should be rare, not designed around".

**Shared-UI** state is the odd one out, and the reason is not its lifetime but its *shape*. Everything above is append-only history. Shared UI state — draft buffers, which files are open, pane layout — is mutable current state that is overwritten in place. Putting mutable rows into an append-only log destroys the log's main property, which is that you can rebuild every derived view from it. So the proposal is that shared UI state is checkpointed, not journalled: a small table keyed by session, overwritten in place, never analysed, never projected to the model, dropped when the session ends, and carrying whatever causal token multi-client needs so a stale face cannot clobber a newer state (invariant 7 owns that mechanism; this schema only has to leave room for the token). Draft text is the one piece of it a user would genuinely be upset to lose across a brain restart, which is the argument for it being persisted at all rather than held only in memory.

#### The rule that makes retention testable

There is one derived rule that does most of the work, and it is worth stating on its own because it constrains the schema everywhere: **deleting anything must not change an analytics answer.**

If a cost query reads a cache-supporting row, then collecting that row silently changes history. So the accounting copy of a fact and the operational copy of it are different rows in different classes. When a response reports that it read 12,000 cached input tokens, that number is written into the forever-class request metadata; the cache handle that made it possible stays in the cache-supporting class and can be collected the moment it expires. The two never share a row.

That gives a black-box test at a public surface, which is what makes this more than a principle: run the named analytics queries, run a full garbage-collection pass, run them again, and every answer must be identical. It also gives the retention machinery a shape — a single collection pass whose deletion predicates are pure functions of durable facts (session closed, epoch superseded, expiry plus grace elapsed, blob unreferenced), rather than a background process making judgements.

### Events, and the tables derived from them

The notes ask for something strict: "the architecture to *strictly* follow an 'events + derived views' model, *even though* this is likely to be quite complicated in practice at the low level, *because* it will make the *high level* guarantees simpler to achieve." And they are equally clear that "for some we store only a projection (for analysis) for others we store the events."

The proposal here is that the event log is the durable source of truth for session content, and the relational tables that carry hierarchy, request and cache metadata are **projections maintained in the same transaction as the append**. Nothing derived is ever computed lazily or refreshed on a timer, so nothing derived can be stale, and dropping every derived table and rebuilding it from the log must produce byte-identical results. That last property is another black-box test, and it is the one that keeps the "strictly events" discipline honest instead of aspirational.

The cost of this is real and should be named: every append writes more than one row, and every projection is code that can be wrong. The alternative — tables as primary, with a log kept only for replication — is cheaper to write and loses the rebuild property. The reason to pay is that this is the schema several other designs express themselves through, and a design where "what the model saw" is derived from the same log as "what it cost" cannot drift between the two.

An event row carries: its globally unique id, the session it belongs to, its **emitter** (face, brain, or limb — invariant 3 says an event is about its emitter, and the walking-skeleton ruling splits tool facts between brain and limb for exactly this reason), the emitter's own sequence number, the emitter's timestamp, a type, and a payload with large parts moved out to blobs.

Two things the event log deliberately does *not* have. It has no single global sequence column, because the sequencing ruling says there is no total order across processes and a column implying otherwise would be a lie the schema tells. Per-emitter monotonic sequence plus enough causal metadata to reconstruct a correct order is the requirement; what exactly that causal metadata is belongs to the event-streaming experiment, and this schema's job is to not foreclose it. Within one process appends are synchronous under a lock and a local order does exist, so recording it as a convenience is fine — as long as nothing reads it as global.

And every timestamp belongs to the clock of the emitter that wrote it, because invariant 10 says no shared clock is assumed across role boundaries. Comparing a limb timestamp with a brain timestamp is therefore approximate. For everything this design needs — tool durations measured by one emitter, cost rollups by day, timesheets to the nearest minutes — that is fine, but it must be an acknowledged property rather than a surprise found later in a query result.

### The session graph

This is the schema's hardest requirement, and the one the notes flag as having been underestimated: "the hierarchy model makes storage semantics central to concurrency, blocking, resume, and compaction. May be larger work than assumed."

#### Scopes, not parent pointers

The obvious model is a `session` table with a nullable `parent_session_id`. It is not enough. A parent blocks, its children complete, it resumes, and later it blocks again — so a plain parent pointer cannot say *which* blocking episode a child belonged to, and the whole point of structured concurrency is that the episode is the unit.

So the schema has a **scope** table. A scope is created when a parent blocks, belongs to exactly one parent session, and is open or closed. Children reference the scope, not the parent directly. Three of the notes' listed demands then fall out rather than needing mechanisms of their own:

*Blocked parent state* is not stored. A parent is blocked exactly when it has an open scope, which is derived — and deriving it is better than storing it, because a stored flag can disagree with the children while a derived one cannot. This is invariant 3 applied to the schema itself.

*Dynamic sibling sets* — "children added to a scope after the parent blocked", which the hierarchy notes say agents may do — are an insert into an open scope. No special case.

*Stuck scopes*, which the analyst stakeholder explicitly wants to query, become a plain SQL question: open scopes whose children have all completed, or open scopes older than some threshold. The known pain the notes describe — "a forgotten `/done` or hung child blocks the parent scope indefinitely" — is intentional behaviour, but it is now visible behaviour.

One thing the scope must *not* do is enforce visibility. Sibling results are hidden from siblings until the parent resumes, but that is a rule about the projection that builds a context, not about storage. Results are stored as soon as they exist. A schema that tried to enforce the rule by withholding the write would be unable to answer "what did that child actually return", which is precisely the analytics the user wants.

#### How a session came to exist

Each session row records its origin, and there are three kinds, matching the hierarchy notes' three launch paths. **Forked** from a parent, at a named position in the parent's event sequence. **Fresh**, from a shared seed context. **Resumed**, from a prior session.

The fork position has to be a reference to a specific point in the parent's history, because the notes are explicitly unsure which point is right: forked subagents are append-mode "possibly w.r.t the parent *as of the message before it sent the subagent tool call* - but that needs experimenting too". The schema does not need to know the answer; it needs to be able to name any position so the experiment can settle it.

Shared seed contexts are first-class rows rather than a property of a session, because the handoff notes describe at most one shared seed context per limb, containing context and attachments, established by an initial API request so that several children share a cache prefix. Several sessions reference one seed, and the cost of that seed is attributable once rather than N times — which is exactly the measurement the fork-beats-fresh claim needs.

Resume is the interesting one. The Resume tool "continues a previous agent session as a new subagent", which means the resumed session joins a *new* scope under whoever resumed it, while pointing at an old session's history. The proposal is that this is a fork from the old session's terminal position: a new session row, new identity, origin `resumed`. Session parentage then never mutates, history stays append-only, a session can be resumed more than once without ambiguity, and "how much did this line of work cost in total" is a graph traversal. The alternative — reopening the old row and re-parenting it — makes the row's history depend on when you look at it, which is the thing analytics-grade storage cannot afford.

#### Session lifecycle, and the user-facing distinction

The notes distinguish user-facing from autonomous sessions by how they complete: a user-facing session completes when the user signals `/done` and the agent then writes a response, while an autonomous one completes at the end of its turn. Both are also subject to their scope. So a session's current state is derived from events — created, running, awaiting user, awaiting siblings, completed with a result, completed with an error — and stored as durable facts rather than as a mutable status column. The exact state set is proposed here, not settled: the notes describe the transitions but never enumerate the states, and `user_facing` is deliberately kept as a parameter orthogonal to agent type, which the schema should preserve rather than collapse.

Agent naming is worth one line because it affects a key: names are auto-generated, get longer with nesting, and the user can edit them. So the name is a mutable attribute, never an identifier.

### Context epochs, compaction continuity, and cache metadata

A compaction is not a new session — the notes are firm that it is a handover *within* a continuing piece of work — but it is a discontinuity in what the model can see. The schema represents that as a **context epoch**: a numbered generation within a session, created by a handover, recording which handover tool call produced it and which attachments were loaded immediately into the fresh context. Session identity persists across epochs; the model-visible projection is scoped to the current epoch plus its seed; the durable record keeps every epoch forever.

This is also what gives context-lifetime retention its anchor, which is a pleasing amount of reuse from one table: "durable for the lifetime of the current context" simply means keyed to an epoch that a later epoch has superseded.

Cache metadata hangs off the same structure, and the notes are explicit that it must be durable: "Backend server cache ids etc should not be ephemeral - they should be tracked in the DB by session, so that we can seamlessly continue on relaunch." Per session and epoch, the harness records the provider, the model, the opaque provider-side handle or prefix identity, the position in the context that the cached prefix corresponds to, when it was last observed to hit, and the expiry it is assumed or observed to have. This is the store that cache-state prediction reads to decide append versus rebuild, so it is shared machinery rather than local bookkeeping.

What exactly a provider gives back to identify a cached prefix differs by provider and is not something to invent here. The schema's requirement is that the handle is opaque, per-provider, and durable; establishing its actual shape is provider-survey work.

### Requests, responses, and the economics

One row per **request attempt**, not per logical request — retries cost money separately, so they are separate rows. Each carries the session and epoch, what triggered it (and the trigger vocabulary is fixed by invariant 2: turn end, tool-loop continuation, cache-nearly-expired handover, explicit resume, and nothing else), whether it was built in append or rebuild mode, the model and provider, the request facts that are not context facts (`model` and `reasoning_effort`, per the walking-skeleton ruling), timestamps for sent / first byte / last byte, the four-valued outcome from invariant 9, and the reported usage broken out far enough to be useful: input tokens, output tokens, cache writes, cache reads, and reasoning tokens where a provider reports them.

Two things about this table matter more than its columns.

First, it is the table that makes the design's economic claims checkable rather than believed, which is why #4 exists. Compaction-earlier-beats-later, forking-beats-fresh, and cache-reads-are-roughly-ten-times-cheaper are all queries over this table joined to the session graph.

Second, **cost must be allowed to be unknown**. A cancelled stream may never deliver its usage metadata, so the honest value for a cancelled request's usage is sometimes null-with-a-reason rather than zero. A zero would make cost queries silently wrong in exactly the case the cancellation-economics question is about. That means every cost query also has a coverage figure — what fraction of requests in this window reported usage at all — and that figure is part of the answer, not a footnote.

Tool executions get their own table with the same discipline about emitters. The brain records that a call was detected in a response and that a result entered the model's view; the limb records the execution itself, its duration, its exit status, and environment facts like hostname. Tool durations, which the analyst stakeholder asks for, are a limb-measured quantity, so they are measured on one clock and are meaningful. A proposed-but-unexecuted call is a first-class state here: it has a detection fact from the brain, no execution fact from the limb, and no fabricated outcome anywhere — which is exactly the accepted walking-skeleton ruling, expressed in the schema.

### Blobs

The notes want message contents, large text and images kept out of the event tables. Two further pressures shape the choice: fork-by-default means the same file content appears in many sessions, and backup-by-default replication means whatever holds blobs gets copied between machines.

Both point the same way: a **content-addressed blob table in the same database**, keyed by content hash, with small text inlined in the event payload below some threshold. Content addressing gives deduplication, which matters a lot when a dozen forked children all carry the same attachment. Keeping it in the same database keeps backup and sync trivially correct — one file to copy, one transaction boundary. The cost is database size and vacuum behaviour, and the escape hatch if that hurts is a separate attached database file rather than loose files on disk, since loose files reintroduce the two-things-to-keep-consistent problem that the single-database choice exists to avoid.

Content addressing does complicate retention: a blob is collectable only when nothing references it, so blob collection is by reachability rather than by an expiry anchor. That is the one place the collection pass needs to do real work.

### Identity, provenance, and the federated copy

The plan carries a requirement from `federated-brain.md` that has not previously reached this design: rather than each brain holding only its own data, the user likes them all storing all of it — "that way I get backups by default. Sync all the data in the background. keep it clear where it came from, don't accidentally duplicate it or get it confused with local data."

Two schema demands follow, and they are cheap to meet now and expensive to retrofit.

Identity must be **globally unique by construction**, not a per-database autoincrement. A sortable random id (UUIDv7 or ULID) gives uniqueness without coordination and keeps insert locality, so background sync cannot produce a collision and cannot need a translation table.

And every durable row carries **provenance**: which brain it originated on. Uniqueness alone would stop duplication but not confusion; "keep it clear where it came from" is a query requirement. Because a local database will contain rows it did not originate, every query has to be explicit about origin scope — and the default for most of them is probably *all* origins, since the point of federation is a merged view.

There is a genuinely nice consequence here worth stating plainly: if every brain stores everything, then "ideally we can query across all connected brains" needs no distributed query engine at all. It is a local SQL query over replicated rows, as fresh as the last sync. That is the cheapest possible satisfaction of that requirement, and it is a strong argument for backup-by-default over per-brain ownership.

Two cautions. Sync needs a per-origin watermark so a peer can resume catching up, which is a durable table here even though the sync mechanism itself is not designed here. And **not everything durable should replicate** — provider refresh tokens are durable state that must never leave the machine, which is an argument for credentials living outside this database entirely rather than being a class within it. That is raised in the questions and in the OAuth design.

### Resume: what the brain reads when it comes back

The why claims restart must continue the work without asking the user. That claim is only meaningful if the set of facts a relaunch needs is enumerable, so here it is, as a contract shared with operator lifecycle: on relaunch the brain finds every session that was not completed; for each, the current context epoch and its cache metadata; every open scope and its children's states; every proposed-but-unexecuted tool call; the last request attempt and its outcome, including whether an interrupted one was recorded as cancelled rather than left dangling; and the session's limb binding, so the limb can be reconnected or restarted. Because a limb is identified by `ssh_host` and `directory` and those are brain-stored, nothing about resume depends on a limb having survived.

Interrupted tool calls need no special representation: the notes already rule that the tool reports something like "tool call interrupted by harness crash" and the agent reasons about state and safety itself. The schema's obligation is to record honestly that execution began and no outcome arrived — which is a different state from "proposed and never started", and the two must not be collapsed, because one is safe to re-run and the other is not.

The one-hour rule stays as the notes leave it — a preference with the user's own hedging intact ("Actually - I don't think that's so clear. Probably this should be optional too") — and the schema's only obligation to it is to record when the brain shut down, so any policy can be computed later rather than baked in now.

### Analytics as a public surface, and why that protects the schema

Per `PROCESS.md`, "the durable storage and query surface: analytics queries are product behavior, not internals". The user is content with plain SQL and no query API is designed. So the deliverable is a set of **named queries checked into the repository** and exercised by black-box tests: cost by session, by day, by model and provider; cache hit rate and cache read share; tool durations; stuck scopes; session classification; and a timesheet-shaped rollup.

This has a consequence that is the neatest answer available to the why's "schema mistakes are the expensive kind": the *queries* are the compatibility surface, not the table shapes. A future migration is free to restructure tables as long as the named queries keep returning the same answers, and the tests say whether they do. That is what turns an expensive class of mistake into an ordinary one.

The same query set is also what the meta limb exposes. The notes want "a read-only tool surface or limb for meta-work", and the limb model makes cross-session search an ordinary limb providing ordinary tools rather than a special feature. So there is one query set with two front doors — SQL for the user, tools for agents — and the limb's tools are thin wrappers rather than a second design. The read-only limb must not project credential rows or shared-UI rows, which is the projection axis from the classification doing real work.

Timesheets need two notions the notes never define. **Project** is proposed as a table keyed by host and root directory, which a session references via its limb binding — the same pair a limb is already identified by. **Working session**, the human notion of a contiguous stretch of the user's own activity, is proposed as derived rather than stored: a gap threshold over event timestamps. Storing it would require the harness to guess when the user stopped working; deriving it lets the user change the threshold afterwards and re-run the query, which is what he will actually want the first time the answer looks wrong.

Indices follow from those access paths rather than being listed for their own sake: session and session-plus-time for reading a conversation, scope-open-ness for the stuck-scope query, origin for the federated split, model-and-provider-plus-time for cost rollups, content hash for blobs, and the emitter-plus-sequence pair for log reads.

### One session's life through the schema

The parts above are only a design if they work as one thing, so here is a single piece of work passing through all of them.

The user starts a session in a repo. A project row exists or is created for that host and root; a session row is created with origin `fresh`, bound to that limb, at context epoch 1. He opens a couple of files and runs a search; those are events with the face and limb as emitters, appended, and — per invariant 2 — triggering nothing. He ends his turn. That triggers one request: a request-attempt row with trigger `turn end`, mode `rebuild` because the context is new, the model and reasoning effort recorded as request facts, and a cache metadata row written when the provider reports a cached prefix. The response streams; the deltas are disposable-stream and never stored; the assembled response is one event, with its large parts in content-addressed blobs. It proposes two tool calls: the brain records the detections, the limb records the executions and their durations, and the usage figures land in the forever class.

The agent decides to parallelise. It calls Task twice. A scope opens under this session, and two child sessions are created with origin `forked` at a named position in the parent's history. The parent is now blocked, and nothing stores that — it is blocked because its scope is open. One child launches a sibling into the same scope; that is an insert. One child is user-facing, so it sits in `awaiting user` until `/done`. Each child's result is the last message part of its final turn, referenced rather than copied. When the last child completes, the scope closes and the parent's next request is triggered by tool-loop continuation.

Later the cache is close to expiring. The agent calls handover. Epoch 2 opens, recording the handover call and the attachments loaded straight into the fresh context. Everything keyed to epoch 1 is now context-lifetime garbage, collectable, and none of it is anything a cost query reads.

Then the machine needs to restart. Graceful shutdown waits for the in-flight request, records the shutdown time, and leaves two proposed tool calls unexecuted with no fabricated outcomes. On relaunch the brain reads exactly the resume contract above, reconnects the limb, runs the pending calls, and carries on. Nobody was asked anything.

A month later the user wants a timesheet. The query reads only forever-class rows — request attempts, tool executions, session and project rows, event timestamps — and derives working sessions by gap threshold. A garbage collection pass ran last week and deleted a great deal of epoch-1 and cache-supporting data; the answer is identical either way, and there is a test that says so. His laptop has meanwhile synced everything from his desktop, so the same query over all origins covers both machines, while an origin filter answers "what did I do on the desktop". No row was duplicated, because ids were globally unique before the second brain existed.

### Thesis, falsification, and invariants

The thesis: **a single SQLite schema, organised as an append-only per-emitter event log with in-transaction derived projections, can represent the hierarchy model's full state — scopes, dynamic siblings, derived blocked-ness, fork positions, epochs, cache handles and unexecuted calls — while every stored datum declares a retention rule and a projection rule, such that restart resumes work without asking the user, garbage collection cannot change an analytics answer, and cross-brain queries are ordinary local SQL over replicated rows.**

It is falsified if: restart needs a fact the schema cannot express, or needs to ask the user something the notes say it should not; blocked-parent state cannot be derived and needs a stored flag to stay correct; derived tables cannot be rebuilt from the log identically, or the in-transaction write cost is unacceptable; any named analytics answer changes across a garbage-collection pass; representing an unknown cancelled cost makes cost queries misleading rather than merely incomplete; or the four-class classification cannot be applied to some real datum without stretching it, which would mean the classification is wrong rather than the datum unusual.

Invariants touched: 5 primarily and directly (this is where it is satisfied or not); 2, because trigger classification is a recorded fact here; 3, because derived-versus-stored is the schema's central technique; 9, because the four-valued outcome and the kept-completed-work rule are storage requirements; 10, because timestamps and sequences are per-emitter with no assumed shared clock; and 1, because credential storage is deliberately kept out of this database rather than classified inside it.

## Parked for later stages

**Interactions flagged for stage 3:** self-modification (durable resumable state is what makes relaunch-onto-new-code possible; plugins may live in the DB so an old schema stays addressable while a cache is valid); compaction-handover and context-updates (both need stored cache metadata for cache-state prediction, and both create rebuild boundaries that are durable facts); forked-subagents (the hierarchy state above *is* this schema's hardest requirement); topology (durable ordered events, cross-brain queries, provenance); multi-client-ui (shared-UI state is its own lifecycle class — live but not durable, and explicitly not model context); cancellation-economics (this schema records its answer); operator-lifecycle (migrations are the risky part of any update).

## Questions for review

- Is the "one hour" resume rule worth designing now, or is it a preference to leave until the harness is actually being restarted often?
- Should shared-UI state live in this database at all, or in a separate store with different durability? Invariant 5 names it as a class; it does not say it shares a home.
- Timesheet use (#3) implies a notion of project and of working session that may not map cleanly onto agent sessions and limbs. Worth designing explicitly, or leave it to SQL over what happens to be recorded?
- **Invariant 5's four classes are treated above as common combinations of two independent axes** — a retention rule and a projection rule — rather than as primitives, because there are durable things that must never be projected (refresh tokens, shared UI state) and the four names cannot say that. Does that refinement satisfy invariant 5, or do you want the invariant reworded?
- Relatedly, "durable" is split above into forever / session-lifetime / context-lifetime, following the notes' own three lifetimes. Is that a refinement of one class or should they be named classes in their own right?
- **Credentials are proposed to live outside this database entirely.** Backup-by-default replication plus durable credential rows would ship your Claude refresh token to every brain you own. Keeping them out keeps the replication rule simple and true ("everything durable syncs"), at the cost of a second store to manage. Ruling wanted, since it also lands in the OAuth design.
- Events-primary with derived tables written in the same transaction is a real cost: every append writes several rows, and every projection is code that can be wrong. The payoff is that nothing derived can be stale and everything derived can be rebuilt. Worth it, or should the relational tables be primary with the log kept only for replication?
- **Resume is proposed as a fork from the old session's terminal position — a new session row, not a reopened one.** This keeps parentage immutable and makes repeated resumes unambiguous, but it means one piece of work is several session rows and every "total cost" question is a graph traversal.
- The event log deliberately has **no global sequence column**, because the sequencing ruling says there is no total order across processes. Confirm that a schema with only per-emitter sequences plus (later) causal metadata is what you want, rather than a convenient global counter that would be a lie once a second process exists.
- **A contradiction with why #4 to record rather than fix**, per the README: #4 promises "actual token usage across all API requests". If a cancelled stream never delivers usage metadata — which is precisely what cancellation-economics suspects — then usage is not available for all requests, and the schema has to hold "cost unknown, because the stream was cancelled" honestly. The design above does that and reports coverage alongside every cost answer, but the why is currently more confident than the mechanism can be.
- **Shared-UI state is described elsewhere as "live but not durable", and the design above partly disagrees**: draft text is the one piece of it a user would be annoyed to lose across a brain restart, so it is proposed as *checkpointed* — a mutable row overwritten in place, outside the append-only log, never analysed, never projected. That makes it durable-in-storage but not history. Is that the right split, or should a half-typed message simply be lost on restart?
- Is "no deletion may change an analytics answer" acceptable as a hard rule? It is what makes retention testable, but it forbids ever deriving a cost figure from a cache-supporting row, which means some facts are deliberately stored twice.
