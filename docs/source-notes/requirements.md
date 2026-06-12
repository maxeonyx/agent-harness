# Agent Harness Requirements and Spike Plan

Status: provisional requirements document.
Purpose: define stakeholder-level requirements, the experimental spikes needed to validate them, and the high-level expectations for eventual core integration.

This document is not an implementation plan yet. It is the behavioural target and validation plan for the implementation process.

Note that “stakeholder” at this stage means “me in a different capacity / mode of interacting with the system.”

---

## 1. Process principles

The harness should not be implemented by building a narrow MVP and hoping it generalises. The design has several perpendicular requirements: worker UX, process improvement, harness development, operations, analytics, security boundaries, coordination, and multi-client UI state. A successful implementation must preserve the breadth of those requirements.

The process itself lives in `process.md`. In short:

1. Disposable spikes in `experiments/` produce evidence; they start with a one-paragraph brief and end with runnable evidence plus an outcome doc. No mid-spike gates, no tests-first requirement.
2. Two gates carry the rigor: spike acceptance and core integration acceptance. Both check against the invariants below; both involve the user.
3. Core integration is fresh design from evidence, never copied spike code, with black-box tests written first at the product-public surfaces.
4. Integration happens small and continuously, alternating with spikes, not batched into a final core phase.

Each spike has two required outcomes:

1. **User acceptance**
   The user reviews the spike behaviour and decides whether it actually proves what it was supposed to prove.

2. **Spike outcome document**
   A short document records:

   * what the spike proved
   * what it failed to prove
   * what aspects should be integrated into core
   * what aspects should explicitly *not* be integrated
   * what requirements pressure appeared

The gates review the spike against the invariants below, not the full requirements set every time. The full stakeholder sweep happens when this document itself is revised - which a spike outcome can trigger.

---

## 1.1 Invariants

The non-negotiables every gate checks against. A change that violates one of these stops and goes to the user.

1. The brain is the only role that drives provider API requests. Provider credentials never reach limbs, faces, plugins, tool schemas, logs, or model context.
2. Recording context, appending context, rebuilding context, and triggering inference are distinct operations. Passive user activity never triggers a model request.
3. User-tool activity is framed as user activity, never as agent tool calls. Each user tool keeps two surfaces: rich interactive UI for the user, compressed context for the model.
4. Face, brain, and limb are logical roles. Co-location versus splitting is a deployment choice over the same logical model.
5. Durable session data is analytics-grade and queryable. Durable, cache-supporting-transient, shared-UI, and disposable-stream data are explicitly distinguished.
6. Subagent concurrency is structured: parents block on children; sibling results stay hidden until the parent resumes.
7. Multi-client UI state is explicitly modeled. Stale clients cannot silently overwrite newer state; the user wins on conflicting edits.
8. Spike code never becomes core by copying. Core integration is a fresh design from evidence.

---

# 2. Stakeholder-level requirements

## 2.1 Worker stakeholder

The worker wants to get work done with minimum coordination overhead.

### Requirement: user work happens in-band

The user can open files, search, run commands, inspect output, edit files, query subagents, and use richer tools without leaving the shared work context. The agent should later understand what happened without requiring the user to manually restate all out-of-band activity.

Test cases:

* User opens a file; the session records which file and which relevant section was viewed.
* User edits a file; the agent later sees a useful diff or summary of the change.
* User runs a command; the agent later sees command, cwd, exit status, and relevant output.
* User searches; the agent sees query, result summary, and selected/opened results.
* User asks a subagent a question; the main session receives the user’s prompt and the subagent response.
* User performs several actions before ending a turn; those actions are included in the next model request.
* User performs tool activity while a model request is in flight; that activity is recorded but does not itself trigger a model request.

### Requirement: user-tool context is compressed by the tool

Each user tool has two surfaces:

1. the real interactive UI shown to the user
2. the compressed context appended to the session

The user-facing UI may contain rich, verbose, visual, or transient state. The model-facing context should contain only what is useful for the agent.

Test cases:

* Large terminal output is summarized or truncated, preserving errors and relevant head/tail sections.
* File browsing records opened/expanded regions, not every visual state.
* Search records the query, relevant results, and opened results, not every pagination detail.
* Repeated cursor movement or UI navigation does not spam context.
* The compressed context is sufficient for the agent to understand what the user likely relied on.
* The context does not imply that “viewed” means “endorsed” or “selected.”

