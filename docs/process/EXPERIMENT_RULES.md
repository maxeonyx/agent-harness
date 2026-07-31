# Experiment Rules

Experiment code is disposable evidence.

Rules:

- Start from a one-paragraph brief in `docs/process/experiments/<name>-brief.md`:
  thesis, what evidence would falsify it, which invariants it touches.
- Put experiment implementation under `experiments/<experiment-name>/`.
- No mid-experiment gates and no tests-first requirement; the experiment must end with
  runnable evidence plus an outcome document.
- Keep tests that express durable behavior outside the experiment directory,
  at the public surfaces listed in `PROCESS.md`.
- Do not import experiment modules from `src/`.
- Do not promote behavior to core until the outcome document exists and the
  user has accepted it (Gate 1).
- Outcome documents live in `docs/process/experiments/` and use
  `docs/process/EXPERIMENT_OUTCOME_TEMPLATE.md`.
