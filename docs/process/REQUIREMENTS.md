# Curated Requirements

This is the live behavioral target. The detailed stakeholder requirements and
test-case lists remain in `docs/source-notes/requirements.md` §2 (verbatim
source material); this file curates the invariants and tracks validation
status. When a spike outcome changes a requirement, record it here — source
notes are never edited locally.

Last reconciled with source notes: gist revision of 2026-06-13.

## Invariants

The non-negotiables every gate checks against. A change that violates one of
these stops and goes to the user.

1. The brain is the only role that drives provider API requests. Provider
   credentials never reach limbs, faces, plugins, tool schemas, logs, or model
   context.
2. Recording context, appending context, rebuilding context, and triggering
   inference are distinct operations. Passive user activity never triggers a
   model request.
3. All activity has multiple views. An event is about its emitter, not *for*
   anyone; the consumer (or some helpful middle layer) does the projection if
   needed. Events to the model, maybe different for different models; events
   to the user, maybe different for different interfaces / settings; context
   rebuild may use a different view to a fresh append, for both. (Reworded at
   Gate 1 of Spike 0, 2026-07-30; the original user-tool framing rule — user
   activity is framed as user activity, never as agent tool calls — still
   holds as one projection of this.)
4. Face, brain, and limb are logical roles. Co-location versus splitting is a
   deployment choice over the same logical model.
5. Durable session data is analytics-grade and queryable. Durable,
   cache-supporting-transient, shared-UI, and disposable-stream data are
   explicitly distinguished.
6. Subagent concurrency is structured: parents block on children; sibling
   results stay hidden until the parent resumes.
7. Multi-client UI state is explicitly modeled. Stale clients cannot silently
   overwrite newer state; the user wins on conflicting edits.
8. Spike code never becomes core by copying. Core integration is a fresh
   design from evidence.
9. Cancellation is modeled well, baked in from the start — it is really,
   really important for a solid UX. Cancellation is an explicit
   request → drain → finalize protocol; anything in flight ends with a
   recorded outcome, and cancelled is distinct from error. (Added at Gate 1
   of Spike 0, 2026-07-30; modeling inspiration: Dicklesworthstone/asupersync.)
10. Roles never assume co-location. Face, brain, and limb must not be
   assumed to share a filesystem — or necessarily to have one (though a
   TUI face can be assumed to have an FS of its own). The same goes for
   other process-local state: environment, working directory, clocks.
   Data crossing a role boundary travels in the event or message itself,
   never by reference to role-local state — a file path is only meaningful
   to the role that created it. Perhaps an exception: a face and limb that
   do have the same FS — it will be common for face and limb to have a
   shared environment. Anything the model sees must be derivable from the
   session log by any consumer (e.g. the system prompt enters the log as
   an event, not as brain-private config). Co-located deployments may
   share the log directly as an optimization; a remote face maintains
   enough of the log for its queries — even if not, it can request enough,
   maybe. (Added 2026-07-30 during the Spike 0 redo, at /dump; face↔limb
   exception noted same day.)

## Requirement areas and validation status

| Area | Source | Validated by | Status |
|------|--------|--------------|--------|
| In-band user work, user-tool compression, user-wins conflicts | source-notes `requirements.md` §2.1 | Spike 1 | not started |
| Process/context edits first-class, rapid tool iteration, context lifecycle | §2.2 | Spike 5 | not started |
| Disposable spikes, safe reload, self-modification | §2.3 | Spikes 0/5 | not started |
| Topology, lifecycle, direct streams, updates/migrations | §2.4 | Spikes 2/7 | not started |
| Analytics-grade storage, data lifecycle | §2.5 | Spike 3 | not started |
| Authority boundaries, credential ownership, tool framing | §2.6 | Spikes 1/2 | not started |
| Structured subagent concurrency, scope legibility | §2.7 | Spike 4 | not started |
| Multi-client state, shared UI state, TUI + web GUI | §2.8 | Spike 6 | not started |

Spike 0 (walking skeleton) validates no requirement area by itself; it is the
shared substrate the others run on.

## Requirement changes from spike evidence

Gate 1 of Spike 0 (walking skeleton), 2026-07-30 — user direction after a
fresh-context review returned findings. Result: redo. The goal of the
skeleton is to have something for the rest of the work to build on: real
provider, real minimal interface, real code and provisional abstractions.

- Invariant 3 reworded (above): "all activity has multiple views."
- Invariant 9 added (above): cancellation baked in from the start.
- The skeleton needs async I/O from the start. There are at least two select
  loops: the face loop and the agent loop. The agent loop manages one
  session's model / tool-call loop, gets additional events from the user, and
  sends events to the interface(s). The brain handles these session /
  subsession loops. Face is an abstraction.
- Invariant 1 (credentials) is "not at all important for the skeleton —
  that's details"; something more principled later. No key scrub now.
- Invariant 2 (context lifecycle) matters for the skeleton: it affects the
  code architecture and wasn't visibly there in the first attempt.