### Requirement: user wins on conflicting edits

The user may act while the model or a tool call is in flight. The harness should prevent stale agent/tool output from silently overwriting newer user work.

Test cases:

* Agent reads file A; user edits file A; agent later attempts stale patch; stale patch is rejected or requires explicit reconciliation.
* User and agent edit disjoint regions; merge or review is possible.
* User and agent edit the same region; user version wins by default.
* The conflict is visible in the session, not hidden in logs.
* Tool output arriving after a user edit is marked as based on an older view of the workspace.

---

## 2.2 Process-improver stakeholder

The process improver wants to improve the ability of both user and agents to work well: prompts, skills, agent definitions, AGENTS.md files, tool descriptions, and process documents.

### Requirement: process/context edits are first-class

Changes to process context should be tracked and should affect future sessions or rebuild boundaries without pretending that the current cached context changed magically.

Test cases:

* User edits AGENTS.md; the edit is recorded in the current session.
* Current warm-cache session is not silently assumed to have the new AGENTS.md in context.
* New session or rebuild uses updated AGENTS.md.
* Agent is told when relevant context changed after its current context was built.
* User edits a skill/tool/agent definition; the change is recorded and can affect future context according to cache/rebuild rules.

### Requirement: all tools/configs can be iterated rapidly

Handover is one configurable tool/config surface among many, not a unique special case. All tools, prompts, schemas, and tool descriptions should support rapid iteration.

Test cases:

* Tool description/schema changes can be detected.
* Warm append-mode session can receive a concise change notification or diff.
* Rebuild-mode session receives the current canonical tool description/schema, not necessarily the historical diff.
* Existing sessions can continue using the tool schema they were given, where needed.
* Reloading one tool does not require restarting the whole harness unless explicitly necessary.
* Broken reload does not brick running sessions.

### Requirement: context lifecycle is explicit

Recording context, appending context, rebuilding context, and triggering inference are different operations.

Only a few things should trigger model API requests:

* agent tool-call loop continuation
* user turn end
* cache-nearly-expired proactive handover/compaction
* explicit resume/continue actions, if supported

Most other context additions should piggyback on the next request.

Test cases:

* User opens a file; no model request is triggered.
* User edits a file; no model request is triggered.
* User terminal output arrives; no model request is triggered.
* User ends a turn; accumulated context is included in the model request.
* Agent receives a tool result; pending piggyback context is included in the next tool-loop request.
* Cache-nearly-expired handover can trigger a model request.
* Tool schema changes during append mode are included as a delta/diff if needed.
* Tool schema changes during rebuild mode are represented by the current canonical schema.
* Rebuild mode does not blindly replay obsolete append-only notices.

---

## 2.3 Harness-developer stakeholder

The harness developer wants rapid iteration, safe self-modification, good testability, and no regressions across perpendicular directions.

### Requirement: spikes are disposable experiments

Experimental spikes should validate behaviour without becoming accidental architecture.

Test cases/checks:

* Spike code lives outside the eventual core implementation path.
* Core cannot accidentally depend on spike internals.
* Each spike produces a short evidence report.
* Before core integration, behaviours proven by the spike are represented as black-box tests.
* Integration uses the spike’s lessons, not necessarily its code.
* Review asks: would we choose this design if starting fresh?
* Review asks: what parts of this spike should explicitly *not* be carried forward?

### Requirement: tool/plugin reload is safe and iterative

The harness should support editing tool implementations, descriptions, and schemas quickly. Existing model context and cache constraints must be respected.

Test cases:

* Session starts with tool schema v1.
* Tool reloads to schema v2.
* Existing warm session either continues with v1 or receives explicit change notification/diff.
* New/rebuilt session receives v2 directly.
* Failed reload is quarantined or rolled back.
* Agent can be informed of tool description/schema changes without forcing a full rebuild when append mode is desired.
* Explicit rebuild/cache-break path exists when needed.

### Requirement: eventual self-modification is safe

The harness should eventually support agents editing plugins and the harness itself, rebuilding, reloading, and resuming work.

