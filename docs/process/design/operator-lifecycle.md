# Operator lifecycle — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why, what, interactions, summary (agent-drafted, unreviewed).** Derives from the deployment and relaunch passages of `source-notes/tech.md`, the brain restart semantics in `source-notes/agent-harness-design.md`, and the operator stakeholder entry in `REQUIREMENTS.md`.

Version negotiation, staged updates, activation and verification, downgrade, safe migrations, and relaunch — locally or remotely — without unnecessarily interrupting in-flight work.

## Summary

What makes this something other than enterprise ceremony is that updates here are frequent, agent-driven and unsupervised. Self-modification intends an agent to edit the harness, rebuild it and relaunch onto the new code as the ordinary way this project develops, and its top-priority requirement — never live-brick — was earned from a real scar. This design inherits that requirement one layer down, where the failure is worse: a binary that will not start, or that starts and cannot read its own database. The mechanisms cannot be inherited with it, because you can no longer rely on the running process to save you. Two further roots shape it. Split deployments mean the two ends of a connection can be different builds — a remote limb copied across last week, a browser tab open for days — and without explicit version checking the failure is a subtly incompatible protocol producing wrong behaviour rather than a clean refusal. And migrations are where data actually dies, because the database holds the user's real work history and cannot be regenerated.

The organising observation is that the notes describe one binary running in several modes, and update risk is proportional to durable ownership rather than spread evenly across them. A face owns nothing but face-local state, so updating it is a restart. A limb owns an environment and no session state, so updating it is a kill, a copy, a relaunch and a reconnect. The brain owns the database, and it is the only role where a bad update can destroy something. So there are three update stories and only the brain's needs the full ceremony.

Version becomes a negotiated fact on every role boundary, established in the connection handshake the design already wants for another reason — the joining peer declares its contributions, catches up, and signals ready, so version belongs in the declaration step rather than in a mechanism of its own. Each end states a supported range; either they intersect and the connection proceeds at the highest common version, or it is refused with a message naming both versions. What happens next has exactly two forms, and which applies is a property of the boundary rather than a runtime decision: **repair** where the far end is deployable from here, which is why a stale remote limb can be silently fixed by pushing a binary as part of ordinary limb creation, and **report** where it is not, which is why a stale browser tab can only be told to reload. There is a fourth boundary that is not between roles — the brain and its database, where the binary's understood schema range is compared against the version the database records — and it is the one where getting the comparison wrong loses data rather than a connection.

Update itself is four distinct steps, and naming them separately matters because the third is the one usually omitted and the one the never-brick requirement actually needs. **Stage** puts the new artefact beside the current one, never over it, so rollback is a pointer move rather than a re-download by the thing that needs rolling back. **Activate** flips the pointer and relaunches. **Verify** happens twice, because there are two different failures: before activation the new binary must prove statically that it can start — a self-check that opens the database read-only, confirms the schema version is in range, validates configuration and exits with a known code — and after activation a liveness criterion must be met before the old artefact is released. Passing the first and failing the second is the exact shape of the original scar. **Roll back** flips the pointer back. That sequence quietly contains a question, and answering it is this design's notable addition: if the new binary dies immediately, the agent that triggered the update died with the process it was running in, so **something has to outlive the relaunch.** The proposal is a small supervisor that owns the activation pointer, performs the four steps, and is what systemd or Task Scheduler actually points at — the same binary in a supervise mode, keeping the single-binary property, with one restriction. Activation never replaces the *running* supervisor, so a supervisor upgrade is a separate and rarer operation with no automatic rollback of its own. That residual risk is real, and the mitigation is that the supervisor stays small enough to rarely need updating.

The database gets the extended treatment because no single mechanism makes it safe. A snapshot before any migration is the actual safety net, and taking that seriously reframes the downgrade question the why asked for a ruling on, because two different needs hide inside it. *The update was immediately bad* is answered completely by the snapshot — restore, flip the pointer back, relaunch — and costs almost nothing, since nothing happened in between. *It ran fine for three days and now I must go back* is a different problem, because restoring discards three days of real work, and supporting it properly taxes every future schema change. Most of the second need is available cheaply by constraining migrations to be **additive** — new tables, nullable columns, indices, never a rename, drop or type change — so an older binary can still read a newer database. That is a plausible standing discipline here specifically because persistence makes the named queries rather than the table shapes the compatibility surface. Where a destructive change is genuinely necessary, the answer is a **downgrade barrier** recorded in the database: a minimum binary version, which an older binary reads first and refuses on, so it fails by saying "this has moved past what I understand" rather than by misreading columns. Federation complicates all of this and is not resolved: a brain holds rows that originated on brains running other versions, and whether an uninterpretable row should be stored verbatim for later interpretation is a candidate worth recording rather than adopting.

