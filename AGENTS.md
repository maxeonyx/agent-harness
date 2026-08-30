# agent-harness - Agent Instructions

This repository is self-contained for development. A standalone clone must
build, test, and release without an `agent-tools` checkout.

## TDD ratchet — read before testing

Run `cargo ratchet`, not plain `cargo test`. A new test must be red when first introduced and committed as `pending`; that expected red test keeps CI green. A new test must not pass when first introduced—doing so makes the ratchet and CI red. Implement only after the red commit, then rerun the ratchet and commit the promotion to `passing`.

## Integration workflow

Run `devenv test` before committing and pushing; it includes `actionlint`, so
workflow syntax is checked offline. Source CI does not run on push. Open a pull
request, merge current `main` into the feature branch, then explicitly dispatch:

```bash
gh workflow run ci.yml --ref <feature-branch> -f pr_number=<number>
```

The repository-serialized run records the required `Ready` check, builds the
release artifacts, auto-merges the pull request, publishes those same artifacts,
and records `integrated-ci` on the exact merge commit.

## Project Status

`agent-harness` is a process-steered experimental workbench for developing an evented face/brain/limb agent harness. The product is intentionally not being implemented directly yet.

The source material imported from the design gist lives in `docs/source-notes/`. Treat those files as source notes, not current truth. The current process state lives in `docs/process/`.

## Operating Rule

The development process is `docs/process/PROCESS.md`. Its principle: spend rigor where mistakes are expensive. Experiments under `experiments/` are near-frictionless (brief in, runnable evidence + outcome doc out, no mid-experiment gates); promotion into `src/` is strict (fresh design from evidence, black-box tests first at the public surfaces, two user-involved gates). "Do not implement forward through uncertainty" is the triage rule for when work stalls — go backwards to the brief, the design, or `docs/process/REQUIREMENTS.md` — not a permission system gating motion.

## Directory Boundaries

| Path | Purpose |
| --- | --- |
| `docs/source-notes/` | Imported design/process notes from the gist |
| `docs/process/` | Live process state, handoffs, experiment plans, and outcomes |
| `experiments/` | Disposable experiment code and fixtures |
| `src/` | Minimal executable/core surface only after tests justify it |
| `tests/` | Black-box product behavior tests |

Experiment code must not become accidental core architecture. Core integration only happens after the relevant experiment outcome exists and behavior is represented by black-box tests.

## Test Boundary

Durable tests target the product-public surfaces, which for this product are:

- the CLI / UI behavior
- the provider wire boundary, via a fake provider: which API requests were actually sent, what triggered them, and what context they contained
- the durable storage and query surface: analytics queries are product behavior, not internals
- the face/brain/limb transport protocol, once it is public

These are product surfaces — the design's central theses (context lifecycle, request triggering, queryability, topology) are observable exactly there. Do not assert internal events, actor messages, private queues, or control-loop implementation details, and do not commit the core to a public event schema before that boundary has been designed.

Fixture setup follows the same rule for product-owned state. Tests may create external conditions a user could bring to the tool, such as files, repos, environment variables, terminals, or fake provider endpoints. They must drive the harness itself through the public surfaces, including initial setup and pointing the harness at a fake provider. Do not seed sessions, queues, context, providers, or workspace runtime state by calling private constructors or writing internal storage directly.

Experiment code under `experiments/` is exempt from tests-first; see `docs/process/EXPERIMENT_RULES.md`.

## Source Note Sync

The design gist `014463e0964bebd0add4b914971c492f` is the upstream for `docs/source-notes/`. To merge new spec/vision changes:

1. Clone the gist and diff it against `docs/source-notes/`.
2. Copy changed and new files verbatim, keeping the exact gist filenames (including any without an `.md` extension) so future diffs stay clean.
3. Fix any process references to notes that moved or were renamed in the gist. Do not log the sync anywhere — whether source-notes matches the gist is answered by rerunning the diff, not by recorded state.
4. A notes-only sync still goes through the explicitly dispatched integration workflow. Give every integration PR a fresh patch version in `Cargo.toml`, `Cargo.lock`, and `docs/version.json` so the one run can publish and attest its exact merge commit.
5. Run the commands below under `devenv` before committing.

Source notes stay verbatim. Curation happens in `docs/process/`, never by editing the imported files.

## Current Active Loop

See `docs/process/HANDOFF.md`.

## Commands

```bash
cargo ratchet
cargo fmt --check
cargo clippy -- -D warnings
```

Plain `cargo test` is blocked by the gatekeeper test for the core package; use `cargo ratchet`. Disposable experiments under `experiments/` may define their own local test workflow.