Test cases:

* Brain restart preserves sessions.
* In-flight model request is resumable or clearly marked interrupted.
* In-flight tool call interruption is represented explicitly.
* Reload/relaunch preserves session metadata and cache-related metadata.
* Failed update can roll back.
* Tool/plugin validation runs before activation.

---

## 2.4 Operator stakeholder

The operator wants to deploy, connect, update, downgrade, and manage harness installations reliably.

### Requirement: face, brain, and limb are logical roles

Roles may be co-located or split. The architecture must work in all valid configurations.

Required configurations:

* `face+brain+limb`
* `face+limb <-> brain`
* `face <-> brain <-> limb`
* `face <-> brain <-> brain <-> limb`
* `face <-> brain+limb <-> face 2`
* optional triangle: `face <-> brain`, `brain <-> limb`, `face <-> limb`

Test cases:

* Same basic scenario runs in each required topology.
* Monolith mode still respects logical role boundaries.
* Brain remains the only role that drives model API requests.
* Limb has no provider credentials.
* Face disconnect does not kill the limb by default.
* User can “disconnect” while the face/brain/limb continue persistently in the background, on both Windows and Linux.
* Ideally the same process can stay open during user disconnect so ongoing requests are not interrupted.
* Brain/limb disconnect triggers reconnect or timeout lifecycle.
* Face reconnect catches up from brain/session state.
* Multiple faces observing the same session eventually see coherent state.
* Proxied brain path preserves identity, routing, and security boundaries well enough to be usable.

### Requirement: optional direct face-limb streaming is an optimization

The brain may act as signalling/NAT-punching/rendezvous server for direct face-limb streams. This is useful for large ephemeral UI streams such as live logs, terminal output, or LSP diagnostics.

The brain remains authoritative for session state, security boundaries, durable records, model context, and API request triggering.

Test cases:

* Brain authorizes direct face-limb stream.
* Brain issues short-lived stream capability.
* Direct stream succeeds; face sees live high-volume output.
* Brain still receives durable compressed output.
* Direct stream fails; system falls back to brain-proxied streaming.
* Brain revokes capability; direct stream stops.
* Brain disconnect does not leave an unauthorized independent face-limb session.

### Requirement: updates, protocol versions, and migrations are safe

Test cases:

* Old face connecting to new brain is compatible or rejected clearly.
* New face connecting to old brain is compatible or rejected clearly.
* Old limb connecting to new brain is compatible or rejected clearly.
* DB migration is idempotent.
* Failed migration leaves recoverable state.
* Auto-update can stage, activate, verify, and downgrade.
* Smooth relaunch can be triggered locally or remotely.

---

## 2.5 Analyst stakeholder

The analyst wants queryable usage data: cost, cache hit rate, session classification, time tracking, tool use, blocked scopes, and failure patterns.

### Requirement: session data is analytics-grade from the start

Test cases:

* Model responses record model, provider, token counts, cost estimate, cache status, and timing.
* Sessions are linked to project, limb, agent type, parent/child relation, and user-facing/autonomous status.
* Tool calls record tool type, duration, result status, output size, and relevant context size.
* User-tool activity records tool type, emitted context size, and enough timing/attention data to support later analysis.
* Query: total cost by project/session/date range.
* Query: cache hit/miss rate by model/session.
* Query: tool duration/failure rate.
* Query: sessions needing timesheet classification.
* Query: blocked/stuck scopes.
* Large blobs are stored separately from hot indexed tables.

### Requirement: data lifecycle is explicit

Some data is durable indefinitely. Some data is durable only to preserve cache/session continuity and should be cleaned up after cache expiry or after rebuild.

Test cases:

* Durable session records survive restart.
* Cache-supporting transient data survives restart while cache is expected to remain useful.
* Cache-supporting transient data is cleaned after expiry.
* Rebuild mode does not depend on expired append-only data.
* Cleanup does not delete data needed for audit, analytics, or session history.
* Large temporary streams can be pruned without damaging canonical session context.

---

## 2.6 Security / authority-boundary stakeholder

The security stakeholder wants clear authority boundaries and no accidental secret exposure. This is not a general agent permission model.

