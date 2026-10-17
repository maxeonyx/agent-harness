# compaction: one agent works in steps, compacting between them

```sh
demos/compaction/run.sh
```

It starts the test brain `compaction` (`dev/drive.mjs`) and prints its link to watch. Every role runs on the scripted model, which costs nothing; the run takes about a minute, most of it `check.sh`'s 15-second style check. `MODEL=sonnet/low demos/compaction/run.sh` runs a real agent on `mandate.md`: about two minutes and a few dozen requests on the subscription.

What it does:

1. `demos/brain/scratch.sh` makes a git repository, `target/compaction-demo/repo`: a Python package that its README describes and nobody has written yet.
2. The brain starts one agent, a chat with Max, on `mandate.md` (on the scripted model, `script.md`, whose lines say each step): write the package in two steps, one after the other, run `./check.sh` after each, and compact before the next step. When both pass, commit, then compact and report.
3. After step 1 the agent calls `compact_and_continue` with a summary. Once that round is over the harness moves its head back to its latest checkpoint, which here is its first message, and appends the carry: a user message holding the summary. The agent carries on in the same turn, with step 1's tool calls gone from its context. The next request reads the cached prefix up to the checkpoint.
4. After step 2 it calls `compact_and_report` with its summary and its report. The checkpoint is now the first carry, so that carry is kept as it was, and only step 2 is replaced. The report lands right after the new carry as its last message, and the turn ends there with no further request.
5. Max answers through a tab (`node dev/drive.mjs say --brain compaction CHAT …`; on the scripted model his words are `answer.md`). His message begins a turn, so it is a checkpoint: the agent works, then compacts and reports again, and Max's words and both reports are kept word for word; only the tool calls between them go.
6. Max presses **Done**, and the agent's last turn writes its result.
7. On the scripted model, a second chat compacts in a loop: its summary scripts another `compact_and_continue`, with no call between. The harness refuses it, saying there is nothing to replace, so the agent carries on instead of compacting again:

```
  → compact_and_continue {"summary":"again"}
  ← error Not compacted: you compacted last and have made no other call since, so there is nothing to replace. Carry on with your next step, or, if your work is done, call `compact_and_report`.
```

`context.sh` prints the context the face shows: one line per entry, who wrote it, and its text. Click the dashed "compacted" card in the face to see the work it replaced. That work is still in the tree, and the card fetches it from `/context/<old head>`.

## Real output

A run on the scripted model at 10:15 NZDT on 2026-10-18, lines cut at 150 characters. A scripted agent has no thoughts of its own, so each compaction's summary carries the script lines for what follows it: after a compaction the summary is the latest words the model reads. That is why `say …` shows inside the summaries.

```
brain compaction on https://mx-hv-mj.local:4420/ (pid 2776219; every role on scripted)
log …/.drive.ignore/compaction/brain.log
end it with: node dev/drive.mjs down --brain compaction
== the agent starts on demos/compaction/script.md, every role on scripted
Max's turn since 10:15:26
   15.8s f82b3a2722de56ea.22  Build textstats in two steps compacted onto its latest checkpoint
   31.4s f82b3a2722de56ea.32  Build textstats in two steps compacted onto its latest checkpoint

== its context now: its first message and each carry are kept; the steps' tool calls are gone
root      scripted
user      (harness,label,max) [harness] Sun 18 Oct 2026 10:14 NZDT. | [harness] Max wrote this (`max-1d47ffaa` quotes it in a brief): | Build textstat
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Step 1 done: textstats/wordcount.py and its tests; ./check.
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Step 2 done: textstats/palindromes.py and its tests; ./chec
assistant I built both steps, and ./check.sh passes. Everything is in one commit on main.

== Max answers
Max's turn since 10:15:42

== its context now: Max's words and both reports are kept verbatim
root      scripted
user      (harness,label,max) [harness] Sun 18 Oct 2026 10:14 NZDT. | [harness] Max wrote this (`max-1d47ffaa` quotes it in a brief): | Build textstat
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Step 1 done: textstats/wordcount.py and its tests; ./check.
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Step 2 done: textstats/palindromes.py and its tests; ./chec
assistant I built both steps, and ./check.sh passes. Everything is in one commit on main.
user      (harness,label,max) [harness] Sun 18 Oct 2026 10:15 NZDT. | [harness] Max wrote this (`max-a654eb42` quotes it in a brief): | Thanks. Add `t
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Added unique and its test; ./check.sh passes; committed. sa
assistant I added unique with a test; ./check.sh passes and it is committed.

  act: tab 0709d3a53e06e379 linked
  act: Max presses Done
