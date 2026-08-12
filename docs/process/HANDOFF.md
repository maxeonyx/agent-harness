# Process Handoff

What this is: the entry point for a fresh agent — current position and
pointers only. History lives in git and the experiment outcome docs; never
accumulate chronology here.

## Current position

- The walking-skeleton experiment is **done** — accepted at Gate 1,
  2026-07-31. The accepted harness lives in
  `experiments/walking-skeleton/` (the user runs it against real
  providers); its evidence record is
  `experiments/walking-skeleton-outcome.md` (under `docs/process/`).
- Current truth: requirements, invariants, and the user's design
  decisions in `REQUIREMENTS.md`; the experiment pool in `PLAN.md`.
  Experiments are a pool, not a sequence — the next one is pulled based
  on what the user wants to work on.
- Design docs are being **written one at a time, with the user**, per
  `design/AGENTS.md`. (A first generation was deleted 2026-08-12 —
  unreviewed agent output; in git history.) Current doc:
  context-updates. Then briefs, then experiments.

## Where things are

| Concern | Location |
|---------|----------|
| Development process and gates | `PROCESS.md`, `EXPERIMENT_RULES.md` |
| Requirements, invariants, design decisions (current truth) | `REQUIREMENTS.md` |
| Experiment pool | `PLAN.md` |
| Design docs and how to write them | `design/` (`design/AGENTS.md` first) |
| Experiment briefs/outcomes | `experiments/` (under `docs/process/`) |
| Deferred event-streaming design inputs | `experiments/event-streaming-notes.md` |
| User's design notes (verbatim source; sync procedure in AGENTS.md) | `../source-notes/` |

## Open questions

- The eventual user-facing command name is undecided.
- OpenAI-compat tool-call encoding is verified against OpenRouter only.
- `tests/gatekeeper.rs` still says "spikes" in its message; fold the
  wording fix into the next code-touching change (it needs a version bump
  per the CI release guard, not worth one alone).
