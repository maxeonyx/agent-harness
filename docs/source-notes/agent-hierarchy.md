# Agent Hierarchy Model — Design Notes

> Status: **incomplete WIP** — being filled in iteratively. Also - highly forked agent threads are not empirically tested and will likely need careful model-facing framing. As to the model, it appears it is given task A, then told to only focus on A.1, then told to only focus on A.1.3. But importantly, it must not continue on to complete A.1.4 or the full A.1. It must end its turn after A.1.3 - this must be reliable for forked agents to work well.

> **Terminology note:** "limb" is a placeholder name used throughout. Candidates: workspace, runtime, context, environment, sandbox. Not yet decided.

---

## Structured concurrency

The agent hierarchy uses structured concurrency. A parent agent blocks when it launches children, and resumes only when all children have finished.

```
parent (suspended)
  ├── child A (autonomous)
  ├── child B (autonomous)
  └── child C (user-facing) ← user interacts here
        ... all three run in parallel ...
parent resumes with { A: result, B: result, C: result }
```

---

## Scopes

A scope is created when a parent blocks. The parent does not continue until all agents in the scope complete. This is textbook structured concurrency — every task has a local owner (the parent). There is no global spawn context.

- There is no "launching into a child scope" — you either block (creating a scope with children) or launch a sibling into the current scope.
- Agents can launch siblings into their own scope. The parent gains a new child it didn't explicitly anticipate.
- Agents can view the *status* of their siblings (they share a scope), but not their *results* — results are only available to the parent when the scope completes.
- Agents cannot view the status of their children — they are suspended until all children complete.

---

## Fork vs fresh

- **Fork by default** — children start from a copy of the parent's conversation context. Good for KV cache reuse. Same limb.
- **Fresh** — required when crossing limb boundaries. Also valid within the same limb (e.g. spinning out a meta/global session in the home dir context).
- **Different limb → always fresh.** Same limb → fork by default, fresh allowed.
- Fresh wins over forked-but-stale when the task is narrow/focused — cheaper than a long-context cache miss if the new agent ends up needing less context.
- The stale-cache case (fresh subagent wanting forked siblings, but parent + all forks are stale): open question, tentatively lean toward fresh for narrow tasks.

---

## The Task tool

Agents launch subagents via a **Task** tool call. Parameters:

- **`task`** — the prompt / instructions for the subagent
- **`agent_type`** — which agent persona to use (see Agent Types below)
- **`user_facing`** — whether the session is user-facing or autonomous. Currently conflated with agent_type informally (some types are subagent-only, some user-facing-only) but kept as a separate orthogonal parameter in the model.
- **`context`** — which limb to run in:
  - omitted or `"self"` → fork, same limb
  - explicit limb id/type → fresh, that limb (including e.g. `"global"` for home dir / no-project context)
  - same limb id explicitly → fresh in same limb

### Optional pre-step (idea, not decided)

A temp fork whose only job is to write a good task prompt, because what belongs in the prompt depends on the target agent type/context. Tradeoff: adds latency and cost to every Task call.

---

## The Resume tool

Continues a previous agent session as a new subagent.

Parameters:
- **`id`** — ID returned by a previous Task call (on success or error)
- **`agent_type`** — open question: can a session be resumed as a different agent type? Does it break cache badly enough to matter, or should behaviour depend on whether cache is already expired?

---

## Agent types

Loosely maps to OpenCode-style agent personas. Currently informal — some types are subagent-only, some user-facing-only, but this is not a hard constraint in the model. Kept orthogonal from `user_facing`.

Not yet fully specified.

---

## The "main thread" pattern

When a parent wants to continue the conversation with the user while also doing parallel work:

1. Parent forks a **user-facing child** — from the user's perspective, the conversation just continues
2. Parent also launches one or more **autonomous siblings** in the same scope
3. User-facing child runs the interactive part; autonomous siblings run in parallel
4. User calls `/done` when satisfied — user-facing child completes
5. Autonomous siblings may still be running; scope completes when **all** are done
6. Parent resumes with all results — it does not see intermediate content, only final responses

---

## User-facing vs autonomous sessions

| | User-facing | Autonomous |
|---|---|---|
| Completion | User signals `/done`, then agent writes response | Completes automatically at end of turn |
| Launch | Requires user permission; only user-facing agents can request this | Can be launched freely by any agent |

Tentative rule: only user-facing agents can request to launch new user-facing sessions. Autonomous agents can only launch autonomous siblings. This avoids autonomous agents creating unexpected blocking dependencies on the user.

User-facing agents are already in a mode where blocking on user input is expected — a permission prompt is just another form of that.

Possible escape hatch: permission requests could expire after a timeout, so even a user-facing agent's request doesn't block indefinitely if the user is absent.

---

## Multiple user-facing sessions

Supported. They appear in a session switcher view. The user can have multiple active user-facing sessions and switch between them.

The user has a button to launch a user-facing session into a scope that doesn't currently have one.

---

## Agent naming

Auto-generated short descriptive names, e.g.:

```
tk-prodsync-findfiles
tk-prodsync-findfiles-refactor-imports
```

- Names get longer with nesting naturally
- User can edit names when spinning out a user-facing scope
- Main-thread child inherits parent name with a suffix (e.g. `'` or `*`) — not yet decided

---

## Results

A result is the last message part in a turn. Prompting determines what's in it — ideally "what the parent needs to know." Failure counts as completion with an error result. Whether one child failing aborts the scope or just propagates an error to the parent needs experimenting.

---

## Brain-owned tools for agents

The brain exposes tools to agents that are not limb-provided, e.g.:

- View session history
- View status of sibling agents
- Task (launch a subagent)
- Resume (continue a previous session as a subagent)

These are brain-native, always available regardless of limb.
