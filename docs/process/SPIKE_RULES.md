# Spike Rules

Spike code is disposable evidence.

Rules:

- Start from a one-paragraph brief in `docs/process/spikes/<name>-brief.md`:
  thesis, what evidence would falsify it, which invariants it touches.
- Put spike implementation under `experiments/<spike-name>/`.
- No mid-spike gates and no tests-first requirement; the spike must end with
  runnable evidence plus an outcome document.
- Keep tests that express durable behavior outside the experiment directory,
  at the public surfaces listed in `PROCESS.md`.
- Do not import experiment modules from `src/`.
- Do not promote behavior to core until the outcome document exists and the
  user has accepted it (Gate 1).
- Outcome documents live in `docs/process/spikes/` and use
  `docs/process/SPIKE_OUTCOME_TEMPLATE.md`.
