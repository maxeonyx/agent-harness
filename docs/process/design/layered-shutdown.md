# Layered shutdown — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why, what, interactions, summary (agent-drafted, unreviewed).** A targeted question rather than a broad design; expected to stop at L2 depth. Originates as a user ruling, 2026-07-31, and may fold into topology or remain a pattern note. Modelling inspiration: `Dicklesworthstone/asupersync`.

The pattern: every layer shuts down what it owns, the layer above holds a timeout backstop, and a descending deadline budget is an idea rather than a ruling — with an explicit instruction to check what asupersync does.

## Summary

The root is that cleanup only works if someone actually owns the thing being cleaned up: work in flight has real side effects, hard-killing it leaves a mess, and a layer can only tidy what it knows it holds. That principle is already settled by the walking-skeleton rulings — no detached tasks, no `process::exit` escape hatches, in-flight work owned as identity plus cancellation plus join handle and always joined, the limb owning its own process trees with no process-table scanning. It is worth being clear at the outset that this makes the principle a **discipline rather than a hypothesis**: no experiment could falsify it, only code could fail to follow it, so testing it is a review activity. The consequence that turns it from a slogan into a constraint is that the set of things a layer must shut down has to be enumerable rather than discovered — ownership is a list you were handed, not a search you perform, because searching can miss things and can find things belonging to someone else. What is genuinely open is the timing.

The timing question came with an instruction to check what asupersync does before deciding, so: it does not hand down a decremented duration. Its budget carries an **absolute deadline** and budgets compose by componentwise minimum, so an inner scope is structurally never looser than its parent, with no arithmetic at each hop and no accumulated error from scheduling delays. The descending-budget idea survives contact with the reference in that better form, with one correction from invariant 10 — an absolute deadline is only meaningful on a shared clock, so across a role boundary it travels as a relative duration that the far end re-anchors locally and conservatively. One subtlety the plain descending budget misses is that a layer does not only wait for its children; it also has its own finalisation afterwards, and for the brain that finalisation is where durability lives. So each layer splits its budget into a children's share and a **reserved finalisation share**, hands down only the former, and treats its own finalisation as masked — which is what stops the brain running out of time exactly when it needs to write the resume contract. Sizing that reserve wants measuring once rather than guessing repeatedly. An earlier draft claimed the parent-held backstop stops at the network — that a brain cannot kill a remote limb and the pattern degrades into far-end self-termination. The user rejected that (2026-08-04): a layer always has kill authority over its children, exercised by command over the protocol — only the kill command crosses the wire, and each layer delivers real kills to what it locally owns. So there is one pattern at every scale; the far-end orphan timeout the notes rule for remote limbs is the fallback for a *vanished* owner, not a second shutdown form. An unresponsive far end is an escalation rather than a limit: the protocol is not the owner's only channel, and a brain that can reach a machine to deploy a limb there can reach it to kill one, so the owner's options end only where it has no second channel at all.

The interesting collision is a shutdown arriving while a soft cancellation is still unwinding, and it is interesting because the two mechanisms have incompatible natures rather than incompatible parameters. Soft cancellation is a message to the model asking it to clean up and then finish, which implies at least one more provider round trip and possibly tool calls — tens of seconds, unbounded by anything the harness controls. Process shutdown must terminate. A deadline long enough to accommodate an agent's cleanup turn would be too long to be a shutdown deadline. So the proposal is to not start an agent-level cleanup turn during shutdown at all: record the cancellation as cancelled-with-cleanup-outstanding, and let the resumed session do the cleaning. That is chosen partly because it is the same move the project has already made once — a proposed-but-unexecuted tool call is valid resumable state with no fabricated outcome — so it converts a timing conflict into a state representation and needs no new concept. Its honest weakness is that if the relaunch never comes, the cleanup never happens, which is why the outstanding record must be durable and surfaced on the next start rather than silently carried. Exit codes then have to distinguish four outcomes, and the distinction that matters is killed-at-the-backstop against failed: the first is a degraded success, the second should trigger an update rollback, and collapsing them means either rolling back on every slow shutdown or never rolling back at all.