included

== what the brain said of its tasks' moves
   15.8s f82b3a2722de56ea.22  Build textstats in two steps compacted onto its latest checkpoint
   31.4s f82b3a2722de56ea.32  Build textstats in two steps compacted onto its latest checkpoint
   46.8s f82b3a2722de56ea.44  Build textstats in two steps compacted onto its latest checkpoint
== the task is done; its result:
[harness] Max has marked this chat done. Write your result: what you did, what you learned, and what is left undone. This is your last message here.
Result: textstats has wordcount (count_words, top, unique) and palindromes, all tested and committed on main; nothing is left undone.

== the repository afterwards
b25a02f Add unique
29aba1b Add wordcount and palindromes
e8cd69a Start textstats

To serve the finished run again: node dev/drive.mjs up --brain compaction scripted
```

A run on Sonnet (`MODEL=sonnet/low`) at 10:17 NZDT on 2026-10-18. After Max's answer it reported without compacting, since the answer did not ask it to; the third compaction is its result, in the Done turn.

```
brain compaction on https://mx-hv-mj.local:4420/ (pid 2780308; every role on sonnet/low)
log …/.drive.ignore/compaction/brain.log
end it with: node dev/drive.mjs down --brain compaction
== the agent starts on demos/compaction/mandate.md, every role on sonnet/low
Max's turn since 10:17:12
   21.6s a9370f2a32ed8cdd.27  This repository is a tiny Python compacted onto its latest checkpoint
   46.9s a9370f2a32ed8cdd.47  This repository is a tiny Python compacted onto its latest checkpoint

== its context now: its first message and each carry are kept; the steps' tool calls are gone
root      claude-sonnet-5-5
user      (harness,label,max) [harness] Sun 18 Oct 2026 10:16 NZDT. | [harness] Max wrote this (`max-81d3ff47` quotes it in a brief): | This repositor
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Step 1 done: textstats/wordcount.py and tests/test_wordcoun
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Both steps done, check.sh passes, committed d5e39a7.
assistant Both steps are built, and `./check.sh` passed after each one (4 tests, style ok). Everything is committed as d5e39a7 and not pushed.  - **St

== Max answers
Max's turn since 10:17:33

== its context now: Max's words and both reports are kept verbatim
root      claude-sonnet-5-5
user      (harness,label,max) [harness] Sun 18 Oct 2026 10:16 NZDT. | [harness] Max wrote this (`max-81d3ff47` quotes it in a brief): | This repositor
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Step 1 done: textstats/wordcount.py and tests/test_wordcoun
user      (compacted) [harness] You compacted your work after this point. | Your summary:  Both steps done, check.sh passes, committed d5e39a7.
assistant Both steps are built, and `./check.sh` passed after each one (4 tests, style ok). Everything is committed as d5e39a7 and not pushed.  - **St
user      (harness,label,max) [harness] Sun 18 Oct 2026 10:17 NZDT. | [harness] Max wrote this (`max-0c73f81a` quotes it in a brief): | Thanks. Add `t
assistant [bash]
user      (call) [result]
assistant [bash]
user      (call) [result]
assistant I added `textstats.wordcount.unique(text)`, which returns the number of distinct words. It counts case-insensitively, using the same word de

  act: tab 7a87f2283dd41fe4 linked
  act: Max presses Done
included

== what the brain said of its tasks' moves
   21.6s a9370f2a32ed8cdd.27  This repository is a tiny Python compacted onto its latest checkpoint
   46.9s a9370f2a32ed8cdd.47  This repository is a tiny Python compacted onto its latest checkpoint
   70.8s a9370f2a32ed8cdd.69  This repository is a tiny Python compacted onto its latest checkpoint
== the task is done; its result:
Nothing is left undone, except that none of the commits are pushed (d5e39a7 and 50eeb1b are local only). I did the work myself and started no other ag

== the repository afterwards
50eeb1b Add wordcount.unique
d5e39a7 Add wordcount and palindromes
288b3cd Start textstats

To serve the finished run again: node dev/drive.mjs up --brain compaction sonnet/low
```