The default personal-use limbs may run in YOLO mode. If a limb wants stricter read/write/tool permissions, that is a limb implementation concern. A tool call failing for permission reasons is acceptable. The harness should not build elaborate permission prompts or agent-blocking approval flows by default.

### Requirement: provider credentials stay brain-owned

Test cases:

* Remote limb cannot read provider API credentials.
* Provider plugin receives a limited capability or pre-authenticated wrapper where possible, not raw provider tokens.
* Provider credentials are not serialized into tool schemas, tool descriptions, model context, logs, user-tool context, or plugin reload diffs.
* Brain remains the only role that can make provider API requests directly.
* Tool/plugin reload cannot accidentally expose secret-bearing state.

### Requirement: user tools and agent tools are framed differently

User-tool activity is useful context, not an agent tool call.

Test cases:

* User terminal output is framed as user activity.
* Agent prompt/tool list does not include user-only tools.
* Agent cannot call user file browser merely because the user used it.
* Agent can refer to user-observed evidence.
* Agent tool UIs may exist for the user, but that does not make them user tools.
* User-tool and agent-tool context are distinguishable in the transcript.

### Requirement: direct connections are capability-bound

Optional direct face-limb paths must not become independent authority paths.

Test cases:

* Direct face-limb stream requires a brain-issued capability.
* Capability is short-lived or renewable through the brain.
* Capability expiry ends the stream or requires renewal.
* Revoked capability stops the stream.
* Direct streaming does not grant model/provider access to the limb or face.

---

## 2.7 Attention / coordination stakeholder

The attention stakeholder wants parallel work to remain legible.

### Requirement: structured subagent concurrency

Parent agents block when launching children. Siblings may run concurrently. User-facing children can continue the conversation until `/done`.

Test cases:

* Parent launches children and becomes suspended.
* Parent resumes only after all children complete.
* Child failure returns an error result to the parent.
* User-facing child requires `/done`.
* User can see active user-facing sessions.
* Agent can see sibling status but not sibling results.
* Parent receives all child results when scope completes.
* Stuck child is visible as blocked state.

### Requirement: sibling workspace scope is explicit

This is primarily a prompting/process issue, but the harness should preserve enough structure to make scopes legible.

Test cases:

* Sibling agents are launched with stated work scopes.
* Parent receives each sibling’s stated result/scope.
* Sibling conflict or overlap can be surfaced if detected.
* Highly forked shared-workspace workflows remain experimental until validated.

---

## 2.8 Multi-client / UI-state stakeholder

The multi-client stakeholder wants more than append-only session sync. Multiple faces may share live UI state: open files, in-progress edits, draft message buffers, selected tool panes, and reactive UI state.

Before this design area is validated, early spikes can use a simple append-only CLI. After this design area is validated, the harness should have a real reactive TUI and a real web GUI sharing the same underlying client state model.

### Requirement: clients can be stale without corrupting state

Test cases:

* Client A observes newer state than client B.
* Client B sends a message from stale state; causal relationship is recorded.
* Stale client does not overwrite newer draft/tool state silently.
* Reconnected client catches up without duplicate events.
* Two clients eventually see consistent session state.

### Requirement: live client UI state can be shared face-to-face

Client UI state should be explicitly modeled, not treated as accidental local widget state.

Examples:

* file tool open state
* selected file
* file edit buffer
* collapsed/expanded file sections
* draft message buffer
* selected session/tool pane
* terminal/log view state
* transient selection/cursor state where useful

Test cases:

* Face A opens a file; Face B can observe compatible open-state if sharing is enabled.
* Face A edits a draft message buffer; Face B receives the update without corrupting local state.
* Two faces edit the same draft buffer; CRDT or equivalent conflict-free state model converges.
* Face disconnects and reconnects; UI state catches up.
* Face-local state remains local when explicitly not shared.
* Shared UI state does not automatically become model-facing context unless the relevant user-tool contract emits context.

### Requirement: reactive TUI and web GUI share the same state model

Test cases:

* TUI and web GUI can attach to the same session.
* TUI and web GUI see the same session events.
* TUI and web GUI can share selected UI state through the same state model.
* Both can display streaming tool/model output.
* Both can recover after reconnect.
* UI-specific rendering differences do not change durable session semantics.