Shutdown, from this design's side, is narrow: which shutdowns exist, what triggers them, and what must be true in the database when the process ends. A signal, the tray GUI, an agent's tool call, a remote command and the supervisor during an update are all the same shutdown with no special paths, which matters because the notes' sequence — wait for in-flight provider requests to complete, record that tool calls were about to run, run them on relaunch — must hold identically whether a human asked or an agent did. The waiting is on the reasoning that a streaming response has largely been paid for already, which is a well-argued guess that cancellation-economics either confirms or improves on. What must be true at exit is the **resume contract**, and it is one list shared verbatim with persistence-analytics rather than restated differently in two places, because if the two ever disagree then resume is broken. The one-hour rule keeps the user's own hedging by becoming configuration rather than behaviour: record the shutdown time, compute the gap, let a setting decide, default to continuing, and ask once for the whole set rather than once per session. Two further pieces follow the same shape as everything above. Detach — dropping the face while the limb continues, re-parented to systemd or Task Scheduler with the brain still using it — works because limb identity is stored in the brain as a recipe rather than derived from the process's mode, and the brain must read it as a face disconnect plus a limb continuation rather than as a shutdown. And remote relaunch is not an administrative operation but the ordinary one: bootstrap is ensure a binary, start it, negotiate, and push-then-retry on failure — so the machine an agent happens to be running on is the local instance of the same operation, not a special case.

## Why

### 1. In this harness, updates are frequent, agent-driven and unsupervised — *safety*

This is the root that makes operator lifecycle something other than enterprise ceremony. The self-modification design intends an agent to edit the harness, rebuild it, and relaunch onto the new code, repeatedly, as the normal way this project develops. Its top-priority why is "never live-brick", earned from a real scar: the first thing hit with Pi's self-modification was an agent self-edit that immediately bricked the harness.

Self-modification handles that at the plugin level — exercise code on load, quarantine and roll back a bad reload. Operator lifecycle is the same requirement at the *deployment* level, where the failure is worse: a bad binary that will not start, or starts and cannot read its own database. So the whys here are inherited from self-modification but the mechanisms cannot be, because you can no longer rely on the running process to save you.

### 2. Split deployments mean the two ends can be different builds — *correctness*

The moment a limb runs on a remote machine, or a face connects from a phone, versions can differ — because a remote binary was copied across last week, or a browser tab has been open for days. Topology creates this problem; this design owns it. Without explicit version negotiation the failure mode is a subtly incompatible protocol producing wrong behaviour instead of a clean refusal.

The notes also want a binary copied to a remote machine and run there as part of ordinary limb creation, which means update and downgrade are not rare administrative events — they are part of connecting to a machine.

### 3. Migrations are where data actually dies — *correctness*

Persistence exists to hold the user's real work history, including material he wants to use for timesheets, and it cannot be regenerated. An update that migrates a schema is therefore the single most dangerous routine operation in the system, and it is made more dangerous by #1: the agent performing it is unsupervised, and by #2: a downgrade may need to read a database a newer version already migrated.

Downgrade is the part usually skipped, and it is exactly what rollback after a bad update requires.

### 4. The user wants it to run like an appliance in the background — *desire*

From the notes: a single binary that can run in any mode, with the brain able to run as a background server with a tray icon and a management GUI. Plus the detach case — transitioning a process to being limb-only, re-parented to systemd or Task Scheduler, with the brain continuing to use it. Background persistence across user disconnects must hold on both Windows and Linux.

The desire is for the harness to be *there*, quietly, rather than something started and stopped around each session.

### 5. Restarting should not cost the work in flight — *correctness + desire*

The notes' graceful shutdown sequence: wait for all in-flight API requests to complete, record that tool calls were about to run, then run them on relaunch and continue. And the judgement call left deliberately open — a brain relaunching within an hour can just continue; beyond an hour, an interactive client should decide whether to resume other agents, while a server should probably continue regardless.

The reason to wait rather than cancel is economic and factual: a response being streamed has largely been paid for, and discarding it wastes both the money and the work. This is the same reasoning as the cancellation-economics question.

## Forward: what these roots force

