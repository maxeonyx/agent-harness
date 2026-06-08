# agent-harness - Agent Instructions

This tool is developed from the `agent-tools` workspace. Work from
`/home/maxeonyx/agent-tools`, not from this repository in isolation.

## Project Status

`agent-harness` is a process-steered experimental workbench for developing an
evented face/brain/limb agent harness. The product is intentionally not being
implemented directly yet.

The source material imported from the design gist lives in
`docs/source-notes/`. Treat those files as source notes, not current truth. The
current process state lives in `docs/process/`.

## Operating Rule

Do not implement forward through uncertainty.

Before product code:

1. Preserve or update the process state.
2. Define black-box tests or review criteria at public UI/API boundaries.
3. Keep disposable experiments under `experiments/`.
4. Keep reusable core code out of `experiments/`.
5. Write a spike outcome document before promoting any behavior into core.

## Directory Boundaries

| Path | Purpose |
|------|---------|
| `docs/source-notes/` | Imported design/process notes from the gist |
| `docs/process/` | Live process state, handoffs, spike plans, and outcomes |
| `experiments/` | Disposable spike code and fixtures |
| `src/` | Minimal executable/core surface only after tests justify it |
| `tests/` | Black-box product behavior tests |

Spike code must not become accidental core architecture. Core integration only
happens after the relevant spike outcome exists and behavior is represented by
black-box tests.

## Test Boundary

Do not test the coding agent harness by asserting internal events, actor
messages, private queues, or control-loop implementation details.

Durable tests must use public UI or public API behavior only. A test may use a
fake model/provider only after the harness exposes a public way to configure
that provider; it must not commit the core to a public event schema before the
product boundary has been designed.

## Current Active Loop

See `docs/process/HANDOFF.md`.

## Commands

```bash
cargo test
cargo fmt --check
cargo clippy -- -D warnings
```

From the workspace root, also run:

```bash
cargo test -p standards
```
