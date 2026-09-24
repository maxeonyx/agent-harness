# Experiment Outcome: forked-subagents

Experiment: forked-subagents, per `forked-subagents-brief.md`. Status: **built and measured, awaiting Gate 1** — the brief's exit condition also needs Max to run a task himself and watch it. Requirements tested: invariant 6 (structured subagent concurrency), with 1, 2, 8 and 9.

## What the experiment is

`experiments/forked-subagents/`: `forks`, a Rust/tokio harness in which a `task` tool call is a scope. The caller is suspended inside the call, its children run concurrently and may open scopes of their own, and the call returns one tool result holding every child's handoff. Children fork (the parent's context, then a tail addressed to them) or start fresh (the shared system prompt, then their assignment). It talks to OpenRouter's chat-completions API with the provider pinned, and its limb is one read-only directory. `forks run` and `forks chat` are for watching it; `forks bench` measures whether forked children stay inside their assignment; `forks rescore` re-scores recorded benchmark runs offline. 35 black-box scenarios drive the binary against a fake provider in under two seconds.

## What the experiment proved

**The scope model holds, and it is observable at the wire.** The scenarios show, from the requests the fake provider received: siblings' requests overlap in time (a barrier that a serial harness cannot pass); the parent's next request comes only after every child has been answered and carries exactly one tool result with every handoff in declared order; a child declared `after` a sibling is sent only once that sibling has finished, and carries its handoff; nested scopes resume bottom-up. Mutating each of these into the wrong behaviour fails the matching test.

**Forking is cache-cheap, on the first request, at realistic prefix sizes.** Across the benchmark, a forked child's *first* request read 82–93% of its prompt from cache on `claude-sonnet-5` and 93–99% on `gpt-5.6-luna`. A fresh child's first request read 0%, every time. Every agent in a run is given a byte-identical system prompt and tool list, which the scenarios assert on every request, because a single difference moves the start of the cached prefix. Two conditions came with it. The provider has to be pinned: unrouted, OpenRouter once sent both parallel children to a backend that had never seen the prefix (`probe/runs.md`). And at a ~1.2k-token parent prefix, children read nothing from cache in four runs out of four, while the parent's own later request with the same prefix did. That is not explained.

**What decides whether a forked child stays inside its assignment is how the assignment reaches it, more than what the assignment says.** The benchmark gives the root a known tree to build — one agent per region, and each region one agent per branch file, which is A → A.1 → A.1.3 — and scores each agent from what it actually read and what totals it claimed. Two knobs vary: the *cut*, meaning what the child's context ends with, and the *words* of its assignment. Valid trials only, both word variants pooled:

| cut | what the child's context ends with | sonnet: leaves that over-reached | sonnet: trees built as asked | luna: leaves that over-reached | luna: trees built as asked |
| --- | --- | --- | --- | --- | --- |
| `full` | the parent's `task` call, then a tool result addressed to this child | 9 of 14 | 0 of 4 | 0 of 42 | 7 of 7 |
| `own` | the same, but its copy of the call lists only its own entry | 10 of 12 | 0 of 4 | 24 of 45 | 2 of 6 |
| `before` | the parent's messages before that call, then a user message with the assignment | 3 of 33 | 5 of 6 | 4 of 37 | 5 of 6 |
| fresh | the system prompt, then a user message with the assignment | 0 of 30 | 4 of 5 | 0 of 48 | 8 of 8 |

On `full` and `own`, Sonnet's failure is the one Max predicted, but a level up: a region agent does not do its own region; it re-issues the root's split. The recorded trees read `root › region_awa`, `root › region_maunga › region_awa`, `root › maunga_region › awa_region2 › awa_region3`, and once an agent launched a `test_probe` child to try the tool out. A child whose context ends in "I called `task` with these agents", followed by a tool result saying "you are one of them", has been told it is both the parent and the child, and Sonnet mostly acts as the parent. When the assignment arrives as a user message instead (`before`, and fresh), it mostly does not.

The words mattered less, and in no consistent direction. `explained` rescued Luna on `own` (23 of 29 leaves over-reached with `stop`, 1 of 16 with `explained`) and did nothing for Sonnet on `own` (6 of 6).

`own` was expected to help, because it hides the siblings' assignments from the child. It was the worst cut for both models.