Which leads to this doc's own conclusion, stated plainly rather than deferred. There is no standalone thesis here beyond one narrow falsifiable claim: that a total shutdown deadline — absolute within a process, re-anchored as a relative duration across boundaries, with each layer reserving a masked finalisation share — can be met while every durable fact needed for resume is written, including by a brain with several active sessions and a limb with live process trees. Everything else is either already ruled or a behaviour to verify. Examining the interaction matrix sharpens that rather than softening it: every piece of this design has a natural home elsewhere and none of the pieces need each other. So the proposal is dispersal — the timing discipline and the exit-code semantics verified in operator-lifecycle, because that is what reads the exit code and depends on the durable write landing inside the deadline; the remote-shutdown verification (kill command crosses, everything dies) and the vanished-owner fallback in topology, because it owns the boundaries; and "cancelled, cleanup outstanding" as a line in persistence's resume contract rather than a table. That leaves this document as a pattern note plus a handful of rows in someone else's test matrix. It is a scoping call, and it is the user's.

## Why

### 1. Cleanup only works if someone actually owns the thing being cleaned up — *correctness*

This is the same root as forked-subagents' why #4, arriving at the process level rather than the task level. Work in flight has real side effects, and hard-killing it leaves a mess. Structured ownership is what makes clean teardown possible at all, because a layer can only tidy what it knows it holds.

The walking-skeleton rulings already committed to this: no detached tasks or threads, no `process::exit` escape hatches, in-flight work owned as identity plus cancellation plus join handle together and always joined, participants returning Results with failures folding into the exit code, and process cleanup by ownership rather than by global observation — the limb owns its process trees, with no process-table scanning anywhere.

So the *principle* is settled. What is not settled is the timing discipline.

### 2. Graceful shutdown that can hang is not graceful — *correctness*

If each layer waits politely for the layer below, one stuck child blocks the whole tree forever, and the user is left with a process that will not die. Hence the backstop: the layer above holds a timeout, so politeness has a limit and the limit is enforced by someone who is not the stuck party.

This mirrors a known pain elsewhere in the design — a forgotten `/done` or hung child blocking a parent scope indefinitely — and the notes accept that as intentional at the *agent* level, where the user is in charge. At the *process* level it is not acceptable, because nobody is there to intervene.

### 3. Shutdown is frequent here, not rare — *safety, inherited from never-live-brick*

Self-modification intends the harness to relaunch onto new code as an ordinary development step, and operator lifecycle wants staged updates and rollback. A shutdown path that is only exercised at the end of the day would be allowed to be sloppy; one exercised many times an hour by an agent cannot be.

Frequency is not itself a root — it is what makes the root bite. The root is self-modification's *never live-brick*, one layer down: if every agent self-edit passes through this path, then a shutdown that loses durable state or fails to terminate turns routine self-improvement into routine breakage. That is why this is a safety why rather than a convenience one.

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

### Naming the layers, and how authority crosses a boundary

In the co-located shape the layers are: the supervising main, the three participants (face, brain, limb), each participant's owned in-flight work, and within the limb, the process groups it spawned. Four levels, each holding a list.

An earlier version of this section claimed the pattern stops working across a role boundary — "a brain cannot kill a limb running on another machine; it can close the connection, and that is all" — and derived a second form of the pattern from that. The user rejected it (2026-08-04) as an invented constraint: "it always has kill authority over its children... it is its children for all intents and purposes, whilst it's blocked on its children. So, obviously, killing it should kill its children." The error was conflating kill authority with OS signal delivery. Authority is exercised by *command over the protocol*, and each layer delivers real kills to what it locally owns: the face tells the brain, the brain tells the limb, the limb kills its own process trees — which it can, because ownership-based cleanup already makes it the only thing allowed to. "If face and brain are remote... you send your kill command, and then the brain does the rest. That's only the kill command that needs to cross." And the budget travels the same way: "when we tell something to die... we give it some amount of time to do so and tell it about how much time it has... it needs to pass that down again."

