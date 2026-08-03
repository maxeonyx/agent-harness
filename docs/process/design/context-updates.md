# Context updates — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why (agent-drafted, unreviewed) · what, interactions, summary — not yet done.** Derives from `source-notes/context-updates.md` and `source-notes/context-and-agent-loop.md`. Progressive disclosure was previously implemented in the user's OpenCode fork, so that part has empirical backing.

This design covers what happens when the world changes *underneath* a running session: a skill is edited, a tool appears or vanishes, AGENTS.md changes, hours pass. The question is not whether to tell the agent — it is how to tell it without destroying the thing that makes the session cheap.

## Why

### 1. The agent's picture of the world goes stale, and acting on a stale picture is wrong — *correctness*

The story from the notes: the agent loads a skill, and then the skill is changed — by the user, by another agent, or by git moving underneath it. Nothing informs the agent, so it keeps working from content that no longer exists. The work it produces conforms to instructions that have been deliberately revised. That is a straightforwardly wrong outcome, not merely an inefficient one.

What makes this a first-order concern rather than an edge case is the rest of this harness. In a conventional harness, mid-session change is rare and mostly the user's fault for meddling. Here, the user editing files mid-session **is the designed normal** — that is the whole of user-turn. And a second agent editing shared instructions is the designed normal too, because self-modification puts the harness's own configuration under agent editing. This design is therefore load-bearing *because* of its siblings: user-turn and self-modification both manufacture exactly the situation that makes context staleness routine.

### 2. The obvious fix is illegal — the cache forbids it — *correctness under a hard constraint*

The natural response to "the context is stale" is "rebuild the context". You cannot, not in general. While the KV cache is warm the context must be treated as immutable — append-only, or at least append-only with respect to some prefix, depending on the provider's caching implementation. Rewriting the system prompt to carry the new skill content would break the cache, and the cache is what makes long sessions affordable at all (see compaction-handover's why #3).

So this design exists in a squeeze. The requirement is "correct the agent's understanding" and the constraint is "you may only add to the end". Every mechanism here is a consequence of that squeeze. This is the root that makes the design non-obvious: without the cache constraint, context updates would not be an interesting problem.

### 3. Bare-minimum notices, because injected content costs attention before it costs money — *quality, then resource*

The note is firm that new content is not included eagerly: the harness provides "the *bare minimum* for the agent to efficiently invalidate its current understanding — to know that viewing the new content is an option".

The first reason is attention, not cost. Dropping the full new text of a changed skill into the middle of a conversation actively derails the agent: it arrives as though it were the current topic, competing with the task in hand, when usually nothing about it needs acting on right now. A one-line notice leaves the agent in charge of whether the change is relevant, which is the correct division of labour — the agent knows what it is doing and the harness does not.

The second reason is genuine resource pressure, and it compounds in an unusual way: once appended, a notice sits in the cached prefix and is re-read on every subsequent request for the rest of the session. An eager content injection is not paid once, it is paid forever. This is the same arithmetic as compaction's why #3 and it points the same direction — keep appended material small, because appended material is permanent.

### 4. Some changes are load-bearing and cannot be notified at all — *correctness boundary*

Not everything can be handled by a notice. The notes list changes that are simply disallowed without a compaction or context rebuild, and the clearest is a **changed limb**: the limb determines the whole context hierarchy, and that hierarchy is load-bearing (limb-model's why #1 — a place's instructions are what let the model act correctly in that place). You cannot append "by the way, you are somewhere else now" and expect correct behaviour.

The same reasoning is why changing the **agent role / mandate** leans disallowed: "model can't be expected to respect role changes that occur later in the context." A late append does not retroactively reframe a conversation. The model's behaviour is anchored by what it was told at the start, and appending a contradiction produces an agent that is inconsistent rather than updated. Working directory and hostname are flagged as maybes on the same grounds; model change is explicitly unclear.

This root matters because it defines the design's edges. Notification is not a universal solvent — part of the deliverable is an honest classification of which context elements are notifiable and which force a rebuild.

### 5. Time passes invisibly — *correctness*