**Fork and fresh cost about the same here, and the cache is not where the money goes.** The two completed Sonnet trees make the comparison: fork (`before`, `stop`) cost $0.20 and fresh (`explained`) cost $0.18. In the fork tree, eight children reading the root's 7k-token context from cache cost $0.037 in total. Output tokens — mostly reasoning — were half the bill in both. On Luna, every arm that built the intended tree cost $0.010–0.012. This is Max's trade-off, "forked subagents have lots of context, but use more cache read on every turn (message, tool call). Fresh subagents have less context but every turn is cheaper", measured on a task whose leaves take two or three turns each. Fresh children never re-read the policy document; the recorded Sonnet `task` calls copy its arithmetic rules into every assignment.

**In-band and out-of-band failure can be kept apart mechanically.** A provider rejection, a panic, the spend cap and the turn limit each fault the agent concerned: its ancestors stay suspended, the run stops, and it says where. None of them becomes a tool result. The benchmark treats a provider-caused fault as an *invalid* trial, excluded from every rate, and keeps budget and runaway faults as scored behaviour. An agent that cannot do its task says so in its handoff and completes.

## What the experiment failed to prove

- **That a forked child can be made to stop at A.1.3 reliably.** Sonnet's best cell, `before` with `stop`, had no leaf over-reach in 18, but only one of its three trees ran to completion. `before` with `explained` had 3 of 15, all from the one tree that completed. In that tree a leaf re-issued its region's split, the depth limit refused it with `Do this work yourself`, and the leaf then did the whole region's work. So the depth-limit wording may itself cause over-reach, and it was not varied.
- **Anything statistically firm.** There are one to five valid trials per cell. All eight Sonnet `before` and fresh trials in the first grid were stopped by a per-trial cap of $0.15, which turned out too tight for a well-behaved Sonnet tree, so none of them has correct totals. The first Sonnet `full`/`stop` trial's evidence was destroyed when two benchmarks wrote into the same directory in the same second; the collision is fixed.
- **Why small prefixes are not inherited**, and whether `session_id` does anything.
- **First-party provider behaviour.** Everything went through OpenRouter to Bedrock (Sonnet) and Azure (Luna). The account's zero-data-retention setting excludes the first-party Anthropic and OpenAI endpoints.

## Rulings made during review

None yet.

## Review summary

One fresh-context review of the build, before the evidence was written up. It found four ways to fool the benchmark's scoring. The worst was that naming a sibling in a handoff counted as over-reach, which biased the result against the `explained` words. It also found that cancelling did not stop retries, that a panicked agent kept a `running` state forever, that `chat` spun a core on stdin EOF, and that a second Ctrl-C was swallowed. All of these were fixed with a test that fails first, and the paid runs were re-scored offline from their recorded wire logs. On this data the corrected scorer changed no over-reach count, though it flagged three agents as unscoreable. Separately, the Luna grid showed that OpenRouter reports a rate limit as HTTP 200 with an error body, which the harness had treated as fatal. Rate limits are now waited out, and the nine trials they had already killed are classed invalid.

## Known accepted limitations

- One `task` call per assistant turn; a second gets an error result.
- The spend cap is a gate checked before each HTTP attempt, not a hard limit; a wide fan-out can land somewhat above it.
- Rescoring old runs infers why a trial faulted from the fault message; runs recorded since then store the reason.

## What must not be integrated

- Any of this code by copying (invariant 8).
- The benchmark's forced tree shape as a model of real use. It exists to put A.1.3 in front of the model and score the result.
- `--cut`, `--words` and `--mode` as product settings. They are treatments.
- OpenRouter as the provider boundary.

## Tests to promote or preserve

`experiments/forked-subagents/tests/scenario.rs` shows the black-box shape a core scope implementation needs. It observes at the provider wire, never internal events. It proves concurrency with a barrier and ordering with message content, not with sleeps. It holds requests open for cancellation. And on every request it asserts that the system prompt and tool list are byte-identical across the tree.

## Invariants check

- **6** — upheld and tested: parents block on children, and sibling results reach only the parent, in one tool result.
- **9** — upheld after review. Cancel starts nothing new anywhere in the tree, keeps in-flight responses and ends every agent with an outcome; `panicked` is recorded.
- **2** — upheld: handoffs are appended inside the parent's one tool result, and only the scope completing triggers the parent's next request.
- **1** — upheld: the key lives only in the process that makes requests, and is absent from the wire logs, summaries and contexts it writes.
- **8** — upheld: everything is under `experiments/`.
- **Newly informed:** `model-framing.md` open questions 1, 4 and 5 now have evidence (above).

## User acceptance

Pending.
