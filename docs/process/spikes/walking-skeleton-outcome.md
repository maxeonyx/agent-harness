# Spike Outcome: walking-skeleton (redo)

Spike: walking-skeleton, rebuilt per the revised brief of 2026-07-30 (Gate 1
of the first attempt: redo; see `walking-skeleton-outcome-v1.md`).
Status: evidence complete from scripted scenarios and an agent-run
real-provider smoke (OpenRouter, 2026-07-30: tool round-trip and
cancel-during-tool both worked); awaiting the user's own smoke run and
Gate 1.
Requirements tested: none by itself (Spike 0 is the shared substrate);
exercises invariants 2, 3, 4, 8, 9.

## What The Spike Proved

- The two-select-loop shape works and stays responsive: the face loop
  (stdin + event rendering) and the brain's session loop (user events +
  in-flight work) communicate only by channels. While a tool call sleeps,
  typed user input is acknowledged at the face before the tool result
  arrives — asserted in the scenario, not just observed.
- Events-about-emitter with consumer projections is expressible and cheap:
  one session event log is the source of truth; the face rendering, the
  model context view, and the recorder JSONL are three projections of the
  same events. `FileOpened` carries facts; the face shows path+bytes, the
  model sees a compressed `[user activity]` framing.
- Piggybacking works at the wire: user activity arriving mid-tool-call
  appends without triggering and rides the next request *after* the tool
  exchange — the `tool_calls` → `tool` adjacency is asserted on the fake
  provider's request log. Append-never-triggers is now observed *between*
  steps (the wire is checked empty after appends, before `/end`), fixing
  the v1 review's evidence gap.
- Cancellation as request → drain → finalize holds up, and it is cheap to
  build on tokio primitives: `/cancel` during a 30s (test) / 60s (smoke)
  bash tool call kills and reaps the child in well under a second, resolves
  the tool to a `Cancelled` outcome (distinct from error), records a tool
  result on the wire for the cancelled call (so the next request is
  protocol-valid), resolves the turn as cancelled, and leaves the session
  usable — the scenario continues with a second successful turn.
- Nothing in flight ends without an outcome, structurally: every spawned
  task sends exactly one resolution message back into the session loop
  (cancel is signalled by token, and the drain waits for the resolution);
  panics are converted to a `Panicked` outcome by a supervisor wrapper.
  Attempts and outcomes are separate events, so the recorder records facts
  — `request_attempt` is not a claim that a request succeeded.
- The context lifecycle is visible in the code: `append` (incremental,
  cache-friendly), `rebuild` (fresh projection of the whole log, a real
  operation with a `/rebuild` surface and a `context_rebuilt` event), and
  per-consumer views are distinct operations on the `Context` type.
- Introspection is just another projection: `/dump` renders the ~exact
  model view as markdown — wire order, verbatim content, tool calls
  visible — with everything the model *cannot* see (non-wire events,
  piggyback/arrival annotations, held entries) in HTML comments, opened in
  `$EDITOR` and returning to the face on exit. Asserted in a scenario and
  smoke-run against the real provider. The dump makes the piggyback answer
  directly observable: concurrent user events are appended *after* the
  tool exchange, never between `tool_calls` and its result.
- System-prompt / environment contributions are modeled (user direction):
  tools, and facts like time, hostname, and model, enter the log as
  `contribution_added` events. A contribution that exists from the start
  composes into the system prompt / the request's tools field; one added
  or changed while the context is active appends an update message (the
  cache-friendly notification-vs-rebuild policy is later work). The
  request builder and the dump both consume one shared projection,
  `request_parts` — after the user caught the first dump omitting tool
  schemas (they were fetched brain-privately at request time), divergence
  between "what was sent" and "what the dump shows" is now a shared-code
  impossibility rather than a rendering discipline. Verified live: the
  real model answered its own model name from the environment section.
- The dump is computed by the *face*, not served by the brain (invariant
  10): `dump_request` is a fact, and when the face sees its own request
  come back on the bus (so the log provably includes everything prior) it
  projects the dump from the shared session log and writes the temp file
  on its own filesystem. The system prompt is a `session_started` event,
  so everything the model sees is derivable from the log by any consumer.
  Shared log is `Arc<Mutex<_>>` for now (append-only; a lock-free log or
  single-threaded model is a recorded TODO).

## What The Spike Failed To Prove

- Rebuild is behaviorally identical to append today (no compaction, no
  cache-expiry policy) — the seam exists, the policy does not.
- Streaming remains absent (deliberately out of scope); interface
  responsiveness during a long *non-streaming* request is proven, token
  streaming is not.
