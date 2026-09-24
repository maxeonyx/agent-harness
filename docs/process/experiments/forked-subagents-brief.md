# Experiment Brief: forked-subagents

Pulled 2026-09-24. Max: "I want to start building those experiments." And, 2026-09-08: "forked subagents is the one that drives me. I want to see "agents as structural concurrency"".

Thesis: agents can be run as structured concurrency — a parent's `task` tool call is a scope; the parent is suspended inside that call while its children run concurrently, possibly nesting scopes of their own; and the call returns one tool result holding every child's handoff. A forked child's context is the parent's context byte for byte, up to and including the `task` call, followed by a tool result addressed to that child alone — so every sibling reads the parent's prefix from the provider cache and pays full price only for its own tail. Falsified if: forked children do not read the parent's prefix from cache on a real provider; or a forked child cannot be framed so that it reliably stops at its own assignment, which Max names as the thing the feature depends on — "It must end its turn after A.1.3 - this must be reliable for forked agents to work well"; or the scope model is formally clean but not understandable when watched running. Invariants touched: 6 (structured subagent concurrency — the subject), 9 (cancelling a suspended parent cancels its whole subtree, every agent ending with a recorded outcome), 2 (a child's handoff is appended to the parent inside one tool result; only the scope completing triggers the parent's next request), 1 (credentials stay in the one process that makes requests), 8 (everything stays in `experiments/`).

## Evidence before the build

A probe on 2026-09-24 (`experiments/forked-subagents/probe/`) sent the fork shape above to `anthropic/claude-sonnet-5` through OpenRouter: a 7.7k-token prefix, one `task` call, two children fired in parallel, each given its own tool result.

- With the provider endpoint pinned, both children read 7,709 of 7,897 prompt tokens from cache and wrote only their ~150–190-token tail — the parent's `task` call plus their own tool result. Four runs out of four, on two providers.
- Unpinned, one run of two missed the cache for both children completely: OpenRouter routed them to a backend that had never seen the prefix. Fork cheapness depends on routing, not only on bytes.
- Told "You are fork 0. Report only k3, then stop.", fork 0 also reported its sibling's key in all three runs whose answers were captured; fork 1 did in one of three. The failure Max predicted appeared on the first attempt with naive framing. Raw numbers: `experiments/forked-subagents/probe/runs.md`.

## What the experiment builds

A small harness under `experiments/forked-subagents/` that Max can run and watch:

- **Scopes.** A `task` call names one or more children. The parent is suspended until every child has finished; then the call returns one tool result carrying each child's handoff. A child's handoff is its last message part — "A result is the last message part in a turn." Children may call `task` themselves, to a depth limit.
- **Ordering inside a scope.** A child may be declared to start only after named siblings finish, and it then receives their handoffs in its assignment. This is the case Max describes as settled — "the parent could say, launch A, B, and C, but B only starts once A finishes" — and not the one he left open, re-wiring dependencies after launch.
- **Fork and fresh.** Fork is the default and follows the thesis. Fresh gives the child only the system prompt and its assignment. Both exist so their costs can be compared on the same task: "forked subagents have lots of context, but use more cache read on every turn (message, tool call). Fresh subagents have less context but every turn is cheaper."
- **Failure.** Two kinds, per Max's L1 answer. A child that cannot do its task says so in its handoff, and that is a completed child. A failure the harness itself cannot get past — the provider rejects the credentials, the spending cap is reached — is not completion: the failing agent is marked faulted, every ancestor stays suspended, and the run stops and says where.
- **Cancellation.** Cancelling cancels the whole tree. Every agent ends with a recorded outcome, and a response that had already completed is kept.
- **A read-only limb.** The children's only tools are `list_dir` and `read_file` over one directory. With nothing writable, siblings sharing a filesystem cannot race — a question this experiment deliberately leaves open.
- **A face you can watch.** An append-only event log, each line prefixed by the agent's path in the tree; a tree snapshot on demand and at the end, with each agent's state, requests, cache reads and writes, and cost; and each agent's final context written out as a file, so a fork's context can be read and seen to be its parent's.
- **Evidence on disk.** Every request and response body is recorded. The spend cap is enforced before each request is sent.

## What the experiment measures

The framing question is measured, not argued. A benchmark gives the root a task with a known tree shape — the root forks one child per region, and each region child forks one child per file — so every level of the A → A.1 → A.1.3 nesting in Max's note is present. Scoring is mechanical, read from the tool log and the handoffs: whether a leaf read any file other than its own, whether a region did its children's work itself, and whether the root's final numbers are correct. It runs across framings of the child's tail, which is `model-framing.md` open questions 1, 4 and 5 put to the model instead of to Max:

- the child's tool result names its assignment and tells it to stop — the framing that failed in the probe;
- the child's tool result also explains that it is one branch of a split, that its siblings hold the other assignments, and that its last message is what the parent receives;
- the child's copy of the `task` call is rewritten to show only its own assignment — which, per the probe, costs nothing extra in cache, because every child writes that turn afresh anyway;
- the child's context ends at the message *before* the parent's `task` call, and the assignment arrives as a new user message — the other option Max names: "possibly w.r.t the parent *as of the message before it sent the subagent tool call* - but that needs experimenting too."

The same benchmark in fresh mode gives the fork-versus-fresh cost comparison. Each result is reported per model — `anthropic/claude-sonnet-5` and `openai/gpt-5.6-luna` (Max, 2026-09-08: "say if we only use claude-sonnet-5 and for gpt-5.6-luna").

## Deliberately out of scope

User-facing children, `/done` and the main-thread pattern (UX & input is not yet elicited); siblings launching siblings into their own scope; re-wiring dependencies after launch; resume; compaction inside a scope; persistence and restart; limbs other than the one local read-only directory; writes and shared-workspace races; attachments and shared seed contexts; two-part launch; the direct provider APIs (OpenRouter's chat-completions API only — `provider-cache-probe` remains the place for first-party wire facts).

## Exit condition

Max runs a task himself and watches the tree suspend, fan out and resume, and the benchmark has produced, for each framing and model, discipline scores, correctness, cache-read share and cost — enough to say which framing makes a forked child stop at A.1.3, or that none does.
