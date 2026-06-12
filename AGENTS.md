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

Fixture setup follows the same rule for product-owned state. Tests may create
external conditions a user could bring to the tool, such as files, repos,
environment variables, terminals, or fake provider endpoints. They must drive
the harness itself through the public command/UI/API, including initial setup.
Do not seed sessions, queues, context, providers, or workspace runtime state by
calling private constructors or writing internal storage directly.

## Source Note Sync

The design gist `014463e0964bebd0add4b914971c492f` is the upstream for
`docs/source-notes/`. To merge new spec/vision changes:

1. Clone the gist and diff it against `docs/source-notes/`.
2. Copy changed and new files verbatim, keeping the exact gist filenames
   (including any without an `.md` extension) so future diffs stay clean.
3. Update the source-notes version line and evidence in
   `docs/process/HANDOFF.md`, and fix any process references to notes that
   moved or were renamed in the gist.
4. Bump the patch version in `Cargo.toml`, `Cargo.lock`, and
   `docs/version.json` — the release guard requires a new version per push.
5. Run the commands below under `devenv` before committing.

Source notes stay verbatim. Curation happens in `docs/process/`, never by
editing the imported files.

## Current Active Loop

See `docs/process/HANDOFF.md`.

## Commands

```bash
cargo ratchet
cargo fmt --check
cargo clippy -- -D warnings
```

Plain `cargo test` is blocked by the gatekeeper test for the core package; use
`cargo ratchet`. Disposable spikes under `experiments/` may define their own
local test workflow.

From the workspace root, also run:

```bash
cargo test -p standards
```
