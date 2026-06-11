# Agent Harness Architecture — Design Notes

> Status: **incomplete WIP** — being filled in iteratively

> **Terminology note:** "limb" is a placeholder name used throughout. Candidates: **workspace**, runtime, **context**, environment, sandbox. Not yet decided.

---

## Core concept

```
clients (faces)  <->  control server (brain)  <->  workspace runtimes (limbs)
```

- **Faces** — TUI, web UI, desktop, IDE plugin, etc.
- **Brain** — owns the agent loop, model calls, session storage, provider credentials, billing, rate limits, compaction, permission decisions
- **Limbs** — the context and execution environment for a project. Own: filesystem, shell, git, grep, formatters, linters, tests, diagnostics, possibly LSP, context curation (what reaches the model from this domain)

**Key rule:** AI provider calls only happen from the brain. Limbs have no model credentials and run no agent loop.

---

## Why

Current problem: N projects × full harness memory (~400–500 MB per busy instance).

Desired model:
```
1 brain (moderate fixed cost)
+ N lightweight limbs (small idle cost, <100 MB target)
+ LSPs: lazy, evictable, active only when useful
```

Memory should scale with active work, not with every open project.

---

## Limb is the context and execution environment

The limb is not just a tool surface — it is the full context environment for a project. It aggregates context from multiple sources and presents a unified environment to the agent. Tools are one aspect of what the limb provides.

The limb is authoritative over:
- **What tools are available** — the limb declares a tool set. The brain may add to it or filter it.
- **What context reaches the model** — e.g. in-repo AGENTS.md files, per-machine AGENTS.md files, checked in agent & skill definitions. excecution related context: truncating large tool outputs, appending LSP info to tool calls. 
- **How execution happens** — the limb server owns the filesystem, shell, git, and all local execution.

The limb does not *store* the session context / tool set / etc. The brain does that. A session keeps a fixed context / tool set / etc. so long as the cache is valid. This means persisting, as well, to allow restarts of the harness. If we suspect the KV cache is no longer warm, we can rebuild the context prior to sending the API request.

Resolved:
- Brain may offer its own tools (e.g. session history, agent status) but does not filter or redefine limb tools by default, unless the user would configure it. The brain does not do any limb tool call processsing, only dispatching.
- A session is bound to exactly one "main" limb. No multi-limb sessions. Switching context = launching a fresh subagent in the other limb. This is because a limb / context might have load bearing instructions in AGENTS.md files etc. - so to do something in that context, you MUST do it via an agent in that context.
- Zero-tool limb (pure model session) is a valid type.

---

## Limb types (examples, not exhaustive)

- **Full local limb** — subprocess in a git repo on the same machine as the brain. Filesystem, shell, git, grep, formatters, linters, tests, LSP (lazy). Lives as long as connected to the brain or until the brain exits.
- **Remote limb** — same capabilities, over SSH + tunnel on a remote server. Lifecycle is related to but distinct from the SSH connection. Stays alive on (un-graceful) disconnect in case of reconnect, but shuts down if not reconnected within a timeout.
- **In-process meta limb** — lives inside the brain process. Provides meta/global tools (e.g. cross-session search, agent status, config edits). No project-local filesystem.
- **Remote limb-as-a-service** — permanently available authenticated endpoint, e.g. with database-backed tools.
- **Read-only limb** — only read/grep/glob, no writes or shell.
- **Pure model limb** — no tools, no injected context beyond the prompt. Use cases: cheap answer from model memory, baseline comparisons, classification tasks.

---

## Limb lifecycle

- Local limb: lives as long as connected to the brain, or exits with the brain.
- Remote limb: shuts down if disconnected and not reconnected within a timeout.
- Limbs that have no brain connection do nothing and might as well not exist — this is fine.
- Graceful shutdown semantics are important.

---

## Brain crash/restart semantics

Brain works out of the SQLite DB. On restart, the brain should pick up where it left off without requiring user interaction to resume threads.

