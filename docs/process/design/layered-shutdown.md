# Layered shutdown — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why (agent-drafted, unreviewed) · what, interactions, summary — not yet done.** A targeted question rather than a broad design; expected to stop at L2 depth. Originates as a user ruling, 2026-07-31, and may fold into topology or remain a pattern note. Modelling inspiration: `Dicklesworthstone/asupersync`.

The pattern: every layer shuts down what it owns, the layer above holds a timeout backstop, and a descending deadline budget is an idea rather than a ruling — with an explicit instruction to check what asupersync does.

## Why

### 1. Cleanup only works if someone actually owns the thing being cleaned up — *correctness*

This is the same root as forked-subagents' why #4, arriving at the process level rather than the task level. Work in flight has real side effects, and hard-killing it leaves a mess. Structured ownership is what makes clean teardown possible at all, because a layer can only tidy what it knows it holds.

The walking-skeleton rulings already committed to this: no detached tasks or threads, no `process::exit` escape hatches, in-flight work owned as identity plus cancellation plus join handle together and always joined, participants returning Results with failures folding into the exit code, and process cleanup by ownership rather than by global observation — the limb owns its process trees, with no process-table scanning anywhere.

So the *principle* is settled. What is not settled is the timing discipline.

### 2. Graceful shutdown that can hang is not graceful — *correctness*

If each layer waits politely for the layer below, one stuck child blocks the whole tree forever, and the user is left with a process that will not die. Hence the backstop: the layer above holds a timeout, so politeness has a limit and the limit is enforced by someone who is not the stuck party.

This mirrors a known pain elsewhere in the design — a forgotten `/done` or hung child blocking a parent scope indefinitely — and the notes accept that as intentional at the *agent* level, where the user is in charge. At the *process* level it is not acceptable, because nobody is there to intervene.

### 3. Shutdown is frequent here, not rare — *inherited urgency*

Self-modification intends the harness to relaunch onto new code as an ordinary development step, and operator lifecycle wants staged updates and rollback. A shutdown path that is only exercised at the end of the day would be allowed to be sloppy; one exercised many times an hour by an agent cannot be.

## Forward: what this forces

- **A ruling on the descending deadline budget.** The idea: a total time budget passed down, each layer spending some and passing the remainder, so the whole shutdown completes within a bound known at the top. Currently an idea. The instruction is to look at what asupersync does before deciding.
- **Named ownership at every layer**, so the set of things a layer must shut down is enumerable rather than discovered.
- **A distinction between shutdown and cancellation.** Cancellation is soft, message-based and deliberately spends tokens to let agents clean up; process shutdown is bounded and must terminate. These two must not be conflated, and the interesting case is a shutdown arriving while a soft cancellation is still unwinding.
- **Exit-code semantics**, since failures fold into the exit code and an update's verification step will read it.

## Parked for later stages

**Scope decision still open:** whether this stays a standalone experiment, folds into topology (where shutdown crosses role boundaries and transports), or remains a documented pattern that other experiments conform to. The last is plausible — there may be no thesis here to falsify, only a discipline to apply.

**Interactions flagged for stage 3:** topology (shutdown across process and network boundaries, and face disconnect leaving brain and limb running); operator-lifecycle (relaunch and staged update depend on bounded shutdown, and read its exit code); forked-subagents (the same ownership root, and the shutdown-during-soft-cancellation case); persistence-analytics (what must be durably recorded before the process ends, so resume works); self-modification (frequent relaunch is what makes this urgent).

## Questions for review

- Should this be an experiment at all, or a pattern note that topology and operator-lifecycle each conform to? It may have no falsifiable thesis of its own.
- Is the descending deadline budget worth adopting, or is a simple per-layer timeout backstop enough for a personal-scale harness?