So there is one pattern, at every scale, local or remote: a kill command with a time budget descends, each layer shuts down what it owns and enforces the budget on what it commands. The far-end orphan timeout the notes rule for remote limbs — "stays alive on (un-graceful) disconnect in case of reconnect, but shuts down if not reconnected within a timeout" — is not a second form of shutdown; it is the fallback for a *vanished* owner, the case where no command can arrive because there is nobody to send it.

What is left of the question the rejected framing was gesturing at is smaller still, and it is an escalation ladder rather than a boundary. A far end stuck badly enough to ignore the kill command has not exhausted its owner's options, because the protocol is not the owner's only channel to it. The brain reached that machine in the first place to create the limb there, and operator-lifecycle's **repair** resolution rests on the brain being able to connect out-of-band, push a binary and relaunch a process. An owner that can deploy a process can kill one: for a remote limb over SSH, the next rung is a fresh connection and a kill of the process group, using the host from the limb's stored configuration.

So the ladder is: send the kill command with its budget; if the far end is unresponsive and is deployable from here, reach it out-of-band and kill it there; and only where the owner has no second channel do its options end at closing the connection, leaving the vanished-owner orphan timeout as the backstop. That last rung is a failure path rather than the design, and it is narrower than it first looked — it is the boundary class operator-lifecycle already calls *report*, not every remote boundary.

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

