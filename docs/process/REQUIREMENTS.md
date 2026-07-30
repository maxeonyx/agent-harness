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
  with tool schemas) "must be impossible somehow".
