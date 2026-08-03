# Limb model — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why (user-involved) · what, interactions, summary — not yet done.** Derives from `source-notes/agent-harness-design.md` (the spine), `source-notes/configuration-model.md`, and the limb references in `source-notes/agent-hierarchy.md`, `tech.md`, and `context-updates.md`. ("Limb" is still a placeholder name — candidates: workspace, context, environment, sandbox.)

A framing note the user corrected: the limb is **not** best understood by contrasting it with the brain (they are almost always the *same process*), and it is **not** a security boundary. Security is a non-concern in this project — a nice-to-have tacked on at the end, never the top. So the whys below are about *environments*, not isolation.

## Why

### 1. The limb is an environment; context and action are place-specific — *correctness/quality (PRIMARY)*

An agent should carry only the instructions and tools for the environment it is acting in. The story, from the user's own work on this project: he has several distinct limbs — one for testing and developing the project, one for editing the meta documentation and agent starting instructions, one for the harness config itself. Separate environments, with separate tools and — the key part — **separate, load-bearing instructions**: a project's instructions are what the model needs to edit and act *correctly* in that environment.

If a single agent implicitly acted across all environments, it would need the instructions for all of them at once, and that does not work well. So we split environments. An agent acts within one, carrying only that environment's context; to act in another environment it interacts with an agent that is acting with respect to *that* environment and has the appropriate context — which the first agent does not need to know. A clean way to separate various kinds of work.

Mechanism (confirmed): a session is bound to exactly one limb; switching context means launching a fresh subagent in the other limb, never borrowing another limb's tools mid-session. To act in a context you must be *in* it.

### 2. Environments are varied, not just filesystems — *capability*

The same abstraction covers genuinely different kinds of environment: filesystem environments; an environment with *nothing* in it (pure model, no tools/context); the harness itself in memory (the **meta limb**, where self-modification happens); and remote environments. Unifying these under "limb" is part of the point.

### 3. Transparent remote environments — *desire (one of the largest motivators)*

A top motivating factor. The vision: the harness SSHes into a remote machine, SCPs a binary across, runs it, has it expose a port, connects to that port via an SSH tunnel — and now a replica of the harness runs remotely, **specifically just providing a limb** to an agent on the local harness. That limb provides instructions for the folder it is in, for the user it runs under there, and maybe for the machine. Crucially it is **transparent from the agent's perspective**: the agent does not deal with SSH or quoting or plumbing — once we have set up (created) the limb in that place, the agent just launches a subagent in that environment.

### 4. Limb factory — exactly the environment you need, when you need it — *capability/desire*

We should be able to construct additional *copies* of a limb. The classic case: a fresh clone of a repo specific to a particular task; in the remote case, the same but remote. The point is having **exactly the environment you need, exactly when you need it, with all the context well-tuned for that environment only.**

### 5. One process, many locations — a memory driver, but only one — *resource*

The harness presents an interface over many filesystem/folder locations while requiring only one harness process — no need for many harness instances. Memory efficiency (one brain of moderate fixed cost + lightweight limbs + lazy, evictable LSPs; ~400–500 MB per full instance was the pain) *was and remains* a driver, but only one of several, and its size is now in doubt because multiple projects still need multiple LSP instances. Not the primary reason.

## Parked for later stages

**Consequences/risks (not whys):**

- Fork-by-default siblings can share one limb's filesystem — a real race surface; multi-agent-same-codebase is poor today. Possible direction: instructions telling forked agents what workspace scope they own, or a borrow-checker-style rule on mutable regions. Unresolved.
- Graceful shutdown / reconnect lifecycle, and brain restart-from-SQLite continuity.
- "Limb" naming undecided.

**"What" material already in the notes:** limb types (full-local, remote, in-process meta, remote-as-a-service, read-only, pure-model); limb is authoritative over available tools + what context reaches the model + how execution happens, but the brain *stores* the session context/tool set (fixed while the KV cache is valid); limb identified by `ssh_host` + `directory`; zero-tool limb is valid.

**Interactions flagged for stage 3:** forked-subagents (crossing a limb boundary is always fresh; the limb factory + a fresh sibling in a freshly-cloned limb *is* the classic parallel-work pattern); self-modification (the meta limb is where self-editing happens); context-updates (per-limb, context-specific tool sets are how progressive disclosure keeps per-session context small; a changed limb requires a compaction/rebuild); topology (face/brain/limb split, streaming, federated brains — brain and limb usually co-process, so the split is logical before it is physical); persistence-analytics (limb config and reconnect state).
