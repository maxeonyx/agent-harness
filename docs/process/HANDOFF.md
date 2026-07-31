# Agent Harness Process Handoff

Status: Spike 0 (walking skeleton) is DONE — Gate 1 accepted 2026-07-31
("The code is nice, and the harness works perfectly. Spike 0 is done")
after seven review rounds, a shared-state rewrite, and the user's own
real-provider use
Active loop: pick the next loop — the queued `modular-components` spike
(open user decision in its brief: standalone library vs harness
infrastructure), or first core integration if the user prefers
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

Spike 0 is accepted. The accepted spike (shared mutable `SessionState`,
synchronous journaled appends, typed channels, three symmetric
face/brain/limb participants each owning one external world) is the
evidence base; its eleven black-box scenarios are the preserved test
shapes. Next: run the queued `modular-components` spike, or begin first
core integration (fresh design from the evidence, black-box tests first
at the public surfaces) — user's choice.

Queued next spike: `modular-components-brief.md` (user-requested:
composable typed config + fully in-process black-box testing; deconfuse
and the user's testing guidelines as inspiration). Open user decision
recorded in the brief: standalone library in the agent-tools ecosystem vs
harness infrastructure.

Queued later experiment (user, expanded 2026-07-31): a principled
event-streaming / replication protocol, with the harness innards rebuilt on
top. Curated design inputs are in
`spikes/event-streaming-notes.md`. Until then the skeleton uses shared state
and typed channels, not a broadcast event bus.

Scope note: the user expanded Spike 0 scope on 2026-06-13 — real provider use
is in scope for spikes ("I want to actually use it"); the fake provider for
tests must be a separate HTTP server serving an OpenAI-compatible API. The
spike implements both behind one adapter (real vs fake is just a base URL).
The gist's `requirements.md` §3 now reflects this (real provider in scope),
resynced into `docs/source-notes/` on 2026-06-20.

## User-Gated Decisions

- Gate 1 (spike acceptance) and Gate 2 (integration acceptance) always involve
  the user; see `PROCESS.md`.
- Gate 1 for the walking-skeleton first attempt closed 2026-07-30: redo, with
  direction (see `REQUIREMENTS.md` requirement changes). Gate 1 for the redo
  closed 2026-07-31: accepted (see `walking-skeleton-outcome.md`).

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
- The revised Spike 0 passed scripted scenarios and an agent-run OpenRouter
  smoke before review round 4. That evidence describes the pre-rewrite event
  bus implementation; round 4's rulings and the rewrite disposition are in
  `walking-skeleton-outcome.md`.
- The round-4 shared-state rewrite completed 2026-07-31 and survived review
  rounds 5-7 (round 7 re-verified to ACCEPT by its own reviewer). Eleven
  black-box scenarios, flake-checked in batches; real-provider smokes for
  tool round-trips, /dump, /cancel, and clean shutdown. Gate 1 accepted the
  same day.

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
