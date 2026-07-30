# Spike Brief: modular-components

Requested by the user, 2026-07-30: "an experiment which is focused on
clean, modular components especially with regard to testing and config."
Inspirations named: the user's testing guidelines (black-box first through
public surfaces; in-process integration with *injected implementations*,
never mock/patch; unit tests only for pure logic) and the user's
`deconfuse` Python library (declare a typed config schema once; explicit
ordered sources; recursive merge for nested components; parent→child value
propagation via `give`; staged partial config; injectable `environ`/`argv`
making sources trivially fakeable in-process).

Thesis: the harness's components (face, brain/session, limb, provider
client, recorder/storage) can be built as ordinary objects constructed
from composable typed config with all I/O injectable at construction —
such that the same black-box scenarios that today require spawning two
binaries can run *fully in-process* with good determinism and performance
(injected clock/time, in-process transport or a loopback HTTP listener in
the same process, injected stdin/stdout for the face), while the
out-of-process wiring (env vars, real terminal, separate fake-provider
server) remains just another composition of the same components. Config
composition should follow deconfuse's shape in Rust terms: define once,
resolve from explicit ordered sources, nested component configs merged
recursively, shared context propagated parent→child without globals.

Falsified if: in-process black-box tests can't reach the same assertion
surfaces as the process-spawning tests (face output, provider wire,
storage queries) without asserting internals; or the config composition
forces components to know about their construction context (globals,
process env reads scattered through component code); or determinism
requires mock/patch-style seams rather than injected implementations.

Invariants touched: 4 (co-location vs splitting as composition of the same
logical components), 10 (no assumed process-local state — config and I/O
arrive explicitly), 8 (spike stays in `experiments/`; anything promoted is
a fresh design).

Open decision (user's, hedged): "Perhaps this would be its own library in
the agent-tools ecosystem though, actually - it's useful for all my
projects." — i.e. whether the config/testing substrate graduates to a
standalone library (a deconfuse-for-Rust sibling tool) or stays harness
infrastructure. The spike should produce evidence for that call, not
presume it.

Exit condition: a version of the walking-skeleton scenario suite (append
never triggers; piggyback; cancel; dump completeness) running fully
in-process — no spawned binaries — deterministically and fast (target:
whole suite well under a second), driving the harness only through public
surfaces, with the components also still composable into the existing
two-binary CLI form; plus a short written comparison of the config
approach against deconfuse's model noting what translated to Rust and what
didn't.
