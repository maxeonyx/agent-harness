# Agent Harness Process Handoff

Status: Spike 0 (walking skeleton) built; evidence complete
Active loop: Spike 0 awaiting Gate 1 (user acceptance of the outcome doc)
Source notes version: gist `014463e0964bebd0add4b914971c492f` cloned 2026-06-08,
resynced 2026-07-30 (gist revision `084e2d3`, Anthropic OAuth references added)

## Current Position

The external design notes are imported in `docs/source-notes/`. The
development process was reworked on 2026-06-13 — operationally in
`docs/process/PROCESS.md` — replacing the heavy per-spike gate machinery with
two gates (spike acceptance, integration acceptance), an invariants list in
`docs/process/REQUIREMENTS.md`, and incremental integration starting from a
walking skeleton.

The repo has explicit boundaries:

- `experiments/` is for disposable spikes (briefed, evidence-out, no mid-spike
  gates).
- `src/` is not a dumping ground for spike architecture.
- Durable tests target the product-public surfaces listed in `PROCESS.md`:
  CLI/UI, the provider wire via fake provider, the storage/query surface, and
  eventually the transport protocol.

## Next Recommended Loop

Gate 1 for Spike 0, then the first small integration.

1. User reads `docs/process/spikes/walking-skeleton-outcome.md` (optionally
   after a fresh-context review) and accepts, redoes, or discards. The
   real-provider smoke check is already done (user-run against OpenRouter,
   2026-06-13, working well).
2. On acceptance: pick the first core slice to integrate (fresh design from
   the evidence, black-box tests first at the public surfaces), or the next
   spike if integration is premature.

Scope note: the user expanded Spike 0 scope on 2026-06-13 — real provider use
is in scope for spikes ("I want to actually use it"); the fake provider for
tests must be a separate HTTP server serving an OpenAI-compatible API. The
spike implements both behind one adapter (real vs fake is just a base URL).
The gist's `requirements.md` §3 now reflects this (real provider in scope),
resynced into `docs/source-notes/` on 2026-06-20.

## User-Gated Decisions

- Gate 1 (spike acceptance) and Gate 2 (integration acceptance) always involve
  the user; see `PROCESS.md`.
- Gate 1 for the walking-skeleton outcome doc is open now.

## Do Not Integrate Yet

- Do not promote the spike's provider client to `src/` as-is; core provider
  adapters are a fresh design (spikes may use real providers freely).
- Do not build a real TUI or GUI.
- Do not choose the final face/brain/limb transport.
- Do not turn `experiments/` code into `src/` code without an accepted spike
  outcome and promoted black-box tests at the public surfaces.

## Evidence So Far

- Source notes imported; process handoff created.
- Subrepo created at `https://github.com/maxeonyx/agent-harness`; CI, release
  pipeline, Pages, and the TDD ratchet gatekeeper are in place and green
  (v0.1.9 released 2026-06-12).
- Source notes resynced from the gist on 2026-06-13: the task-handoff design
  moved to `handoff-improvements.md` and was expanded; `user-turn.md` now
  states GUI/web support is built in from the start; new notes
  `reference-codebases.md`, `stretch-goals.md`, and `tui-styling.md` were added.
- Source notes resynced from the gist on 2026-06-20: new notes
  `federated-brain.md`, `open-code-inspiration.md`, and `throbber-design.md`
  added; `reference-codebases.md` expanded (Oh My Pi, re-implement-don't-copy
  rule); `tui-styling` renamed to `tui-styling.md`. The AI-authored
  implementation process plan was removed from the gist's `process.md` (it
  lives only in `docs/process/PROCESS.md` now); the gist's `process.md` keeps
  only the user's own informal process notes.
- Source notes resynced from the gist on 2026-06-20 (gist `8491f05`):
  `requirements.md` §3 Spike 0 now states the walking skeleton runs against a
  real provider in scope (same binary, plain HTTP, real endpoint or fake
  provider serving the same API), reconciling the spec with the 2026-06-13
  scope expansion already recorded in `walking-skeleton-brief.md`.
- Source notes resynced from the gist on 2026-07-30 (gist `084e2d3`, authored
  2026-07-24): new note `anthropic-oauth-references.md` — reference
  implementations for getting a Claude subscription (OAuth) working with
  third-party harnesses; no other notes changed.
- Process reworked on 2026-06-13 (gist + repo in the same change): two gates,
  invariants, walking-skeleton-first, tests-first scoped to core at public
  surfaces, test primitives extracted from spikes rather than pre-built.
  Curated requirements extracted to `docs/process/REQUIREMENTS.md`.
- Earlier history (initial scaffold through CI baseline) is in the git log of
  the subrepo; commits `235358d` → `857676d`.
- Spike 0 built on 2026-06-13 in `experiments/walking-skeleton/`: face+brain+
  limb in one process, OpenAI-compatible provider client, separate
  fake-provider HTTP server, JSONL recorder. The exit-condition scenario
  passes (`cargo test` in the spike dir); outcome doc at
  `docs/process/spikes/walking-skeleton-outcome.md`.

## Standards Backlog

The first standards run passed these onboarding-relevant checks for
`agent-harness`:

- workspace routing
- vision and process
- OpenCode skill docs
- tests present
- code standards
- fast/slow checks

Known remaining failures:

- Auto-update integration is not implemented.
- Manual attestations are missing for agentic concerns.
- Release, CI-green, Pages, version artifact, and install-link checks need the
  public DNS/custom-domain cycle. Release and GitHub Actions Pages deployment
  now exist.
- Standalone publishability still needs the shared workspace path dependency
  story to be resolved.

## Open Questions

- The eventual user-facing command name is still undecided.
- OpenAI-compat tool-call encoding is verified against OpenRouter (user smoke
  run); other endpoints (Anthropic compat, OpenAI direct, local) unverified.
- Task-handoff design (`docs/source-notes/handoff-improvements.md`) is source
  material, not an implemented API contract.
