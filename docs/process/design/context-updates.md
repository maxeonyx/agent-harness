# Context updates and progressive disclosure

Provenance: why layer reviewed by Max 2026-08-12; core claims reviewed by Max 2026-08-12 with line-item corrections (notice minimality, tool-schema logic, cold-context revival, economics as fundamental — recorded in REQUIREMENTS.md). Detail layer, interactions, and summary not yet written.

Sources: `docs/source-notes/context-updates.md`, `docs/source-notes/context-and-agent-loop.md`, `docs/process/REQUIREMENTS.md` §"Decisions from design review".

## Why

**Why 1 — the user changes things while sessions are live.** Max iterates on skills, AGENTS.md files, tool schemas, and prompts constantly (the process-improver stakeholder). Sessions are long-lived and many run at once. So a live agent will routinely hold facts that have gone stale. For example: an agent loaded the `github` skill an hour ago; Max has since rewritten its merge instructions; the agent is about to merge a PR the old way. Root: **correctness** — the agent's actions should follow from current reality, not from a snapshot of it. Elapsed time is a sub-case: "the time is roughly X" is a fact that goes stale ("It can be many weeks in some cases!").

**Why 2 — the cache forbids the obvious fix.** The obvious fix for a stale fact is to edit the context in place. The cache forbids that: a cached context is append-only, and editing any earlier byte forfeits the prefix. This is not a why — it is the **constraint** that shapes the solution space (append a notice, or wait for rebuild).

**Why 3 — a quiet session must cost nothing.** Facts change whether or not any session is active. If change notices caused API requests, every edit to a skill would bill every idle session that ever loaded it. Root: **irreducible resource pressure** — the why under piggybacking.

**Why 4 — up-front content is paid by every session.** Skill and tool descriptions in real-world cases "take up massive context paid on *every* session" (source notes). Root: the same resource pressure as Why 3, at session start instead of mid-session. This is the why under progressive disclosure, and why this doc covers both topics: they are one economics.

## What

### Core

1. A context is append-only while cached. The system section (system prompt + tool schemas) changes only at a rebuild.

2. A context has several cache prefixes at once, nested: the system section; everything up to each fork boundary; the whole context so far. User-facing sessions also keep a cache point ~n−2 messages back, so message undo lands on a warm prefix.

3. When a fact changes (a skill, an AGENTS.md, a tool schema, time passing): every future rebuild includes it automatically — nothing to design there. A live session may additionally get an appended notice. Some changes just wait for rebuild, and waiting is safe in one of two ways: the old behavior is kept valid (the limb keeps accepting old tool calls — claim 7), or the change doesn't affect correctness.

4. Notice decision 1 — notify at all? Only if the change could alter the agent's actions. That is what actionability means: if it would not change the agent's actions in any way, it doesn't need to know.

5. Notice decision 2 — how much does the notice carry? A spectrum: from the vaguest pointer ("something has gone stale") through a name/path, up to the information itself. The choice is economic — the same economics as claim 10, plus the agent's reaction to each form. All else equal, the minimum. Whatever the form, the agent must be able to re-discover reality reliably and cheaply — it should never have to reload everything just to be sure.

6. A notice never causes an API request. It piggybacks: appended, then carried by the next request that happens for a real reason (turn end, tool-loop continuation, handover). A quiet session never pays.

7. Old tool calls keep working: the limb accepts calls against a schema any live session still has cached, for as long as that session could still issue them. The bound is the question "is there ever going to be another use of this tool code version, or not?" — the obligation ends when every session holding the old schema has been rebuilt or discarded. (Sessions need not actually get rebuilt until they are loaded up again; and a session revived as warm — claim 9 — carries its old schemas past cache expiry.)

8. A rebuild produces the canonical current context: new facts in, old notices dropped. The fresh rebuild is the new snapshot; notices are events that get rolled in.

9. The ideal for an expired ("old cold") context: revive it as warm — re-send it exactly as it was (never rebuilt; it is the event log of that session), append notices (perhaps copious), and keep going. The cost logic: compacting re-bills the whole context at input anyway; for the ~same money (cache write is ~1.25× input), pay cache write instead and don't compact. The carve-out is correctness. An old context contains old info; where that matters for correctness, notices or rebuild are needed. We're relatively sure tool schemas and tool presence can't be fixed via notices, and we don't want to keep old tool code versions around forever — so a rebuild may have to be forced *if the context contains tool description content that is stale in a correctness-affecting way*. The same logic applies to any other correctness-affecting stale content — perhaps subagent description content, for example. Perhaps a user option to compact. Not fully settled.

10. The fundamental is the economics of a contingent choice. A notice instead of content means: unconditionally smaller input, plus conditional billing of an extra turn (more cache read) in the branch where the agent does fetch. Content instead of a notice means: unconditionally larger input at input cost, no extra turn. Which side wins depends on how often the branch is taken. Progressive disclosure at session start is exactly this choice — descriptions up front, content on demand.
