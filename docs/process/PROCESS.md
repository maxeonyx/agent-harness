# Agent Harness Development Process

This is the operational process for this repo. The upstream rationale lives in `docs/source-notes/process.md`; this file maps it onto how work actually runs here. If they conflict, fix one of them — do not improvise a third process.

## Principle

Spend rigor where mistakes are expensive. Experiment code is cheap to be wrong about — that is its value. Core is where mistakes compound. The process is near-frictionless before promotion into core, then strict at promotion.

## Live artifacts

| Artifact | Location |
| --- | --- |
| Curated requirements + invariants | `docs/process/REQUIREMENTS.md` |
| Experiment briefs and outcome docs | `docs/process/experiments/` |
| Process handoff | `docs/process/HANDOFF.md` |
| Raw imported design notes | `docs/source-notes/` (verbatim, never edited locally) |

## The loop

1. **Pull the next experiment from the pool** (`PLAN.md`) — the user picks based on what they want to work on next. Write a one-paragraph experiment brief in `docs/process/experiments/<name>-brief.md`: thesis, what evidence would falsify it, which invariants it touches.
2. **Experiment freely** under `experiments/<name>/` per `EXPERIMENT_RULES.md`. No mid-experiment gates, no tests-first requirement. Timeboxed. Ends with runnable evidence plus an outcome doc (`EXPERIMENT_OUTCOME_TEMPLATE.md`).
3. **Gate 1 — experiment acceptance.** The user reads the outcome doc against the invariants in `REQUIREMENTS.md`, optionally with a fresh-context review session first. Accept, redo, or discard.
4. **Integrate small and soon.** Promotion means: re-design the core slice fresh from the evidence (never copy experiment code), write black-box tests first at the public surfaces below, implement the smallest coherent slice, keep `cargo ratchet` green.
5. **Gate 2 — integration acceptance.** Tests green, invariants checked (expanded checklist: the integration expectations at the end of `PLAN.md`), fresh-context review accepts, user accepts.
6. Update `HANDOFF.md`. Go to 1.

Do not batch integrations. Alternate experiment → small integration → experiment.

## Public test surfaces

Durable black-box tests target the product-public surfaces:

- the CLI / UI behavior
- the provider wire boundary, via a fake provider: which API requests were actually sent, what triggered them, and what context they contained
- the durable storage and query surface: analytics queries are product behavior, not internals
- the face/brain/limb transport protocol, once it is public

Internal events, private queues, and actor messages remain off-limits for durable tests. Test primitives (fake provider, fake workspace, scenario runner) are extracted from the first experiments once real pressure exists, not designed up front.

## Review mechanics

Adversarial review is a fresh-context session (a subagent or a new Claude Code session) that has not seen the implementation work, used at exactly the two gates. It either accepts or returns findings; it does not fix. The user is the standing reviewer at both gates.

## When stuck, go backwards

"Do not implement forward through uncertainty" is a triage rule, not a permission system: implementation uncertainty → back to design; behavior too underspecified to evidence → back to the brief or `REQUIREMENTS.md`; design contradicts an invariant → stop and ask the user; experiment pressure reveals a requirements gap → update `REQUIREMENTS.md` (the full stakeholder sweep happens there, not per step).

## Ask the user when

- invariants or stakeholder requirements conflict
- an experiment suggests the architecture direction is wrong
- an integration would significantly narrow the product
- behavior is too underspecified to test
- a review rejects a gate and the way back is not obvious

Not merely because implementation is hard.