An agent has no clock. Between its last response and the user's next message, an hour or six weeks may have gone by, and nothing in the context distinguishes those cases. The failure is concrete: the agent resumes as though continuous — assuming its branch is current, that the command it ran is still meaningful, that "just now" was just now — and acts on a world that has moved. Hence the special handling: inject elapsed time past roughly an hour, below which there is no point.

### 6. The up-front context budget is contended, and it is paid on every single session — *resource, with a real tradeoff*

Progressive disclosure has its own root. The user wants a large library of skills and tools available, but skill and tool descriptions "can otherwise take up massive context paid on *every* session". Every description present up front is a permanent tax on every session that never uses it.

The tradeoff is named honestly in the notes and must not be flattened: always-up-front cost versus conditional, repeated, cached-input cost from tool calls that fetch more detail on demand. Gating skills behind more broadly applicable ones, and writing descriptions that say *when to load*, is the mechanism. Note the dependency the note draws out — this only works if the descriptions are well written, which is why it wants an information-architecture skill and a skill-writing workflow alongside. The mechanism has a documentation prerequisite.

The limb model contributes to the same goal from a different direction: a subagent given a specific limb gets a context-specific tool set, so those tools need not exist in every session at all. Progressive disclosure and the limb model are two solutions to one pressure.

## Forward: what these roots force

Chaining forward from the roots, before detailing anything:

- **Two modes, and a prediction.** Because of #2, the harness must operate in either append mode or rebuild mode, and it must *decide which* by predicting cache state. That prediction becomes a first-class piece of machinery — and it is shared with compaction, which needs the same judgement.
- **Rebuild is free exactly when you were already paying.** If a cache miss is expected anyway, there is no reason not to optimise: refresh AGENTS.md and skill content, truncate old tool output harder, canonicalise everything. So rebuild is not a fallback, it is an *opportunity* that arrives on a schedule set by cache expiry.
- **Every context element needs a declared update policy.** From #4: notifiable, notifiable-with-full-injection (the schema-changed-tool case, where a bare notice would leave the agent calling a tool wrongly), or rebuild-only. This classification is the core deliverable.
- **Change detection is required and is not free.** Something must notice that a skill file, an AGENTS.md, a tool set, or an option set changed. That is a watcher, and watchers live in the limb — which means the limb reports change and the brain decides what to do about it.
- **Notices must not trigger requests.** Invariant 2 and `context-and-agent-loop.md` are explicit: appending context is not the same as asking the model. A change notice piggybacks on the next real request. Getting this wrong turns a file save into a paid API call.
- **Rebuild must not replay history.** A rebuild produces the canonical *current* context; it must not carry forward the append-only notices that described how the old context got out of date, since they are now noise describing a superseded state.

## Parked for later stages

**"What" material already in the notes:** the change categories (skill content, new skills, tool availability, changed tool schemas needing full injection, AGENTS.md and global/machine/user context, available agent types and limbs for the subagent tool, other tool option sets); the >1h time rule; the disallowed set (limb definitely, cwd/hostname maybe, model unclear, agent role leaning no); skill gating and load-when descriptions.

**Open experiments named in the notes:** provider cache semantics for append versus rebuild versus forks — what append mode is actually *with respect to* for a forked subagent (possibly the parent as of the message before the subagent tool call, which needs testing per `context-and-agent-loop.md`).

**Interactions flagged for stage 3:** user-turn and self-modification (both *create* the mid-session changes this design handles — the causal link in why #1); compaction-handover (shares the cache-state prediction; a handover is the big rebuild moment, and its old→new context diff is the same information a notice carries, at a different boundary); limb-model (per-limb tool sets serve progressive disclosure; a changed limb is the canonical rebuild-only change); forked-subagents (fork is append mode with respect to an ambiguous baseline); persistence-analytics (notices and rebuild boundaries are durable session facts, and cache-state prediction needs stored cache metadata).

## Questions for review

- Is the attention-before-cost framing of why #3 right, or is the honest root simply resource? The notes emphasise cost; the attention argument is mine.
- Why #1 claims this design is load-bearing *because* user-turn and self-modification create routine mid-session change. That reframes context-updates from a good-taste item to something closer to soul. Do you agree, and if so should it move bucket in `PLAN.md`?
- Should cache-state prediction be its own experiment? It is now required by context-updates, compaction-handover, and forked-subagents alike, and all three are blocked on the same unknown provider cache semantics.
