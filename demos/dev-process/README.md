# dev-process: the dev process, run by the harness

```sh
demos/dev-process/run.sh
```

Then open https://mx-hv-mj.local:4325 and watch the tree (another port: `PORT=4400`; a new run: `FRESH=1`). It takes about three minutes on Opus and spends a few dozen requests of the subscription. The face keeps serving the finished run until you stop it.

What it does:

1. `scratch.sh` makes `target/dev-process/textstats.git`, a bare repository standing in for GitHub, and `textstats`, your clone of it: a Python package whose README says what `wordcount` should be.
2. The brain reads `roles/*.md`: orchestrator, designer and engineer, each with its model, effort, limbs, places and the roles it can start. They are the same files Claude Code reads through `.claude/agents/`. Only the orchestrator holds `max`, your limb, whose tool is `ask`. The brain reads the files again every two seconds; an edit is a new version of the role, and a running task of that role is told at its next request.
3. The command line starts an orchestrator in your clone, as you, with `mandate.md`. Like every task you start, it is a chat with you: when it ends a turn it waits on you, and it is done only when you press **Done**. The mandate is your words, so the orchestrator can quote it.
4. The orchestrator does two things at once. It `spawn`s an engineer (`wordcount`) and a designer (`unicode-design`), each in its own clone, and gives both your mandate with `quotes`: it reaches each agent verbatim, labelled as yours, after the brief. And it decides the one choice the mandate says is yours, then asks you in the `background`, so the question waits in your queue while the work goes on.
5. Each agent commits and pushes its branch. Once both are back and you have answered, the orchestrator checks the branches on origin, reports, and waits on you.

What to do on the page:

- **Waiting on you.** The orchestrator's question is there with an answer box. Answer it: the orchestrator gets your answer as a message at its next request.
- **Interrupt the orchestrator** while it waits on its agents. The core refuses ("the task is waiting on its children; talk to it through an aside") and the page offers **Open an aside**: a chat with a fork of the orchestrator, whose first message is whatever is in the compose box. The aside is one more of the agents the orchestrator waits on. When the others are done it is told so; you decide when it is done, and press **Done**: it summarises what came of your talk, and that goes back to the orchestrator with the others' results.
- **Done** on the orchestrator once it has reported: its last turn writes its result.
- **The header.** How much of the subscription is used, and when each window resets.

## Real output

