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
5. Max answers through a tab (`act PORT say CHAT …`). The agent works, then compacts and reports again. Max's words and both reports are kept word for word; only the tool calls between them go.
6. Max presses **Done**, and the agent's last turn writes its result.

`context.sh` prints the context the face shows: one line per entry, who wrote it, and its text. Click the dashed "compacted" card in the face to see the work it replaced. That work is still in the tree, and the card fetches it from `/context/<old head>`.

## Real output

A run at 04:08 NZT on 2026-10-18 (on `PORT=4410`). The chat's name is its mandate's first words. Lines are cut at 150 characters.

```
== the agent starts on demos/compaction/mandate.md
   30.5s 01faae7260e38761.30  This repository is a tiny Python compacted onto its latest checkpoint
   61.1s 01faae7260e38761.52  This repository is a tiny Python compacted onto its latest checkpoint

== its context now: its first message and each carry are kept; the steps' tool calls are gone
root      claude-opus-5-5
user      (label,max) [harness] Max's mandate to you (`max-81d3ff47` quotes it in a brief): | This repository is a tiny Python package with no code ye
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Step 1 done: wrote textstats/wordcount.py (count_words via 
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Step 2 done: textstats/palindromes.py (is_palindrome on low
assistant [thinking] | [bash]
user      (call) [result]
assistant I built both steps, and `./check.sh` passes: 10 tests OK, style ok. Everything is in one commit on `main`, `52a39f0 Add wordcount and palind

== Max answers
  act: Max sends "Thanks. Add `textstats.wordcount.unique(text: str) -> int`, the number of distinct words, with a test, run the check, commit, and re

== its context now: Max's words and both reports are kept verbatim
root      claude-opus-5-5
user      (label,max) [harness] Max's mandate to you (`max-81d3ff47` quotes it in a brief): | This repository is a tiny Python package with no code ye
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Step 1 done: wrote textstats/wordcount.py (count_words via 
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Step 2 done: textstats/palindromes.py (is_palindrome on low
assistant [thinking] | [bash]
user      (call) [result]
assistant I built both steps, and `./check.sh` passes: 10 tests OK, style ok. Everything is in one commit on `main`, `52a39f0 Add wordcount and palind
user      (label,max) [harness] Max wrote this (`max-0c73f81a` quotes it in a brief): | Thanks. Add `textstats.wordcount.unique(text: str) -> int`, th
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Added textstats.wordcount.unique(text) -> int = len(count_w
assistant I added `textstats.wordcount.unique(text) -> int`. It returns `len(count_words(text))`, so it splits words exactly the way `count_words` doe
```

Here the agent compacted before its commit, so the commit's tool call stays in its context before the report. Each report is an ordinary answer after its carry, so it streamed as text. The cache around each compaction, from the brain's log (`chat` stands for the chat's name):

```
   30.2s  chat: responded, ToolUse, 1 calls (2 in, 11647 cached, 318 out)
   30.5s  chat compacted onto its latest checkpoint
   32.5s  chat: responded, ToolUse, 1 calls (4 in, 10861 cached, 87 out)
   ...
   60.9s  chat: responded, ToolUse, 1 calls (2 in, 13442 cached, 211 out)
   61.1s  chat compacted onto its latest checkpoint
   63.4s  chat: responded, ToolUse, 1 calls (4 in, 11154 cached, 130 out)
   ...
  124.8s  chat: responded, ToolUse, 1 calls (2 in, 13268 cached, 170 out)
  125.1s  chat compacted onto its latest checkpoint
  127.9s  chat: responded, EndTurn, 0 calls (4 in, 12043 cached, 186 out)
```

The request after each compaction reads the prefix up to the checkpoint from cache.
