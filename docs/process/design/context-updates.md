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

Three levels of context maintenance, cheapest first — the original vision: keep using the warm context; rebuild the existing context (incorporate notices etc.); compact (make a fresh context).

1. A context is append-only while we believe it's cached. The system section (system prompt + tool schemas) changes only at a rebuild.

2. A context has several cache prefixes at once, nested: the system section; everything up to any fork boundary; the whole context so far. User-facing sessions also keep a cache point ~n−2 messages back, so message undo lands on a warm prefix.

3. When a fact changes (a skill, an AGENTS.md, a tool schema, time passing): every future rebuild includes it automatically — nothing to design there. Then the unique thing: a live session may additionally get an appended notice, so that the agent can know about the change. Changes can just wait for rebuild, if waiting is safe in one of two ways: agent behavior based on the old information is kept valid (eg. the limb keeps accepting old tool calls — claim 7), or the change doesn't affect correctness of the agent's behaviour.

4. Notice decision 1 — notify at all? Only if the change could alter the agent's actions. That is what actionability means: if it would not change the agent's actions in any way, it doesn't need to know.

5. Notice decision 2 — how much does the notice carry? A spectrum: from the vaguest pointer ("something has gone stale") through a name/path, up to the information itself. The choice is economic — the same economics as claim 9, plus the agent's reaction to each form. All else equal, the minimum. Whatever the form, the agent must be able to re-discover reality reliably and cheaply — it should never have to reload everything just to be sure.

6. A notice never causes an API request. It piggybacks: appended, then carried by the next request that happens for a real reason (user message, tool result). An inactive session never pays.

7. One notable correctness example: Tool calls issued by the agent should always work. If a new version of a tool is loaded, in particular if it has a different *description* (including schema), then limb should issue calls against the old tool version on any live session that still contains the old description. This means retaining two or more versions of the tool implementation while there is any session. Why? We don't think issuing a notice for tool description changes or tool call changes is sufficient for correct agent behaviour. Thus a rebuild is required for tool version changes, but we also don't want to *force* all sessions to rebuild immediately. The bound is the question "is there ever going to be another use of this tool code version, or not?" — the obligation ends when every session holding the old schema has been rebuilt or *would be rebuilt before it could be used* (see 8.).

8. The ideal for an expired ("old cold") context: revive it as warm — re-send it exactly as it was (never rebuilt; it is the event log of that session), append notices (perhaps copious), and keep going. The cost logic: compacting re-bills the whole context at input anyway; for the ~same money (cache write is ~1.25× input), pay cache write instead and don't compact. The carve-out is correctness. An old context contains old info; where that matters for correctness, notices or rebuild are needed. We're relatively sure tool schemas and tool presence can't be fixed via notices, and we don't want to keep old tool code versions around forever — so a rebuild may have to be forced *if the context contains tool description content that is stale in a correctness-affecting way*. The same logic applies to any other correctness-affecting stale content — perhaps subagent description content, for example. Perhaps a user option to compact. Not fully settled.

9. The economics of notice content & frequency is based on the following. A "reference" type notice is eg. "`skill-a` has new content". A "full" notice would instead be the full new content of `skill-a`, or perhaps a diff. Choosing "reference" instead of full means: unconditionally smaller input, plus conditional billing of an extra turn (more cache read) in the branch where the agent does fetch for the full content. Content instead of a notice means: unconditionally larger input at input cost, no extra turn. Which side wins depends on how often the branch is taken. Progressive disclosure at session start is exactly this choice — descriptions up front, content on demand. Frequency of notices (ie. whether they should be debounced or not) depends on how important it is for an agent to know about the content, how likely it is to overreact to the notice, and also the raw token cost of the notices themselves.
