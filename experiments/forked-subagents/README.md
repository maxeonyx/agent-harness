# forked-subagents

Disposable experiment. Brief: `docs/process/experiments/forked-subagents-brief.md`.

`forks` runs agents as structured concurrency. A `task` call is a scope: the caller is suspended inside the call while its children run at the same time, possibly opening scopes of their own, and the call returns one tool result holding every child's report. A forked child's context is the parent's messages cloned, so it serializes to the same bytes and the provider serves the prefix from cache; only the child's own tail is new.

Two binaries: `forks`, and `fake-provider` — a separate HTTP server that answers from a script and records every request, which is what the scenario tests assert against.

## Run it

```bash
cd experiments/forked-subagents
cargo run --release --bin forks -- run --dir <a directory> "<a task>"
cargo run --release --bin forks -- chat --dir <a directory>
cargo run --release --bin forks -- bench --reps 1
cargo run --release --bin forks -- --help
```

The key comes from `$OPENROUTER_API_KEY` or `keys.ignore.env` beside this README. Runs are recorded under `runs.ignore/<timestamp>-<label>/`: `wire.jsonl` (every request and response body, tagged with the agent's path), `agents/<path>.md` (each agent's final context), `summary.json`. Both directories are gitignored.

`agents/*.md` render the same way for every agent, so a fork's context can be seen to be its parent's:

```bash
diff runs.ignore/<run>/agents/root.md runs.ignore/<run>/agents/root.alpha.md
```

In `chat`, a line is a message to the root and ends your turn. While a turn is running only `/tree` and `/cancel` are accepted; `/quit` exits. Ctrl-C cancels a `run`.

## The knobs

Every agent in a run gets the identical system prompt and the identical tool list — Anthropic caches tools, then system, then messages, so any difference between parent and child breaks the child's inherited prefix. The depth limit is therefore enforced by answering an over-deep `task` call with an error tool result, never by taking the tool away. All per-agent framing lives in the tail.

- `--cut full|own|before` — what a forked child inherits. `full`: the parent's messages through the assistant turn that called `task`, then a tool result addressed to this child. `own`: the same, except this child's copy of the `task` arguments holds only its own entry. `before`: the parent's messages up to, not including, that assistant turn, then a user message with the assignment.
- `--words stop|explained` — what the assignment says. `stop` is the probe's naive framing. `explained` also says it is one branch of a split, names the siblings holding the other assignments, and says its final message is exactly what its parent receives. These two texts are the experimental treatment; they are constants in `src/framing.rs`.
- `--mode fork|fresh|declared` — `declared` honours each `task` entry's own `fresh` flag, which is how the model routes; `fork` and `fresh` force every child.

The defaults (`full`, `explained`, `declared`) are a choice about what is pleasant to watch, not a finding. The benchmark is what decides between them.

## The benchmark

`forks bench` generates a fixture and gives the root a fixed task that forces the shape: the root forks one agent per region, each region agent forks one per branch file, so `A → A.1 → A.1.3` is on the page.

The fixture is `ledgers/<region>/<branch>.txt` — 2 regions, 3 branches each, ~40 dated amounts per file — plus `ledgers/POLICY.md`, a ~7,000-token accounts manual. The manual is mostly genuine-sounding boilerplate, and one section of it decides the arithmetic: comments and `VOID` lines carry no amount, a `REFUND` line is subtracted rather than added, and a `DUP` line is skipped. The root task says to read the manual and that the root is the only agent that should.

That is what makes fork and fresh a real choice rather than a cost difference on nothing. A forked child inherits the manual from the parent's cache and pays almost nothing for it. A fresh child knows only what its parent wrote into its assignment, so it either re-reads the manual or gets the refunds wrong. Without it the tree's contexts were around 1,200 tokens — the size at which forked children were observed not to read the parent's prefix from cache at all, so the comparison would have been run in the regime where forks cannot win.

Scoring is mechanical, read from the recorded tool calls and reports:

- **leaf over-reach** — a branch agent read another branch's ledger, called `task`, or named another branch in its report;
- **region over-reach** — a region agent read a branch ledger itself, or named a branch outside its region;
- **re-read policy** — an agent below the root read `POLICY.md` for itself. Not over-reach: for a fresh child it is the only way to learn the rules, and it is what fresh pays instead of inheriting them. Counted in its own column;
- **structure** — 2 region agents, 3 leaves each, nothing deeper;
- **correct** — the branch, region and grand totals in the root's final block match the fixture.

Over-reach is scored against the parent's raw `task` text, not the framed assignment: under `--words explained` the assignment names the siblings, and scoring on that would make every leaf look like it owned every branch.

A trial gets `--max-depth 2` — exactly the shape the task asks for — and `--max-cost 0.15`; both are printed when the benchmark starts, along with the whole-benchmark `--budget`. A trial that reaches its own cap is a scored trial, faulted and almost certainly wrong. Only `--budget` stops the benchmark.

`--grid model@provider,...`, `--reps N`, and `--cut`/`--words`/`--mode` taking comma-separated lists, sweep the cross product. `--budget` caps the whole benchmark and stops it cleanly.

## Cost

`--max-cost` (default $0.50 per run) is checked before each request. A scope puts many requests in the air at once, so the check charges each in-flight request the most any single request has cost so far; without that the cap is overshot by a whole fan-out. It is still a gate, not a hard limit: one already-sent request can always land above the cap, and a wide tree can overshoot by more.

A response that does not report its usage is a fault — a cap cannot be enforced against a cost the provider did not state.

`--request-timeout` (default 300 seconds) bounds a single request. Without it a provider that accepts a request and never answers hangs its agent, and every ancestor with it, for as long as it likes. A timed-out request is a transient failure and is retried.

Reaching the cap is an out-of-band fault, and out-of-band faults behave the same way whatever caused them: the failing agent is marked `faulted`, every ancestor stays `suspended` because its scope never returned, the run stops, and the exit code is non-zero. An agent that simply cannot do its task says so in its report; that is a completed agent.

## What it deliberately does not do

From the brief's out-of-scope list: user-facing children, `/done` and the main-thread pattern; siblings launching siblings into their own scope; re-wiring dependencies after launch; resume; compaction inside a scope; persistence and restart; limbs other than the one local read-only directory; writes and shared-workspace races; attachments and shared seed contexts; two-part launch; and the first-party provider APIs — OpenRouter's chat-completions API only.

Beyond those: one `task` call per assistant turn (a second in the same turn gets an error result), and no streaming.

## Evidence

```bash
cargo test
```

Under a second, and nothing in it waits for wall-clock time to pass.

`tests/scenario.rs` drives the `forks` binary against the fake provider and asserts on what it printed and on what the provider received. Concurrency is proved by a barrier: the children's requests are not answered until both are in flight together, which a harness that ran them one after another could never satisfy. Ordering is proved by content — the dependent child's request contains its dependency's report, so it cannot have been built before it. Cancellation and the spend cap hold requests open at the provider and release them when the test is ready. Every test also asserts the system prompt and tool list are byte-identical across every agent in the run.

The fake provider speaks HTTP/1.1 itself, on a thread per connection. An off-the-shelf server with a connection thread pool stalled under CPU contention: it stopped reading sockets it had accepted, requests sat unread in the kernel, and the suite took three minutes instead of one second while still passing.