- **Version as a negotiated fact on every role boundary**, with an explicit incompatibility response — refuse clearly rather than proceed hopefully.
- **Update as distinct staged steps**: stage, activate, verify, and roll back — so a bad build fails at verification rather than at first use. The verification step is what #1 needs and is the part most easily omitted.
- **Migrations forward *and* backward**, or an explicit ruling that downgrade past a migration is unsupported and blocked rather than merely inadvisable.
- **Graceful shutdown by ownership**, already ruled from walking-skeleton evidence: every layer shuts down what it owns, in-flight work is owned and joined, no detached tasks and no `process::exit` escape hatches. The parent-held timeout backstop and descending deadline budget are a recorded pattern, and are their own targeted question.
- **A resume contract shared with persistence** — what "in flight" means when the process ends must be exactly what the schema can represent.
- **Relaunch must be remotely triggerable**, since limbs are remote and self-modification wants to update whatever machine it happens to be running on.

## What

The exit condition for this work is modest and worth keeping in view: operational lifecycle assumptions are credible enough for core design. Not a finished deployment story — just enough that the core is not built on a guess about how it gets updated.

The organising observation is that the notes describe one binary running in several modes, and lifecycle risk is not spread evenly across them. Update risk is proportional to durable ownership. A face owns nothing durable but face-local state, so updating it is a restart. A limb owns an environment but no session state — the brain stores what a session is, and the limb rebuilds context on demand — so updating a limb is a kill, a copy, a relaunch and a reconnect. The brain owns the database, which holds work that cannot be regenerated, and it is the only role where a bad update can destroy something. So there are three update stories, not one, and only the brain's needs the full staged ceremony. That is the shape of this design.

### Version as a negotiated fact, and the two answers to a mismatch

Every role boundary carries a protocol version, and the moment to establish it is the connection handshake — which is a handshake the design already wants for another reason. The event-streaming notes describe a joining peer declaring its contributions, catching up, and signalling ready. Version belongs in the declaration step of that same handshake rather than in a mechanism of its own. Each end states the versions it supports as a range with a preference; either they intersect and the connection proceeds at the highest common version, or they do not and the connection is refused with a message naming both versions and what to do about it. Refusing clearly rather than proceeding hopefully is the whole point: the failure this prevents is a subtly incompatible protocol producing wrong behaviour, which is far worse than a connection error.

The interesting part is what happens next, and it differs by whether the mismatching end is deployable from here. A remote limb is: the brain can copy a compatible binary across and relaunch it, so a limb version mismatch has an automatic repair path and can be handled silently as part of ordinary limb creation. A face is not: a browser tab that has been open for days, or a phone, has to be told to reload, and the only correct response is a clear message to the user. A brain the face connects to is not deployable from the face either. So mismatch handling is a single negotiation with two resolutions — repair, or report — and which applies is a property of the boundary, not a runtime decision.

There is a fourth boundary that is not between roles: the brain and its database. The database records the schema version it is at, and the binary records the range of schema versions it understands. That comparison is the same negotiation, and it is the one where getting it wrong loses data rather than a connection.

### Stage, activate, verify, roll back

The update sequence is four distinct steps, and the reason to name them separately is that the third is the one usually omitted and the one why #1 actually needs.

**Stage** places the new artefact beside the current one and never over it. Rollback is then a pointer move rather than a re-download, which matters when the thing that needs rolling back is the thing that would do the downloading.

**Activate** flips the pointer and relaunches.

**Verify** happens twice, because there are two different failures. Before activation, the new binary is asked to prove statically that it can start: a self-check subcommand that opens the database read-only, confirms the schema version is within the range it understands, validates its configuration, and exits with a known code. That catches "will not start" and "cannot read its own database" without risking the running system. After activation, a liveness criterion has to be met before the old artefact is released — the new process came up, accepted a connection, and resumed its sessions. Passing the first and failing the second is exactly the shape of the scar in why #1, so both are needed.

**Roll back** flips the pointer back and relaunches the old artefact.

Which raises the question the sequence quietly contains: **who is left to do the rolling back?** If the harness updates itself and the new binary dies immediately, the agent that triggered the update died with the process it was running in. Something has to outlive the relaunch.

The proposal is that the launcher is that something: a small supervisor that owns the activation pointer, performs stage/activate/verify/rollback, and is what systemd or Task Scheduler actually points at. It can be the same binary in a supervise mode, keeping the single-binary property, with one important restriction — activating a new version never replaces the *running* supervisor. The supervisor keeps executing the old code until it is deliberately restarted, so a supervisor upgrade is a separate, rarer operation with no automatic rollback of its own. That residual risk is real, and the mitigation is that the supervisor stays small enough to rarely need updating. This component is not in the notes, so it is a proposal rather than a settled thing, and it is in the questions below.

