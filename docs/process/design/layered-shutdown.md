# Layered shutdown — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why, what (agent-drafted, unreviewed) · interactions, summary — not yet done.** A targeted question rather than a broad design; expected to stop at L2 depth. Originates as a user ruling, 2026-07-31, and may fold into topology or remain a pattern note. Modelling inspiration: `Dicklesworthstone/asupersync`.

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

## What

The principle is settled, so most of this section is about the two things that are not: the timing discipline, and what happens when a bounded process shutdown collides with an unbounded soft agent cancellation. It stops at L2, and the closing question of whether this deserves to be an experiment at all gets a plain answer rather than a deferral.

### What is already settled, restated as the mechanism to apply

From the walking-skeleton rulings, and not re-argued: no detached tasks or threads and no `process::exit` escape hatches; in-flight work is owned as identity plus cancellation plus join handle together, and always joined; participants return Results and failures fold into the exit code; the limb owns and cleans up its process trees, with group lifetime equal to operation lifetime on every resolution path and no process-table scanning anywhere. Every layer shuts down what it owns.

The consequence that makes this a design rather than a slogan is that the set of things a layer must shut down has to be **enumerable rather than discovered**. A layer that has to go looking for its children — scanning a process table, checking a registry — has already lost the property, because looking can miss things and can find things that belong to someone else. Ownership is a list you were handed, not a search you perform.

### Naming the layers, and where "the layer above" loses its authority

In the co-located shape the layers are: the supervising main, the three participants (face, brain, limb), each participant's owned in-flight work, and within the limb, the process groups it spawned. Four levels, each holding a list.

Across a role boundary this stops working, and it is worth being explicit about it because the pattern is usually described as if a parent can always enforce. A brain cannot kill a limb running on another machine. It can close the connection, and that is all. So the timeout backstop only exists where the layer above has kill authority — inside a process tree on one machine — and across a network boundary the pattern degrades into something different: **the far end must self-terminate on loss of its owner.**

That is not a new mechanism to invent, because the notes already describe exactly it for remote limbs: a remote limb "stays alive on (un-graceful) disconnect in case of reconnect, but shuts down if not reconnected within a timeout", and "limbs that have no brain connection do nothing and might as well not exist". So the pattern has two forms — a parent-held timeout where there is kill authority, and an orphan timeout at the far end where there is not — and the second is already ruled. What matters is not conflating them, because a design that assumes the first everywhere will quietly leave remote processes running.

### What asupersync actually does

The instruction was to look before deciding, so: asupersync does not hand down a decremented duration at each layer. Its `Budget` carries an **absolute deadline** (plus a poll quota, a cost quota and a priority), and budgets compose by `meet` — componentwise `min`, with priority taking `max` — so an inner scope's budget is the intersection of its own and everything above it. A child can only ever be tighter than its parent, never looser, and that is structural rather than arithmetic.

Two further details are directly useful here. Its cancellation protocol is request → drain → finalize, which is the same shape as invariant 9, with finalizers running **masked** — cancellation deferred while cleanup runs — and separately budgeted. And its cancel reasons are ordered by severity (User < Timeout < FailFast < ParentCancelled < Shutdown), with cleanup budgets scaling inversely with severity: the more urgent the reason, the less time cleanup gets. Its own documented caveat is worth carrying too, because it is the honest limit of the whole approach: budgets are sufficient conditions only on paths with a published responsiveness bound, and non-cooperative work can still delay quiescence indefinitely. The walking skeleton already has a concrete instance of exactly that — a read blocked on a writerless FIFO lingers on the blocking pool because `spawn_blocking` is uncancellable — which is a reminder that a deadline discipline bounds the *cooperative* tree and the process exit is what reaps the rest.

So the descending-budget idea survives contact with the reference, but in a better form than "pass down a slightly shorter budget": an absolute deadline propagated and intersected, which needs no arithmetic at each hop and does not accumulate error from scheduling delays.

One correction is needed before adopting it, and it comes from invariant 10. An absolute deadline is only meaningful on a shared clock, and no shared clock is assumed across role boundaries. So a deadline crosses a boundary as a **relative duration**, which the far end converts to its own local absolute deadline — conservatively, since transit time is unmeasurable without a shared clock and should be assumed spent rather than free. Inside a process, absolute; across a boundary, relative and re-anchored.

### Reserve, then descend

The subtlety that the plain descending budget misses is that a layer does not only wait for its children. It also has its own finalisation to do after they finish, and for the brain that finalisation is where durability lives — writing the resume contract that operator lifecycle depends on. If a layer hands its entire remaining budget to its children, it can find itself out of time exactly when it needs to write the thing that makes restart work.

So the discipline is: a layer splits its budget into a children's share and a reserved finalisation share, hands down only the children's share, and treats its own finalisation as masked against the children's deadline — bounded by the reserve rather than by the deadline that governed them. The reserve is what keeps the total bound honest; masking without a reserve would just move the unboundedness. Asupersync's masked, separately-budgeted finalizers are the same idea, which is some reassurance that it is not an invention.

Sizing the reserve is the part that needs evidence rather than argument. It is bounded by how long the largest durable write can take, which for a brain with many active sessions is not obviously small. That is measurable, and it is the sort of thing that should be measured once rather than guessed at repeatedly.

### Shutdown arriving while a soft cancellation is unwinding

This is the interesting case, and it is interesting because the two mechanisms have incompatible natures rather than incompatible parameters.

