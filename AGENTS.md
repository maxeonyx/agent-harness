# agent-harness

A personal agent harness. Max's vision is the gist https://gist.github.com/maxeonyx/014463e0964bebd0add4b914971c492f — his own notes, and the driver. Derive what he wants from it; don't treat a sentence in it as a rule.

The repo was reset on 2026-10-16 (NZT). The earlier attempt is the unconnected branch `v1` (plus `forked-subagents`, `handoffs`, `provider-cache-probe`): evidence of what earlier agents tried and measured, none of it accepted by Max, and none of its process or framing carries over.

## What is settled

- The full vision has many aspects that interact pairwise, so it is built up incrementally: prototype a part against only a few of the other aspects, accept imperfect products, integrate, and repeat until the whole behaves well along every dimension. (High confidence.)
- No permission prompts in the harness. An environment is defined with its permissions; extending them means changing that definition, possibly by another agent. (High confidence; he calls prompts a bad abstraction.)
- No tests in the repo except truly black-box end-to-end scenarios Max can run and watch as a demonstration. Otherwise agents test manually by using the harness. (High confidence.)
- The goal is foundational software as well as Max's own harness: it covers all the essential complexity and relentlessly minimises everything beyond it, across the whole vertical stack. (High confidence; rewriting down to the machine code is a far stretch goal.)

## How the work runs

The process is the harness's own agent model, run by hand on Claude Code until the harness can run it; then the work moves onto the harness.

- **Tasks and scopes.** Each piece of work is a task given to one agent, ending in a result: its last message, saying what the parent needs to know. A parent launches a scope of tasks and makes no further decision until every result is in. A task may add siblings to its own scope.
- **Roles** (`.claude/agents/`). The orchestrator is the session Max starts, at medium effort or lower; it holds the direction and launches every scope. A `designer` (xhigh) decides a model; an `engineer` (medium) builds. Only the orchestrator launches designers, because design at high effort everywhere burns Max's usage before he gets what he wants.
- **Direction comes from design; execution is cheap.** What to build next is back-chained from the vision to what Max actually wants. Building is fail-fast: try to get it right first time, expect to have built the wrong thing, try again. Checks are `cargo fmt` and `cargo clippy`, nothing more.
- **Models first.** The core concepts are Rust types in one crate; everything else is written in terms of them. When a mechanism is built, ask how the same concept applies to every other aspect: ideally each concept exists once. An engineer who finds the model wrong, or finds itself building a second mechanism for an existing concept, stops and returns that as its result instead of redesigning.
- **Spikes, then integration.** A spike exercises one thesis against a few aspects, on a branch, and is never merged: its result carries what it learned. Integration into `main` is clean and complete, and deletes what it replaces.
- **Parallel streams.** Each engineer works in its own clone, `~/agent-harness-workspace/ah-<branch>`, owns the region of the repo its task names, and merges its own PR with a merge commit. A conflict outside that region is returned to the orchestrator, not resolved.
- **Short contexts.** A task is sized to fit one context. The orchestrator keeps its state in `ORCHESTRATOR.ignore.md` in its clone, so a fresh orchestrator resumes from that and `git log`.
- **Max's time.** The orchestrator brings him only what he alone knows: materially different shapes, how consequences rank, what he actually wants. It batches these. Every integration lands with a demo under `demos/` that he can run and watch.

## Bx

Max's build-system design, Buix: https://gist.github.com/maxeonyx/fa2f14a7fae0acc46bdafcb08b7cc58a. He thinks it can share a lot with the harness and is happy to be proven wrong (medium confidence, untested). A designer deciding a core model takes Bx as a second input.