Verification's exit code is read by the supervisor, which is why exit-code semantics matter and why they need to distinguish "shut down cleanly", "shut down but the backstop killed something", and "failed". The first two may both be acceptable; the third should trigger rollback. Those semantics are layered-shutdown's to define; this design is the consumer that makes them load-bearing.

### The database is the part that cannot be re-downloaded

Everything else in an update is replaceable. This is the part that is not, and it deserves the extended treatment because no single mechanism makes it safe — it takes a snapshot, a discipline and a barrier together, and then federation complicates all three.

#### Snapshot first, because it is cheaper than reversibility

Before any migration runs, copy the database. With SQLite this is cheap and can be done consistently, and it is the actual safety net — more reliable than a hand-written down-migration, because a snapshot cannot be subtly wrong the way inverse DDL can.

Taking snapshots seriously reframes the downgrade question, which is the thing the why asked for a ruling on. There are two different needs hiding inside "downgrade":

The first is *the update was immediately bad*. Here the snapshot is a complete answer: restore the file, flip the pointer back, relaunch. No down-migration is needed at all, because no new work happened after the migration.

The second is *the update ran fine for three days and now I must go back*. Here the snapshot is useless, because restoring it discards three days of the user's actual work. This is the case that genuinely requires reversible migrations, and it is much more expensive to support, because it constrains every future schema change.

Those are different enough that they want different answers, and the user should pick. Supporting the first always is nearly free. Supporting the second is a standing tax.

#### Additive discipline, and a barrier when it cannot hold

There is a cheaper way to get most of the second need: if migrations are constrained to be **additive** — new tables, new nullable columns, new indices, never a rename, drop or type change — then an older binary can still read a newer database, and downgrade is free without any down-migrations existing. Given that this project's schema is a query surface with named queries as the compatibility contract (see persistence-analytics), additive-only is a plausible default rather than an unreasonable restriction.

Destructive changes will occasionally be genuinely necessary. The proposal for those is a **downgrade barrier** recorded in the database: a minimum binary version the database now requires. An older binary reads that first, finds it is below the minimum, and refuses clearly — which is the "explicit ruling that downgrade past a migration is blocked rather than merely inadvisable" the why asked for, expressed as data rather than as documentation. It also fails in the right direction: the old binary says "this database has moved past what I understand" instead of reading it and misinterpreting columns.

So the rule reads: additive by default and freely reversible; destructive changes are explicit, take a snapshot, and set a barrier.

#### The federated complication

Backup-by-default replication means a brain holds rows that originated on other brains, which may be running different versions. So rows can arrive that the local schema cannot represent — a case that does not exist in a single-machine design and that the notes do not address.

The minimum requirement is that the sync boundary version-negotiates like every other boundary. Beyond that, there is a candidate answer worth writing down but not adopting: since the whole point of "them each storing all the data" is backups, a peer that cannot interpret an incoming row could still store it verbatim and interpret it after a later upgrade — losing queryability temporarily rather than losing data permanently. That is speculative and unruled; the honest position is that this is unresolved, and the design's obligation for now is to not foreclose it.

### Shutting down, and the resume contract

The operator's side of shutdown is narrower than layered-shutdown's: which shutdowns exist, what triggers them, and what must be true in the database when the process ends.

Shutdown can be triggered by a signal, by the tray or management GUI, by a tool call from an agent doing self-modification, by a remote command, or by the supervisor during an update. All of them are the same shutdown; none of them is a special path. That matters because the notes' sequence — wait for all in-flight API requests to complete, remember that tool calls were about to run, then run them on relaunch and continue — must hold identically whether a human asked or an agent did.

The reason to wait rather than cancel is economic and factual: a response being streamed has largely been paid for, so discarding it wastes both the money and the work. That is currently a well-reasoned guess, and cancellation-economics is the measurement that either confirms it or makes shutdown faster.

The resume contract is the checklist of durable facts that must be true at exit, and it is shared verbatim with persistence-analytics rather than restated differently: every in-flight request either completed and was recorded, or was recorded with a cancelled or panicked outcome and never left dangling; every proposed-but-unexecuted tool call is durably present with no fabricated outcome; every open scope's state is derivable; the current context epoch and its cache metadata are written; the shutdown time is recorded. Relaunch then reads exactly that set and nothing else. If the two designs ever disagree about this list, resume is broken — which is why it is one list.