- If interrupted while waiting on a model response: safe to resend and continue.
- If interrupted mid-tool-call: the tool reports something like "tool call interrupted by harness crash"; the agent reasons about state and safety itself.
- Limbs that were connected may have shut down; they reconnect or are restarted as needed.

Graceful shutdown is a priority — crashes should be rare, not designed around.

---

## Concurrency and shared mutable state

Fork-by-default means parallel sibling agents may share the same limb and the same filesystem. This is a real race surface — experience with multiple agents working on the same codebase simultaneously is poor.

Possible directions:
- Clear instructions and context telling forked agents what scope of the workspace they own
- Some kind of borrow-checker-style rule on mutable workspace regions (tentative, not designed)

Not resolved. Needs careful thought.

---

## Face↔Brain split

Not the main focus yet — but likely non-trivial to do well. OpenCode's current experience sets a high bar. The hard part is the brain↔limb split; the face↔brain split is deferred but not dismissed.

---

## Face↔Brain identity / auth

Not designed here — defer to existing solutions (e.g. how OpenCode, Goose, or ACP handle it).

---

## Refernce: Pi

Repo: `github.com/badlogic/pi-mono` — TypeScript monorepo, MIT, Bun-compiled self-contained binary.

### What Pi does well

- Clean agent loop with injectable `streamFn` and swappable tool execution
- Tools are plain objects with swappable execution backends (`BashOperations`, `ReadOperations`, etc.)
- Self-contained Bun binary — `scp` to remote, no Node/npm needed on target
- Extension system with ~30 lifecycle hooks, tool registration, provider registration
- `@mariozechner/pi-agent-core` is genuinely lightweight (~60KB source, lazy provider loading)

### Approach: rebuild fresh harness, but with close reference

Investigate other harnesses. Steal *designs*, NOT code. 

- codex
- pi
- goose
- openclaw
- hermes

### RPC mode is the wrong boundary

Pi's `--mode rpc` exposes the full agent loop headlessly. That's a remote brain, not a remote limb. Not what we want.

---

## Session storage: prefer OpenCode's SQLite model

| | Pi | OpenCode |
|---|---|---|
| Format | JSONL, one file per session | SQLite, single DB for all sessions |
| Discovery | Filesystem glob + file peek (O(n)) | SQL query with indexes (O(1)) |
| Branching | In-file parentId tree | New session row with parent_id FK |
| Multi-project | Separate dirs per cwd | Single DB, scoped by project_id |
| Compaction | `compaction` entry in JSONL | `CompactionPart` on assistant message |
| Migration | In-memory version bump + file rewrite | SQL schema migrations via Drizzle |

**Preference:** OpenCode's SQLite approach. Single DB, indexed, queryable. Better for a brain managing many projects.

### What storage needs to represent (partial)

Beyond basic conversation history, the hierarchy model places additional demands:

- Conversation threads, message contents, and relationships between threads (parent/child/sibling)
- Blocked parent state — a parent suspended waiting on a child scope
- Dynamic sibling sets — children added to a scope after the parent blocked
- User-facing session lifecycle state
- Resume targets — enough state to restart a session as a subagent
- Compaction/handover continuity
- API response metadata — cost, token usage, model, provider, per-message
- (more TBD)

This is more than a format swap — it is a purpose-built storage layer for the hierarchy model.

---

## More Efficient Task Handoffs

Design note: make handover and task launch cheaper by carrying context by
reference, and by sharing a cached prefix for fresh parallel subagents.

There are two separate ideas here.

### Context attachments on handoffs and tasks

The handover or task-producing agent should not only write a text handoff. It
can also attach context by reference: files, search results, notes, plans,
diffs, logs, or other artifacts.

The next agent then starts with the handoff message plus the referenced context
already attached. This reduces startup turns where the next agent would
otherwise need to ask for or rediscover that context.

This applies to both:

