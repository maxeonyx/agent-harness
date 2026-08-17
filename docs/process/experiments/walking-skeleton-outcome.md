# Experiment Outcome: walking-skeleton

Experiment: walking-skeleton, rebuilt per the revised brief of 2026-07-30 (the first attempt's Gate 1 verdict was redo; its outcome doc was retired when this one superseded it — see git history). Status: **accepted at Gate 1, 2026-07-31** — "The code is nice, and the harness works perfectly. Spike 0 is done." Requirements tested: none by itself (the walking-skeleton experiment is the shared substrate); exercises invariants 2, 3, 4, 8, 9, 10.

## What the accepted experiment is

`experiments/walking-skeleton/`: a working single-process harness the user runs against real providers (OpenRouter). One shared, journaled `SessionState` (synchronous appends under one lock) is the source of truth; three symmetric participants — face (owns the TUI), brain (owns the provider connection), limb (owns the environment) — each run an {inbox + select loop + owned in-flight work} shape connected by typed channels; main supervises. Eleven black-box scenarios drive it through stdin/stdout and the fake-provider wire, flake-checked in repeated batches.

## What the experiment proved

- The in-band collaboration substrate works end-to-end: appends never trigger requests; turn end triggers exactly one request carrying accumulated context; user activity mid-tool-call stays responsive and piggybacks after the tool exchange without splitting `tool_calls`/`tool` adjacency on the wire (asserted on the fake provider's request log, including _between_ steps).
- Cancellation as request → drain → finalize is buildable and cheap on tokio: four-valued outcomes, every attempt resolves, drain structurally cannot start work, cancel/completion ties resolve deterministically (completed work is kept; the turn still finalizes cancelled), and the limb kills and reaps whole process trees it owns in well under a second.
- Shared-state-plus-typed-channels is sufficient co-located architecture: the event bus the first rewrite carried was removed as premature replication design, and nothing user-visible was lost. Sequencing by substrate (the lock), not by a participant, held up in code.
- The journal (JSONL of appends, seq-validated) supports atomic /rebuild and is latent event sourcing for the later streaming experiment; it also carries the resumable unexecuted-tool-call state.
- Introspection via a shared projection works: /dump renders exactly the request builder's view plus invisible facts as comments, so it cannot silently miss what the model sees.
- Real-provider use (tool round-trips, /cancel, /dump, clean shutdown) works with the same binary as the fake-provider tests — real vs fake is just a base URL.

## Rulings made during review (current truth in REQUIREMENTS.md)

The review loop surfaced design questions the user ruled on; original wording preserved here as the record:

- Cancelled turns keep completed responses, never execute proposed calls: "we should wait for the completion of the assistant response because I don't think there's any point throwing that away. It cost us money, and it's probably good. But I don't think we should execute on the tool calls..."; "the tool call by itself is actually valid, and we could action that later if we wanted"; "we should be able to resume later and then execute the tool call that we had pending."
- Sequencing: "there is no sequencer unless... A and B share a process... we accept there's no total order for the events across the two. We don't synchronize." In one process "we do synchronize, and therefore we do have a sequencer. But the brain is not the sequencer. The brain is another participant." And: "the question is whether to _synchronously append_ or _asynchronously append_. ie. do we wait, or not? If we can synchronously append, we don't need to worry."
- Event streaming deferred: "the event stream needs to be principled, it needs to be structured well in order to be worth it" — simpler hard-coded setup now; the stream is a later experiment (inputs curated in `event-streaming-notes.md`, where the limb "is definitely an event peer!!").
- Symmetry: "definitely, the three should be symmetric, and their roles should be defined before we go later on into the other experiments." "Any provider state is part of the brain conceptually, just like ephemeral UI state is part of the face."
- Tool facts: "both brain and limb should be recording stuff about tools. Brain needs to say when a tool call is detected in response and needs to know when a tool result is going in the context / to the model, whereas limb is more 'in charge' of the actual execution (or not)... Definitely hostname is a fact that comes from limb."
- Face vs TUI: "The TUI (stdin/stdout) is conceptually separate from the _face process_... synchronous takeover etc. ... Rendering != face innards."
- Process ownership: "limb should own and clean up processes on graceful shutdown" (no global process-table observation); container/namespace enforcement a hedged later idea ("do it in some kind of container maybe?").
- Discipline: "test flake is a bug. make sure to make it impossible."
- Shutdown pattern (deferred): "every layer should think about how it's shutting down gracefully in response to a cancellation," with parent timeout backstops and a descending deadline-budget idea; "I don't know what asupersync does here. We should look."

## Review summary

Seven fresh-context "thermonuclear" review rounds. Rounds 1-3 hardened the first rewrite (limb loop ownership, structural drain, process-group lifetime, supervised main, a monotonic close-watch fixing a real ~1-in-10 flake). Round 4's findings triggered the architecture discussion that produced the shared-state rewrite. Rounds 5-7 hardened the rewrite (biased cancel/completion ties both directions, atomic rebuild, drain on failure paths, no fabricated outcomes anywhere, backpressure-cycle removal, id-collision guard, bounded test-harness waits). Round 7's reviewer re-verified the dispositions and returned ACCEPT. Notable process point: several reviewer findings were reversed by user ruling rather than fixed — the findings log is process history, not design truth; rulings live in REQUIREMENTS.md.

## Known accepted limitations

- A read blocked on a writerless FIFO lingers on the blocking pool until a writer appears (tokio spawn_blocking is uncancellable).
- The stdin thread stays blocked on the tty when shutdown does not come through stdin; process exit reaps it.
- The face deliberately waits for the user's editor before exiting.
- True completion-wins-cancel races are not black-box testable deterministically; the guarantee is structural (biased selects).
- "Replay is not a no-op" is not discriminable at the public surface until session resume exists; the rebuild test pins losslessness only.

## What must not be integrated

- Any of this code by copying (invariant 8).
- Env-var-only configuration; the unrestricted bash tool.
- The in-process channel wiring as _the_ transport decision — topology is Experiment 2's question.
- The JSONL journal as the storage design — storage is Experiment 3's question (the journal's append-log shape is a design input, not a decision).

## Tests to promote or preserve

`tests/scenario.rs` — eleven scenarios at the public surfaces (CLI in, face output + provider wire out): append-never-triggers observed between steps; mid-tool responsiveness + piggyback adjacency; cancel drain + session continuation (during a request and during a tool call); descendant process-tree kill and completed-tool descendant cleanup (asserted on fixture-recorded PIDs only); blocked-read cancel; quit-during-tool drain; unexecuted-call wire omission; /rebuild losslessness; /dump content. The interactive harness (send, wait_for, drain_seen, requests-between, graceful-first cleanup through stdin EOF, bounded waits everywhere) is the durable black-box shape.

## User acceptance

Accepted (Gate 1), 2026-07-31: "The code is nice, and the harness works perfectly. Spike 0 is done." Acceptance followed the seven review rounds, the flake-checked scenario suite, and the user's own real-provider use of the harness.
