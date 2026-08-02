# Compaction / handover — design scoping

Provisional. Derives from `source-notes/compaction.md` (the primary source), the "Compaction note" in `source-notes/context-updates.md` (the freshest user thinking, 2026-08-03), `source-notes/handoff-improvements.md`, and `source-notes/context-and-agent-loop.md`. One thing makes this doc different from its siblings: part of the design is already *empirically validated* in the user's OpenCode fork, which `source-notes/open-code-inspiration.md` names the behavioral source of truth where features overlap. So the doc keeps two ledgers — **proven in the fork** (port the behavior) and the usual **settled / open / experiment**.

## What compaction is for, and why it's called handover

Compaction lets work continue past context limits and keeps active context short and cheap — but it is lossy, so everything hangs on what is kept. The fork's empirical finding: calling it a **handover** produces better results than "summarise" or "compact", because the word itself implies that everything a successor needs must be passed forward — goals, progress, decisions, blockers, next steps, and "most importantly: preserve the goal and the current place in the overall work." **Proven in the fork.**

The naming has a counter-note, kept verbatim: "I think that 'handover' is a bit of an 'othering' term so compaction tool might be better, but needs to be essentially the same." So: the model-facing name is genuinely unsettled; the behavior is not. **Open (cosmetic), settled (behavioral).**

## The core mechanics: appended instructions, and the what-changes contract

The freshest user note says how compaction actually happens on the wire: **by instructions appended to the end of the conversation** — ideally as system parts, "depends on provider / model support". Not a rewrite of history, not an out-of-band prompt: the conversation itself ends with the handover work. **Settled; experiment** for the system-part support per provider.

And the instructions are not generic. The note's core demand is that the handover be explicit about the *new situation*:

- **What is changing?**
- **What will still be in the system prompt** — and therefore does not need repeating?
- **What will NOT be in the system prompt anymore** — and therefore *does* need to be kept, by writing it forward?

This contract is the genuinely novel content of this design, and it has a quiet prerequisite worth stating: to answer those three questions, the agent must be told what the fresh context *will look like* — the rebuild plan — before the handover completes. The harness knows (it knows the new AGENTS.md, the new tool schemas, the new skill set that a rebuild will assemble); the handover flow must surface that to the agent as a diff between its current context and its successor's. This is the same honesty requirement as the walking-skeleton introspection ruling — the request builder and `/dump` share one projection so they can't diverge — pointed at the *future* context instead of the current one. **Open, proposed:** the rebuild-plan diff becomes part of the stage-two instructions (next section).

## The two-stage flow

**Proven in the fork**, recently, and worth porting closely:

1. The agent calls the handover tool.
2. The harness responds not with a tool result but with an injected **user message** carrying everything needed for a good handover — the instructions, and (proposed above) the what-changes/stays/leaves diff. The message also "allows & encourages the agent to call other tools to tidy things up prior to completion" — commit work, update notes files, finish the bookkeeping a successor shouldn't inherit.
3. The agent completes the handover with a second call.

Why the shape matters: no cache break is required — the flow is pure append — and the fork's economics note is kept with its hedge: "I think it is typically worth it at a conservative assumption of 5x cheaper cache reads."

`handoff-improvements.md` generalizes this as "two part launch" (call → injected instructions → confirming call) and judges it: for handover, worth it, "because handover is important"; for subagents "probably not", though it "might give us a good cache point for diverging the parent agent into forked subagents". Kept as stated. **Proven in the fork (handover); open (elsewhere).**

## Attachments: the successor starts loaded

From `handoff-improvements.md`, and now reinforced by the compaction note ("Compaction's going to be a structured thing"): the handover call takes **attachments** — files and resources that are loaded immediately into the fresh context, appearing as already-executed tool calls. Instead of the successor spending its first several turns re-reading the same files (each a cache-read round trip), and instead of the predecessor burning output tokens re-summarizing what a file already says, the handover names the files and the harness loads them. AGENTS.md-style context can be preloaded the same way. **Settled in intent; experiment** in mechanics (these are the same attachment mechanics as subagent launching — see the symmetry section).

The **stateful handover document** is the aspirational completion of this: rather than a one-shot summary produced under pressure at the end, a document the agent *maintains throughout the session* — updated as goals shift and work completes — so that at handover time most of the passing-forward is already written. The pragmatic version already works today: "a gitignored file in the project that the agent writes to and reads from. Works reasonably well. Proper first-class support for this is a future design question." A maintained handover doc is the natural first attachment. **Aspiration, kept as such.**

## When handover happens

Three triggers, all compatible with invariant 2 (cache-nearly-expired handover is one of the only four legitimate request triggers):

- **The agent decides.** Agent-controlled handover is the fork-proven path — "the agent can call it proactively before cache expires, rather than waiting for the user to notice."
- **The harness suggests.** The cache-aware idea from compaction.md: the harness tracks KV-cache expiry and triggers/proposes compaction *just before* expiry — for example while a long command runs or while waiting for an absent user — so that "a much shorter context is sitting warm when the command completes or the user replies," and if the cache lapses anyway, the miss is cheap. This requires cache-expiry tracking as durable per-session state (persistence-analytics interaction; tech.md: "Backend server cache ids etc should not be ephemeral"). **Settled idea; experiment** for the timing policy.
- **The user asks.** The explicit `/handover`-style command, as in the fork today.

