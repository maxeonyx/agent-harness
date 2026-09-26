# forked-subagents

Disposable experiment. Brief: `docs/process/experiments/forked-subagents-brief.md`.

`forks` runs agents as structured concurrency. A `task` call is a scope: the caller is suspended inside the call while its children run at the same time, possibly opening scopes of their own, and the call returns one tool result holding every child's report. A forked child's context is the parent's messages cloned, so it serializes to the same bytes and the provider serves the prefix from cache; only the child's own tail is new.

Two binaries: `forks`, and `fake-provider` — a separate HTTP server that speaks both backends' APIs, answers from a script and records every request, which is what the scenario tests assert against.

## Run it

```bash
cd experiments/forked-subagents
cargo run --release --bin forks -- run --dir <a directory> "<a task>"
cargo run --release --bin forks -- chat --dir <a directory>
cargo run --release --bin forks -- bench --reps 1
cargo run --release --bin forks -- --help
```

There are two backends. `--backend claude`, the default, is Anthropic's Messages API on Max's Claude subscription, using the token opencode keeps in `~/.local/share/opencode/opencode.db`. `--backend openrouter` is OpenRouter's chat-completions API, with the key from `$OPENROUTER_API_KEY` or `keys.ignore.env` beside this README. Runs are recorded under `runs.ignore/<timestamp>-<label>/`: `wire.jsonl` (every request and response body, tagged with the agent's path), `agents/<path>.md` (each agent's final context), `summary.json`. Both directories are gitignored.

`agents/*.md` render the same way for every agent, so a fork's context can be seen to be its parent's:

```bash
diff runs.ignore/<run>/agents/root.md runs.ignore/<run>/agents/root.alpha.md
```

In `chat`, a line is a message to the root and ends your turn. While a turn is running only `/tree` and `/cancel` are accepted; `/quit` exits.

Ctrl-C (or `/cancel`) cancels: nothing new starts, the responses already in flight are waited for and kept, and then `forks` closes. Ctrl-C again force-cancels: the responses still in flight are abandoned, and `forks` closes at once with exit code 130. Either way every agent ends with a recorded outcome and `summary.json` is written.

`--panic-in <agent path>` is fault injection: it makes that agent's task panic, which is how the tests watch the harness record a panicked agent. Every agent ends with a recorded outcome — `completed`, `cancelled`, `faulted`, `suspended` or `panicked` — and a `summary.json` is written even when the root itself panicked.

## Watching it

Every line is prefixed with the agent's path. There are two kinds of line. An event is something the harness did. A bracketed header is a message entering that agent's context, followed by its exact text, each line behind a `│`. Nothing in a message is trimmed or paraphrased: the `task` call's arguments, each child's own tail, whole tool results, and the tool result the parent is resumed with.

```
root                         [2 user]
    │ Use the task tool to launch two agents at once: one reads face.rs and one reads limb.rs; …
root                         request sent
root                         request returned   in 1212 (cached 1210, written 0)  out 212  $0.0000  via claude subscription, billed to five_hour
root                         [3 assistant thinking] (the provider did not show it)
root                         [3 assistant tool_use task toolu_01LqrMFkAjmUMVoFXWA6DjcC]
    │ {"agents":[{"name":"face_reader","task":"Read the file face.rs …","fresh":true},{"name":"limb_reader", …}]}
root                         scope opened: face_reader, limb_reader — suspended
root › face_reader           context: root's messages 0–1, then
root › face_reader           [2 user]
    │ You are agent `face_reader`.
    │
    │ Your assignment:
    │ Read the file face.rs …
root › face_reader           [3 assistant tool_use read_file toolu_01NF29qtSHoimTCjxcc5iQ6t]
    │ {"path":"face.rs"}
root › face_reader           [4 tool read_file toolu_01NF29qtSHoimTCjxcc5iQ6t]
    │ //! What you watch while the tree runs: …
```

The number in the header is the message's place in that agent's context, the same number as in `agents/<path>.md`. A child's `context:` line says which of its parent's messages it starts with; those were already shown under the parent, and everything after them is shown under the child. A message holds text, reasoning and tool calls, and each is shown as its own block under the same number. The wire format differs from what is shown in one way: on the claude backend, one turn's tool results travel together as one user message. `wire.jsonl` has the bodies exactly as sent.

`/tree` shows an agent that is still going with the time it has been going, not the time it took.

## The knobs

Every agent in a run gets the identical system prompt and the identical tool list — Anthropic caches tools, then system, then messages, so any difference between parent and child breaks the child's inherited prefix. The depth limit is therefore enforced by answering an over-deep `task` call with an error tool result, never by taking the tool away. All per-agent framing lives in the tail.

- `--cut full|own|before` — what a forked child inherits. `full`: the parent's messages through the assistant turn that called `task`, then a tool result addressed to this child. `own`: the same, except this child's copy of the `task` arguments holds only its own entry. `before`: the parent's messages up to, not including, that assistant turn, then a user message with the assignment.
- `--words stop|explained` — what the assignment says. `stop` is the probe's naive framing. `explained` also says it is one branch of a split, names the siblings holding the other assignments, and says its final message is exactly what its parent receives. These two texts are the experimental treatment; they are constants in `src/framing.rs`.
- `--mode fork|fresh|declared` — `declared` honours each `task` entry's own `fresh` flag, which is how the model routes; `fork` and `fresh` force every child.

The default cut is `before`, the one the benchmark found keeps Sonnet's children in their lane (`docs/process/experiments/forked-subagents-outcome.md`). The other defaults (`explained`, `declared`) are a choice about what is pleasant to watch, not a finding.

## The benchmark

`forks bench` generates a fixture and gives the root a fixed task that forces the shape: the root forks one agent per region, each region agent forks one per branch file, so `A → A.1 → A.1.3` is on the page.

The fixture is `ledgers/<region>/<branch>.txt` — 2 regions, 3 branches each, ~40 dated amounts per file — plus `ledgers/POLICY.md`, a ~7,000-token accounts manual. The manual is mostly genuine-sounding boilerplate, and one section of it decides the arithmetic: comments and `VOID` lines carry no amount, a `REFUND` line is subtracted rather than added, and a `DUP` line is skipped. The root task says to read the manual and that the root is the only agent that should.

That is what makes fork and fresh a real choice rather than a cost difference on nothing. A forked child inherits the manual from the parent's cache and pays almost nothing for it. A fresh child knows only what its parent wrote into its assignment, so it either re-reads the manual or gets the refunds wrong. Without it the tree's contexts were around 1,200 tokens — the size at which forked children were observed not to read the parent's prefix from cache at all, so the comparison would have been run in the regime where forks cannot win.

Scoring is mechanical, read from the recorded tool calls and reports:

- **leaf over-reach** — a branch agent read another branch's ledger, called `task`, or named another branch in its report;
- **region over-reach** — a region agent read a branch ledger itself, or named a branch outside its region;
- **re-read policy** — an agent below the root read `POLICY.md` for itself. Not over-reach: for a fresh child it is the only way to learn the rules, and it is what fresh pays instead of inheriting them. Counted in its own column;
- **structure** — 2 region agents, 3 leaves each, nothing deeper;
- **correct** — the branch, region and grand totals in the root's final block match the fixture;
- **invalid** — the trial is not evidence about the model at all, and is excluded from every rate and mean above.

A trial is invalid when the run ended in something the model had no part in: a provider or transport fault, a panic in the harness, or a cancellation. A trial that hit its own spend cap or its turn limit is *not* invalid — a tree that spends its budget is exactly what the benchmark is there to catch. Without that split, three luna trials that were rate-limited to death two seconds in showed up as `0/0` everything at `$0.0001`, dragging a combo's rates towards zero while saying nothing.

Over-reach is scored against the parent's raw `task` text, not the framed assignment: under `--words explained` the assignment names the siblings, and scoring on that would make every leaf look like it owned every branch.

A trial gets `--max-depth 2` — exactly the shape the task asks for — and `--max-cost 0.15`; both are printed when the benchmark starts, along with the whole-benchmark `--budget`. A trial that reaches its own cap is a scored trial, faulted and almost certainly wrong. Only `--budget` stops the benchmark.

`--grid model@provider,...`, `--reps N`, and `--cut`/`--words`/`--mode` taking comma-separated lists, sweep the cross product. `bench` defaults to `--mode fork`, unlike `run` and `chat`, which default to `declared`. `--budget` caps the whole benchmark and stops it cleanly.

Over-reach is judged on what an agent did, not on what it wrote: a read counts only if the limb answered it, and a report counts only if it *claims a total* for a branch that is not this agent's — naming a sibling to say you left it alone is discipline, not a breach, and `--words explained` hands every child its siblings' names. Which branches an agent owns comes from the ledger paths its assignment names, so a parent cannot widen its child's licence by mentioning other branches in prose. A leaf whose assignment names no ledger and whose own name matches no branch is reported as unscoreable rather than quietly scored clean.

The cache columns separate the question the brief asks. `child cache read` and `child first-request cache` cover agents below the root only — the first request of a forked child is the direct measure of whether it inherited the parent's prefix — while `all-agent cache read` includes the root's own re-reads, which dilute the comparison. `cache written` is the tokens paid to fill the cache.

## Rescoring

```bash
cargo run --bin forks -- rescore runs.ignore/<timestamp>-bench [--json rows.json]
```

Scores a benchmark that has already been paid for, again, offline, and prints the old verdict beside the new one.

Runs made since the harness started recording `fault_kind` say outright why they faulted. Older ones carry only the fault message, which rescoring reads back: `spend cap reached` and `agent ran past N turns` are the model's own doing, while `provider returned …`, `request failed`, `N attempts failed`, `rate limited for longer than …` and `agent task panicked` are not. Every message the harness has produced is covered; anything unrecognised is left unclassified and the trial is kept, because discarding paid evidence on a guess is worse than keeping a doubtful row. Rescoring says how many trials fell into each of the three. It walks the trial directories rather than `trials.json`, because `trials.json` is written once at the end and two benchmarks that started in the same second used to share a directory — the second to finish overwrote the first's index. `wire.jsonl` is the ground truth for what each agent did: a request body carries the previous turn's tool results, which is how a read that failed is told from one that worked. Expected totals come from that benchmark's own `fixture/`, never from today's generator. Nothing is written back. It reads OpenRouter's wire format only, and refuses a benchmark run on the claude backend.

## Cost

The claude backend is not billed per token: every request costs $0, and `--max-cost` never trips. What it spends is the subscription's usage limits, and `via` names the limit a request was billed to (`five_hour` is the plan). Only the depth and turn limits bound a run there.

`--max-cost` (default $0.50 per run) is checked before each request. A scope puts many requests in the air at once, so the check charges each in-flight request the most any single request has cost so far; without that the cap is overshot by a whole fan-out. It is still a gate, not a hard limit: one already-sent request can always land above the cap, and a wide tree can overshoot by more.

A response that does not report its usage is a fault — a cap cannot be enforced against a cost the provider did not state.

Every HTTP attempt is charged separately against the cap and checked against cancellation, retries included. A retry is new work, and a drain that starts new work is not a drain.

A rate limit is not a failure, it is the provider asking you to come back, and it gets its own patience. OpenRouter sends one as **HTTP 200 carrying an `error` object whose `code` is 429**, so a rate limit is recognised by that code, not by the HTTP status. `Retry-After` is honoured when present; otherwise the wait starts at a twenty-fourth of `--rate-limit-patience` (default 120 seconds) and doubles, and a rate limit that never lifts becomes a provider fault. Ordinary transient failures keep their four quick attempts; waiting out a rate limit is counted against patience instead.

`--request-timeout` (default 300 seconds) bounds a single attempt. Without it a provider that accepts a request and never answers hangs its agent, and every ancestor with it, for as long as it likes. A timed-out request is a transient failure and is retried.

Reaching the cap is an out-of-band fault, and out-of-band faults behave the same way whatever caused them: the failing agent is marked `faulted`, every ancestor stays `suspended` because its scope never returned, the run stops, and the exit code is non-zero. An agent that simply cannot do its task says so in its report; that is a completed agent.

## What it deliberately does not do

From the brief's out-of-scope list: user-facing children, `/done` and the main-thread pattern; siblings launching siblings into their own scope; re-wiring dependencies after launch; resume; compaction inside a scope; persistence and restart; limbs other than the one local read-only directory; writes and shared-workspace races; attachments and shared seed contexts; two-part launch.

Beyond those: one `task` call per assistant turn (a second in the same turn gets an error result), and no streaming.

## Evidence

```bash
cargo test
```

Under two seconds. One test waits on wall-clock time — `a_silent_provider_times_out_and_the_retry_succeeds`, which spends 0.2s on the timeout it is testing plus 0.5s of retry backoff — and `chat_does_not_spin_after_stdin_closes` samples CPU over a fixed window, because a rate needs one. Nothing else does: no test passes because an interval elapsed.

`tests/scenario.rs` drives the `forks` binary against the fake provider and asserts on what it printed and on what the provider received. Concurrency is proved by a barrier: the children's requests are not answered until both are in flight together, which a harness that ran them one after another could never satisfy. Ordering is proved by content — the dependent child's request contains its dependency's report, so it cannot have been built before it. Cancellation and the spend cap hold requests open at the provider and release them when the test is ready. Every test also asserts the system prompt and tool list are byte-identical across every agent in the run.

The fake provider speaks HTTP/1.1 itself, on a thread per connection. An off-the-shelf server with a connection thread pool stalled under CPU contention: it stopped reading sockets it had accepted, requests sat unread in the kernel, and the suite took three minutes instead of one second while still passing.

## The page

`page/` is an [underview](https://github.com/maxeonyx/underview) page about this experiment: what was run, down to every request as sent; what it claims and what each claim rests on; the code as nested boxes, where an arrow means "delete that and this breaks"; how data moves; every type; and the cross-cutting concerns. Everything in it is read from the recorded runs and the source when it is built:

```bash
cd page && bun install && bun build --compile --target=browser ./index.html --outdir out
```

It needs `target/release/forks` and the benchmark runs under `runs.ignore/`. `page/deps.json` is the dependency graph, made by deleting each of the crate's items in turn and recording what `cargo check --all-targets` then fails on.