---

# 3. Experimental spikes

## Spike 0: walking skeleton

Purpose: a toy face+brain+limb loop running end-to-end against a fake provider, as the shared substrate every later spike needs.

Should include:

* basic session loop, single process, append-only CLI
* fake model provider behind an adapter boundary
* minimal face, brain, and limb abstractions
* user-tool context append path
* agent-tool call path
* simple persistence or pluggable recorder

Test primitives (fake provider, fake workspace, scenario runner, deterministic clock, context/request assertions) are extracted from this and the next spikes as real pressure appears - they are not designed up front.

Important constraint:

This code is not core. It is the shared experimental scaffold for spikes.

Exit condition:

A scripted toy scenario runs end-to-end: user activity appends context without triggering a request, a turn end triggers a request to the fake provider with the accumulated context, and an agent tool call round-trips.

Required spike outcomes:

* user acceptance
* spike outcome document listing what to integrate and what not to integrate

---

## Spike 1: user-tool context contract

Purpose: validate the central in-band collaboration thesis.

Requirements covered:

* worker in-band activity
* user-tool compression
* user-tool framing
* user conflict basics

Features:

* file user tool
* terminal or search-like user tool
* compressed context projection
* user activity included in the next relevant model request
* no automatic model request from passive context append

Key tests:

* file open/edit context
* command/search context
* large output compression
* user activity does not trigger API request
* user turn end includes accumulated context
* agent tool-loop request includes piggyback context
* user-tool output framed as user activity, not agent tool call
* stale edit/user-wins smoke test

Exit condition:

The user-tool contract is expressive and disciplined enough to build around.

Required spike outcomes:

* user acceptance
* spike outcome document listing what to integrate and what not to integrate

---

## Spike 2: actor topology / transport / lifecycle

Purpose: validate face/brain/limb as logical roles across co-located, split, proxied, and multi-face configurations.

Requirements covered:

* operator topology
* brain/limb separation
* face disconnect lifecycle
* optional direct face-limb stream
* security boundary basics

Required configurations:

* `face+brain+limb`
* `face+limb <-> brain`
* `face <-> brain <-> limb`
* `face <-> brain <-> brain <-> limb`
* `face <-> brain+limb <-> face 2`
* optional triangle direct stream

Key tests:

* same scenario works in each topology
* monolith still respects logical boundaries
* limb has no provider credentials
* brain is only API request driver
* face disconnect leaves limb/brain running where topology permits
* user can “disconnect” and the face/brain/limb continue persistently in the background, on both Windows and Linux
* ideally the same process stays open during user disconnect so ongoing requests are not interrupted
* brain/limb disconnect triggers reconnect/timeout
* face reconnect catches up
* multiple faces see coherent state
* direct stream succeeds/falls back
* brain can act as signalling/capability server for direct stream

Exit condition:

Splitting and co-location are deployment choices over the same logical model.

Required spike outcomes:

* user acceptance
* spike outcome document listing what to integrate and what not to integrate

---

## Spike 3: persistence, resume, and analytics-grade storage

Purpose: validate storage before it calcifies.

This spike is about tracking durable and transient state nicely enough to support restart, resume, analytics, and later cache-aware behaviour. It should not own the full context lifecycle model; that belongs to the live editing/tool reload spike.

Requirements covered:

* persistence/resume
* analytics-grade schema
* data lifecycle/cleanup
* cache-supporting durability
* in-flight state representation

Features:

* SQLite-backed sessions/events
* message/tool/user-tool records
* model response metadata
* cache metadata
* transient cache-supporting data
* cleanup policy
* basic analytics queries
* in-flight model/tool state representation

Key tests:

* restart resumes sessions
* session list and hierarchy survive restart
* in-flight model request state is represented
* in-flight tool call state is represented
* cache-supporting transient data survives restart while useful
* transient cache-supporting data is cleaned after expiry
* cleanup does not delete durable session/analytics data
* cost/cache/tool queries work
* blocked/stuck scope queries work
* large blobs separate from hot tables
* schema supports later append/rebuild/request-triggering state without forcing a rewrite

Exit condition:

The storage model supports worker, operator, analyst, and future context-lifecycle requirements without obvious rewrite.

