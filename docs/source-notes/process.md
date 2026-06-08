# process

we'll implement spikes of various bits, but *explicitly* marked as experiments, labelled so, under separate part of the repo. that's for getting the design right, and when we move to a different aspect we do that again. We integrate into core only afterwards, and we do so cleanly, perfectly and minimally - when we integrate into core we leave NOTHING half done. we consider the full quadratic component interaction matrix and we trim complexity HARD over many refactors.

Each slice/spike should excercise the thesis of that slice. The good thing about these spikes is that they *don't* have to fit into the whole- so we can eg. go without evented model, or go without multiple UIs, or go without plugins, etc. in these spikes. but core then must deal with that integration, and come out perfect.

We set up the repo first. development process before product, always. we define black box tests before we start implementing, always. we set up additional checks like linting, formatting, version checks, CI checks etc, before we start implementing too. We do our best to do things right the first time, and when it inevitably goes a bit wrong, we improve it for next time.


# Agent Harness Implementation Process Plan

Status: provisional process plan
Purpose: define how an agent should shepherd implementation of the agent harness without collapsing the design into a premature MVP.

The agent following this plan is **not tasked with delivering the product directly**. The agent is tasked with maintaining the process: keeping the requirements live, creating and enforcing feedback loops, launching focused subagents, collecting evidence, sending work backwards when gates fail, and making sure progress on the product happens without narrowing the full design.

Product progress happens through controlled spikes, tests, reviews, and integrations. The process-steward agent owns the loops.

---

## 1. Core operating rule

Do not implement forward through uncertainty.

If design, tests, architecture, or evidence become underdetermined, the process goes backwards:

* implementation uncertainty sends work back to the design loop
* test inadequacy sends work back to the test loop
* architecture mismatch sends work back to the spike or requirements loop
* failed adversarial review sends work back to the loop being reviewed
* stakeholder/convex-hull mismatch sends work back to requirements

No loop exits merely because work was done. A loop exits only when its predeclared tests pass or an adversarial reviewer accepts the evidence.

---

# 2. Roles used by the process

## Process steward

The main agent running this process.

Responsibilities:

* maintain the current requirements document
* maintain the spike plan
* decide which loop is active
* launch subagents for focused work
* require tests/reviews before exits
* collect evidence
* write spike outcome documents
* prevent spike code from becoming accidental architecture
* send work backwards when gates fail

The process steward should avoid doing large implementation work directly. Its job is orchestration and quality control.

## Design subagent

Explores design implications, alternatives, and failure modes.

Outputs:

* design sketch
* open questions
* tradeoffs
* risky assumptions
* recommended tests

## Test subagent

Turns requirements into black-box tests, scenario tests, fixtures, and acceptance criteria.

Outputs:

* tests or executable scenarios
* explanation of what wrong designs would fail
* gaps where behaviour is not yet testable

## Implementation subagent

Builds spike/core code only after tests or review criteria exist.

Outputs:

* implementation
* evidence that tests/checks pass
* notes on unexpected design pressure

## Adversarial review subagent

Tries to reject the loop exit.

Outputs:

* accepted/rejected decision
* missed requirements
* hidden architecture narrowing
* untested behaviour
* suggestions for which earlier loop to return to

## User

The user is required at key gates:

* spike scoping
* test adequacy review where behaviour is UX-sensitive
* spike acceptance
* core integration acceptance
* decisions where tradeoffs affect the full harness shape

---

# 3. Global outer loop: process stewardship loop

This loop wraps the whole project.

## Activity

Maintain the live state of the project:

1. Read current requirements, spike outcome documents, tests, and implementation state.
2. Identify the next smallest process move that reduces uncertainty.
3. Choose the active loop:

   * requirements loop
   * test harness loop
   * disposable spike loop
   * spike outcome loop
   * core integration loop
   * regression/convex-hull loop
4. Launch subagents as needed.
5. Collect results.
6. Decide whether the current loop can exit.

## Check

At the end of each iteration, ask:

* Is the next action still aligned with the full requirements?
* Are we making progress on evidence, not just code?
* Is any subagent trying to patch forward through a design problem?
* Are tests/reviews strong enough to catch the wrong architecture?
* Is there a clear loop to return to if this fails?

## Exit condition

This loop does not exit until the project is intentionally stopped or the core harness has reached the agreed integration target.

## Go back path

If any active loop lacks tests, review criteria, or adversarial exit gates, return to the relevant design/test loop before continuing.

---

# 4. Requirements maintenance loop

