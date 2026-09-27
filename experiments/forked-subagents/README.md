# forked-subagents

Disposable experiment. Brief: `docs/process/experiments/forked-subagents-brief.md`.

`forks` runs agents as structured concurrency. A `task` call is a scope: the caller is suspended inside the call while its children run at the same time, possibly opening scopes of their own, and the call returns one tool result holding every child's report. A forked child's context is the parent's messages cloned, so it serializes to the same bytes and the provider serves the prefix from cache; only the child's own tail is new.

The parent writes one `task` call: a `shared` part, written once for every child, and each child's own short `task`. So a child's tail is two parts. The shared part is the same bytes for every sibling and ends in an explicit cache breakpoint; then comes the child's own part. The first sibling that waits on no other starts first, and the rest start when its first request has come back: that request is what writes the shared part to the cache, and a request sent before it returns cannot read it.

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
    │ Use the task tool to launch two agents at once: one reads face.rs and one reads limb.rs, … Put what they both need to know in shared.
root                         request sent
root                         request returned   in 1330 (cached 0, written 1328)  out 239  $0.0000  via claude subscription, billed to seven_day
root                         [3 assistant tool_use task toolu_01GvaPGaRz9E2LZVMpcgVMzm]
    │ {"shared":"You are helping inspect a Rust codebase. …","agents":[{"name":"face_reader","task":"Read the file face.rs …"},{"name":"limb_reader", …}]}
root                         scope opened: face_reader, limb_reader — suspended
root › face_reader           context: root's messages 0–3, then
root › face_reader           [4 tool task toolu_01GvaPGaRz9E2LZVMpcgVMzm] ← cache breakpoint
    │ This work was handed to these agents, running at the same time: `face_reader`, `limb_reader`. Each is given what follows, written once for all of them, and then an assignment of its own.
    │
    │ You are helping inspect a Rust codebase. …
root › face_reader           [5 user]
    │ You are agent `face_reader`.
    │
    │ Your assignment:
    │ Read the file face.rs and report the first sentence of its module doc comment.
    │
    │ The other agents are doing their assignments right now, …
