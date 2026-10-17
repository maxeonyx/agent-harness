# compaction: one agent works in steps, compacting between them

```sh
demos/compaction/run.sh
```

Watch https://mx-hv-mj.local:4410 while it runs (another port: `PORT=4400`). It takes about two minutes on Opus and a few dozen requests on the subscription.

What it does:

1. `demos/brain/scratch.sh` makes a git repository, `target/compaction-demo/repo`: a Python package that its README describes and nobody has written yet.
2. The brain starts one agent, a chat with Max, on `mandate.md`: write the package in two steps, one after the other, run `./check.sh` after each, and compact before the next step. When both pass, commit, then compact and report.
3. After step 1 the agent calls `compact` with a summary. Once that round is over the harness moves its head back to its latest checkpoint, which here is its first message, and appends the carry: a user message holding the summary. The agent carries on in the same turn, with step 1's tool calls gone from its context. The next request reads the cached prefix up to the checkpoint.
4. After step 2 it compacts again. The checkpoint is now the first carry, so that carry is kept as it was, and only step 2 is replaced. Then it writes its report, from its summary, as its last message: an ordinary answer that ends the turn, so it streams as text and lands after the carry.
5. Max answers through a tab (`act PORT say …`). The agent works, then compacts and reports again. Max's words and both reports are kept word for word; only the tool calls between them go.
6. Max presses **Done**, and the agent's last turn writes its result.

`context.sh` prints the context the face shows: one line per entry, who wrote it, and its text. Click the dashed "compacted" card in the face to see the work it replaced. That work is still in the tree, and the card fetches it from `/context/<old head>`.

## Real output

A run at 03:21 NZT on 2026-10-18 (on `PORT=4410`). Lines are cut at 150 characters.

```
== the agent starts on demos/compaction/mandate.md
   32.4s 498899f39bed03ee.29  orchestrator compacted onto its latest checkpoint
   68.9s 498899f39bed03ee.58  orchestrator compacted onto its latest checkpoint

== its context now: its first message and each carry are kept; the steps' tool calls are gone
root      claude-opus-5-5
user      (label,max) [harness] Max's mandate to you (`max-5b7da237` quotes it in a brief): | This repository is a tiny Python package with no code yet; `README.md` says wh
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Step 1 done: wrote `textstats/wordcount.py` (count_words via regex `(?:[^\W_]|')+
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Both steps done and committed as a57ac88 "Add wordcount and palindromes modules w
assistant I built both modules and committed them as `a57ac88` ("Add wordcount and palindromes modules with tests"). `./check.sh` passes: 9 tests, style check o

== Max answers
  act: Max sends "Thanks. Add `textstats.wordcount.unique(text: str) -> int`, the number of distinct words, with a test, run the check, commit, and report again."

== its context now: Max's words and both reports are kept verbatim
root      claude-opus-5-5
user      (label,max) [harness] Max's mandate to you (`max-5b7da237` quotes it in a brief): | This repository is a tiny Python package with no code yet; `README.md` says wh
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Step 1 done: wrote `textstats/wordcount.py` (count_words via regex `(?:[^\W_]|')+
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Both steps done and committed as a57ac88 "Add wordcount and palindromes modules w
assistant I built both modules and committed them as `a57ac88` ("Add wordcount and palindromes modules with tests"). `./check.sh` passes: 9 tests, style check o
user      (label,max) [harness] Max wrote this (`max-0c73f81a` quotes it in a brief): | Thanks. Add `textstats.wordcount.unique(text: str) -> int`, the number of distinct w
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Added `unique(text) -> int` to textstats/wordcount.py (`return len(count_words(te
assistant I added `textstats.wordcount.unique(text)`, which returns the number of distinct words in the text. It's committed as `233e573` ("Add wordcount.unique
```

Each report is an ordinary answer after its carry, so it streamed as text. The cache around each compaction, from the brain's log:

```
   32.1s  orchestrator: responded, ToolUse, 1 calls (2 in, 11051 cached, 413 out)
   32.4s  orchestrator compacted onto its latest checkpoint
   34.9s  orchestrator: responded, ToolUse, 1 calls (4 in, 10247 cached, 105 out)
   ...
   68.7s  orchestrator: responded, ToolUse, 1 calls (2 in, 13745 cached, 284 out)
   68.9s  orchestrator compacted onto its latest checkpoint
   75.7s  orchestrator: responded, EndTurn, 0 calls (4 in, 10901 cached, 590 out)
```

The last line is the report: one more request than reporting in the compaction call itself would take, read almost wholly from cache.
