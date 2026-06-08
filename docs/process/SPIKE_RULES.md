# Spike Rules

Spike code is disposable evidence.

Rules:

- Put spike implementation under `experiments/<spike-name>/`.
- Write or choose black-box tests before implementation.
- Keep tests that express durable behavior outside the experiment directory.
- Do not import experiment modules from `src/`.
- Do not promote behavior to core until a spike outcome document exists.
- Outcome documents use `docs/process/SPIKE_OUTCOME_TEMPLATE.md`.
