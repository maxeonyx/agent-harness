# dev-process: the dev process, run by the harness

```sh
demos/dev-process/run.sh
```

Then open https://mx-hv-mj.local:4345 and watch the map (another port: `PORT=4400`; a new run: `FRESH=1`). Each role runs on its file's model and effort; it takes about three minutes and spends a few dozen requests of the subscription. `MODEL=sonnet` runs every role on Sonnet instead, which is cheaper; `MODEL=scripted` costs nothing but only shows the page, since the orchestrator then just echoes its mandate. The face keeps serving the finished run until you stop it (Ctrl-C, which also ends the workspaces it started).

What it does:

1. `scratch.sh` makes `target/dev-process/textstats.git`, a bare repository standing in for GitHub, and `textstats`, your clone of it: a Python package whose README says what `wordcount` should be.
2. The brain reads `roles/*.md`: orchestrator, designer and engineer, each with its model, effort, limbs, places and the roles it can start. They are the same files Claude Code reads through `.claude/agents/`. The brain reads the files again every two seconds; an edit is a new version of the role, and a running task of that role is told at its next request.
3. Max starts an orchestrator in your clone at a tab, as you do at the page (the script runs `examples/act`), with `mandate.md`. Like every task you start, it is a chat with you: when it ends a turn it waits on you, and it is done only when you press **Done**. The mandate is your words, so the orchestrator can quote it.
4. The orchestrator does two things at once. It `spawn`s an engineer (`wordcount`) and a designer (`unicode-design`), each in its own clone, and gives both your mandate with `quotes`: it reaches each agent verbatim, labelled as yours, after the brief. And it decides the one choice the mandate says is yours, so the work goes on without you.
5. Each agent commits and pushes its branch. Once both are back, the orchestrator checks the branches on origin and reports, naming the choice it made for you, and waits on you.

What to do on the page:

- **The choice it made for you.** The orchestrator's report says what it decided about lone apostrophes and why. Reply to overturn it: your reply is the next message it reads.
- **An aside** while the orchestrator waits on its agents. Write to it and send: a chat with a fork of the orchestrator opens, and what you wrote is its first message. On the map it is one more member of the scope the orchestrator waits on. When the others are done it is told so; you decide when it is done, and press ✓ **Done** twice: it summarises what came of your talk, and that goes back to the orchestrator with the others' results.
- **Sending while an agent works.** Open a working agent and write to it. Every send takes two presses: the first stages it (the button reads **Send?** and the box is outlined), the second sends; any change to the words lets the stage go. **Send** (Enter, Enter) reaches it with its next request; **Queue** (Alt+Enter, Alt+Enter), shown mid-turn, waits for its turn to end, so on the orchestrator it waits until the agents are back and it has reported. Either way the model is told where in its work you wrote, as "Max wrote this while you were working, before your last 4 tool calls (from `toolu_…` on), asking that it wait for the end of your turn". Shift+Enter is a new line; on a phone Enter is a new line and the buttons send.
- **Pause, Stop, Abandon.** Beside the box: ‖ **Pause** while it works (at once; ▶ **Resume** undoes it), then ■ **Stop** to have it end its turn and wait for you; ✕ **Abandon** (twice) stops it and what it started, and while that tidies up the same button is **End now**. Words sent to an agent you paused carry it on. Hover a node for the same actions, or right-click it (long-press on a phone) for them as a menu.
- **What a context holds.** The context gauge in an open node's header; click it for the split.
- ✓ **Done** (twice) on the orchestrator once it has reported: its last turn writes its result. Thirty minutes after the whole project is done it folds into a small card on a shelf below anything live; click the card for the root's log, or ⌄ to unfold it.
- **The header.** How much of the subscription is used, and when each window resets.

## Real output

A run on Sonnet (`MODEL=sonnet`) at 04:12 NZDT on 2026-10-18, with nobody at the page: 12 requests, two minutes to the report. The brain's log, with request, first-byte, dispatch and completion lines dropped, paths made relative and lines cut at 170 characters:

```
harness-brain 65a3d6fdc6dd+dirty
    0.0s 66ea5088543990a4.1  brain launched, build 279bba386ad5
    0.0s 66ea5088543990a4.1  limb meta defined
    0.0s 66ea5088543990a4.1  limb web defined
    0.0s 66ea5088543990a4.1  limb user defined
    0.0s 66ea5088543990a4.1  limb browser defined
    0.0s 66ea5088543990a4.1  limb roles defined
face: https://mx-hv-mj.local:4375
    0.0s 66ea5088543990a4.2  roles connected
    0.0s 5b58d3333c201891.1  environment browser defined
    0.0s 5b58d3333c201891.1  environment chat defined
    0.0s 5b58d3333c201891.1  environment designer defined
    0.0s 5b58d3333c201891.1  environment engineer defined
    0.0s 5b58d3333c201891.1  environment orchestrator defined
    0.0s 5b58d3333c201891.1  role roles/browser.md changed
    0.0s 5b58d3333c201891.1  role roles/chat.md changed
    0.0s 5b58d3333c201891.1  role roles/designer.md changed
    0.0s 5b58d3333c201891.1  role roles/engineer.md changed
    0.0s 5b58d3333c201891.1  role roles/orchestrator.md changed
    0.0s 66ea5088543990a4.4  tab 16cf324d6ae80bdd is Max's
    0.0s 16cf324d6ae80bdd.1  limb target/dev-process/textstats defined
    0.0s 16cf324d6ae80bdd.1  textstats needs two things created (orchestrator)
    0.0s 16cf324d6ae80bdd.1  as seen at 66ea5088543990a4.3
    0.1s 66ea5088543990a4.6  target/dev-process/textstats connected
    0.1s 98a37fe7dc62ff54.1  target/dev-process/textstats described: 5 tools
    0.1s 66ea5088543990a4.8  user connected
    0.1s 51d2669dd7475440.1  user described: 1 tools
    0.1s 66ea5088543990a4.10  textstats needs two things has a new root, on claude-sonnet-5-5
    2.2s 66ea5088543990a4.13  textstats needs two things's bash made {"command": "cat README.md; ls; cat check.sh; git branch -a"}
    2.2s 66ea5088543990a4.13  textstats needs two things: responded, ToolUse, 1 calls (4 in, 0 cached, 66 out)
    9.7s 66ea5088543990a4.18  textstats needs two things's spawn made
    9.7s 66ea5088543990a4.18  textstats needs two things: responded, ToolUse, 1 calls (2 in, 10775 cached, 909 out)
    9.7s 66ea5088543990a4.19  limb target/dev-process/textstats-wordcount defined
    9.7s 66ea5088543990a4.19  textstats needs two things-wordcount created (engineer)
    9.7s 66ea5088543990a4.19  limb target/dev-process/textstats-unicode-design defined
    9.7s 66ea5088543990a4.19  textstats needs two things-unicode created (designer)
   10.1s 66ea5088543990a4.20  target/dev-process/textstats-unicode-design connected
   10.1s 0667cc533db3e133.1  target/dev-process/textstats-unicode-design described: 5 tools
   10.1s 66ea5088543990a4.22  target/dev-process/textstats-wordcount connected
   10.1s a2241d057bc1fe91.1  target/dev-process/textstats-wordcount described: 5 tools
   10.1s 66ea5088543990a4.24  textstats needs two things-wordcount has a new root, on claude-sonnet-5-5
   10.1s 66ea5088543990a4.25  textstats needs two things-unicode has a new root, on claude-sonnet-5-5
   11.6s 66ea5088543990a4.29  textstats needs two things-wordcount's bash made {"command": "cat README.md check.sh; ls -R . | grep -v .git | head -30"}
   11.6s 66ea5088543990a4.29  textstats needs two things-wordcount: responded, ToolUse, 1 calls (4 in, 0 cached, 71 out)
   14.9s 66ea5088543990a4.35  textstats needs two things-unicode's bash made {"command": "cd target/dev-process/textstats-unicode-design && git log --oneline | head; ls …
   14.9s 66ea5088543990a4.35  textstats needs two things-unicode's skill made {"name": "design"}
   14.9s 66ea5088543990a4.35  textstats needs two things-unicode: responded, ToolUse, 2 calls (4 in, 0 cached, 276 out)
   16.6s 66ea5088543990a4.42  textstats needs two things-wordcount's bash made {"command": "cat > textstats/wordcount.py <<'E'\nfrom collections import Counter\n\n\ndef …
   16.6s 66ea5088543990a4.42  textstats needs two things-wordcount: responded, ToolUse, 1 calls (2 in, 11900 cached, 776 out)
   18.4s 66ea5088543990a4.47  textstats needs two things-wordcount: responded, EndTurn, 0 calls (2 in, 12239 cached, 125 out)
   90.3s 66ea5088543990a4.48  textstats needs two things-unicode's bash made {"command": "cd target/dev-process/textstats-unicode-design && grep -rIl \"Bx\" . --exclude-…
   90.3s 66ea5088543990a4.48  textstats needs two things-unicode: responded, ToolUse, 1 calls (2 in, 13381 cached, 9367 out)
  106.4s 66ea5088543990a4.53  textstats needs two things-unicode's bash made {"command": "cd target/dev-process/textstats-unicode-design && rm /tmp/ref_unicode.py && mkd…
  106.4s 66ea5088543990a4.53  textstats needs two things-unicode: responded, ToolUse, 1 calls (2 in, 18320 cached, 2055 out)
  109.4s 66ea5088543990a4.58  textstats needs two things-unicode's bash made {"command": "cd target/dev-process/textstats-unicode-design && git add docs/unicode.md && gi…
  109.4s 66ea5088543990a4.58  textstats needs two things-unicode: responded, ToolUse, 1 calls (2 in, 28023 cached, 326 out)
  117.9s 66ea5088543990a4.63  textstats needs two things-unicode: responded, EndTurn, 0 calls (2 in, 30121 cached, 1123 out)
  119.5s 66ea5088543990a4.67  textstats needs two things's bash made {"command": "git fetch -q; git log --oneline origin/wordcount origin/unicode-design | head"}
  119.5s 66ea5088543990a4.67  textstats needs two things: responded, ToolUse, 1 calls (2 in, 11115 cached, 75 out)
  123.7s 66ea5088543990a4.72  textstats needs two things: responded, EndTurn, 0 calls (2 in, 13338 cached, 554 out)
  166.1s 66ea5088543990a4.73  tab b724e86d79a84904 is Max's
  166.1s b724e86d79a84904.1  Done prompt delivered to textstats needs two things
  166.1s b724e86d79a84904.1  as seen at 66ea5088543990a4.72
  170.0s 66ea5088543990a4.77  textstats needs two things: responded, EndTurn, 0 calls (4 in, 13469 cached, 510 out)

== the task is done; its result:

**Done**
- I ran both tasks in one scope. An engineer built `textstats/wordcount.py` (`count_words`, `top`) and `tests/test_wordcount.py`. `./check.sh` passed with 5 tests. It is…
- A designer wrote `docs/unicode.md` with the decision, the rejected alternatives and what is unsupported. It is on origin `unicode-design` at `9d5838d`. I confirmed bot…
- **Your choice:** a lone apostrophe is not a word, because a word must contain a letter or digit. To overturn it, drop the "no letter or digit left" clause.

**Learned**
- The designer amended the engineer's plan after the engineer had already built from it. It added NFC normalisation, counting combining marks as word characters, and tri…
- Running the two tasks in parallel meant the design landed after the build. It also reached me as a different plan from the one I had given the engineer.

**Left undone**
- `wordcount` still has the plain version. It needs the designer's change, about 15 lines: the doc's snippet as the tokenizer, tests for the cases in the doc, and the RE…
- Neither branch is merged to `main`. I did not open PRs.
- Chinese, Japanese and Thai are deliberately unsupported. Say if you need them counted.

harness-brain: SIGINT: shutting down; requests in flight have 300s to finish (again to stop now)
  186.2s 66ea5088543990a4.78  brain shutting down, within 300s
harness-brain: shut down
```

