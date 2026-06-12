# process

we'll implement spikes of various bits, *explicitly* marked as experiments, labelled so, under a separate part of the repo. that's for getting the design right, and when we move to a different aspect we do that again. We integrate into core only afterwards, and we do so cleanly, perfectly and minimally - when we integrate into core we leave NOTHING half done.

Each slice/spike should exercise the thesis of that slice. The good thing about these spikes is that they *don't* have to fit into the whole - so we can eg. go without evented model, or go without multiple UIs, or go without plugins, etc. in these spikes.

We set up the repo first. development process before product, always. we set up checks like linting, formatting, version checks, CI checks etc, before we start implementing. We do our best to do things right the first time, and when it inevitably goes a bit wrong, we improve it for next time.

The point of the process is getting things right, tested, and cleanly integrated - not the process itself.

---

# Agent Harness Implementation Process Plan

Status: process plan, second revision
Purpose: define how implementation of the agent harness proceeds without collapsing the design into a premature MVP, and without the process eating the project.

## Core principle: spend rigor where mistakes are expensive

Spike code is cheap to be wrong about - that is its entire value. Core is where mistakes compound. So the process is near-frictionless before promotion into core, then strict at promotion: black-box tests before core implementation, always; fresh design, never copied spike code; review at exactly the points where being wrong is costly.

The first revision of this plan wrapped every spike in design/test/implementation/adversarial-review gates, ran a full stakeholder-requirements sweep after every step, prescribed five named subagent roles, and batched all integration into a final core phase - maximizing the very "quadratic component interaction matrix" problem it worried about. This revision replaces that with two gates, an invariants list, and incremental integration.

## Live artifacts

1. **Requirements doc** (`requirements.md`) - the behavioural target. It carries a short **invariants** section: the non-negotiables every gate checks against. The full stakeholder sweep happens only when the requirements doc itself is revised, not after every step.
2. **Spike briefs and outcome docs** - every spike starts with a one-paragraph brief and ends with an outcome doc.
3. **Process handoff** - keeps the process resumable by a fresh agent: active loop, current target, evidence state, open questions, do-not-integrate warnings.

## The loop

1. **Pick the next question.** The smallest question whose answer unblocks the most design. Write a one-paragraph spike brief: the thesis, what evidence would falsify it, which invariants it touches.
2. **Spike freely** under `experiments/`. No mid-spike gates and no tests-first requirement - the spike exists to discover the behaviour shape, so TDD-ing code we are about to throw away is theatre. Timeboxed. A spike must end with runnable evidence plus an outcome doc: what it proved, what it failed to prove, what to integrate, what explicitly not to integrate, and what requirements pressure appeared.
3. **Gate 1 - spike acceptance.** The user (plus optionally a fresh-context review session) reads the outcome doc against the invariants. Accept, redo, or discard.
4. **Integrate small and soon.** Promotion means: re-design the core slice fresh from the evidence (never copy spike code), write black-box tests first at the public surfaces, implement the smallest coherent slice, keep all existing core tests green.
5. **Gate 2 - integration acceptance.** Tests green, invariants checked, fresh-context review accepts, user accepts.
6. Update the handoff. Go to 1.

Do not batch integrations. Alternate spike → small integration → spike, so the component interaction matrix is paid down incrementally instead of reconciled in one big-bang core phase at the end.

## Public test surfaces

Black-box tests for core target the product-public surfaces. For this product those are:

- the CLI / UI behaviour
- the provider wire boundary, via a fake provider: what API requests were actually sent, what triggered them, and what context they contained
- the durable storage and query surface: analytics queries are product behaviour, not internals
- the face/brain/limb transport protocol, once it is public

These are product surfaces, not implementation details - the central theses of the design (context lifecycle, request triggering, queryability, topology) are observable exactly here. Internal events, private queues, and actor messages remain off-limits for durable tests.

Test primitives (fake provider, fake workspace, scenario runner, deterministic clock) are extracted from the first spikes once real pressure exists. They are not designed up front: pre-building a test rig for a system that does not exist yet means designing it against an imagined architecture, which is precisely the architecture leakage to avoid.

## Review mechanics

Adversarial review is a fresh-context session that has not seen the implementation work, used at exactly the two gates. Its job is to reject fake success, catch architecture narrowing, and check the invariants. It either accepts or returns findings; it does not fix. The user is the standing reviewer at both gates.

## First move

A walking-skeleton spike: a toy face+brain+limb loop running end-to-end against a fake provider, single process, append-only CLI. Every later spike needs this substrate, and the test primitives fall out of it rather than preceding it. It is still a spike - disposable, briefed, and gated like any other.

## When stuck, go backwards

"Do not implement forward through uncertainty" is a triage rule for when work stalls, not a permission system gating motion:

- implementation uncertainty → back to design
- behaviour too underspecified to evidence → back to the brief or the requirements doc
- design contradicts an invariant → stop and ask the user
- spike pressure reveals a requirements gap → update the requirements doc (this is where the full stakeholder sweep happens)

## Ask the user when

- invariants or stakeholder requirements conflict
- a spike suggests the architecture direction is wrong
- an integration would significantly narrow the product
- behaviour is too underspecified to test
- a review rejects a gate and the way back is not obvious

Not merely because implementation is hard. Hard implementation goes back through design and spike loops.

## Short version

Spikes are cheap evidence; gates guard only spike acceptance and core integration. An invariants list replaces the per-step stakeholder sweep. Tests-first applies to core at its public surfaces, not to disposable spikes. Integration happens small and continuously, starting from a walking skeleton. The handoff keeps it all resumable.