- Only one face, one session, one in-flight tool at a time. Parallel tool
  calls from one response are executed sequentially.
- Provider dialect coverage is unchanged from v1: OpenRouter works; other
  endpoints unverified.
- Real-provider cancel coverage: the smoke runs exercised
  cancel-during-tool; cancel-during-request is scenario-asserted against
  the fake provider only.

## What Should Be Integrated

Shapes, not code (invariant 8):

- One event log per session as source of truth, with face/model/recorder as
  projections. The piggyback-ordering rule living *in the model-view
  projection* (facts in arrival order; the view keeps tool exchanges
  intact) was the key simplification — no queueing machinery.
- The resolution-message discipline: in-flight work = a task + a
  cancellation token + exactly one resolution message. Cancel = signal,
  then await the same resolution path as success. Four-valued outcomes.
- Attempt/outcome as separate recorded events.
- The interactive scenario-test harness shape: drive stdin, read stdout
  live, observe the wire between steps.

## What Must Not Be Integrated

- Any of this code by copying (invariant 8).
- The broadcast-channel bus as *the* transport decision — it is one
  in-process stand-in for the eventual face/brain/limb transport.
- The `Vec<Event>` in-memory log and JSONL recorder as the storage design;
  SQLite/storage is a later experiment (user direction).
- Env-var-only configuration, the unrestricted bash tool.

## Tests To Promote Or Preserve

`tests/scenario.rs` — three scenarios at the public surfaces (CLI in, face
output + provider wire out): append-never-triggers observed between steps;
mid-tool responsiveness + piggyback adjacency; cancel drain + session
continuation. The interactive harness (send, wait_for, requests-between)
is the durable black-box shape. These assert face output and wire only —
no recorder-internal event names — so they can be re-derived without
freezing the event taxonomy.

## Requirements Pressure

- None new. The Gate 1 direction of 2026-07-30 (invariant 3 reworded,
  invariant 9 added, deferrals) is already recorded in `REQUIREMENTS.md`.

## New Risks Or Open Questions

- The session loop processes one resolution at a time and `drain` awaits
  inline; with parallel tool calls or multiple sessions this single-loop
  shape needs rethought (structured concurrency — asupersync territory).
- `TurnEnd` during a live turn is recorded but does not re-trigger; whether
  it should queue a follow-up turn is undecided.
- The broadcast bus drops events for lagged consumers (recorder prints a
  warning). Fine for a spike; a real recorder needs a lossless path.
- Cancelling a provider request drops the connection; whether providers
  bill for it, and whether a cancel should instead race a short grace
  window, is unknown.
- The face renders its own echo from the bus (multi-client-shaped), which
  means user input acknowledgment round-trips through the brain. Fine
  in-process; adds latency once the transport is real.
- "~exact as the model sees it" rests on projection determinism: the face
  projects the dump from the same log with the same code, so in-process it
  is exact. Once views become per-model/per-face or the roles split, the
  dump may need reconciling against what the brain actually sent.
- Wire ordering of concurrent user events (user direction, 2026-07-30,
  after inspecting /dump behavior): currently they are appended after the
  tool exchange. "I think it maybe should be chronological where possible,
  although the model might get a little confused when a tool call is not
  right next to its result? Possibly we should even re-write model
  responses to occur after the user events that happened while we were
  waiting for it? Maybe not though — that's a trickier one. Technically
  there's just no total ordering." Open design question for the context
  view; /dump exists to make experiments here observable.

## Invariants Check

2. Upheld and now visible in the code: append/rebuild/view are distinct
   operations on `Context`; appending never triggers (asserted at the wire,
   between steps); triggering is explicit (`TurnEnd` when idle).
3. Upheld as reworded: events are emitter-centric facts; face and model
   views are projections and demonstrably differ (`FileOpened`,
   tool outcomes).
4. Upheld by construction, more honestly than v1: the brain talks to faces
   only via the event bus and user-event channel; the face never touches
   the provider; the limb owns tool execution and its own drain. Still
   co-located in one process; splitting is untested (as the notes expect).
8. Upheld — everything lives in `experiments/walking-skeleton/`.
9. Upheld within scope: request → drain → finalize with four-valued
   outcomes; every attempt resolves; cancelled is not an error; the child
   process is reaped, never abandoned.
10. Upheld after the /dump correction: no paths cross role boundaries, the
   face writes its own dump file, the system prompt is in the log. The
   in-process shared log is an explicit deployment optimization, and the
   projection-determinism caveat below is the residual risk.
1. Explicitly out of scope for the skeleton (user direction at Gate 1).

