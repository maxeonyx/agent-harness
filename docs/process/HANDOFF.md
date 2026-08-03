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
- Design scoping is **complete through all four stages for all fourteen
  experiments**. `design/README.md` defines the method (why → what →
  interactions → summary, top-down-bottom-up), the review-provenance
  convention, and the doc conventions. Every pool entry has a doc with a
  summary, why, what, interactions and an index.
- **The docs are unevenly settled and say so.** Each doc's `Stages:` line
  records which stages were drilled with the user and which are
  agent-drafted. The whys of the four soul designs plus self-modification
  are user-involved; everything else is agent-drafted and unreviewed.
  Every doc ends with "Questions for review".
- `design/INTERACTIONS.md` is the portfolio view: machinery several
  experiments need and none owns (cache-state prediction being the
  largest), places where two designs genuinely disagree, and which parts
  of the matrix are empty.
- Next: **the user reviews the design docs**, which is expected to produce
  iteration on the agent-drafted stages. Then briefs, then experiments —
  `PLAN.md`'s "Readiness and dependencies" section carries what can start
  in parallel and what is blocked on what. Scheduling notes: user-turn
  needs the user hands-on; forked-subagents is the one he'd "quite like to
  see designed" and is now designed.

## Where things are

| Concern | Location |
|---------|----------|
| Development process and gates | `PROCESS.md`, `EXPERIMENT_RULES.md` |
| Requirements, invariants, rulings (current truth) | `REQUIREMENTS.md` |
| Experiment pool, status, readiness and dependencies | `PLAN.md` |
| Per-experiment design docs, and the method that produced them | `design/` (`design/README.md` first) |
| Cross-cutting interactions, shared machinery, design conflicts | `design/INTERACTIONS.md` |
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
- The design docs raise their own open questions; those live in each doc
  rather than being duplicated here. The ones that affect more than one
  design are in `design/INTERACTIONS.md` under "Where two designs actually
  disagree" — notably topology's centralise-once why against its
  federation why, both of which are the user's.
- Whether `context-updates` should move from the good-taste bucket to the
  unique-soul bucket: its own doc argues user-turn and self-modification
  make routine mid-session change a soul concern rather than a
  nice-to-have. Left in place pending the user's ruling.
- Whether `layered-shutdown` should be an experiment at all, or a pattern
  note folded into operator-lifecycle and topology. Its own design work
  concluded the latter.
- Whether `topology` and `modular-components` should merge. Both docs now
  argue against merging, on the grounds that their falsification surfaces
  differ and a merged failure would be unattributable.