Soft cancellation is a *message to the model* — your task has been cancelled, please clean up then call done — and it deliberately spends tokens because cleanup is the point. That implies at least one more provider round trip and possibly tool calls after it. Tens of seconds, sometimes more, and not bounded by anything the harness controls. Process shutdown is bounded and must terminate. A shutdown deadline that could accommodate an agent-level cleanup turn would be too long to be a shutdown deadline.

Three ways out. Let the agent-level cleanup run inside the shutdown budget and cut it off when the deadline expires — which leaves a half-cleaned agent, though at least the record says so. Block shutdown until cleanup completes — which abandons boundedness, and boundedness is the entire point of the backstop. Or **do not start an agent-level cleanup turn during shutdown at all**: record the cancellation as cancelled-with-cleanup-not-yet-performed, and let the resumed session perform the cleanup after relaunch.

The third is proposed, for a reason beyond its own merits: it is the same move the project has already made once. A proposed-but-unexecuted tool call is valid resumable state, gets no fabricated outcome, and may execute on a later resume. Un-run agent cleanup is the same shape of thing, so this converts a timing conflict into a state representation — which is the trade this design keeps preferring, and it means no new concept is needed, only an existing one applied.

It has one honest weakness. If the relaunch never comes, the cleanup never happens. So the pending-cleanup record must be durable and must be surfaced on the next start, whenever that is — visible to the user and to the agent, not silently carried. It also means persistence has to be able to represent "cancelled, cleanup outstanding" distinctly from "cancelled, cleanup done", which is a small addition to the resume contract rather than a new table.

There is a case this does not cover: shutdown arriving while cleanup is *already mid-turn*, with a provider request in flight. That is not special — it is the ordinary rule that shutdown waits for in-flight requests to complete and does not start new ones. The cleanup turn finishes or it does not, and either way it is recorded.

### Exit codes

Operator lifecycle's verification step reads the exit code, so it has to distinguish outcomes that mean different things. Clean shutdown within the deadline is success. Shutdown that completed but where the backstop had to kill something is a distinct code, because it is a degraded success rather than a failure and it should show up in operational data without triggering a rollback. Participant failure folding into the exit code is a third. A panic is a fourth.

The distinction that matters most is the second against the third: killed-at-the-backstop may be acceptable, while failed should roll back. Collapsing them means either rolling back on every slow shutdown or never rolling back at all.

### Is there a thesis here?

Worth answering plainly rather than deferring, since the parked scope question has been sitting open. The answer is: mostly no, with one narrow exception.

The ownership principle is a discipline, not a hypothesis. It is already ruled, it was already demonstrated in the walking skeleton, and there is no experiment that could falsify it — only code that could fail to follow it. Testing a discipline is a review activity, not an experiment.

The narrow falsifiable claim is the timing one: **a total shutdown deadline propagated as an absolute deadline within a process and a re-anchored relative duration across boundaries, with each layer reserving a masked finalisation share, can be met while every durable fact needed for resume is written — including by a brain with several active sessions, a limb with live process trees, and an in-flight provider request.** That is falsified if meeting the deadline costs durability, or if the reserve cannot be sized without being so generous that the total bound stops being useful. The shutdown-during-soft-cancellation ruling is falsifiable too, but only as a behaviour to verify rather than a thesis to test.

Which supports the conclusion the parked note already suspected: this does not need to be its own experiment. It is a pattern note plus a small number of rows in another experiment's test matrix — most naturally operator lifecycle's, since that is what actually reads the exit code and depends on the durable write completing, with the cross-boundary form belonging wherever role boundaries are being exercised. The scope call is the user's, and stage 3 is where it gets settled; this section's contribution is that there is no design left here that would justify a standalone experiment.

Invariants touched: 9, because shutdown is a cancellation with a bound and the four-valued outcome must survive it; 10, because a deadline cannot cross a role boundary as an absolute time; and 5, because the reserved finalisation share exists precisely so durable state gets written.

## Parked for later stages

**Interactions flagged for stage 3:** topology (shutdown across process and network boundaries, and face disconnect leaving brain and limb running); operator-lifecycle (relaunch and staged update depend on bounded shutdown, and read its exit code); forked-subagents (the same ownership root, and the shutdown-during-soft-cancellation case); persistence-analytics (what must be durably recorded before the process ends, so resume works); self-modification (frequent relaunch is what makes this urgent).

## Questions for review

- Should this be an experiment at all, or a pattern note that topology and operator-lifecycle each conform to? It may have no falsifiable thesis of its own.
- Is the descending deadline budget worth adopting, or is a simple per-layer timeout backstop enough for a personal-scale harness? The asupersync check is now done and reported above: it uses an **absolute deadline composed by componentwise min**, not a decremented duration, which is cheaper and less error-prone than the original idea. Adopting it needs one correction for invariant 10 — deadlines cross role boundaries as relative durations and are re-anchored locally.
- **The proposed answer to shutdown-during-soft-cancellation is to not run the agent's cleanup turn at all during shutdown**, recording it as cancelled-with-cleanup-outstanding and letting the resumed session do it. That reuses the accepted unexecuted-tool-call pattern rather than inventing anything, but it means a shutdown that is never followed by a relaunch leaves cleanup permanently undone — so the outstanding record has to be surfaced on next start.
- Each layer reserving a **masked finalisation share** of its budget is proposed so the brain cannot run out of time exactly when it needs to write the resume contract. Sizing that reserve wants measurement rather than a guess; is that worth measuring once as part of whichever experiment absorbs this?
- Exit codes are proposed to distinguish **killed-at-the-backstop from failed**, because the former is a degraded success and the latter should trigger an update rollback. Collapsing them means either rolling back on every slow shutdown or never rolling back.