The orchestrator's report, which it ended its turn with and waited on you:

```
Both branches are on origin. `wordcount` is at `af46176` and `unicode-design` is at `9d5838d`.

- **`wordcount`:** the engineer built `textstats/wordcount.py` (`count_words`, `top`) and `tests/test_wordcount.py`. `./check.sh` passes with 5 tests, including “don’t” and `don't` counting as the same single word.
- **`unicode-design`:** the designer wrote `docs/unicode.md`, with the decision, the rejected alternatives and what is unsupported.

**Your choice, a lone apostrophe: I decided it is not a word.** A word must contain at least one letter or digit. Counting it would put stray quote marks and `''` into `top()`. The cost is that apostrophes get trimmed from the ends of a word, so `dogs'` counts as `dogs` and `'tis` as `tis`. To overturn it, drop the "no letter or digit left" clause; the doc says this too.

**The two branches don't match yet.** The designer amended the plan the engineer built from, in three ways:
- NFC normalisation, and combining marks count as word characters. Without this, `हिन्दी` splits into pieces.
- Apostrophes are trimmed from both ends of a run.
- The README sentence needs updating to match.

The engineer built the plain version, so a follow-up task is needed on `wordcount`. It is about 15 lines, using the doc's snippet and new tests. I haven't launched it, since you asked for a report once both were back.

Chinese, Japanese and Thai are deliberately unsupported: a run between spaces and punctuation counts as one "word". Say if you need to count those scripts.
```
