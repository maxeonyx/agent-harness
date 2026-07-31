# Process Handoff

What this is: the entry point for a fresh agent — current position and
pointers only. History lives in git and the spike outcome docs; never
accumulate chronology here.

## Current position

- Spike 0 (walking skeleton) is **done** — accepted at Gate 1, 2026-07-31.
  The accepted harness lives in `experiments/walking-skeleton/` (the user
  runs it against real providers); its evidence and rulings record is
  `spikes/walking-skeleton-outcome.md`.
- Current truth: requirements and invariants in `REQUIREMENTS.md`; the
  provisional experiment plan in `PLAN.md`.
- Next: revise `PLAN.md` against the Spike 0 evidence (a planning session
  with the user — the plan was extracted, not yet revised), then run the
  next experiment. The user wants a few experiments before first core
  integration, user-facing ones first; Spike 1 (user-tool context
  contract) is the leading candidate.

## Where things are

| Concern | Location |
|---------|----------|
| Development process and gates | `PROCESS.md`, `SPIKE_RULES.md` |
| Requirements, invariants, rulings (current truth) | `REQUIREMENTS.md` |
| Experiment plan and status (provisional) | `PLAN.md` |
| Spike briefs/outcomes | `spikes/` |
| Deferred event-streaming design inputs | `spikes/event-streaming-notes.md` |
| User's design notes (verbatim source; sync procedure in AGENTS.md) | `../source-notes/` |

## Source notes sync state

Gist `014463e0964bebd0add4b914971c492f`, last resynced 2026-07-31:
`requirements.md` was removed from the gist and the local copy (it was
AI-derived planning material, not source; redistributed into
`REQUIREMENTS.md` and `PLAN.md`).

## Open questions

- The eventual user-facing command name is undecided.
- OpenAI-compat tool-call encoding is verified against OpenRouter only.
- Modular-components: standalone library vs harness infrastructure
  (user decision pending; see PLAN.md).