This loop keeps the behavioural target current.

## Activity

1. Read the current requirements document.
2. Compare it against:

   * new user decisions
   * spike outcomes
   * failed tests
   * reviewer objections
   * implementation discoveries
3. Identify requirements that are missing, stale, too broad, or too narrow.
4. Update requirements only after the implication is clear.

## Check

A design or review subagent must adversarially check:

* Does the update preserve the full stakeholder breadth?
* Does it accidentally demote an important future requirement?
* Does it confuse a spike-only idea with a core requirement?
* Does it introduce implementation detail as if it were product behaviour?
* Does every new requirement have a plausible test/review strategy?

## Exit condition

Exit when the requirements are current enough that the next spike/core loop can be scoped without inventing missing behaviour.

## Go back path

If a requirement cannot be tested or reviewed, send it to the design loop for clarification.

If a requirement conflicts with another stakeholder angle, send it to user review before any implementation continues.

---

# 5. Pre-spike A loop: test harness primitives

Purpose: build the test language before building harness behaviour.

This is not a product implementation loop. It creates the machinery later loops use to test behaviour.

## Activity

Iterate on test primitives:

1. Sketch the scenario language.
2. Define fake actors:

   * fake face
   * fake brain
   * fake limb
   * fake model provider
   * fake user tool
   * fake agent tool
   * fake filesystem/workspace
3. Define assertions:

   * model request was triggered
   * model request was not triggered
   * model-facing context matches expectation
   * durable event/session record matches expectation
   * topology/lifecycle events occurred
   * crash/restart resumed correctly
4. Implement the smallest primitive needed.
5. Write example tests against fake behaviour.

## Check

Tests must exist before each primitive is considered done.

A test subagent checks:

* Can this primitive express later spike behaviours?
* Is it black-box enough to survive implementation changes?
* Does it test visible/system behaviour rather than internals?
* Can it simulate failure and restart, not just happy paths?

An adversarial reviewer checks:

* Would a bad harness architecture still pass these tests?
* Are we accidentally designing the core architecture inside the test harness?
* Are the primitives too weak to catch context/request lifecycle errors?

## Exit condition

Exit only when later spikes can use these primitives without building new local test rigs.

Required evidence:

* passing primitive tests
* at least one example scenario for user-tool context
* at least one example scenario for request-triggering/no-triggering
* at least one example scenario for restart/resume
* adversarial review accepted

## Go back path

If a later spike cannot express its tests using these primitives, return to this loop and extend the test harness.

---

# 6. Pre-spike B loop: disposable spike harness foundation

Purpose: build a minimal experimental harness substrate that multiple spikes can run against.

This code is not core. It is a disposable workbench.

## Activity

Iterate on a minimal experimental harness:

1. Basic session loop.
2. Minimal face abstraction.
3. Minimal brain abstraction.
4. Minimal limb abstraction.
5. Fake or real model adapter boundary.
6. User-tool context append path.
7. Agent-tool call path.
8. Simple recorder/persistence hook.
9. Simple streaming hook.

Before each piece is implemented, write or select the test harness scenario that will prove it.

## Check

Tests must exist at the start of each inner implementation loop.

A review subagent checks:

* Does the foundation support multiple spikes?
* Is it clearly marked disposable?
* Is anything here becoming accidental core architecture?
* Can this foundation be thrown away without losing the tests?

## Exit condition

Exit when at least two planned spikes can plausibly run on this foundation and the user accepts it as an experimental scaffold.

Required evidence:

* tests pass
* foundation can run a toy model/tool/user-tool loop
* foundation can record enough state for assertions
* spike outcome note says what not to copy into core

## Go back path

If the foundation starts forcing architecture decisions that belong to core, stop and redesign the foundation as thinner/disposable.

If a spike needs behaviour the foundation cannot express, return here only if the behaviour is broadly reusable. Otherwise keep the spike local.

---

# 7. Generic spike loop

Every spike follows this loop. The spike’s specific subject changes, but the process does not.

## Activity

1. Scope the spike with the user.
2. Identify which requirements it is meant to test.
3. Identify which requirements it might accidentally damage.
4. Design the black-box tests first.
5. Build or adapt the disposable spike harness.
6. Implement the smallest behaviour needed to exercise the tests.
7. Run tests continuously.
8. When unexpected design pressure appears, stop and return to design/tests.
9. Produce a spike outcome document.
10. Ask the user for acceptance.

## Inner design loop

Activity:

* sketch the behaviour
* trace implications across all stakeholder requirements
* identify what must be impossible
* identify what would count as fake success

Check:

* adversarial design review by second agent

Exit:

* design is clear enough that tests can be written before implementation

Go back:

* if tests cannot be written cleanly, return to design
* if the design contradicts another requirement angle, return to requirements maintenance

## Inner test loop

Activity:

* write black-box tests
* write failure cases
* write at least one “wrong architecture should fail” test
* define user acceptance checks

Check:

* adversarial test review by second agent

Exit:

* tests would fail if the spike built the wrong thing

Go back:

* if tests are too implementation-specific, rewrite tests
* if tests expose missing behaviour, return to design

## Inner implementation loop

Activity:

* implement minimum behaviour
* run tests
* inspect evidence
* keep code disposable unless explicitly integrating into core

Check:

* tests pass
* implementation subagent reports unexpected pressure
* adversarial reviewer checks for architecture narrowing

Exit:

* tests pass and reviewer accepts the evidence

Go back:

* if tests fail because implementation is wrong, continue implementation loop
* if tests fail because design is wrong, return to design loop
* if tests are inadequate, return to test loop

## Spike outcome loop

Activity:

Write a short outcome document:

* what the spike proved
* what it failed to prove
* what should be integrated
* what should explicitly not be integrated
* which tests should become core tests
* which requirements were affected
* what new risks/open questions appeared

Check:

* adversarial review against the full convex hull of requirements
* user acceptance

Exit:

* user accepts the spike outcome
* reviewer accepts that the outcome document distinguishes evidence from architecture

Go back:

* if the spike did not prove enough, return to spike design/test loop
* if it changed requirements, return to requirements maintenance
* if it revealed a better spike ordering, return to process stewardship loop

---

# 8. Specific spike loops

## Spike 1 loop: user-tool context contract

Purpose: validate the central in-band collaboration thesis.

Focus:

* file user tool
* terminal/search-like user tool
* compressed model-facing context
* user activity included in next relevant request
* passive context append does not trigger inference
* user-tool context is framed as user activity

Tests must be written first for:

* file open/edit context
* command/search context
* large output compression
* user activity does not trigger model request
* user turn end includes accumulated context
* agent tool-loop request includes piggyback context
* user-tool output is not framed as an agent tool call

Exit gate:

* tests pass
* user accepts that the interaction feels like in-band work
* adversarial reviewer accepts that the user-tool contract is not just “chat with logs attached”

Go back:

* if context is too noisy, return to user-tool compression design
* if agent cannot understand user activity, return to model-facing framing design
* if passive events trigger unwanted inference, return to context lifecycle design

---

## Spike 2 loop: actor topology / transport / lifecycle

Purpose: validate face/brain/limb as logical roles across co-located, split, proxied, and multi-face configurations.

Topologies to test:

* `face+brain+limb`
* `face+limb <-> brain`
* `face <-> brain <-> limb`
* `face <-> brain <-> brain <-> limb`
* `face <-> brain+limb <-> face 2`
* optional direct face-limb stream via brain signalling/capability

Tests must be written first for:

* same scenario works in each topology
* monolith still respects logical role boundaries
* brain is only actor that drives model API requests
* limb has no provider credentials
* face disconnect leaves limb/brain running where topology permits
* brain/limb disconnect triggers reconnect or timeout
* face reconnect catches up
* direct face-limb stream succeeds/falls back
* brain-issued capability controls direct stream

Exit gate:

* topology matrix passes
* user accepts lifecycle behaviour
* adversarial reviewer accepts that co-location and splitting are deployment choices over the same logical model

Go back:

* if monolith bypasses boundaries, return to topology design
* if split mode requires different behaviour, return to role-boundary design
* if direct streaming weakens authority boundaries, return to security-boundary design

---

## Spike 3 loop: persistence, resume, and analytics-grade storage

Purpose: validate storage before it calcifies.

This spike is about durable and transient state, restart/resume, and queryability. It does not own the full append/rebuild/request-triggering context lifecycle; that belongs to Spike 5.

Tests must be written first for:

* restart resumes sessions
* session list and hierarchy survive restart
* in-flight model request state is represented
* in-flight tool call state is represented
* cache-supporting transient data survives restart while useful
* cache-supporting transient data is cleaned after expiry
* cleanup does not delete durable session/analytics data
* cost/cache/tool queries work
* blocked/stuck scope queries work
* large blobs are separate from hot indexed tables
* schema can support later context lifecycle without rewrite

Exit gate:

