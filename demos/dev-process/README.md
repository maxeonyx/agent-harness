# dev-process: the dev process, run by the harness

```sh
demos/dev-process/run.sh
```

Then open https://mx-hv-mj.local:4325 and watch the map (another port: `PORT=4400`; a new run: `FRESH=1`). It takes about three minutes on Opus and spends a few dozen requests of the subscription. The face keeps serving the finished run until you stop it.

What it does:

1. `scratch.sh` makes `target/dev-process/textstats.git`, a bare repository standing in for GitHub, and `textstats`, your clone of it: a Python package whose README says what `wordcount` should be.
2. The brain reads `roles/*.md`: orchestrator, designer and engineer, each with its model, effort, limbs, places and the roles it can start. They are the same files Claude Code reads through `.claude/agents/`. The brain reads the files again every two seconds; an edit is a new version of the role, and a running task of that role is told at its next request.
3. The command line starts an orchestrator in your clone, as you, with `mandate.md`. Like every task you start, it is a chat with you: when it ends a turn it waits on you, and it is done only when you press **Done**. The mandate is your words, so the orchestrator can quote it.
4. The orchestrator does two things at once. It `spawn`s an engineer (`wordcount`) and a designer (`unicode-design`), each in its own clone, and gives both your mandate with `quotes`: it reaches each agent verbatim, labelled as yours, after the brief. And it decides the one choice the mandate says is yours, so the work goes on without you.
5. Each agent commits and pushes its branch. Once both are back, the orchestrator checks the branches on origin and reports, naming the choice it made for you, and waits on you.

What to do on the page:

- **The choice it made for you.** The orchestrator's report says what it decided about lone apostrophes and why. Reply to overturn it: your reply is the next message it reads.
- **An aside** while the orchestrator waits on its agents. Its header offers **Aside** instead of **Interrupt** (the core refuses to interrupt it: "the task is waiting on its children; talk to it through an aside"). Write first if you like, then press **Aside**: a chat with a fork of the orchestrator opens, and what you wrote is its first message. On the map it is one more member of the scope the orchestrator waits on. When the others are done it is told so; you decide when it is done, and press **Done** twice: it summarises what came of your talk, and that goes back to the orchestrator with the others' results.
- **Sending while an agent works.** Open a working agent and write to it. **Send now** (Ctrl+Enter) reaches it with its next request; **At turn end** (Ctrl+Shift+Enter) waits for its turn to end, so on the orchestrator it waits until the agents are back and it has reported. Either way the model is told where in its work you wrote, as "Max wrote this while you were working, before your last 4 tool calls (from `toolu_…` on), asking that it wait for the end of your turn". Enter alone is a new line.
- **What a context holds.** The context gauge in an open node's header; click it for the split.
- **Done** on the orchestrator once it has reported: its last turn writes its result. Thirty minutes after the whole project is done it folds into a small card on a shelf below anything live; click the card for the root's log, or ⌄ to unfold it.
- **The header.** How much of the subscription is used, and when each window resets.

## Real output

A run at 12:08 NZT on 2026-10-17, with your part played in headless Chromium on the map, the way you would click it: open the orchestrator while it waits on its agents (its header offers **Aside**, and the core refuses an interrupt: "the task is waiting on its children; talk to it through an aside"), write "While they work: when the engineer is done, I want it told before you report. Keep the README short.", press **Aside**, and press **Done** on the aside once it was told the agents were done. The background question was left for you. The brain's log, with request, dispatch and completion lines dropped and lines cut at 170 characters:

```
    0.0s     1  brain launched, build harness-brain 0.0.0
    0.0s     2  limb meta defined
    0.0s     3  limb max defined
    0.0s     4  limb user defined
    0.0s     5  role roles/designer.md changed
    0.0s     6  environment designer defined
    0.0s     7  role roles/engineer.md changed
    0.0s     8  environment engineer defined
    0.0s     9  role roles/orchestrator.md changed
    0.0s    10  environment orchestrator defined
    0.0s    11  limb /home/maxeonyx/agent-harness-workspace/ah-face-land/target/dev-process-run/textstats defined
    0.0s    12  orchestrator created (orchestrator)
    0.0s    13  /home/maxeonyx/agent-harness-workspace/ah-face-land/target/dev-process-run/textstats connected
    0.0s    14  /home/maxeonyx/agent-harness-workspace/ah-face-land/target/dev-process-run/textstats described: 5 tools
    0.0s    15  user connected
    0.0s    16  user described: 1 tools
    0.0s    17  orchestrator has a new root, on claude-opus-5-5
    3.4s    20  orchestrator's bash made {"command": "cat README.md; ls -R | head -30; cat check.sh; git log --oneline | head", "grep": ""}
    3.4s    21  orchestrator: responded, ToolUse, 1 calls (4 in, 0 cached, 110 out)
   15.0s    26  orchestrator's background made
   15.0s    27  orchestrator's spawn made
   15.0s    28  orchestrator: responded, ToolUse, 2 calls (2 in, 8059 cached, 1027 out)
   15.0s    30  limb /home/maxeonyx/agent-harness-workspace/ah-face-land/target/dev-process-run/textstats-wordcount defined
   15.0s    31  orchestrator-wordcount created (engineer)
   15.0s    32  limb /home/maxeonyx/agent-harness-workspace/ah-face-land/target/dev-process-run/textstats-unicode-design defined
   15.0s    33  orchestrator-unicode-design created (designer)
   15.0s    35  orchestrator's ask made {"question":"Should a lone apostrophe (`'` or `’` on its own, or a run of only apostrophes) count as a word in `count_words`? I've
   15.3s    37  /home/maxeonyx/agent-harness-workspace/ah-face-land/target/dev-process-run/textstats-unicode-design connected
   15.3s    38  /home/maxeonyx/agent-harness-workspace/ah-face-land/target/dev-process-run/textstats-unicode-design described: 5 tools
   15.3s    39  /home/maxeonyx/agent-harness-workspace/ah-face-land/target/dev-process-run/textstats-wordcount connected
   15.3s    40  /home/maxeonyx/agent-harness-workspace/ah-face-land/target/dev-process-run/textstats-wordcount described: 5 tools
   15.3s    41  orchestrator-unicode-design has a new root, on claude-opus-5-5
   15.3s    42  orchestrator-wordcount has a new root, on claude-opus-5-5
   17.6s    47  orchestrator-wordcount's bash made {"command": "cat README.md check.sh; ls -R | grep -v .git | head -30; git log --oneline | head", "grep": ""}
   17.6s    48  orchestrator-wordcount: responded, ToolUse, 1 calls (4 in, 0 cached, 100 out)
   19.1s    53  orchestrator-unicode-design's bash made {"command": "cat AGENTS.md README.md; ls -la; git log --oneline -5; cat check.sh; find . -path ./.git -prune -o -t
   19.1s    54  orchestrator-unicode-design: responded, ToolUse, 1 calls (4 in, 0 cached, 224 out)
   24.2s    59  orchestrator-unicode-design's skill made {"name": "design"}
   24.2s    60  orchestrator-unicode-design's bash made {"command": "./check.sh; python3 --version; python3 -c \"\nimport re,unicodedata\nfor s in ['café', 'cafe\\u0301',
   24.2s    61  orchestrator-unicode-design: responded, ToolUse, 2 calls (2 in, 10389 cached, 375 out)
   26.0s    67  orchestrator-wordcount's bash made {"command": "cat > textstats/wordcount.py <<'EOF'\n\"\"\"Word counts: a word is a run of letters, digits or apostrophes
   26.0s    68  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 9020 cached, 988 out)
   30.8s    74  orchestrator-wordcount: responded, EndTurn, 0 calls (2 in, 9399 cached, 436 out)
   42.6s    75  a face changed focus
   45.2s    76  orchestrator-aside created (an aside of orchestrator)
   46.2s    78  a face changed focus
   53.9s    80  orchestrator-aside: responded, EndTurn, 0 calls (4 in, 8448 cached, 507 out)
  112.3s    81  orchestrator-unicode-design's bash made {"command": "python3 - <<'EOF'\nimport unicodedata as u\nfrom itertools import groupby\nfrom collections import Co
  112.3s    82  orchestrator-unicode-design: responded, ToolUse, 1 calls (2 in, 11328 cached, 8025 out)
  130.9s    87  orchestrator-unicode-design's write made {"path": "docs/unicode.md", "content": "# How `count_words` treats non-ASCII text\n\n## Decision\n\nA word is a m
  130.9s    88  orchestrator-unicode-design: responded, ToolUse, 1 calls (2 in, 16129 cached, 1890 out)
  134.6s    94  orchestrator-unicode-design's bash made {"command": "./check.sh && git status --short && git add docs/unicode.md && git diff --staged --stat && git commit
  134.6s    95  orchestrator-unicode-design: responded, ToolUse, 1 calls (2 in, 24560 cached, 253 out)
  134.6s    97  orchestrator-unicode-design's bash failed
  139.1s   100  orchestrator-unicode-design's bash made {"command": "git status --short && git add docs/unicode.md && git diff --staged --stat && git commit -q -m \"Decid
  139.1s   101  orchestrator-unicode-design: responded, ToolUse, 1 calls (2 in, 26473 cached, 384 out)
  149.4s   106  orchestrator-unicode-design: responded, EndTurn, 0 calls (2 in, 26783 cached, 1017 out)
  149.4s   107  Unblocked prompt delivered to orchestrator-aside
  153.2s   110  orchestrator-aside: responded, EndTurn, 0 calls (4 in, 9714 cached, 183 out)
  173.7s   111  a face changed focus
  176.6s   112  Done prompt delivered to orchestrator-aside
  182.4s   115  orchestrator-aside: responded, EndTurn, 0 calls (4 in, 10271 cached, 451 out)
  185.1s   119  orchestrator's bash made {"command": "git fetch -q; for b in wordcount unicode-design; do echo \"== $b\"; git log --oneline origin/$b | head -3; git diff 
  185.1s   120  orchestrator: responded, ToolUse, 1 calls (2 in, 9504 cached, 154 out)
  186.4s   124  a face changed focus
  192.5s   126  a face changed focus
  196.9s   127  orchestrator: responded, EndTurn, 0 calls (2 in, 11452 cached, 1127 out)
```

The aside's last turn, which went back to the orchestrator with the engineer's and the designer's results (trimmed):

```
Max said, verbatim (`max-35986c6f`): "While they work: when the engineer is done, I want it told before you report. Keep the README short."

What came of the talk:
- **"I want it told before you report":** this was ambiguous. I gave Max two readings: […] He picked neither and closed the aside, so nothing is settled.
- **README:** I said neither mandate touches it, that I'd check both branches left it alone […]
- **Lone apostrophe:** Max hasn't answered the background question yet. […]

Do differently now:
1. Before reporting, check that neither branch changed `README.md` […]
2. Take reading (1) as the default. When Max answers the apostrophe question, and if he says yes, `resume` the engineer with the change before the final report. […]
3. In the report, say I took his "told" remark as reading (1), so he can correct it.
```

The orchestrator then reported, and waits on you, with the apostrophe question still open.