Two consequences worth being explicit about. Interrupted tool calls need no special mechanism, because the notes already rule that the tool reports something like "tool call interrupted by harness crash" and the agent reasons about state and safety itself; the requirement is only that "started, no outcome" is stored distinctly from "proposed, never started". And durable state must be written *before* the shutdown deadline expires, which is the constraint that makes layered-shutdown's reserved finalisation budget necessary rather than elegant.

The one-hour question keeps the user's own hedging: relaunching within an hour can just continue, beyond an hour the first client would have to decide whether to resume other agents, in server mode it should probably continue regardless — and then "Actually - I don't think that's so clear. Probably this should be optional too." Taking that at face value, the least-committal design is that this is configuration, not behaviour: record the shutdown time, compute the gap on relaunch, and let a setting decide, defaulting to continue. Interactive mode asks once for the whole set rather than once per session, because a brain may be running many sessions and asking per session is the annoying version.

### The three shapes, and detaching

The notes require the single binary to run as client (TUI or GUI), brain server (optionally with a GUI, in the background, with a tray icon and a management GUI), or limb server (probably no GUI needed); all three at once; splitting out additional limbs and accepting additional clients; and the common configuration of client and limb together connecting to a remote brain.

Mapped onto the three update stories above: the client is updated freely and can be restarted at will. The limb is updated by push from the brain, and because it holds nothing durable, a version mismatch is repaired rather than reported. The brain is the one wrapped by a supervisor, running with a snapshot-before-migrate discipline and a two-phase verification, and it is the reason any of this ceremony exists.

Detaching is the interesting transition: a client+limb process drops its face and continues as limb-only, re-parented to systemd or Task Scheduler, with the brain continuing to use it. Three things have to be true for that to work. The limb's identity and its connection to the brain must survive the change of role composition, which means limb identity cannot be derived from the process's mode — it is `ssh_host` plus `directory`, stored in the brain, exactly as the notes have it. The brain must see this as a face disconnect plus a limb continuation, not as a shutdown; a detach that looks like a shutdown would trigger the resume machinery for no reason. And the surviving process must not remain a child of a terminal session that is going away, which is the actual mechanical work — `setsid` on Linux, re-parenting to Task Scheduler on Windows — and is also where background persistence across user disconnects on both platforms gets tested. The face's local state is discarded or handed to another face; which of those is multi-client's question, not this one.

### Remote relaunch

The notes want a binary copied to a remote machine and run there as part of ordinary limb creation, which makes remote bootstrap a normal operation rather than an administrative one. Bootstrap is: ensure a compatible binary exists at a known path on the target, start it, negotiate. If negotiation fails, push a binary and retry — which is the repair path from the mismatch section, and it is the same code whether this is the first connection or a recovery.

Self-modification wants more than that: it wants to update whatever machine it happens to be running on. So the staged sequence has to be executable remotely, which means the supervisor needs to exist on remote machines too, and the trigger has to be a command that crosses the role boundary. The machine you are editing from is not a special case; it is the local instance of the same operation.

### Putting it back together: an agent updates the harness

An agent has edited the harness source and built a new binary. It calls the relaunch tool.

The new artefact is staged beside the current one. The supervisor runs its self-check: the binary starts, opens the database read-only, finds the schema version is one behind what it wants, confirms the migration is additive, and exits clean. The supervisor snapshots the database, applies the migration, and flips the pointer. The old brain is asked to shut down: it waits for the two in-flight provider requests to finish and records them, leaves three proposed tool calls unexecuted, writes cache metadata and the shutdown time, and exits with the clean code within its deadline.

The new brain starts. It reads the resume contract, finds four sessions with open scopes, reconnects to two local limbs and one over SSH. The SSH limb is still running last week's binary, so negotiation fails; the brain copies the new binary across, relaunches it, and negotiates successfully — the user never learns this happened. The pending tool calls run. The gap since shutdown was ninety seconds, so everything continues without asking. The supervisor sees liveness and releases the old artefact.

Meanwhile a phone had a web face open. Its tab is on the old protocol version, so its next request is refused with a message telling the user to reload. It reloads and reattaches to the same sessions.

Had the new brain instead crashed on startup, the supervisor would have seen a failure exit code rather than liveness, flipped the pointer back, restored the snapshot, and relaunched the old binary onto the pre-migration database — losing nothing, because nothing happened in between. The agent that started all this wakes up in the old harness and is told the update was rolled back.

### Thesis, falsification, and invariants

The thesis: **an unsupervised agent-driven update can be made safe without live-bricking, by negotiating version at every boundary with two resolutions (repair where the far end is deployable from here, report where it is not), by splitting update into stage / activate / two-phase verify / roll back under a supervisor that outlives the relaunch, and by making the database safe through snapshot-before-migrate plus additive-by-default migrations with an explicit recorded downgrade barrier — while the resume contract shared with persistence means a relaunch continues in-flight work without asking the user.**

