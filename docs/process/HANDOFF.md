# Agent Harness Process Handoff

Status: process reworked to the two-gate model; no product behavior implemented
Active loop: ready to start Spike 0 (walking skeleton)
Source notes version: gist `014463e0964bebd0add4b914971c492f` cloned 2026-06-08,
resynced 2026-06-13 (gist revision of 2026-06-13)

## Current Position

The external design notes are imported in `docs/source-notes/`. The
development process was reworked on 2026-06-13 — upstream in the gist
(`process.md` second revision) and operationally in
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

Spike 0: walking skeleton (see source-notes `requirements.md` §3).

1. Write the brief in `docs/process/spikes/walking-skeleton-brief.md`.
2. Build a toy face+brain+limb loop end-to-end against a fake provider:
   single process, append-only CLI, user-tool context append path, agent-tool
   call path, simple recorder.
3. Evidence target: a scripted scenario where user activity appends context
   without triggering a request, a turn end triggers a request to the fake
   provider with the accumulated context, and an agent tool call round-trips.
4. Write the outcome doc; bring it to Gate 1.

## User-Gated Decisions

- Gate 1 (spike acceptance) and Gate 2 (integration acceptance) always involve
  the user; see `PROCESS.md`.
- The walking-skeleton brief is the first artifact to confirm with the user
  before building.

## Do Not Integrate Yet

- Do not build a real provider adapter.
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
  `reference-codebases.md`, `stretch-goals.md`, and `tui-styling` were added.
- Process reworked on 2026-06-13 (gist + repo in the same change): two gates,
  invariants, walking-skeleton-first, tests-first scoped to core at public
  surfaces, test primitives extracted from spikes rather than pre-built.
  Curated requirements extracted to `docs/process/REQUIREMENTS.md`.
- Earlier history (initial scaffold through CI baseline) is in the git log of
  the subrepo; commits `235358d` → `857676d`.

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
- The exact shape of the fake provider (which provider API it mimics first,
  and how strictly) is a Spike 0 design question.
- Task-handoff design (`docs/source-notes/handoff-improvements.md`) is source
  material, not an implemented API contract.
