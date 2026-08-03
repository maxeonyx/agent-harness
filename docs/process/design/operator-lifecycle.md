# Operator lifecycle — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why, what (agent-drafted, unreviewed) · interactions, summary — not yet done.** Derives from the deployment and relaunch passages of `source-notes/tech.md`, the brain restart semantics in `source-notes/agent-harness-design.md`, and the operator stakeholder entry in `REQUIREMENTS.md`.

Version negotiation, staged updates, activation and verification, downgrade, safe migrations, and relaunch — locally or remotely — without unnecessarily interrupting in-flight work.

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

## Parked for later stages

**Interactions flagged for stage 3:** self-modification (this is the same never-brick requirement at deployment level, and the agent-driven relaunch loop is the demanding case); topology (version negotiation exists because roles split; remote binary deployment is how remote limbs come to exist); persistence-analytics (migration safety, and the resume contract); forked-subagents (a relaunch mid-scope must not orphan children or lose blocked-parent state); layered-shutdown (the deadline-budget pattern is the mechanism this design needs).

## Questions for review

- Is downgrade-past-a-migration worth supporting, or should it be explicitly blocked? Supporting it constrains every future schema change.
- The one-hour resume rule appears here and in persistence-analytics. Should it be a single designed behaviour, or is it a preference to defer until the harness is genuinely restarting often?
- Should verification after activation be automated (a self-test the new build must pass before the old one is released) given the agent doing the update is unsupervised?
- **A supervisor/launcher is proposed above, and it is a new component the notes do not mention.** Something has to outlive the relaunch to roll back a binary that dies on startup, and the agent that triggered the update cannot, because it dies with the process. Proposal: the same binary in a supervise mode, owning the activation pointer, being what systemd or Task Scheduler points at — but activation never replaces the running supervisor, so a supervisor upgrade has no automatic rollback of its own. Does that fit your single-binary-any-mode intent, or does it feel like a second thing to maintain?
- Which downgrade do you actually want? *The update was immediately bad* is answered completely by snapshot-before-migrate and costs almost nothing. *It ran fine for three days and now I must go back* needs genuinely reversible migrations and taxes every future schema change. The design above proposes the first always, plus additive-only migrations to get most of the second for free.
- Is additive-by-default (new tables, nullable columns, indices; never renames, drops or type changes) an acceptable standing discipline, with destructive changes taking a snapshot and setting a recorded downgrade barrier? It is cheap now and constraining later.
- The one-hour rule is proposed as **configuration rather than behaviour** — record the shutdown time, compute the gap, let a setting decide, default to continue, and ask once for the whole set rather than per session. That is the least-committal reading of your own hedging, but it does mean a knob exists.
- Replication across brains on different versions is not resolved. A candidate worth your reaction: a peer that cannot interpret an incoming row stores it verbatim and interprets it after a later upgrade — losing queryability temporarily rather than data permanently. Speculative and unruled.