It is falsified if: an update can brick the harness in a way no verification step catches; the supervisor cannot itself be updated safely enough to be acceptable; additive-only proves too restrictive in practice, so the barrier becomes the normal case rather than the exception; a snapshot-and-restore is not actually sufficient for the immediately-bad case; version negotiation cannot be folded into the peer handshake and needs its own mechanism; detach cannot preserve limb identity and the brain's connection on both Windows and Linux; or resume needs a fact the shared contract does not include, which would mean the two designs have diverged.

Invariants touched: 4, because all of this is about deployment shapes over one logical model; 10, because update and relaunch cross role boundaries and may not assume shared filesystem or clock; 5 and 9, because the resume contract is a statement about durable state and recorded outcomes; and 2, because explicit resume is one of the four legitimate request triggers, so a relaunch that continues work is triggering requests on purpose.

## Interactions

This design exists because of two of its siblings and is consumed by a third, which makes its boundaries unusually easy to draw. Topology creates the split deployments that make version negotiation necessary; self-modification creates the frequent unsupervised updates that make verification and rollback necessary; and persistence holds the thing an update can destroy. Almost everything else in the portfolio is either irrelevant to it or supplies it with a single parameter.

### What this experiment owns

Version as a negotiated fact on every boundary, including the brain-to-database boundary that is not between roles, with the two resolutions — repair where the far end is deployable from here, report where it is not. The four-step update sequence, and in particular the verification step, which is the one usually omitted and the one the never-brick requirement actually needs. The supervisor: the component that outlives a relaunch, owns the activation pointer, and is what systemd or Task Scheduler points at. Database safety as a discipline rather than a mechanism — snapshot before migrate, additive by default, an explicitly recorded downgrade barrier when a change cannot be additive. Remote bootstrap and remote relaunch, including the case where the machine being updated is the one the agent is running on. And detach: dropping the face while the limb continues, re-parented, with the brain still using it, on both Windows and Linux.

Two gaps that limb-model records and cannot fill belong here rather than there, and this design claims them: where on a remote machine the copied binary lives, and how the remote platform and architecture are detected so that the right build is sent. Both are properties of deployment rather than of what a limb is.

### Self-modification: the same requirement, one layer down

The never-brick requirement is inherited wholesale and the mechanisms cannot be, and the line between the two designs is sharp enough to state as a rule. Self-modification owns failure recovery *inside a process that is still running* — load-time validation, exercising the code, quarantine so nothing bad is ever swapped in, and rollback with attribution good enough to blame the right plugin. This design owns the case where nothing is running to do the recovering. That is why the supervisor lives here: self-modification's what states the requirement — never-live-brick for the shell needs an actor that survives the shell — and deliberately does not choose between an external supervisor and a small launcher. This design chooses, and self-modification assumes the choice rather than re-deciding it.

One connection changes what the verify step does, and it was not visible before both docs reached stage 2. Self-modification pins a plugin version set per session and requires pinned versions to remain addressable while a session is warm. Persistence gives those versions a reachability-based retention rule. So an update that migrates the database is capable of collecting a plugin version some warm session still depends on, and the failure would surface as a warm session whose system prompt describes a plugin that no longer exists. The self-check before activation should therefore confirm that every pinned plugin version is still addressable under the new schema, alongside confirming the schema version is in range. That is one extra assertion in a step that already exists, and it is much cheaper than discovering it later.

Also assumed from self-modification and not tested here: the classification of a change as schema-identical, additive or breaking, and the recovery path in the shell — the CLI safe mode with the last-known-good plugin set. That path is a shell capability by design, and it is the reason a bad plugin edit is not this design's problem.

### Topology: version lives in a handshake this design does not own

Version negotiation folds into the join handshake topology already needs — a peer declares its contributions, catches up, and signals ready — so this design contributes a field to the declaration step rather than a mechanism of its own. That is the whole of the coupling on the protocol side, and it is why version negotiation is listed in topology's what as something it hands off rather than something it builds.

What this design assumes from topology and does not test: the transport contract of per-link ordering plus at-least-once with sender-assigned identity, the six configurations, and the ruling that sequencing belongs to the substrate. What topology assumes from this design: that a mismatching peer refuses clearly rather than proceeding hopefully, and that a remote limb can be repaired by push. The `face ↔ brain ↔ brain ↔ limb` configuration is where mismatch matters most, because there are two protocol hops, and it is also where the federated cross-version replication question lives — which remains unresolved between this design and persistence, and is recorded in `INTERACTIONS.md` rather than settled in either.

