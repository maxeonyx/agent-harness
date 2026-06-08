# Compaction & Handover — Design Notes

> Status: **incomplete WIP** — being filled in iteratively

---

## Purpose of compaction

Compaction allows work to continue beyond model context limits. It also reduces cost by keeping active context short — but it is lossy, so the quality of what is kept and discarded matters enormously.

---

## The handover framing

Calling compaction a **handover** (rather than "summarise" or "compact") produces better results. The word naturally implies that all relevant information must be passed forward — goals, progress, decisions, blockers, next steps.

Current approach in OpenCode: a `/handover` command backed by a prompt (`handover.md`) that explicitly instructs the model to:
- Pay extra attention to any handover notes at the start of the conversation
- Carry all relevant context forward
- Maintain the goal and current place in the overall work

This works well in practice.

---

## Ideal compaction behaviour

In an ideal world, the model would:
- Discard only information that will definitely not be needed again
- Keep everything a future agent would need to start from scratch efficiently
- Most importantly: preserve the goal and the current place in the overall work

---

## Agent-controlled handover

Giving the agent control over when to trigger a handover (rather than waiting for the harness to force it) is useful — the agent can call it proactively before cache expires, rather than waiting for the user to notice.

**NOTE**: done in opencode fork - handover tool works well. just recently done: "two stage" handover - two tools (handover and handover_complete) - the agent calls the first one, which then injects a user message with all the required info for the agent to perform a good hand over. it also allows & encourages the agent to call other tools to tidy things up prior to completion of the handover.

Benefit of the handover tool is that no cache break is required. I think it is typically worth it at a conservative assumption of 5x cheaper cache reads.

---

## Cache-aware compaction (idea)

If compaction is cheap and reliable enough, the harness could track KV cache expiry and trigger compaction *just before* the cache expires — e.g. while waiting for a long-running command to finish, or while waiting for the user to respond.

Effect: a much shorter context is sitting warm when the command completes or the user replies. If the cache expires before that happens, it's a much cheaper miss to resume from.

Caveat: requires the compaction/handover procedure to be well-tuned. If it loses important info, compacting frequently makes things worse, not better.

---

## Stateful handover document (aspiration)

Rather than compaction producing a one-shot summary, the ideal is a **stateful document that the agent actively maintains** throughout the session — updating it as goals shift, work completes, and new context accumulates.

Current pragmatic approach: a gitignored file in the project that the agent writes to and reads from. Works reasonably well. Proper first-class support for this is a future design question.
