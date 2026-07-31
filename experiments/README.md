# Experiments

Disposable experiments live here.

An experiment exists to make one aspect of the design excellent in isolation:
one core flow, proposition, code path, code style, protocol shape, or other
focused thesis. It may deliberately ignore the rest of the product shape so the
targeted question can be answered cleanly.

Rules:

- Each experiment gets its own subdirectory.
- Each experiment must name the specific thesis it is testing.
- Each experiment must state what it explicitly does not support.
- Each experiment starts from a one-paragraph brief under
  `docs/process/experiments/`: thesis, what evidence would falsify it, which
  invariants it touches. No tests-first requirement inside the experiment —
  it must simply end with runnable evidence.
- Experiment code is reference material only. Do not port it wholesale into
  core.
- Core integration reimplements the proven behavior deliberately, using the
  experiment outcome as evidence and design input.
- Each experiment must produce an outcome document under
  `docs/process/experiments/` before any behavior is considered for core
  integration.
- The outcome document states what to integrate, what not to integrate, and what
  the experiment revealed about the larger design.