Required spike outcomes:

* user acceptance
* spike outcome document listing what to integrate and what not to integrate

---

## Spike 4: structured subagents

Purpose: validate hierarchy, blocking, fork/fresh, and attention semantics.

Requirements covered:

* structured concurrency
* user-facing children
* autonomous siblings
* parent blocking
* result propagation
* attention/blocked-state visibility

Features:

* Task tool
* Resume tool if needed
* fork vs fresh
* parent suspension
* sibling status
* user-facing child
* `/done`
* result/error propagation

Key tests:

* parent suspends while children run
* parent resumes only when all children complete
* sibling status visible
* sibling results hidden until parent resumes
* user-facing child completes on `/done`
* failed child returns error result
* abandoned/stuck child visible
* fresh session required across limb boundary
* fork default within same limb

Exit condition:

Structured concurrency is usable and understandable, not just formally clean.

Required spike outcomes:

* user acceptance
* spike outcome document listing what to integrate and what not to integrate

---

## Spike 5: live editing, tool reload, schema stability, and context lifecycle

Purpose: validate rapid iteration for tools, plugins, descriptions, schemas, prompts, and process context, including the append/rebuild/request-triggering model.

Requirements covered:

* harness-developer rapid iteration
* process-improver tool/config iteration
* context lifecycle
* append mode vs rebuild mode
* schema stability
* reload failure safety
* live editing of process/tool context

Features:

* plugin/tool v1/v2
* tool description/schema diff
* in-place reload
* existing session behaviour
* new/rebuilt session behaviour
* failed reload quarantine/rollback
* explicit append/rebuild mode
* request-triggering rules
* process/context file edits, such as AGENTS.md or tool definitions

Key tests:

* existing warm session keeps v1 or receives explicit diff/notice
* new session receives v2
* rebuild-mode context receives canonical v2
* failed reload does not brick existing sessions
* tool implementation can reload without full harness restart
* agent can be shown schema/description diff
* explicit cache break/rebuild path works
* user opens/edits files without triggering a model request
* user terminal output arrives without triggering a model request
* user turn end triggers a model request with accumulated context
* agent tool-loop continuation triggers a model request with piggyback context
* cache-nearly-expired handover/compaction can trigger a model request
* append mode includes relevant deltas
* rebuild mode uses canonical current state
* rebuild mode does not blindly replay obsolete append-only notices
* AGENTS.md/process edit is recorded without pretending the current warm context changed magically
* next rebuild/new session uses the updated process context

Exit condition:

Tool iteration and process-context editing are fast without corrupting warm sessions, model context, or request-triggering semantics.

Required spike outcomes:

* user acceptance
* spike outcome document listing what to integrate and what not to integrate

---

## Spike 6: multi-client, CRDT UI state, streaming, and real UIs

Purpose: validate the hardest part of the client state model.

Before this spike, earlier prototypes may use a simple append-only CLI. After this spike, the design should have a real reactive TUI and a real web GUI sharing the same underlying state model.

Requirements covered:

* multi-client state
* stale clients
* shared face-to-face UI state
* CRDT or equivalent conflict-free client state
* face-owned vs session-owned tool sessions
* streaming consistency
* reconnect/catch-up
* reactive TUI
* web GUI

Features:

* two faces on one session
* reactive TUI
* web GUI
* shared client state model
* CRDT or equivalent for editable shared UI state
* draft message buffer sharing
* file tool open/edit state sharing
* stale client state
* event ordering
* streaming model/tool output
* face reconnect
* tool ownership experiment

Key tests:

* two clients eventually see same ordered durable events
* stale client send is represented causally
* stale client does not overwrite newer tool/draft state
* reconnect catches up without duplicates
* streaming output is visible while durable context remains compressed
* tool session ownership semantics are explicit
* Face A opens a file; Face B can observe compatible open-state if sharing is enabled
* Face A edits a draft message buffer; Face B receives the update
* two faces edit the same draft buffer; CRDT/equivalent state converges
* file edit buffer can be shared without corrupting durable file state
* face-local state remains local when explicitly not shared
* shared UI state does not automatically become model-facing context
* TUI and web GUI can attach to the same session
* TUI and web GUI use the same state model despite different rendering