* storage tests pass
* analyst queries are demonstrated
* restart/resume behaviour is demonstrated
* adversarial reviewer accepts that the schema is not just transcript storage
* user accepts the state/query model

Go back:

* if analytics require parsing transcripts, return to schema design
* if resume requires hidden process state, return to persistence design
* if transient data cannot be cleaned safely, return to data lifecycle design
* if later context lifecycle would force rewrite, return to schema design

---

## Spike 4 loop: structured subagents

Purpose: validate hierarchy, blocking, fork/fresh, and attention semantics.

Tests must be written first for:

* parent suspends while children run
* parent resumes only when all children complete
* sibling status visible
* sibling results hidden until parent resumes
* user-facing child completes on `/done`
* failed child returns error result
* abandoned/stuck child is visible
* fresh session required across limb boundary
* fork default within same limb

Exit gate:

* tests pass
* user accepts the attention model
* adversarial reviewer accepts that this is structured concurrency, not global uncontrolled spawning

Go back:

* if parent can proceed before children finish, return to hierarchy design
* if sibling results leak early, return to scope semantics
* if user-facing child blocks the system confusingly, return to UX/attention design
* if fork/fresh rules are unclear, return to context/session design

---

## Spike 5 loop: live editing, tool reload, schema stability, and context lifecycle

Purpose: validate rapid iteration for tools, plugins, descriptions, schemas, prompts, process context, and append/rebuild/request-triggering semantics.

Tests must be written first for:

* existing warm session keeps v1 or receives explicit diff/notice
* new session receives v2
* rebuild-mode context receives canonical v2
* failed reload does not brick existing sessions
* tool implementation reloads without full harness restart
* agent can be shown schema/description diff
* explicit cache break/rebuild path works
* user opens/edits files without triggering model request
* user terminal output arrives without triggering model request
* user turn end triggers model request with accumulated context
* agent tool-loop continuation triggers model request with piggyback context
* cache-nearly-expired handover/compaction can trigger model request
* append mode includes relevant deltas
* rebuild mode uses canonical current state
* rebuild mode does not replay obsolete append-only notices
* process edit is recorded without pretending warm context changed magically
* next rebuild/new session uses updated process context

Exit gate:

* tests pass
* user accepts reload/edit behaviour
* adversarial reviewer accepts that context append, context rebuild, persistence, and inference triggering are distinct

Go back:

* if reload breaks warm sessions, return to schema stability design
* if context additions trigger accidental requests, return to request lifecycle design
* if rebuild mode behaves like append replay, return to context lifecycle design
* if process edits create silent context mismatch, return to process-context design

---

## Spike 6 loop: multi-client, CRDT UI state, streaming, and real UIs

Purpose: validate the hardest client state model.

Before this spike, earlier prototypes may use a simple append-only CLI. After this spike, the design should have a real reactive TUI and a real web GUI sharing the same underlying state model.

Tests must be written first for:

* two clients eventually see same durable events
* stale client send is represented causally
* stale client does not overwrite newer tool/draft state
* reconnect catches up without duplicates
* streaming output is visible while durable context remains compressed
* tool session ownership is explicit
* Face A opens a file; Face B can observe compatible open-state if sharing is enabled
* Face A edits a draft buffer; Face B receives the update
* two faces edit the same draft buffer; CRDT/equivalent state converges
* file edit buffer can be shared without corrupting durable file state
* face-local state remains local when explicitly not shared
* shared UI state does not automatically become model-facing context
* TUI and web GUI attach to the same session
* TUI and web GUI use the same state model despite different rendering

Exit gate:

* tests pass
* user accepts real TUI/web behaviour
* adversarial reviewer accepts that UI state is explicitly modeled, not handwaved as append-only session sync

Go back:

* if CRDT/shared state corrupts drafts or edit buffers, return to UI state design
* if shared UI state leaks into model context accidentally, return to user-tool context design
* if TUI and web GUI require different semantics, return to client state design
* if reconnect duplicates or loses state, return to lifecycle/persistence design

---

## Spike 7 loop: operator update/relaunch/protocol lifecycle

Purpose: validate deployment and operational lifecycle.

Tests must be written first for:

* old/new component compatibility succeeds or fails clearly
* incompatible protocol rejected safely
* DB migration idempotent
* failed migration recoverable
* update can stage, activate, and verify
* failed update downgrades
* relaunch preserves session/cache metadata
* remote relaunch can be triggered safely
* ongoing requests are not interrupted unnecessarily during graceful relaunch

Exit gate:

