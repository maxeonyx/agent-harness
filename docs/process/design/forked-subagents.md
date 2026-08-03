# Forked subagents — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why (user-involved) · what, interactions, summary — not yet done.** Derives from `source-notes/agent-hierarchy.md` (the spine), `source-notes/handoff-improvements.md`, and `source-notes/context-and-agent-loop.md`.

## Why

Seven roots, drilled back past the mechanisms the notes state to the situations — or the user's own desires — that actually drive them. Each is tagged with what it bottoms out in.

### 1. Pay for shared context once, not N times — *resource*

The note says fork is "good for KV cache reuse". The situation underneath: you've spent a whole session building context, and now you want three subagents on three parts of it. Fresh, each one re-reads everything you already paid for — three full-price passes over the same tokens. Forking makes the children share the parent's warm prefix, so the context is paid for once.

### 2. Decomposition keeps intermediate junk out of the parent — *resource (context)*

If the parent does everything itself, a hundred tool calls of intermediate junk sit in its context forever, bloating and dulling it. A subtask keeps that junk *inside the child*; only the task result and the important context come back. The parent stays small and sharp.

### 3. The user wants to see and run his own work as structured concurrency — *desire*

Not just for the agent — for the user. He wants to *work in this shape* (decompose, own, join) because it's how he wants to run work, and he wants a **map of his current work**: to see what's in flight and how it's structured.

### 4. Clean teardown at boundaries — *correctness + desire*

Work in flight has real side effects: files, commits, branches, external state. Hard-killing a line of work mid-action leaves a half-finished mess to untangle by hand. Structured ownership is what makes clean teardown *possible*: each task knows exactly what it owns, so it can be told to clean up and finish, unwinding only its own side effects. The desire underneath: **the user wants to change his mind about work and trust it unwinds itself cleanly, rather than leaving him the mess.**

This is not specific to cancellation. The same cleanup pattern runs at any boundary:

- **cancellation** — the involuntary case;
- **normal task/subtask completion** — return clean, not just return;
- **deliberate compaction at milestones** — tidy before handing the context forward.

### 5. Attachments make fresh siblings cheap — *resource*

If a task names a file to three parallel children, without attachments all three read it independently — three redundant reads of the same bytes. Attaching it once, executed in a shared init step, removes that. Note: a shared prefix already makes attachments largely redundant for *forked* children; the real usecase is *fresh siblings*.

### 6. Somewhere to keep working while subagents run — *desire*

Today, delegating stops the conversation — you wait. The user wants *somewhere to continue to record thoughts and do his own work while subagents do their thing* (the main-thread pattern: a user-facing child continues the workspace while autonomous siblings grind in parallel).

### 7. No manufactured obligation without consent — *safety/consent*

"An agent shan't manufacture an obligation for me without consent." An autonomous agent that could spawn a user-facing session would create a surprise blocking dependency on an absent human. So autonomous agents spawn only autonomous children; only user-facing agents (already in a consenting, blocking-on-user mode) may request user-facing ones.

## Parked for later stages

Not whys — recorded now so they survive into the "what" and "interactions" stages.

**Consequence to manage in "what":** stop-reliably — a forked child, told to do only A.1.3, must end its turn there and not barrel on to A.1.4. It carries the parent's whole plan, so overrun is the risk. Untested; make-or-break for the forked model.

**Cancellation mechanics (raw "what" material):**

- Cancellation is a *message* — "your task has been cancelled, please clean up then call done" — not a kill.
- Propagation completes **upward**: children clean up and finish first, then the parent (a parent can't tidy correctly until its children have). TBC whether a message also goes on the downward pass or only the upward pass.
- **Soft by design** — it keeps burning tokens on purpose, because cleanup is the point — with a **hard stop** available later as a backstop.
- Forked children need care: each cleans up only *its own* subtask's side effects, not shared parent state.

**Open threads:**

- Not necessarily a tree — could be a DAG. TBC whether a sibling/grand-sibling must be *explicitly passed down* to be waited on (user leans yes).
- What powers does a user-facing sibling get over its siblings — can it send them messages? Unsure.
- A user-facing sibling is still a subtask, so it still returns only a summary to the parent. Open UX question: how is that surfaced to the user effectively?

**Interaction flagged for stage 3:** cleanup-at-boundary (why #4) is shared machinery with compaction/handover's "tidy up before completion" milestone step — same ownership root, two experiments.
