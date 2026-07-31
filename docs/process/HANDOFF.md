# Process Handoff

What this is: the entry point for a fresh agent — current position and
pointers only. History lives in git and the experiment outcome docs; never
accumulate chronology here.

## Current position

- The walking-skeleton experiment is **done** — accepted at Gate 1,
  2026-07-31. The accepted harness lives in
  `experiments/walking-skeleton/` (the user runs it against real
  providers); its evidence and rulings record is
  `experiments/walking-skeleton-outcome.md` (under `docs/process/`).
- Current truth: requirements, invariants, and the user's
  soul-of-the-design weighting in `REQUIREMENTS.md`; the experiment pool
  in `PLAN.md`. Experiments are a pool, not a sequence — the next one is
  pulled based on what the user wants to work on.
- Next: the user picks the next experiment from the pool. Several
  experiments are expected before first core integration.

## Where things are

| Concern | Location |
|---------|----------|
| Development process and gates | `PROCESS.md`, `EXPERIMENT_RULES.md` |
| Requirements, invariants, rulings (current truth) | `REQUIREMENTS.md` |
| Experiment pool and status | `PLAN.md` |
| Experiment briefs/outcomes | `experiments/` (under `docs/process/`) |
| Deferred event-streaming design inputs | `experiments/event-streaming-notes.md` |
| User's design notes (verbatim source; sync procedure in AGENTS.md) | `../source-notes/` |

## Open questions

- The eventual user-facing command name is undecided.
- OpenAI-compat tool-call encoding is verified against OpenRouter only.
- Modular-components: standalone library vs harness infrastructure
  (user decision pending; see PLAN.md).
- `tests/gatekeeper.rs` still says "spikes" in its message; fold the
  wording fix into the next code-touching change (it needs a version bump
  per the CI release guard, not worth one alone).
