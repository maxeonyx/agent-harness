# Topology — the decoupled monolith — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why (agent-drafted, unreviewed) · what, interactions, summary — not yet done.** Derives from `source-notes/tech.md`, `source-notes/agent-harness-design.md`, and `source-notes/federated-brain.md`, with deferred design inputs in `experiments/event-streaming-notes.md`.

Face, brain and limb are logical roles. This design is about what it takes for that sentence to be true in practice — co-located in one process, split across processes on one machine, or spread over machines and networks, all running the same logic.

## Why

### 1. Deployment shape is a circumstance of the user's life, not a design choice — *desire*

The story is the user's actual setup: a Windows desktop, a Linux VM on the same box, a laptop, remote servers, potentially a cloud instance. Work happens across all of them. The harness cannot sensibly have an opinion about which machine is "the" machine, because the answer changes hour to hour. So the roles have to be assignable to wherever they need to be, and the arrangement has to be configuration rather than architecture.

The failure this avoids is the obvious one: a harness that only works when everything is local, plus a bolted-on remote mode that is permanently second-class and half-broken.

### 2. Long-running work must not be hostage to a UI session — *desire*

Concretely, from the notes: the user wants to be able to detach the GUI from a running process — "re-parenting it to systemd or task scheduler. The brain keeps using it" — and have the brain carry on. Restated as the requirement: closing a laptop lid, killing a terminal, or losing a network connection must not stop an agent that is halfway through a job. Ideally it does not even interrupt an in-flight request, and it must hold on both Windows and Linux.

This is a desire with teeth, because it forces the face to be a genuinely detachable observer rather than the process that owns the work. Anything the face owns is by definition lost when it goes away, so this root determines the ownership split.

### 3. One logical model, or you end up writing the system twice — *correctness*

If in-process and split deployments run different code paths, they drift, and the split path is the one that quietly rots — it is exercised less and it fails in ways local development never sees. The decoupled monolith exists to prevent that: the same components, the same messages, the same protocol, with only the transport substituted (channel, IPC, or network).

This is the root behind invariant 10 — roles never assume co-location. No shared filesystem, environment, working directory or clock across a role boundary; data crossing a boundary travels *in the message*. That discipline is annoying precisely where it matters, because the local case would work fine without it, and only the remote case breaks later.

### 4. Centralising the agent loop avoids N copies of everything that should exist once — *resource/correctness*

The brain is the only role that drives provider requests. The reason is not primarily about protecting secrets — it is that rate limits, billing, session management and provider connection state are things there should be exactly *one* of. The notes' non-goals say it plainly: no "running a full remote copy of OpenCode/Pi/Goose per project", no "duplicating rate limits, billing, or session management per project". Distributing the agent loop means distributing all of that, and then reconciling it.

Credentials staying brain-owned (invariant 1) falls out of this as a consequence, and is worth having, but it is not the motivating force. The motivating force is that a coherent view of cost and rate limits is impossible if N machines are independently talking to providers.

### 5. Output should feel live — *quality*

The face needs tool stdout and model tokens fairly directly; the brain should not buffer a response before forwarding it. Hence the two paths in the notes: the brain proxying the stream as the safe default (because the network may simply not allow the face to reach the limb — remote limb behind SSH, different segment), and a direct face↔limb stream as a fast path when topology permits, with the brain still in the loop for the agent loop, model calls and permissions.

The important consequence is that the fast path is an *optimisation with a fallback*, never a requirement, and the fallback must be durable rather than degraded.

### 6. Transparent remote environments — *desire (inherited)*

This is limb-model's why #3, and topology is the machinery that makes it true: SSH in, copy a binary, run it, expose a port, tunnel to it, and now a remote limb is available to a local agent that knows nothing about SSH. Recorded here as inherited rather than re-drilled, because the root belongs to the limb model.

### 7. Federation gives one view of many machines, and backups for free — *desire*

The user likes the idea of brains connected to each other: establish persistent communication between laptop and desktop, or Windows and Linux VM, or a cloud instance, then connect any client to any brain via transparent proxying and see a merged session list. A limb has a primary brain affinity — commonly a brain per machine that hosts limbs — but the user can override it.

The freshest thinking adds a second motive: rather than each brain holding only its own data, they all store all of it, so "I get backups by default. Sync all the data in the background." With the caution attached — keep provenance clear, do not duplicate or confuse remote data with local. `source-notes/analytics.md` wants the same thing from the query side: results that span all connected brains.

This is the most speculative root here, and the notes treat it as a like rather than a requirement.

## Forward: what these roots force

- **Every component owns exactly one external world**, and the three are symmetric — face owns the TUI, brain owns the provider connection, limb owns an environment. This was already ruled from walking-skeleton evidence; roots #2 and #3 are why it matters, since ownership is exactly what determines what survives a disconnect.
- **A transport abstraction with at least three implementations** (channel, IPC, network), and a test strategy that runs the same scenario over each — otherwise root #3 is aspiration.
- **Sequencing belongs to the substrate, not a participant.** Already ruled: within a process, participants synchronise; across processes there is no total order. Whether same-machine IPC is close enough to sequence is explicitly unresolved and is a question this experiment should settle.
- **Reconnect and catch-up are first-class**, from #2: the face must be able to rejoin and be brought up to date without duplicates, which means durable ordered events and a resume point per client.
- **Version negotiation across the boundary**, since split deployment means the two sides can be different builds — this is where topology hands off to operator-lifecycle.
- **Provenance and globally unique identity on durable data**, from #7, so background sync cannot conflate machines. Recorded in `PLAN.md` under persistence-analytics, because it is a schema demand.

## Parked for later stages

**Required configurations from the notes:** `face+brain+limb`; `face+limb ↔ brain`; `face ↔ brain ↔ limb`; `face ↔ brain ↔ brain ↔ limb`; `face ↔ brain+limb ↔ face 2`; plus the optional direct face↔limb stream as a brain-authorised capability with durable compressed fallback.

**Design inputs deferred here:** the sequencer-is-substrate model, replicas, proposals versus facts, peer handshake, limb as event peer, and cancellation as a replicated fact — all in `experiments/event-streaming-notes.md`. Also the layered graceful-shutdown and descending-deadline-budget pattern, which is its own targeted question.

**Explicitly deferred by the notes:** face↔brain identity and auth — defer to existing solutions (OpenCode, Goose, ACP). ACP is named as the face↔brain shape; MCP as a plausible brain↔limb shape, with the caveat that limbs must not make direct provider calls even if MCP would allow routing completions back through the brain, and that it is unclear whether this is needed at all.

**Interactions flagged for stage 3:** limb-model (remote limbs are the demanding case; brain and limb are usually the same process, so the split is logical before it is physical); multi-client-ui (multiple faces on one session is a topology configuration *and* a state-model problem — the two experiments meet exactly at "two faces, one session"); operator-lifecycle (version negotiation and staged update across a split boundary); persistence-analytics (durable ordered events, provenance, cross-brain queries); self-modification (the replication protocol is shell, its data formats may be soft middle); modular-components (in-process composition is the same wiring problem seen from the testing side — strong merge candidate).

## Questions for review

- Federation (#7) is the one root drawn from a "like the idea" note rather than lived friction. Should it be scoped out of this experiment entirely and left as a schema-compatibility constraint only?
- Is the direct face↔limb stream worth carrying at this stage, or is brain-proxied streaming sufficient until latency is measured and found wanting?
- The notes leave same-machine IPC sequencing unresolved. Should this experiment be the one that settles it, given it is the fact everything else's ordering guarantees rest on?