root › face_reader           request sent
root › face_reader           request returned   in 1873 (cached 1328, written 543)  out 69  $0.0000  via claude subscription, billed to seven_day
```

The number in the header is the message's place in that agent's context, the same number as in `agents/<path>.md`. A child's `context:` line says which of its parent's messages it starts with; those were already shown under the parent, and everything after them is shown under the child. A message holds text, reasoning and tool calls, and each is shown as its own block under the same number. `← cache breakpoint` marks the block that carries an explicit cache breakpoint; `agents/<path>.md` marks the same message `(cache breakpoint)`. The wire format differs from what is shown in one way: on the claude backend, a run of tool results and user text travels as one user message, so above, blocks 4 and 5 are one user message. `wire.jsonl` has the bodies exactly as sent.

`/tree` shows an agent that is still going with the time it has been going, not the time it took.

## The knobs

Every agent in a run gets the identical system prompt and the identical tool list — Anthropic caches tools, then system, then messages, so any difference between parent and child breaks the child's inherited prefix. The depth limit is therefore enforced by answering an over-deep `task` call with an error tool result, never by taking the tool away. All per-agent framing lives in the tail.

- `--cut result|before|call` — where a forked child's tail starts. `result`: the parent's messages through the assistant turn that called `task`; the answer to that call is the shared part, with the breakpoint on it (on the last of that turn's tool results, when it made other calls too), and the child's own part is a text block after it in the same user message. `before`: the parent's messages up to, not including, that assistant turn, then the shared part, with the breakpoint, and the child's own part as user text. `call`: the parent's messages through the assistant turn that called `task`, which already holds the `shared` argument, with the breakpoint on that turn's last block; the answer to the call is the child's own part alone, so the shared text is in the child's context once rather than twice. OpenRouter refuses `result` and `call`: it documents breakpoints only on the text parts of a message. The likely translation of one on a tool result is a text block inside it, which Anthropic rejects (`probe/shared-context.md`).
- `--identity agent|task` — how the child's own part names it. `agent`: "You are agent `x`", a new agent launched by the split. `task`: "Your next task, and only this one, is `x`", the same conversation carrying on with one of the tasks it split into. A fresh child has no conversation to carry on, so it is always an agent. The texts are constants in `src/framing.rs`.
- `--mode fork|fresh|declared` — `declared` honours each `task` entry's own `fresh` flag, which is how the model routes; `fork` and `fresh` force every child. A fresh child's context is the system prompt, then the shared part, with the breakpoint, and its own part, so fresh siblings share a cached prefix too.

The defaults (`before`, `agent`, `declared`) are a choice about what is pleasant to watch, not a finding.

Every level of forking leaves one breakpoint in its descendants' contexts, and Anthropic accepts four per request, one of them the automatic one at the end (`probe/breakpoints.py`). So `--max-depth` is at most 3.

## The benchmark

`forks bench` generates a fixture and gives the root a fixed task. `--task` picks which. Both are scored mechanically, from the recorded tool calls and reports.

### `--task ledgers`, the default

The root task forces the shape: the root forks one agent per region, each region agent forks one per branch file, so `A → A.1 → A.1.3` is on the page.

The fixture is `ledgers/<region>/<branch>.txt` — 2 regions, 3 branches each, ~40 dated amounts per file — plus `ledgers/POLICY.md`, a ~7,000-token accounts manual. The manual is mostly genuine-sounding boilerplate, and one section of it decides the arithmetic: comments and `VOID` lines carry no amount, a `REFUND` line is subtracted rather than added, and a `DUP` line is skipped. The root task says to read the manual and that the root is the only agent that should.

That is what makes fork and fresh a real choice rather than a cost difference on nothing. A forked child inherits the manual from the parent's cache and pays almost nothing for it. A fresh child knows only what its parent wrote into `shared` and its `task`, so it either re-reads the manual or gets the refunds wrong. Without it the tree's contexts were around 1,200 tokens — the size at which forked children were observed not to read the parent's prefix from cache at all, so the comparison would have been run in the regime where forks cannot win.

Scoring is mechanical, read from the recorded tool calls and reports:

- **leaf over-reach** — a branch agent read another branch's ledger, called `task`, or named another branch in its report;
- **region over-reach** — a region agent read a branch ledger itself, or named a branch outside its region;
- **re-read policy** — an agent below the root read `POLICY.md` for itself. Not over-reach: for a fresh child it is the only way to learn the rules, and it is what fresh pays instead of inheriting them. Counted in its own column;
- **structure** — 2 region agents, 3 leaves each, nothing deeper;
- **correct** — the branch, region and grand totals in the root's final block match the fixture.

### `--task projects`

The root is asked what Max asks, and is not handed the tree: "One agent per project: how many clones, what branch, one-line summary." It is told that a worktree counts as a clone and what shape its final lines take, and nothing about how deep to go.

The fixture is `work/<project>/<checkout>/`: `datacentral` with 23 clones and 2 worktrees, `ticker` with 3 and 1, `nzxcom` with 2 and 1, and `gta` with 2. A clone has a `.git` directory with `HEAD`, `config`, `COMMIT_EDITMSG` and `description`. A worktree has a `.git` file, `gitdir: ../<clone>/.git/worktrees/<name>`, so its branch is in another clone of the same project. `config` lists `main` as well as the checked-out branch, so only `HEAD` gives the right answer. Look-alikes are not checkouts: `notes/`, a directory with a bare `HEAD` and no `.git`, one with only `.gitignore` and `.github/`, and one with `.git.bak/`. `gta-mcp/README.md` says the schema it serves lives in the DataCentral clone, which is a reason to look there.

- **project over-reach** — a project agent called `task`, read or listed another project's directory, or reported a branch for another project's checkout. Which project it owns comes from the `work/<project>` path its assignment names, or else its own name;
- **branches right** — checkouts whose `<project>/<checkout>: <branch>` line in the root's final message is right;
- **counts right** — projects whose `<project>: <number>` line is right. A `<project>:` line whose value is not a number is its summary, and is not a count;
- **structure** — one agent per project, nothing deeper;
- **correct** — every branch and every count right, and no look-alike listed as a clone.

### Both

A trial is **invalid** when it is not evidence about the model at all; it is excluded from every rate and mean.

A trial is invalid when the run ended in something the model had no part in: a provider or transport fault, a panic in the harness, or a cancellation. A trial that hit its own spend cap or its turn limit is *not* invalid — a tree that spends its budget is exactly what the benchmark is there to catch. Without that split, three luna trials that were rate-limited to death two seconds in showed up as `0/0` everything at `$0.0001`, dragging a combo's rates towards zero while saying nothing.

Over-reach is scored against the parent's raw `task` text, not the framed tail: the shared part names every sibling, and scoring on that would make every leaf look like it owned every branch.

A trial gets `--max-depth 2` — exactly the ledger tree, and one level more than the projects task asks for, so a project agent that splits again is seen doing it — and `--max-cost 0.15`; both are printed when the benchmark starts, along with the whole-benchmark `--budget`. A trial that reaches its own cap is a scored trial, faulted and almost certainly wrong. Only `--budget` stops the benchmark.

`--grid model@provider,...`, `--reps N`, and `--cut`/`--identity`/`--mode` taking comma-separated lists, sweep the cross product. `bench` defaults to `--mode fork`, unlike `run` and `chat`, which default to `declared`. `--budget` caps the whole benchmark and stops it cleanly.

Over-reach is judged on what an agent did, not on what it wrote: a read counts only if the limb answered it, and a report counts only if it *claims a result* for work that is not this agent's, a total for another branch or a branch for another project's checkout — naming a sibling to say you left it alone is discipline, not a breach, and the shared part hands every child its siblings' names. Which branches an agent owns comes from the ledger paths its assignment names, so a parent cannot widen its child's licence by mentioning other branches in prose. A leaf whose assignment names no ledger and whose own name matches no branch is reported as unscoreable rather than quietly scored clean.

The cache columns separate the question the brief asks. `child cache read` and `child first-request cache` cover agents below the root only — the first request of a forked child is the direct measure of whether it inherited the parent's prefix — while `all-agent cache read` includes the root's own re-reads, which dilute the comparison. `cache written` is the tokens paid to fill the cache. `shared-part hits` counts, in every scope, the siblings after the first whose first request read the shared part from the cache, out of all siblings after the first. The first sibling writes it; a later one read it when its first request read more from the cache than the sibling that read least. The trial line also gives the tokens they read beyond it.

## Rescoring

```bash
cargo run --bin forks -- rescore runs.ignore/<timestamp>-bench [--json rows.json]
```

Scores a benchmark that has already been paid for, again, offline, and prints the old verdict beside the new one.

Runs made since the harness started recording `fault_kind` say outright why they faulted. Older ones carry only the fault message, which rescoring reads back: `spend cap reached` and `agent ran past N turns` are the model's own doing, while `provider returned …`, `request failed`, `N attempts failed`, `rate limited for longer than …` and `agent task panicked` are not. Every message the harness has produced is covered; anything unrecognised is left unclassified and the trial is kept, because discarding paid evidence on a guess is worse than keeping a doubtful row. Rescoring says how many trials fell into each of the three. It walks the trial directories rather than `trials.json`, because `trials.json` is written once at the end and two benchmarks that started in the same second used to share a directory — the second to finish overwrote the first's index. `wire.jsonl` is the ground truth for what each agent did: a request body carries the previous turn's tool results, which is how a read that failed is told from one that worked. The expected answer comes from that benchmark's own `fixture/`, read from disk, never from today's generator. Nothing is written back. It reads both backends' wire formats. An older trial recorded `words` where a newer one records `identity`, and is shown with it.

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

`tests/scenario.rs` drives the `forks` binary against the fake provider and asserts on what it printed and on what the provider received. Concurrency is proved by a barrier: once the first child's first request is answered, its next request and its sibling's first are not answered until both are in flight together, which a harness that ran them one after another could never satisfy. Ordering is proved by content — the dependent child's request contains its dependency's report, so it cannot have been built before it. Cancellation and the spend cap hold requests open at the provider and release them when the test is ready. Every test also asserts the system prompt and tool list are byte-identical across every agent in the run.

The fake provider speaks HTTP/1.1 itself, on a thread per connection. An off-the-shelf server with a connection thread pool stalled under CPU contention: it stopped reading sockets it had accepted, requests sat unread in the kernel, and the suite took three minutes instead of one second while still passing.

## The page

`page/` is an [underview](https://github.com/maxeonyx/underview) page about this experiment: what was run, down to every request as sent; what it claims and what each claim rests on; the code as nested boxes, where an arrow means "delete that and this breaks"; how data moves; every type; and the cross-cutting concerns. Everything in it is read from the recorded runs and the source when it is built:

```bash
cd page && bun install && bun build --compile --target=browser ./index.html --outdir out
```

It needs `target/release/forks` and the benchmark runs under `runs.ignore/`. `page/deps.json` is the dependency graph, made by deleting each of the crate's items in turn and recording what `cargo check --all-targets` then fails on.
