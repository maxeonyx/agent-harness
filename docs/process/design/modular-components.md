# Modular components — construction, config and testability — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why (agent-drafted, unreviewed) · what, interactions, summary — not yet done.** User-requested, 2026-07-30, wording preserved: "an experiment which is focused on clean, modular components especially with regard to testing and config". Derives from that request, the `deconfuse` and testing references in `source-notes/reference-codebases.md`, and the engineering discipline section of `REQUIREMENTS.md`.

The thesis: the harness's components are ordinary objects, constructed from composable typed config, with all I/O injected at construction — so the same components compose into an in-process test harness or an out-of-process deployment, and neither is a special case.

## Why

### 1. A test suite only protects you if you actually run it — *friction*

The walking skeleton's black-box scenarios spawn two binaries. That works and it is honest, but it is slow, and the target here is the whole suite well under a second. The root is not tidiness. A suite that takes long enough to be annoying gets run less, then gets run only in CI, then stops being the thing that tells you whether you just broke something. Speed is what keeps a test suite load-bearing.

This matters more than usual in this project, because self-modification means an *agent* is editing the harness and needs to know within seconds whether it just broke it. Slow tests degrade the self-modification loop directly.

### 2. Determinism must come from construction, not from patching — *correctness*

The user's testing guidelines are explicit: black-box first through public surfaces, injected implementations, never mock or patch. And the standing rule that test flakes are bugs — races structurally excluded, not made unlikely or retried away.

The root is that mock/patch achieves determinism by reaching *inside* the thing under test, which contradicts testing it as a black box. You end up with tests coupled to internals, which break on refactors that changed nothing observable, and which pass when the real composition is broken. Injection gets the same determinism without the coupling: the component takes a clock, an environment, a filesystem, a provider endpoint, and the test supplies real implementations that happen to be controllable.

### 3. If in-process composition is a test-only path, it is a lie — *correctness*

This is topology's why #3 arriving from the other direction. In-process wiring must be *just another composition of the same components*, exercised by real deployments and not only by tests. Otherwise the fast test suite validates a code path that no user ever runs, which is worse than having no fast suite: it is confidence pointed at the wrong thing.

The two experiments therefore share a thesis and a falsification condition, and are strong candidates to merge.

### 4. A component that knows its construction context cannot be built twice — *correctness*

The config model follows the user's `deconfuse` library in Rust terms: a typed schema defined once, explicit ordered sources, recursive merge for nested components, parent→child propagation without globals, and injectable environ and argv.

The no-globals rule looks like taste and is not. If a component reads a global, or an environment variable, or argv directly, then two instances of it cannot coexist in one process with different configuration — which is exactly what roots #1 and #3 require. Ambient state is what makes in-process composition impossible. So propagation-without-globals is *forced* by the goal, not chosen for elegance.

### 5. The user may want this beyond the harness — *desire, hedged*

Recorded with the hedge intact, because it is a live decision rather than a settled one: "Perhaps this would be its own library in the agent-tools ecosystem though, actually - it's useful for all my projects." The experiment should produce evidence for that call rather than presuming it either way.

## Forward: what these roots force

- **Everything ambient becomes a constructor parameter** — clock, environment, argv, filesystem, network, provider base URL, and any source of randomness or identity.
- **The fake provider is a component, not a fixture.** It already exists as a separate HTTP server serving the same OpenAI-compatible API, so real versus fake is just a base URL. In-process composition means it must also be constructible in-process without changing what is being tested.
- **A typed config schema with explicit ordered sources**, and a recursive merge that composes for nested components — the Rust port of deconfuse's model, which is itself a deliverable worth comparing back against the Python original.
- **The assertion surfaces must be reachable in-process without asserting internals.** This is the sharp falsification: if the in-process form can only be tested by reaching inside, the thesis fails.
- **Setup follows the same rule as assertion.** Per the test boundary in `AGENTS.md`, tests may create external conditions a user could bring — files, repos, env vars, fake endpoints — but must drive the harness through its public surfaces, including its own setup. Injection must not become a back door for seeding sessions or context directly.

## Parked for later stages

**Falsification, as the user framed it:** the thesis fails if in-process tests cannot reach the same assertion surfaces without asserting internals, or if config forces components to know their construction context, or if determinism needs mock/patch seams.

**Exit condition:** the walking-skeleton scenario suite running fully in-process *and* still composable into the two-binary CLI form, plus a written comparison with deconfuse. Touches invariants 4, 8 and 10.

**Also relevant from the notes:** `trunc` should gain a library mode and be used by default for command output, with the agent required to supply a grep term. That is a component with exactly this shape — injectable, configurable, used by the limb — so it is a natural first citizen of this model.

**Interactions flagged for stage 3:** topology (the same composition problem from the deployment side — merge candidate); limb-model (context layer composition is the same recursive-merge problem applied to context rather than config, which `PLAN.md` already flags as an overlap and merge candidate); self-modification (the shell provides construction frameworks, the soft middle provides implementations — this design decides what a plugin is *constructed* as); persistence-analytics (an injected database is what makes storage tests fast); multi-client-ui (two faces in one test process is only possible under this model).

## Questions for review

- Should this merge with topology? They share a thesis, and proving one without the other seems to leave the claim half-tested.
- Should the deconfuse port be a separate agent-tools library from the outset, given you already suspect it wants to be one — accepting that the harness then depends on an immature library?
- Does "whole suite well under a second" survive contact with a real provider requirement, or does it apply only to the fake-provider suite?