Soft cancellation is a *message to the model* — your task has been cancelled, clean up and end your turn — and it deliberately spends tokens because cleanup is the point. (The exact framing text is the design's to choose; the notes specify only that cancellation spends tokens so agents can clean up.) That implies at least one more provider round trip and possibly tool calls after it. Tens of seconds, sometimes more, and not bounded by anything the harness controls. Process shutdown is bounded and must terminate. A shutdown deadline that could accommodate an agent-level cleanup turn would be too long to be a shutdown deadline.

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

## Interactions

The interaction matrix is the stage where this design's scope question finally gets an answer, because the answer turns out to be an interaction rather than a judgement about the material. The what already concluded that there is no standalone thesis here beyond a timing claim. Examining the matrix says something sharper: every piece of this design has a natural home in another experiment, and none of the pieces need each other. That is what makes it a pattern note rather than an experiment.

**What this design owns as content**, wherever it is eventually verified: the timing discipline — a total budget carried as an absolute deadline within a process and as a re-anchored relative duration across a role boundary, with each layer reserving a masked finalisation share; the exit-code semantics; and the ruling that an agent-level cleanup turn is not started during shutdown at all.

### Where each piece lands

**Operator-lifecycle** is the natural host for the timing rows, and for a concrete reason rather than a tidy one: it is what reads the exit code, and it is what depends on the durable write completing inside the deadline. The exit-code distinction that matters most — killed-at-the-backstop as a degraded success against failed as a rollback trigger — exists precisely because operator-lifecycle's verification step consumes it, and collapsing the two means either rolling back on every slow shutdown or never rolling back at all. Sizing the reserved finalisation share wants measuring once rather than guessing repeatedly, and the natural place to measure it is a brain with several active sessions, which is operator-lifecycle's fixture already.

**Topology** hosts the cross-boundary verification, because it owns the boundaries. Under the 2026-08-04 correction there is no second form to host: the same kill-command-with-budget descends over the protocol, each layer killing what it locally owns, and only the command crosses the wire. What topology's `face ↔ brain ↔ limb` configuration verifies is exactly that — a shutdown initiated at the face completes remotely with nothing left running — plus the one genuine fallback: a far end whose owner has *vanished* (not commanded it) self-terminates on the orphan timeout, which the notes already rule for remote limbs and limb-model restates from the limb's side.

**Persistence-analytics** is the host for one state distinction. If an agent's cleanup turn is not run during shutdown, then "cancelled, cleanup outstanding" must be storable distinctly from "cancelled, cleanup done", and it must be surfaced on the next start rather than silently carried. That is a line in the resume contract rather than a table, and it reuses the pattern the project has already accepted once: a proposed-but-unexecuted tool call is valid resumable state with no fabricated outcome. The whole ruling is that shape of thing applied to un-run cleanup, which is why no new concept is needed.

**Forked-subagents** owns the finish procedure this design declines to run during shutdown, and the boundary between them is clean. Cleanup is not a cancellation feature; it is what ending a task means, parameterised by reason. `INTERACTIONS.md` records that four designs describe pieces of that one mechanism and none owns it, and adds the parameter this design contributes to it: the reasons differ in whether the agent gets a turn at all. Cancellation and compaction give it one; shutdown does not. That is the whole of this design's input to the finish-procedure cluster, and the cluster's ownership is unresolved in the portfolio view rather than here.

### What is assumed, and what is discarded

Assumed and not tested: the ownership principle itself, which is already ruled from walking-skeleton evidence and is a discipline rather than a hypothesis — reviewable, not falsifiable. The resume contract's content, which is persistence's. The set of role boundaries, which is topology's. And a controllable clock as an injected implementation, which is modular-components' port and is what would make a deadline test deterministic rather than timing-dependent.

Discarded, and briefly: **self-modification** supplies the urgency — a shutdown path exercised many times an hour by an agent cannot be sloppy — but urgency is not a mechanism and there is nothing to coordinate. **Cancellation-economics** looks adjacent and is not: the problem with an agent cleanup turn during shutdown is that it is unbounded in *time*, and no billing fact changes that. **User-turn, context-updates and multi-client-ui** have no relationship with this design at all; the closest thing is that a face disconnect leaves the brain and limb running, which is topology's ownership claim and is tested there. **OAuth** and **modular-components** are unconnected apart from the injected clock. **Limb-model** contributes the already-ruled orphan-timeout behaviour rather than needing anything.

That leaves the scope conclusion the what reached, now supported by the matrix rather than only by introspection: this is a pattern note whose content disperses into three other experiments, and the dispersal above is the proposal. It is a scoping call and it is the user's; both this doc and operator-lifecycle carry it as a question.

## Questions for review

- Should this be an experiment at all, or a pattern note that topology and operator-lifecycle each conform to? It may have no falsifiable thesis of its own.
- Is the descending deadline budget worth adopting, or is a simple per-layer timeout backstop enough for a personal-scale harness? The asupersync check is now done and reported above: it uses an **absolute deadline composed by componentwise min**, not a decremented duration, which is cheaper and less error-prone than the original idea. Adopting it needs one correction for invariant 10 — deadlines cross role boundaries as relative durations and are re-anchored locally.
- **The proposed answer to shutdown-during-soft-cancellation is to not run the agent's cleanup turn at all during shutdown**, recording it as cancelled-with-cleanup-outstanding and letting the resumed session do it. That reuses the accepted unexecuted-tool-call pattern rather than inventing anything, but it means a shutdown that is never followed by a relaunch leaves cleanup permanently undone — so the outstanding record has to be surfaced on next start.
- Each layer reserving a **masked finalisation share** of its budget is proposed so the brain cannot run out of time exactly when it needs to write the resume contract. Sizing that reserve wants measurement rather than a guess; is that worth measuring once as part of whichever experiment absorbs this?
- Exit codes are proposed to distinguish **killed-at-the-backstop from failed**, because the former is a degraded success and the latter should trigger an update rollback. Collapsing them means either rolling back on every slow shutdown or never rolling back.
- **If this is not an experiment, is the proposed dispersal right?** The interactions section proposes that the timing discipline and exit codes are verified in operator-lifecycle, the remote-shutdown verification and vanished-owner fallback in topology, and the cleanup-outstanding state as a line in persistence's resume contract. That leaves this doc as a pattern note with no experiment of its own. Confirm, or name a different host.

## Index

| Aspect | L1 | L2 | L3 |
|---|---|---|---|
| Model framing | | | |
| Wire & cache | | | |
| Tool surface | | | |
| UX & input | | | |
| Ownership & placement | S | §Naming the layers, and how authority crosses a boundary | |
| Lifecycle | E | §Reserve, then descend | |
| Storage | P | §Shutdown arriving while a soft cancellation is unwinding | |
| Economics | | | |
| Security | | | |
| Testing & verification | P | §Exit codes | |
| Code shape | S | §What is already settled, restated as the mechanism to apply | |
| Dev workflow & references | S | §What asupersync actually does | |
| Core migration | | | |