- Deferred by user direction: tool-calling details / robustness / improvements
  (a later experiment); provider streaming ("event based but a kind of
  ephemeral event that's just extra complexity right now"); a full
  error-case variety in the fake provider; per-model / per-interface view
  variation; SQLite storage design (a later experiment); subagents (out of
  scope).

Further direction during the redo, 2026-07-30 (at /dump review):

- Invariant 10 added (above): roles never assume co-location. Trigger: the
  first /dump had the brain write a temp file for the face's editor.
- /dump must be in from the start: "I really, really want an easy way of
  introspecting on the ~exact text *as the model sees it*."
- Shared-log concurrency: "you can be more clever than arc mutex... it's
  append only. mutex on cleanup, though" — then walked back to "just do
  arc mutex for now. leave it todo" (TODO recorded on `Context`).
- System prompt / environment contributions modeled (user direction): "a
  contribution that exists from the start comes with an addition to the
  system prompt, but a contribution that changes or gets added while a
  context is active comes with an update appended to the context.
  Examples: skills, tools, environment facts like time, hostname, model."
- The rendering of API requests must share code with the rendering of the
  context dump — a dump missing something the model sees (as happened
  with tool schemas) "must be impossible somehow". Scope ruling (user):
  `model` and `reasoning_effort` staying outside that shared projection is
  fine — "those are not part of the *context*, only part of the
  *request*".
- Limb ownership (user, at review round 1): "the brain runs the agent
  loop for a session, but the limb owns a particular environment
  including the context that it provides. A session has a limb at the
  logical level, but not at the memory ownership level necessarily."
- Structured lifecycle: the skeleton "should lead by example on this
  stuff" (no detached threads, no process::exit escape hatches).
- Future experiment (user): a "refined event-based replication protocol —
  build a somewhat generic event streaming system and re-build the
  harness innards on top of that." At this point one shared bus and untyped
  channels were accepted until then; the 2026-07-31 ruling below supersedes
  that interim design.

Further direction during review round 2 fixes, 2026-07-31:

- Process ownership (user): "limb should own and clean up processes on
  graceful shutdown." Cleanup by ownership, not by global observation —
  no process-table scanning (pgrep/proc-walking) in app or tests, and no
  blocking process cleanup inside the async app. Rejected as hacky: a
  test-harness Drop that enumerated global child processes via pgrep.
- Sandboxing idea (user, hedged): "do it in some kind of container
  maybe?" — a PID namespace / cgroup per limb would be the
  kernel-enforced form of the same ownership principle. Not spike scope;
  candidate for a later experiment.

Review round 4 and architecture discussion, 2026-07-31:

- A cancelled turn may retain an assistant response containing proposed but
  unexecuted tool calls: "the tool call by itself is actually valid, and we
  could action that later if we wanted." It is resumable state: "we should be
  able to resume later and then execute the tool call that we had pending from
  the last time we were accessing the session." The wire projection omits
  unexecuted calls (the model never sees a call that never ran); no synthetic
  outcomes are fabricated; /dump shows them as invisible-facts comments.
- Sequencing is a property of the deployment substrate, never the brain or
  another participant. One process synchronizes and therefore has a
  sequencer. Across processes that are too far away to sequence, accept that
  there is no total order and do not synchronize. Whether same-machine IPC
  is close enough remains unclear.
- In-process appends are synchronous method calls under a lock. The
  synchronous-versus-asynchronous append question begins at a process
  boundary and is deferred.
- Face, brain, and limb are symmetric; every component owns exactly one
  external world. Face owns terminal/UI, brain owns the provider connection,
  and limb owns the environment (filesystem, processes, tools). These roles
  are locked before later experiments. Provider state belongs to the brain
  just as ephemeral UI state belongs to the face.
- The current limb has the same shape as face and brain: inbox + select loop
  + owned in-flight work, not request/reply slots. In the later streaming
  design the limb is an event peer (including streamed tool results and file
  watching).
- Desired shutdown pattern, deferred beyond this spike: "every layer should
  think about how it's shutting down gracefully in response to a
  cancellation." Each layer gracefully shuts down what it owns; its parent
  has a timeout backstop and kills on expiry. A descending global deadline
  budget is an idea, not yet a ruling; check what asupersync does.
- "test flake is a bug. make sure to make it impossible." Races must be
  structurally excluded, not retried away.

Further direction during review round 5, 2026-07-31:

- Tool facts are recorded by both brain and limb, split by ownership:
  "Likely both brain and limb should be recording stuff about tools. Brain
  needs to say when a tool call is detected in response and needs to know
  when a tool result is going in the context / to the model, whereas limb
  is more 'in charge' of the actual execution (or not) of tool calls.
  Definitely hostname is a fact that comes from limb." Concretely: the limb
  holds the shared-state handle and appends execution facts (ToolStarted);
  the brain appends context facts (request outcomes, tool results entering
  the model view).
- The TUI is the face's external world, not its innards: "The TUI
  (stdin/stdout) is conceptually separate from the *face process* due to
  exactly this reason - synchronous takeover etc. ... Rendering != face
  innards." Synchronous tty takeover (the /dump editor) and file reads are
  the face's owned in-flight work; the face loop keeps selecting, buffers
  display output during takeover, and is never blocked blind.

Deferred replication, topology, peer-lifecycle, and shutdown inputs are
curated in `spikes/event-streaming-notes.md`.