* tests pass
* user accepts operational behaviour
* adversarial reviewer accepts that update/relaunch/protocol lifecycle is real, not a later script

Go back:

* if relaunch loses session/cache state, return to persistence design
* if update can strand components, return to protocol design
* if migration can corrupt data, return to schema/migration design
* if graceful shutdown interrupts active work unnecessarily, return to lifecycle design

---

# 9. Core integration loop

This loop integrates proven behaviours into the actual harness core.

Core integration only happens after relevant spike outcome documents exist.

## Activity

1. Choose a narrow integration target.
2. Gather:

   * relevant requirements
   * relevant spike outcome documents
   * tests to promote into core
   * “do not integrate” notes
3. Re-design the core shape fresh.
4. Write or port black-box tests before implementation.
5. Implement the smallest coherent core slice.
6. Run all existing core tests.
7. Run regression checks across stakeholder requirements.
8. Review architecture.
9. Decide whether to continue, refactor, or go back.

## Check

Tests must exist before implementation starts.

An adversarial review subagent checks:

* Is this integrating behaviour or copying spike architecture?
* Does it preserve future GUI/multi-client/topology/plugin/analytics requirements?
* Does it maintain explicit context lifecycle?
* Does it maintain authority boundaries?
* Does it distinguish durable session data, transient cache-supporting data, shared UI state, and disposable stream data?
* Would we choose this design fresh?

## Exit condition

Exit only when:

* core tests pass
* relevant spike tests have been promoted or replaced
* adversarial review accepts the integration
* user accepts the integration behaviour
* architecture review says this is clean enough to build on

## Go back path

If tests fail due to implementation errors, continue implementation loop.

If tests reveal design mismatch, return to core design.

If implementation exposes missing spike evidence, return to the relevant spike loop.

If integration narrows future requirements, return to requirements maintenance or architecture design.

If spike code is being cargo-culted, stop and redesign fresh.

---

# 10. Regression-across-requirements loop

This loop runs after every spike outcome and every core integration.

## Activity

Review the change against all stakeholder requirements:

* worker
* process improver
* harness developer
* operator
* analyst
* security / authority boundary
* attention / coordination
* multi-client / UI state

For each stakeholder, ask what got better, worse, or newly constrained.

## Check

Adversarial review subagent must try to find:

* a stakeholder requirement silently weakened
* a future feature made awkward
* a hidden security boundary erosion
* an inference-triggering ambiguity
* analytics/queryability lost to blobs/transcripts
* UI state handwaving
* spike code becoming architecture
* insufficient tests

## Exit condition

Exit when the reviewer fails to find a blocking regression, or when the regression is explicitly accepted by the user and documented as a requirements change.

## Go back path

If a regression is found, return to the loop that caused it:

* requirements issue → requirements maintenance loop
* missing tests → test loop
* bad architecture → design loop
* bad implementation → implementation loop
* insufficient evidence → spike loop

---

# 11. Handoff and continuity loop

This loop keeps the process itself resumable.

## Activity

At the end of each major process iteration, update a process handoff:

* current active loop
* current requirements version
* active spike/core target
* tests written
* tests passing/failing
* subagents launched and results
* decisions made
* open questions
* next recommended loop
* known “do not integrate” warnings

## Check

A review subagent or test checklist verifies:

* a fresh agent could resume the process without guessing
* open questions are explicit
* no failed gate is described as passed
* no spike result is described as core architecture

## Exit condition

Exit when the handoff is good enough that another agent could continue the process.

## Go back path

If the handoff loses the current place or hides uncertainty, rewrite it before continuing product work.

---

# 12. Stopping rules

The process-steward agent must stop and ask the user when:

* stakeholder requirements conflict and no local decision is obvious
* a spike reveals that the current architecture direction may be wrong
* a core integration would narrow the product significantly
* a test cannot be written because behaviour is underspecified
* an adversarial reviewer rejects a loop exit and the next direction is not obvious
* a subagent recommends a major requirement change

The process-steward agent should not ask the user merely because implementation is hard. Hard implementation goes through design/test/spike loops.

---

# 13. Short version

The agent following this plan is not building the product directly.

It maintains a process where:

* requirements define the behavioural target
* tests or adversarial reviews guard every loop exit
* disposable spikes produce evidence
* spike outcome documents say what to integrate and what not to integrate
* core integration happens only after behaviour is tested and accepted
* failed gates send work backwards
* subagents slowly make product progress under process control
* the full convex hull of requirements is checked after every meaningful step

