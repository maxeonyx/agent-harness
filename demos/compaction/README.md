# compaction: one agent works in steps, compacting between them

```sh
PORT=4400 demos/compaction/run.sh
```

Watch https://mx-hv-mj.local:4400 while it runs. It takes about two minutes on Opus and a few dozen requests on the subscription.

What it does:

1. `demos/brain/scratch.sh` makes a git repository, `target/compaction-demo/repo`: a Python package that its README describes and nobody has written yet.
2. The brain starts one agent, a chat with Max, on `mandate.md`: write the package in two steps, one after the other, run `./check.sh` after each, and compact before the next step. When both pass, commit and end the turn with `compact_and_report`.
3. After step 1 the agent calls `compact_and_continue` with a summary. Once that round is over the harness moves its head back to its latest checkpoint, which here is its first message, and appends the carry: a user message holding the summary. The agent carries on in the same turn, with step 1's tool calls gone from its context. The next request reads the cached prefix up to the checkpoint.
4. After step 2 it calls `compact_and_report`. The checkpoint is now the first carry, so that carry is kept as it was, and only step 2 is replaced. The report becomes the agent's message, and Max reads it as the first message of the compacted context.
5. Max answers through a tab (`act PORT say …`). The agent works, then compacts and reports again. Max's words and both reports are kept word for word; only the tool calls between them go.
6. Max presses **Done**, and the agent's last turn writes its result.

`context.sh` prints the context the face shows: one line per entry, who wrote it, and its text. Click the dashed "compacted" card in the face to see the work it replaced. That work is still in the tree, and the card fetches it from `/context/<old head>`.

## Real output

A run at 02:22 NZT on 2026-10-18 (on `PORT=4410`). Lines are cut at 150 characters.

```
== the agent starts on demos/compaction/mandate.md
   31.1s 0043bfea2b412e6d.25  orchestrator compacted onto its latest checkpoint
   65.6s 0043bfea2b412e6d.47  orchestrator compacted onto its latest checkpoint, and reported

== its context now: its first message and each carry are kept; the steps' tool calls are gone
root      claude-opus-5-5
user      (label,max) [harness] Max's mandate to you (`max-67c79234` quotes it in a brief): | This repository is a tiny Python package with no code yet; `README.md` says wh
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Step 1 done: wrote textstats/wordcount.py (count_words via regex `(?:[^\W_]|')+`
user      (compacted) [harness] You compacted your work after this point.
assistant I built both steps of the `textstats` package from the spec in `README.md`. All 10 tests pass under `./check.sh`, along with the style check. Everythi

== Max answers
  act: Max sends "Thanks. Add `textstats.wordcount.unique(text: str) -> int`, the number of distinct words, with a test, run the check, commit, and report again."

== its context now: Max's words and both reports are kept verbatim
root      claude-opus-5-5
user      (label,max) [harness] Max's mandate to you (`max-67c79234` quotes it in a brief): | This repository is a tiny Python package with no code yet; `README.md` says wh
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Step 1 done: wrote textstats/wordcount.py (count_words via regex `(?:[^\W_]|')+`
user      (compacted) [harness] You compacted your work after this point.
assistant I built both steps of the `textstats` package from the spec in `README.md`. All 10 tests pass under `./check.sh`, along with the style check. Everythi
user      (label,max) [harness] Max wrote this (`max-0c73f81a` quotes it in a brief): | Thanks. Add `textstats.wordcount.unique(text: str) -> int`, the number of distinct w
user      (compacted) [harness] You compacted your work after this point.
assistant I added `textstats.wordcount.unique` with two tests, and all 12 tests pass under `./check.sh`, along with the style check. It's committed as `98e7c4e`
```

The agent left the summary of `compact_and_report` empty both times, because the report said everything. That run's carries say only "You compacted your work after this point."; the carry now also says "Your report follows." The cache after each compaction, from the brain's log:

```
   30.8s  orchestrator: responded, ToolUse, 1 calls (2 in, 11227 cached, 287 out)
   31.1s  orchestrator compacted onto its latest checkpoint
   33.6s  orchestrator: responded, ToolUse, 1 calls (4 in, 10413 cached, 99 out)
   ...
   65.6s  orchestrator compacted onto its latest checkpoint, and reported
   68.0s  orchestrator: responded, ToolUse, 1 calls (4 in, 10943 cached, 100 out)
```
