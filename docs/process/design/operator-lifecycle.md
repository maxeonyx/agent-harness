# Operator lifecycle — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why (agent-drafted, unreviewed) · what, interactions, summary — not yet done.** Derives from the deployment and relaunch passages of `source-notes/tech.md`, the brain restart semantics in `source-notes/agent-harness-design.md`, and the operator stakeholder entry in `REQUIREMENTS.md`.

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

## Parked for later stages

**Exit condition:** operational lifecycle assumptions are credible enough for core design.

**Deployment shapes the notes require:** single binary running as client (TUI or GUI), brain server (optionally with GUI, background, tray icon, management GUI), or limb server (no GUI needed); all three at once; splitting out additional limbs and accepting additional clients; the common configuration of client+limb connecting to a remote brain; and the detach transition to limb-only.

**Interactions flagged for stage 3:** self-modification (this is the same never-brick requirement at deployment level, and the agent-driven relaunch loop is the demanding case); topology (version negotiation exists because roles split; remote binary deployment is how remote limbs come to exist); persistence-analytics (migration safety, and the resume contract); forked-subagents (a relaunch mid-scope must not orphan children or lose blocked-parent state); layered-shutdown (the deadline-budget pattern is the mechanism this design needs).

## Questions for review

- Is downgrade-past-a-migration worth supporting, or should it be explicitly blocked? Supporting it constrains every future schema change.
- The one-hour resume rule appears here and in persistence-analytics. Should it be a single designed behaviour, or is it a preference to defer until the harness is genuinely restarting often?
- Should verification after activation be automated (a self-test the new build must pass before the old one is released) given the agent doing the update is unsupervised?