The caveat that governs all three is kept intact, because it is the quality bar for the whole experiment: "requires the compaction/handover procedure to be well-tuned. If it loses important info, compacting frequently makes things worse, not better."

## One mechanism, three doors: the symmetry with subagent launching

`handoff-improvements.md` lays out three paths that share one structure — **context, attachments, task**:

- **Forked task** — the parent's context is the seed; children diverge from it.
- **Fresh task** — a built seed context, attachments, then instructions.
- **Handover** — "Similar structure to fresh variant of task tool: context, attachments, and task."

The design direction this doc proposes, for review: **a handover is launching a fresh continuation of yourself.** Same machinery as fresh-task launching — build the successor's context (a rebuild: canonical new system prompt, new schemas, no stale append-notices), execute the attachments, deliver the task (the handover text) — with two differences that are identity, not mechanics: the successor *is* the session (same session identity, same user-facing thread, no visible seam) rather than a child of it; and there is no scope, no suspended parent, no result to return. If the shared mechanism holds, this experiment and forked-subagents are two views of one design, and whichever runs first should shape the shared parts with the other in view. **Open, needs-review.**

## Port vs experiment

**Port from the fork, matching behavior closely** (per open-code-inspiration.md, consult the fork source before inventing): the handover framing and prompt content; the two-stage flow with tidy-up encouragement; agent-controlled triggering; the attachment-taking handover call.

**Needs experiment here:**

1. System-part appends across providers — what each API actually allows at the tail of a conversation, and the fallback framing where system parts aren't supported.
2. The what-changes/stays/leaves contract — does giving the agent the rebuild-plan diff measurably improve what it writes forward?
3. Continuation quality — the seam test. The PLAN exit is behavioral: a fresh context resumes the work "without the user noticing a seam". Judging this honestly is itself work: candidate evidence is the successor's first turns (does it re-ask, re-read, or retread?), with the user as the standing judge at the gate.
4. Cache-aware proactive timing — expiry tracking, the trigger policy, and whether mid-wait compaction actually leaves a warm short context often enough to pay.
5. The economics — measured, not assumed: handover cost (the two stages + attachments) vs the cache-read savings it buys, against the 5x-cheaper hedge.

## Interactions with other experiments

- **forked-subagents** — the shared context/attachments/task machinery above; the two-part launch as a deliberate cache point for forking.
- **context-updates** — a handover *is* the big rebuild moment: the successor's context is canonical (new AGENTS.md, new schemas), and obsolete append-only change notices must not replay into it. The two designs describe the same boundary from opposite sides: context-updates handles changes *without* a rebuild; handover is how a rebuild happens *well*.
- **persistence-analytics** — cache ids and expiry as durable session state; handover continuity across harness restarts; the cost measurements above land in the analytics surface.
- **user-turn** — accumulated user-activity context is part of what a handover must consider passing forward; whether it's summarized forward, attachment-ized, or dropped is open and belongs in the what-stays/what-leaves contract.
- **limb-context** — the rebuild re-derives limb-injected context at its current versions; the handover contract's "what will still be in the system prompt" answer comes partly from the limb.

Exit (from PLAN.md): an agent can hand over well enough that a fresh context resumes the work without the user noticing a seam, at a cost that beats letting the cache expire.

## The matrix

Levels, statuses, and aspect definitions per `README.md`. The Why column is the motivating story. Blank = not addressed.

| Aspect | Why (the story) | Behavior | Mechanics | Verified | Interacts with |
|---|---|---|---|---|---|
| Model framing | "summarise" lost the goal and successors wandered; the handover word alone fixed it | F handover framing; P what-changes/stays/leaves contract | P rebuild-plan diff in stage two | F handover word (fork) | context-updates |
| Wire & cache | a handover that breaks the cache pays for the thing it tries to save | F two-stage, pure append, no cache break | E system parts per provider; E proactive expiry timing | F two-stage flow (fork) | forked-subagents |
| Tool surface | harness-forced compaction fires at the worst moment; the agent knows when it can afford to pause | F handover tool; S attachments | O two-part launch beyond handover | F (fork) | forked-subagents |
| UX & input | the user should not be able to tell a handover happened mid-thread | O user-thread continuity details | | E the seam test | |
| Ownership & placement | | | O timing judgement TS-side (gate/judgement pattern) | | ts-vs-rust |
| Lifecycle | session identity must survive the context swap | P handover = fresh continuation of self | | | forked-subagents |
| Storage | cache expiry cannot be tracked if cache ids die with the process | S cache ids/expiry durable | O expiry tracking shape | | persistence-analytics |
| Economics | the 5x-cheaper-cache-reads margin is assumed, not measured here | | E measured handover cost vs savings | | |
| Security | | | | | |
| Testing & verification | quality is the whole game: "if it loses important info, compacting frequently makes things worse, not better" | E the seam test, user as judge | O quality evidence: successor first turns (re-ask/re-read/retread) | | |
| Code shape | | P one machinery, three doors (shared with task launching) | | | forked-subagents |
| Dev workflow & references | | S fork source first (open-code-inspiration.md) | | | |
| Core migration | | O the contract becomes core's context-lifecycle API | | | context-updates |