Detach is worth one sharpening. Topology's what observes that detaching the face is not a configuration but a *transition* between two of them, which makes it a different class of test than the configuration matrix contains. The mechanical work — `setsid` on Linux, re-parenting to Task Scheduler on Windows, and the brain seeing a face disconnect plus a limb continuation rather than a shutdown — belongs here. The face's local state, and whether it is discarded or handed to another face, is multi-client-ui's.

### Persistence: one resume contract, and the schema's evolution rules

The resume contract is one list shared verbatim, and the reason it is not restated differently in the two docs is that if they ever disagree, resume is broken. This design is the consumer: it guarantees that the list is true at exit, and reads exactly that list and nothing else at relaunch.

Three things flow the other way. The database records its schema version, so the binary-to-database comparison is possible at all. The downgrade barrier is a row rather than documentation, which is what makes an old binary refuse rather than misread. And additive-by-default is a constraint on how that schema evolves — plausible specifically because persistence makes the named queries rather than the table shapes the compatibility surface, so restructuring is allowed as long as the answers hold. Without that property, additive-only would be a much heavier tax than it looks.

What this design assumes and does not test: the schema itself, the retention rules, and whether garbage collection preserves analytics answers. Its interest in retention is narrow and specific — a migration must not collect something a warm session or an open scope still needs.

### Layered-shutdown: this design is where it lands

Layered-shutdown's what concludes that it has no falsifiable thesis of its own beyond a timing claim, and that it is a pattern note plus a small number of rows in another experiment's test matrix. This design is the natural host for those rows, and the reason is concrete rather than administrative: this is what reads the exit code, and this is what depends on the durable write completing before the deadline. So the proposal is that the timing discipline — an absolute deadline within a process, re-anchored as a relative duration across a role boundary, with each layer reserving a masked finalisation share — is verified here, along with the exit-code distinction between killed-at-the-backstop and failed, because collapsing those two means either rolling back on every slow shutdown or never rolling back at all. Remote shutdown itself is one pattern, not two (user ruling 2026-08-04: kill authority is exercised by command over the protocol, only the command crosses the wire); verifying it across boundaries, plus the vanished-owner orphan timeout, belongs to topology. That is a scope call the user has not ruled on, and it is in the questions.

The shutdown-during-soft-cancellation ruling has one consequence for this design: a relaunch may find sessions recorded as cancelled-with-cleanup-outstanding, and it must surface them rather than silently carrying them, which makes it a line in the resume contract rather than a mechanism.

### Forked-subagents and cancellation-economics: thinner than they look

A relaunch mid-scope sounds dangerous and is largely defused by decisions already made elsewhere. Blocked-parent state cannot be lost because it is not stored — a parent is blocked exactly when it has an open scope, and that is derived. A limb that died with the brain can be re-materialised because limb identity is a recipe rather than a handle. So what remains is narrow: the resume contract must include every open scope and its children's states, and a relaunch must never fabricate a child result to make a scope look finished. Hierarchy semantics are assumed from forked-subagents and not tested here.

Cancellation-economics supplies a parameter, not a design. Shutdown currently waits for in-flight provider requests rather than cancelling them, on the reasoning that a response being streamed has largely been paid for. **This design assumes the conservative behaviour and does not test it.** If the measurement shows cancellation genuinely stops billing, shutdown gets faster and nothing about the sequence changes; if it confirms the belief, the current design stops being tentative. Either way it is a tuning result arriving into an unchanged mechanism.

### OAuth: a fourth durable store, with no ceremony around it

Credentials were proposed to live outside the session database, and the gap that opened for this design — a fourth durable thing with its own format that an update can break, whose versioning nobody owns — was raised here. **Ruled 2026-08-04 the other way: credentials live inside the database**, as a durable-never-projected row class with replication scoped by brain profile. So the gap closes: the snapshot-before-migrate discipline, the schema version comparison and the downgrade barrier cover credential rows because they cover the database, with nothing extra for this design to own. (The OS keychain may still appear as a *security root* — a key encrypting those rows at rest — which is a security mechanism decoupled from the home of record and changes nothing about migration ceremony beyond one more reason a restored row can turn out invalid, which credentials already are: they invalidate through external actions, and the recovery path is re-authentication.)