## Review Result

Thermonuclear review round 1 (fresh-context, 2026-07-30) returned 8
findings. User triage and the resulting changes:

- Fixed: the drain race (a cancel/quit drain could process a completion
  and launch new work — now `record_resolution` is shared but only the
  normal path can advance; a drain structurally cannot start work). Note:
  the completion-wins-the-race case is not black-box testable
  deterministically; the guarantee is structural, not race-tested.
- Fixed: the limb is its own loop owning its environment; the brain holds
  a channel, never the limb ("a session has a limb at the logical level,
  but not at the memory ownership level necessarily" — user). The limb
  describes its own contributions.
- Fixed: structured lifecycle throughout ("this skeleton should lead by
  example" — user): no `process::exit`, no detached threads; the input
  thread owns parsing and is joined; all tasks joined with failures
  propagated. Judo bonus: the face render loop collapsed to a pure event
  consumer.
- Fixed: stderr pipe deadlock in bash execution (concurrent pipe reads);
  recorder opens once and writes async.
- Fixed: cancel-during-provider-request now has a black-box scenario (the
  earlier claim was overstated).
- Ruled fine (user): `model` and `reasoning_effort` living outside
  `request_parts` — "those are not part of the *context*, only part of
  the *request*".
- Ruled deferred (user): mid-context contribution updates stay unwired
  ("not important right now"), with a comment at the site; the shared
  bus / untyped face channel is fine for now — a refined event-based
  replication protocol (generic event streaming system, harness innards
  rebuilt on top) is queued as a later experiment.

Thermonuclear review round 2 (fresh-context, 2026-07-31) returned findings
on in-flight-work ownership and cancellation completeness. Fixes:

- Cancelling a bash tool kills the whole process tree, not just the shell:
  each child gets its own process group; the drain signals the group (a
  non-blocking syscall) and reaps the shell asynchronously. Red-proven:
  the descendant test fails with the group kill disabled.
- Non-bash tools are cancellation-aware (`cancellable` races the body
  against the token). Accepted limitation, documented: a read blocked on
  a writerless FIFO lingers on the blocking pool until a writer appears.
- In-flight work is owned: the session loop holds identity, cancellation
  token, and join handle together, and always joins — a panic joins as a
  `Panicked` outcome, a dropped limb reply resolves `Panicked`, never a
  vanished operation.
- `/dump` renders outside the context lock (`dump_snapshot` under the
  lock, linear `render` outside).
- Test coverage added: descendant-tree kill, blocked-read cancel,
  quit-during-tool drain, rebuild-preserves-view.

User direction during round 2 fixes: "limb should own and clean up
processes on graceful shutdown" — cleanup by ownership, never by global
observation. The test harness's first cut (a Drop that pgrep-walked
descendants and shelled out to `kill -9`) was rejected as hacky and
replaced: cleanup drives the skeleton's own graceful chain through the
real user surface (stdin EOF → face Quit → brain drain → limb kills its
process group), and process-death assertions target PIDs the test's own
fixture recorded, not process-table pattern scans. The user's hedged
container idea ("do it in some kind of container maybe?") is recorded in
REQUIREMENTS.md as the kernel-enforced form of the same principle — a
possible later experiment, not spike scope.

Thermonuclear review round 3 (fresh-context, 2026-07-31) returned three
findings, all fixed:

- A *completed* tool could leak backgrounded descendants (group cleanup
  existed only on cancellation). Now the process group's lifetime is the
  operation's lifetime: it is killed on every resolution path. Test-first:
  `completed_tool_does_not_leak_descendants` failed red before the fix.
- Top-level concurrency was not supervised: a dead face could hang the
  brain, and a failed join `.expect` skipped later joins. Main is now the
  supervisor — an auxiliary ending during a live session triggers a
  graceful brain shutdown (in-flight work still drained, limb still
  cleans up), every task is joined, failures set the exit code.
- The bash cancellation drain aborted its pipe-reader task without
  joining it; the group kill closes every pipe writer, so the drain now
  joins the readers instead.

The supervisor fix initially flaked (~1 in 10 suite runs). User ruling:
"test flake is a bug. make sure to make it impossible." Root cause: the
supervisor decided "orderly vs early" by *consuming* SessionClosed from
its bus receiver — a consuming check of a monotonic fact, so the first of
two orderly auxiliary exits could swallow the event and the second was
misclassified as a mid-session death. Fixed structurally: the watch
remembers (`SessionClosedWatch.seen`), making the predicate monotone like
the fact it tracks. 40 consecutive full-suite runs green after the fix.

## User Acceptance

Pending.
