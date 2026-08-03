# Compaction / handover — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why (user-involved) · what, interactions, summary — not yet done.** Derives from `source-notes/compaction.md`, the "Compaction note" in `source-notes/context-updates.md` (freshest thinking), and `source-notes/handoff-improvements.md`. Part of this design is already empirically validated in the user's OpenCode fork (`source-notes/open-code-inspiration.md` names the fork the behavioural source of truth where features overlap).

A naming caveat up front, because it colours everything: "handover" is an imperfect term. It is *othering* — it implies the agent is finished and a stranger picks up from scratch. That is roughly the mechanics, but not the feel the user wants. The intent is that **the agent considers itself continuing, yet still does everything it would need to do to hand over to a fresh agent.** The word is a stand-in for the *quality* being aimed at, not the feeling. The model-facing name stays open.

## Why

Priority, from the user: **#1 is most important; #3 and #4 together are the secret sauce.**

### 1. Continuity of long work past the context limit — *correctness*

Work longer than the context window simply dies mid-task without compaction. The situation: you're hours into a task, the context fills, and the agent stops with the goal lost. The user wants long work to *continue*, intact.

### 2. Above all, don't lose the goal and current place — *correctness (the cardinal sin)*

Compaction is lossy, so this is the quality bar: the one thing that must survive is intent and where you are in the work. This is what the "handover" framing is reaching for — the *quality* of continuation — rather than a "summarise"/"compact" framing. Not a claim that one word is good and another bad; a claim about what the operation must achieve.

### 3. Keep active context cheap — *resource* — **secret sauce**

Cache reads and writes are much cheaper than fresh input — roughly 10×. But cache-read cost re-accrues the *entire* context history on *every* subsequent tool response or user message. So once the context is long enough, its sheer length outweighs the per-token cache discount: even at 10× cheaper reads, 10× more context than a compacted version would carry means you are losing money versus having compacted earlier. Compacting earlier saves on *every* future turn.

### 4. The agent picks the moment, not the clock — *quality + resource* — **secret sauce**

A harness-forced cut fires mid-thought and loses more; the agent knows the good seams. Proactive, agent-chosen cutting is precisely what *enables* #3 — compact earlier rather than later — and the agent is who knows when it can afford to. Proactive timing also lets compaction happen in otherwise-dead time (waiting on a long command, or an absent user), so a short warm context is ready when work resumes, and a lapsed cache is a cheap miss.

Empirical knowledge from prior experiments: a compaction fired *just before an agent reports back to its parent* must be a **separate flow** from mid-task compaction. A compaction that *feels* mid-task makes the successor carry on with an already-*done* task instead of simply reporting back. So "compact-then-report-back" needs its own tool/flow distinct from "compact-and-continue". This is really an instance of #6 — the successor must be told which situation it is in.

### 5. Don't break the cache doing the thing meant to save the cache — *resource/correctness*

The two-stage append-only flow exists for this. It is the obvious piece that OpenCode currently gets wrong. Agents handle a compaction flow appended to the end of the context fine — it is not complicated, and the general shape is very likely well-represented in training data.

### 6. To write forward correctly, you must know what the fresh context will and won't contain — *correctness*

Obvious once followed through: a good compaction is one that empowers the next agent going forward. To know what to *spend effort preserving*, you must know what is *already covered* by the fresh context (and so needn't be repeated), which lets you leave out what won't be needed and concentrate on what will. This means the flow must inject facts about the new situation — notably the **diff between the current system prompt/context and the successor's**, so differences don't silently become stale or incorrect.

### 7. (maybe) Record what matters as it happens — *unsettled*

A stateful handover document the agent maintains throughout the session, rather than a one-shot end summary. Kept as a *maybe* — the user is not sure about it. Not treated as a settled why.

## Parked for later stages

- **Attachments** — the compaction/handover call can preload files/resources into the fresh context (structured compaction), saving the successor's opening turns of re-reading and their cache-read round-trips. Same resource root as forked-subagents' fresh-sibling attachments.
- **Two-stage flow's cleanup value** — the injected middle step exists so the agent tidies up *while it still has full context* to know for sure what it owns and no longer needs. Wait, and those facts are lost and it leaves crap around.

**Interaction flagged for stage 3:** the two-stage "tidy up before completing" step *is* cleanup-at-boundary from forked-subagents (why #4) — same ownership root, two experiments. Also interacts with context-updates: a handover is the big rebuild moment, and the old→new context diff (#6) is the same boundary context-updates handles without a rebuild.
