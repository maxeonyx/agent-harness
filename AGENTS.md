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
2. Define black-box tests or review criteria.
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
| `tests/` | Black-box process and behavior tests |

Spike code must not become accidental core architecture. Core integration only
happens after the relevant spike outcome exists and behavior is represented by
black-box tests.

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