- **Handover** — continuing work in a fresh agent.
- **Task launch** — launching subagents.

Provisional shape:

```text
handover(context, attachments, task)
```

The handover tool is for continuation rather than parallel work. The current
agent provides the continuation task plus any context and attachments the next
agent should start with.

### Shared seeded prefix for fresh parallel subagents

For fresh parallel subagents, the task tool can accept:

- `shared_context`
- `shared_attachments`
- `tasks[]`

The harness first creates a shared seed context containing the common context
and attachments. It sends an initial request that effectively tells the model
to wait for task-specific instructions. This warms or caches the shared prefix.

Each parallel subagent is then launched by appending a different task-specific
suffix to that same shared prefix:

```text
shared seed:
  common instructions
  shared context
  shared attachments
  "wait for task-specific instructions"

forks:
  shared seed + task A
  shared seed + task B
  shared seed + task C
```

This lets all fresh subagents reuse the same cached prefix.

Task tool variants:

1. **Fresh subagent task**

   ```text
   task(shared_context, shared_attachments, tasks[])
   ```

   Used when children do not inherit the parent context.

2. **Forked subagent task**

   ```text
   task(tasks[])
   ```

   Used when children inherit the parent context or prefix, so no shared
   context needs to be reattached.

Main benefit:

- Attachments save turns and rediscovery.
- The shared seed flow saves cost for fresh parallel subagents by making the
  common context a reusable cached prefix.

Open design constraints:

- Attachments are context references, not a commitment to a specific wire
  schema yet.
- Public behavior and user-visible APIs must drive tests. Do not turn this note
  into internal event-schema tests.
- The harness needs a clear story for attachment lifetime, permissions,
  redaction, and whether an attachment is snapshot or live reference.

---

## Pi ecosystem notes

- Extensions are TypeScript modules, hot-reloadable, installable as npm packages
- `pi install npm:@foo/pi-tools` or `pi install git:github.com/foo/bar`
- Extension compatibility goal: extensions written for stock Pi should mostly work against the fork — but behavioural compatibility is not guaranteed

---

## Goose

Not yet investigated. May be relevant — reportedly moving toward ACP-based client/control-server separation. Key question: can it act as a central brain with project-local MCP-style runtimes, no credentials in runtimes?

---

## ACP / MCP notes (provisional)

- **ACP** — solves face↔brain (client/editor ↔ agent process). Non-trivial to do well, but not the current focus.
- **MCP** — plausible shape for the brain↔limb protocol. Limb exposes tools as MCP resources/tools. If MCP supports routing model completion requests back to the brain, that may be fine — limbs should not make direct provider calls, but could request completions via the brain. Whether this is even needed is unclear.

---

## Streaming

The face needs real-time output fairly directly from the limb — tool stdout, model tokens, etc. The brain should not buffer everything before forwarding.

Default path: brain proxies streaming output from limb to face. This is the safe default given that network topology may not allow the face to reach the limb directly (e.g. remote limb behind SSH, different network segment).

Fast path: if the face can connect directly to the limb (topology permitting), it does so, bypassing the brain for streaming. Brain is only in the path when needed (agent loop, model calls, permissions).

---

## Permissions / safety model

Not yet designed. Principle: strict and principled, or not at all. A half-measures permissions model is worse than none — it creates false confidence without real protection.

---

## Limb configuration

A limb is currently identified by:
- `ssh_host` — the machine it runs on (empty/local for same-machine limbs)
- `directory` — the working directory / project root

These are stored in the brain. That's likely all that's needed to connect, start, and reconnect a limb.

Other per-limb config that may be needed (not yet decided):
- Display name
- Which agent types are allowed to use it
- Resource limits

---

## Non-goals

- Better web UI (not yet — OpenCode already sets a high bar here)
- Running a full remote copy of OpenCode/Pi/Goose per project
- Moving provider credentials onto remote machines
- Making each limb an autonomous agent
- Duplicating rate limits, billing, or session management per project
