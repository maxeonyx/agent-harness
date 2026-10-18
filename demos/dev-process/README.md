# dev-process: the dev process, run by the harness

```sh
demos/dev-process/run.sh
```

It runs on the test brain `dev-process` (`dev/drive.mjs`): open the link it prints and watch the map (a new run: `FRESH=1`). Every role runs on Sonnet at low effort, about a dozen requests on the subscription and under a minute to the report (`MODEL=…` for another; `MODEL=scripted` only shows the page). It finishes once the orchestrator waits on you and leaves the brain serving the run; `node dev/drive.mjs down --brain dev-process` ends it and the workspaces it started.

What it does:

1. `scratch.sh` makes `target/dev-process/textstats.git`, a bare repository standing in for GitHub, and `textstats`, your clone of it: a Python package whose README says what `wordcount` should be.
2. The brain reads `roles/*.md`: orchestrator, designer and engineer, each with its model, effort, limbs, places and the roles it can start. The designer and engineer are the same files Claude Code reads through `.claude/agents/`. The brain reads the files again every two seconds; an edit is a new version of the role, and a running task of that role is told at its next request.
3. Max starts an orchestrator in your clone at a tab, as you do at the page (the script runs `node dev/drive.mjs chat`), with `mandate.md`. Like every task you start, it is a chat with you: when it ends a turn it waits on you, and it is done only when you press **Done**. The mandate is your words, so the orchestrator can quote it.
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

A run on Sonnet at low effort at 04:32 NZDT on 2026-10-18, with nobody at the page: 11 requests, the report 25 s after the chat started. What the script prints: the brain's link, then the orchestrator's chat once it waits on you (`drive log`, each tool result cut short).

```
brain dev-process on https://mx-hv-mj.local:4470/ (pid 2429793; every role on sonnet/low)
log …/.drive.ignore/dev-process/brain.log
end it with: node dev/drive.mjs down --brain dev-process
Max's turn since 04:32:24
dfd2c4a126caf181.1.1 "textstats needs two things": orchestrator on claude-sonnet-5-5 in …/target/dev-process/textstats
harness: [harness] Max's mandate to you (`max-88b0a10d` quotes it in a brief):
max: textstats needs two things, which can go in parallel, so start them in one scope:

    1. An engineer builds `textstats/wordcount.py` and its tests as the README says, on the branch `wordcount`, runs `./check.sh`, commits and pushes.
    2. A designer decides how `count_words` should treat non-ASCII text (accented letters, curly apostrophes, scripts without spaces). It writes the decision, and the alterna … (10 lines, 1120 chars)
  → bash {"command":"cat README.md; ls; git branch -a"}
  ← # textstats

    Small text statistics, standard library only, Python 3. … (17 lines, 532 chars)
agent: Decision: a lone apostrophe is not a word (a word must contain a letter or digit); both curly ’ and straight ' count as apostrophes.
  → spawn {"brief":"Max's mandate (max-88b0a10d): textstats needs (1) an engineer building textstats/wordcount.py and tests as the README says, on branch `wordcount`, running ./check.sh, commit+push; (2) a desi … (1 lines, 1406 chars)
  ← ## textstats needs two things-wordcount
    I built `textstats/wordcount.py` with `count_words` and `top`, plus `tests/test_wordcount.py`. `./check.sh` passes, and the work is pushed to `origin wordcount`.
     … (19 lines, 2001 chars)
  → bash {"command":"git fetch -q; git log --oneline origin/wordcount | head -3; git log --oneline origin/unicode-design | head -3"}
  ← 6cf8ee0 Add wordcount
    a12c781 Start textstats
    edfeaba Add docs/unicode.md … (5 lines, 111 chars)
agent: Both branches are on origin. I did not read `docs/unicode.md` or the wordcount code myself, so the points below are the agents' own reports.

    - **`wordcount`** (`6cf8ee0`): `textstats/wordcount.py` and `tests/test_wordcount.py`, with `./check.sh` passing. There are 5 tests, including “don’t” as one word and a lone apostrophe not counting.
    - **`unicode-design`** (`edfeaba`): `docs/unicode.md` with  … (8 lines, 1082 chars)
state: Max's turn since 04:32:24
child 6fd3711b01abcb1b.19.2 "textstats needs two things-wordcount": result since 04:32:16
child 6fd3711b01abcb1b.19.4 "textstats needs two things-unicode": result since 04:32:19

== the brain serves on; its log is .drive.ignore/dev-process/brain.log
end it with: node dev/drive.mjs down --brain dev-process
```