A run at 11:32 NZT on 2026-10-17. Lines are cut at 170 characters; request, dispatch and completion lines are dropped. Your part was played over the face's HTTP, as the page does it: at 15 s interrupt the orchestrator (refused), open an aside saying "How are the two agents getting on? Anything I should know before they finish?", then tell it "If the designer finds curly apostrophes need anything beyond the plain rule, I want the engineer told before the orchestrator reports.", answer the ask, and press **Done** on the aside once it was told the agents were done.

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
    0.0s    11  limb textstats defined
    0.0s    12  orchestrator created (orchestrator)
    0.0s    13  user connected
    0.0s    14  textstats connected
    0.0s    15  textstats described: 5 tools
    0.0s    16  user described: 1 tools
    0.0s    17  orchestrator has a new root, on claude-opus-5-5
    3.8s    20  orchestrator's bash made {"command": "cat README.md; ls -R | head -30; cat check.sh; git log --oneline | head", "grep": ""}
    3.8s    21  orchestrator: responded, ToolUse, 1 calls (4 in, 0 cached, 117 out)
   18.0s    26  orchestrator's background made
   18.0s    27  orchestrator's spawn made
   18.0s    28  orchestrator: responded, ToolUse, 2 calls (2 in, 8060 cached, 1369 out)
   18.0s    30  orchestrator's ask made {"question":"textstats: should a lone apostrophe (`'` or `’` on its own) count as a word? I've told the engineer no: a run made on
   18.0s    33  limb textstats-wordcount defined
   18.0s    34  orchestrator-wordcount created (engineer)
   18.0s    35  limb textstats-unicode-design defined
   18.0s    36  orchestrator-unicode-design created (designer)
   18.2s    37  textstats-unicode-design connected
   18.2s    38  textstats-unicode-design described: 5 tools
   18.3s    39  textstats-wordcount connected
   18.3s    40  textstats-wordcount described: 5 tools
   18.3s    41  orchestrator-aside created (an aside of orchestrator)
   18.3s    43  orchestrator-unicode-design has a new root, on claude-opus-5-5
   18.3s    44  orchestrator-wordcount has a new root, on claude-opus-5-5
   20.5s    49  orchestrator-wordcount's bash made {"command": "cat README.md check.sh; ls -R; git log --oneline | head", "grep": ""}
   20.5s    50  orchestrator-wordcount: responded, ToolUse, 1 calls (4 in, 0 cached, 89 out)
   21.1s    55  orchestrator-aside's bash made {"command": "git fetch -q origin 2>&1; git branch -r; for b in wordcount unicode-design; do git log --oneline -3 origin/$b 
   21.1s    56  orchestrator-aside: responded, ToolUse, 1 calls (4 in, 8456 cached, 153 out)
   21.1s    58  orchestrator-aside's bash failed
   22.6s    61  orchestrator-unicode-design's bash made {"command": "cat AGENTS.md README.md; ls -R | head -50; cat check.sh; git log --oneline | head", "grep": ""}
   22.6s    62  orchestrator-unicode-design's skill made {"name": "design"}
   22.6s    63  orchestrator-unicode-design: responded, ToolUse, 2 calls (4 in, 0 cached, 196 out)
   28.4s    71  orchestrator-aside: responded, EndTurn, 0 calls (2 in, 10048 cached, 499 out)
   28.4s    72  orchestrator-wordcount's bash made {"command": "cat > textstats/wordcount.py <<'EOF'\n\"\"\"Word counts: a word is a run of letters, digits or apostrophes
   28.4s    73  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 9134 cached, 898 out)
   28.5s    77  words delivered to orchestrator-aside
   30.7s    82  orchestrator-wordcount's bash made {"command": "git status --short && git add textstats/wordcount.py tests/test_wordcount.py && git commit -qm \"Add wordc
   30.7s    83  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 9502 cached, 168 out)
   33.4s    87  orchestrator-aside: responded, EndTurn, 0 calls (4 in, 10397 cached, 276 out)
   37.5s    89  orchestrator-wordcount: responded, EndTurn, 0 calls (2 in, 10443 cached, 545 out)
   86.9s    90  orchestrator-unicode-design's bash made {"command": "python3 --version; python3 - <<'EOF'\nimport unicodedata as u, re\ncafe_nfd = u.normalize(\"NFD\", \"
   86.9s    91  orchestrator-unicode-design: responded, ToolUse, 1 calls (2 in, 10549 cached, 5942 out)
  137.7s    96  orchestrator-unicode-design's write made {"path": "docs/unicode.md", "content": "# How `count_words` treats non-ASCII text\n\nThe rule: text that a reader
  137.7s    97  orchestrator-unicode-design: responded, ToolUse, 1 calls (2 in, 15113 cached, 4512 out)
  137.7s   100  file docs/unicode.md changed by orchestrator-unicode-design's write
  142.9s   103  orchestrator-unicode-design's bash made {"command": "python3 - <<'EOF'\nimport unicodedata as u\ndef words(text):\n    t = u.normalize(\"NFC\", text.lower
  142.9s   104  orchestrator-unicode-design: responded, ToolUse, 1 calls (2 in, 21354 cached, 450 out)
  153.5s   109  orchestrator-unicode-design's bash made {"command": "git add docs/unicode.md && git diff --cached --stat && git commit -q -m \"Decide how count_words trea
  153.5s   110  orchestrator-unicode-design: responded, ToolUse, 1 calls (2 in, 25889 cached, 233 out)
  156.1s   115  orchestrator-unicode-design's bash made {"command": "git push -u origin unicode-design 2>&1 | tail -3; git fetch -q && git log --oneline -1 origin/unicode
  156.1s   116  orchestrator-unicode-design: responded, ToolUse, 1 calls (2 in, 26566 cached, 147 out)
  169.3s   121  orchestrator-unicode-design: responded, EndTurn, 0 calls (2 in, 26884 cached, 1307 out)
  169.3s   122  Unblocked prompt delivered to orchestrator-aside
  172.3s   125  orchestrator-aside's bash made {"command": "git fetch -q origin; for b in wordcount unicode-design; do echo \"== $b\"; git log --oneline -3 origin/$b; don
  172.3s   126  orchestrator-aside: responded, ToolUse, 1 calls (4 in, 10967 cached, 152 out)
  175.2s   131  orchestrator-aside's bash made {"command": "git show origin/wordcount:textstats/wordcount.py", "grep": ""}
  175.2s   132  orchestrator-aside: responded, ToolUse, 1 calls (2 in, 11293 cached, 163 out)
  184.6s   137  orchestrator-aside: responded, EndTurn, 0 calls (2 in, 12674 cached, 859 out)
  184.7s   138  Done prompt delivered to orchestrator-aside
  192.2s   141  orchestrator-aside: responded, EndTurn, 0 calls (4 in, 13123 cached, 663 out)
  197.7s   145  orchestrator's spawn made
  197.7s   146  orchestrator: responded, ToolUse, 1 calls (4 in, 9842 cached, 430 out)
  197.7s   148  orchestrator-wordcount-unicode created (resuming orchestrator-wordcount)
  201.2s   151  orchestrator-wordcount-unicode's bash made {"command": "git fetch -q && git show origin/unicode-design:docs/unicode.md", "grep": ""}
  201.2s   152  orchestrator-wordcount-unicode: responded, ToolUse, 1 calls (4 in, 10669 cached, 92 out)
  208.1s   157  orchestrator-wordcount-unicode's bash made {"command": "cat > textstats/wordcount.py <<'EOF'\n\"\"\"Word counts: see README.md and docs/unicode.md for wha
  208.1s   158  orchestrator-wordcount-unicode: responded, ToolUse, 1 calls (2 in, 11570 cached, 872 out)
  210.8s   163  orchestrator-wordcount-unicode's bash made {"command": "git add README.md tests/test_wordcount.py textstats/wordcount.py && git commit -qm \"Apply unicode
  210.8s   164  orchestrator-wordcount-unicode: responded, ToolUse, 1 calls (2 in, 12833 cached, 200 out)
  214.9s   169  orchestrator-wordcount-unicode: responded, EndTurn, 0 calls (2 in, 13822 cached, 349 out)
  217.4s   173  orchestrator's bash made {"command": "git fetch -q && git log --oneline origin/wordcount && git log --oneline origin/unicode-design && git show origin/wor
  217.4s   174  orchestrator: responded, ToolUse, 1 calls (2 in, 12472 cached, 126 out)
  226.2s   179  orchestrator: responded, EndTurn, 0 calls (2 in, 13272 cached, 785 out)
```

The aside's result, as the orchestrator received it with the engineer's and the designer's when its `spawn` settled (trimmed):

```
## orchestrator-aside

Summary of the aside with Max:

- **Max asked** how the two agents were getting on. While they ran, neither had pushed yet. […]
- **Max's instruction (`max-28492172`):** "If the designer finds curly apostrophes need anything beyond the plain rule, I want the engineer told before the orchestrator reports."
- **What happened:** both branches are on origin. […] The designer does want more than the plain rule. […]
- **What must be done now:** resume `wordcount` before reporting to Max. […]
```

So the orchestrator resumed the engineer (`orchestrator-wordcount-unicode`) before it reported, and now waits on you.