The interaction oauth flagged from its side — a provider plugin update must not invalidate a live session's auth — resolves cleanly and is worth recording as resolved. Refresh state lives outside both the plugin and the binary, so neither a plugin reload nor a binary relaunch can invalidate it. The only residual case is a change to the credential store's own format, which is the gap above.

### What turned out to be empty

Nothing in this design has a meaningful relationship with user-turn, context-updates, compaction-handover or multi-client-ui. The whole of the multi-client interaction is that a stale browser tab is told to reload, which is the report resolution applied to a boundary that is not deployable from here, and needs no coordination between the two experiments. Compaction and context-updates care about the cache surviving a relaunch, and that is satisfied by persistence storing cache handles durably — a demand on the schema, not on this design. Limb-model contributes the recipe property and the two remote gaps claimed above, and otherwise the two do not meet. Modular-components is upstream of this design's testing rather than its content, with one substantive constraint inherited rather than invented: a background brain must never prompt for configuration, so the interactive resolver is a source only a face with a tty may install.

## Questions for review

- Is downgrade-past-a-migration worth supporting, or should it be explicitly blocked? Supporting it constrains every future schema change.
- The one-hour resume rule appears here and in persistence-analytics. Should it be a single designed behaviour, or is it a preference to defer until the harness is genuinely restarting often?
- Should verification after activation be automated (a self-test the new build must pass before the old one is released) given the agent doing the update is unsupervised?
- **A supervisor/launcher is proposed above, and it is a new component the notes do not mention.** Something has to outlive the relaunch to roll back a binary that dies on startup, and the agent that triggered the update cannot, because it dies with the process. Proposal: the same binary in a supervise mode, owning the activation pointer, being what systemd or Task Scheduler points at — but activation never replaces the running supervisor, so a supervisor upgrade has no automatic rollback of its own. Does that fit your single-binary-any-mode intent, or does it feel like a second thing to maintain?
- Which downgrade do you actually want? *The update was immediately bad* is answered completely by snapshot-before-migrate and costs almost nothing. *It ran fine for three days and now I must go back* needs genuinely reversible migrations and taxes every future schema change. The design above proposes the first always, plus additive-only migrations to get most of the second for free.
- Is additive-by-default (new tables, nullable columns, indices; never renames, drops or type changes) an acceptable standing discipline, with destructive changes taking a snapshot and setting a recorded downgrade barrier? It is cheap now and constraining later.
- The one-hour rule is proposed as **configuration rather than behaviour** — record the shutdown time, compute the gap, let a setting decide, default to continue, and ask once for the whole set rather than per session. That is the least-committal reading of your own hedging, but it does mean a knob exists.
- Replication across brains on different versions is not resolved. A candidate worth your reaction: a peer that cannot interpret an incoming row stores it verbatim and interprets it after a later upgrade — losing queryability temporarily rather than data permanently. Speculative and unruled.
- **Should this experiment absorb layered-shutdown's timing rows?** Layered-shutdown's own conclusion is that it has no standalone thesis, and this design is what reads the exit code and depends on the durable write landing inside the deadline. The proposal above is that the deadline and reserve discipline plus the exit-code distinction are verified here, while remote-shutdown verification and the vanished-owner orphan timeout go to topology. That is a scoping call, not a design one, and it is yours.
- ~~The credential store has no version, snapshot or migration story.~~ Resolved 2026-08-04: credentials live inside the session database, so this design's existing ceremony covers them and no fourth durable store exists.
- The self-check before activation is proposed to confirm that **every plugin version a warm session pins is still addressable** under the new schema, not only that the schema version is in range. That is one extra assertion in an existing step, and it exists because plugin-version retention is reachability-based and a migration could collect one.
- Two gaps limb-model records are claimed here rather than there: **where the copied binary lives on a remote machine, and how the remote platform and architecture are detected** so the right build is sent. Confirm that is the right home, since it is arguably part of what creating a remote limb means.

## Index

| Aspect | L1 | L2 | L3 |
|---|---|---|---|
| Model framing | | | |
| Wire & cache | | | |
| Tool surface | P | §Remote relaunch | |
| UX & input | P | §The three shapes, and detaching | |
| Ownership & placement | S | §The three shapes, and detaching | |
| Lifecycle | P | §Shutting down, and the resume contract | |
| Storage | P | §The database is the part that cannot be re-downloaded | §Additive discipline, and a barrier when it cannot hold |
| Economics | E | §Shutting down, and the resume contract | |
| Security | | | |
| Testing & verification | P | §Stage, activate, verify, roll back | |
| Code shape | | | |
| Dev workflow & references | | | |
| Core migration | | | |