Exit condition:

The UI/client state model is credible for real multi-client use, not just append-only transcript sync.

Required spike outcomes:

* user acceptance
* spike outcome document listing what to integrate and what not to integrate

---

## Spike 7: operator update/relaunch/protocol lifecycle

Purpose: validate deployment and operational lifecycle.

Requirements covered:

* auto-update
* downgrade
* relaunch
* protocol versioning
* migrations
* remote lifecycle

Features:

* single binary role simulation
* version negotiation
* update staging
* activation/verification
* downgrade/rollback
* migration safety
* smooth relaunch

Key tests:

* old/new component compatibility succeeds or fails clearly
* incompatible protocol rejected safely
* DB migration idempotent
* failed migration recoverable
* update can stage/activate/verify
* failed update downgrades
* relaunch preserves session/cache metadata
* remote relaunch can be triggered safely
* ongoing requests are not interrupted unnecessarily during graceful relaunch

Exit condition:

Operational lifecycle assumptions are credible enough for core design.

Required spike outcomes:

* user acceptance
* spike outcome document listing what to integrate and what not to integrate

---

# 4. Provisional core integration expectations

The eventual core should not be a direct copy of any spike. It should integrate the behaviours proven by the spikes into a coherent architecture.

## Core should likely include

* clear face/brain/limb role boundaries
* support for co-located and split role topologies
* SQLite-backed session/event storage
* durable and transient data lifecycle
* explicit context lifecycle: record, append, rebuild, trigger
* user-tool contract: interactive UI plus compressed context
* agent-tool contract: callable tool plus optional user-facing UI
* append mode and rebuild mode
* cache-aware metadata and cleanup
* structured subagent hierarchy
* multi-client-safe state model
* shared UI state model for real TUI/web clients
* plugin/tool reload path
* schema/description stability across warm sessions
* analytics-grade metadata from the start
* protocol/version/migration story
* operator lifecycle hooks for relaunch/update/downgrade
* authority boundaries that keep provider credentials brain-owned

## Core should not prematurely include

* full GUI polish beyond what the multi-UI spike needs to validate the state model
* full browser integration
* perfect terminal undo/fork semantics
* federated brain UX beyond validated routing basics
* highly forked shared-workspace automation as a default workflow
* elaborate agent permission prompts or permission theatre
* a general harness-owned permission model for limb filesystem/tool operations

## Core integration gates

These are the checklist for integration acceptance (Gate 2 in `process.md`). They expand on the invariants in section 1.1.

### Gate 1: behavioural coverage

Do the black-box tests still cover the full stakeholder breadth?

### Gate 2: architecture cleanliness

Would we choose this design fresh, knowing what the spikes taught us?

### Gate 3: no accidental narrowing

Does this implementation accidentally make future GUI, multi-client, topology, analytics, or plugin reload requirements awkward?

### Gate 4: no fake durability

Is each piece of stored data clear about whether it is durable session history, cache-supporting transient state, analytics data, shared UI state, or disposable stream data?

### Gate 5: no accidental inference triggering

Does context append remain separate from API request triggering?

### Gate 6: no silent context mismatch

Can the system distinguish warm append-mode deltas from rebuild-mode canonical context?

### Gate 7: no spike-code cargo culting

Are we integrating proven behaviour, not importing experimental structure blindly?

### Gate 8: no hidden security boundary erosion

Do provider credentials and secret-bearing capabilities remain brain-owned and absent from limbs, plugins, schemas, logs, and model context unless explicitly designed otherwise?

### Gate 9: no UI state handwaving

Is shared client state explicitly modeled, especially draft buffers, file edit state, selected tools, and reconnect/staleness behaviour?

### Gate 10: spike outcome review completed

Has the spike produced a document saying what to integrate, what not to integrate, and what requirements pressure appeared?

---

# 5. Short version

The implementation process should prove the harness as a shared in-band work system while building the core incrementally.

Spike 0 builds a walking skeleton; test primitives are extracted from it.
Each later spike validates one risky behavioural cluster.
Each spike ends with user acceptance plus a document saying what to integrate and what not to integrate.
Core integration happens small and continuously: tests first at the public surfaces, fresh design from evidence, gated by the invariants.
