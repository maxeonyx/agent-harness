# Spike Brief: walking-skeleton

Revised at Gate 1, 2026-07-30 (redo). The first attempt proved the wire
format and the append/trigger split but was a single blocking thread; the
user's Gate 1 direction: the skeleton's goal is to have something for the
rest of the work to build on — real provider, real minimal interface, real
code and provisional abstractions — and it needs async I/O from the start.
The first attempt's evidence and review findings are in git history
(`walking-skeleton-outcome-v1.md`, retired).

Thesis: a minimal evented face+brain+limb harness can run end-to-end in one
process with async I/O and at least two select loops — the face loop (user
input in, event rendering out) and the agent loop (one session's model /
tool-call loop) — communicating by events. An event is about its emitter,
not *for* anyone; consumers (or a helpful middle layer) do projections: the
face projects events for display, context building projects events into
context entries. Interface events != context entries — the agent loop has
different event building blocks than the user interface. Context appending
stays decoupled from request triggering; user activity arriving while the
agent loop is mid-flight piggybacks on the next request (per source-notes
`context-and-agent-loop.md`) and never splits a `tool_calls` message from
its `tool` result. Cancellation is baked in from the start as an explicit
request → drain → finalize protocol with four-valued outcomes
(ok / err / cancelled / panicked) — cancelled is not an error, and nothing
in flight ends without a recorded outcome (modeling inspiration:
Dicklesworthstone/asupersync; runtime is tokio — evaluating asupersync
itself is a candidate later experiment).

Tool calls, concretely, as events: tool call starts → event (face shows it);
the agent loop waits on tool completion, or cancellation, or further user
events; tool call finishes → event (face shows it), and the tool-call
context entry is appended.

The context lifecycle exists as provisional abstractions: append, rebuild,
and view are distinct operations. Rebuild exists as a real operation but
carries no compaction policy (later spike). The recorder records facts, not
intentions: attempts, completions, failures, and cancellations with
correlation, never a `request_sent` written before the request succeeds.

The provider boundary is unchanged from v1: OpenAI-compatible
chat-completions over plain HTTP; one adapter, real endpoint or fake
provider by base URL. The fake provider stays a separate HTTP server and
additionally scripts bash `sleep` tool calls so concurrent user and agent
actions are actually exercisable.

Falsified if: the two loops cannot stay responsive (user events during an
in-flight request or running tool are dropped, block, or force a request);
piggybacked activity cannot be ordered correctly on the wire; cancellation
cannot resolve an in-flight tool call to a definite recorded outcome with
the session returning to a usable state; or the event/projection split
collapses (consumers end up needing emitter-specific handling that
projection was supposed to absorb).

Invariants touched: 2 (append / rebuild / view as distinct operations —
visible in the code architecture this time), 3 (multiple views, as
reworded), 4 (face/brain/limb as logical roles; the brain talks to faces
only via events), 9 (cancellation protocol), 8 (everything stays in
`experiments/`; this is scaffold, not core). Invariant 1 is deliberately
out of scope for the skeleton (user direction: details; something more
principled later).

Out of scope (user direction, 2026-07-30): provider streaming, full
error-case variety in the fake provider, per-model / per-interface view
variation, tool-call robustness (partial failure recovery), SQLite,
subagents, compaction policy.

Exit condition: scripted scenarios pass against the fake provider —
(a) the v1 scenario still holds: passive activity appends without
triggering, ending the turn triggers exactly one request carrying the
accumulated context; (b) concurrency: while a scripted bash `sleep` tool
call runs, user activity arrives, the face remains responsive, and the
activity piggybacks on the next request without splitting the tool-call
exchange; (c) cancellation: a cancel during an in-flight tool call drains
and finalizes to a recorded cancelled outcome, visible at the face, with
the session usable afterwards — plus a manual smoke session against a real
provider endpoint.
