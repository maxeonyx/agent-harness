# Context updates and progressive disclosure

Provenance: why layer reviewed by Max 2026-08-12 ("Yes, that's great! That's correct."). What/interactions/summary not yet written.

Sources: `docs/source-notes/context-updates.md`, `docs/source-notes/context-and-agent-loop.md`, `docs/process/REQUIREMENTS.md` §"Decisions from design review".

## Why

**Why 1 — the user changes things while sessions are live.** Max iterates on skills, AGENTS.md files, tool schemas, and prompts constantly (the process-improver stakeholder). Sessions are long-lived and many run at once. So a live agent will routinely hold facts that have gone stale. For example: an agent loaded the `github` skill an hour ago; Max has since rewritten its merge instructions; the agent is about to merge a PR the old way. Root: **correctness** — the agent's actions should follow from current reality, not from a snapshot of it. Elapsed time is a sub-case: "the time is roughly X" is a fact that goes stale ("It can be many weeks in some cases!").

**Why 2 — the cache forbids the obvious fix.** The obvious fix for a stale fact is to edit the context in place. The cache forbids that: a cached context is append-only, and editing any earlier byte forfeits the prefix. This is not a why — it is the **constraint** that shapes the solution space (append a notice, or wait for rebuild).

**Why 3 — a quiet session must cost nothing.** Facts change whether or not any session is active. If change notices caused API requests, every edit to a skill would bill every idle session that ever loaded it. Root: **irreducible resource pressure** — the why under piggybacking.

**Why 4 — up-front content is paid by every session.** Skill and tool descriptions in real-world cases "take up massive context paid on *every* session" (source notes). Root: the same resource pressure as Why 3, at session start instead of mid-session. This is the why under progressive disclosure, and why this doc covers both topics: they are one economics.
