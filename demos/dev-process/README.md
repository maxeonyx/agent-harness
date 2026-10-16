# dev-process: the dev process, run by the harness

```sh
demos/dev-process/run.sh
```

Then open https://mx-hv-mj.local:4325 and watch the tree (another port: `PORT=4400`; a new run: `FRESH=1`). It takes about three minutes on Opus and spends a few dozen requests of the subscription. The face keeps serving the finished run until you stop it.

What it does:

1. `scratch.sh` makes `target/dev-process/textstats.git`, a bare repository standing in for GitHub, and `textstats`, your clone of it: a Python package whose README says what `wordcount` should be.
2. The brain reads `roles/*.md`: orchestrator, designer and engineer, each with its model, effort, limbs, places and the roles it can start. They are the same files Claude Code reads through `.claude/agents/`. Only the orchestrator holds `max`, your limb, whose tool is `ask`. The brain reads the files again every two seconds; an edit is a new version of the role, and a running task of that role is told at its next request.
3. The command line starts an orchestrator in your clone, as you, with `mandate.md`, and delivers `words.md` to it as your own words.
4. The orchestrator does two things at once. It `spawn`s an engineer (`wordcount`) and a designer (`unicode-design`), each in its own clone, and gives both your words with `quotes`: they reach each agent verbatim, labelled as yours, after the brief. And it decides the one choice the mandate says is yours, then asks you in the `background`, so the question waits in your queue while the work goes on.
5. Each agent commits and pushes its branch. The orchestrator cannot finish until you have answered; once both agents are back and you have, it checks the branches on origin and reports.

What to do on the page:

- **Waiting on you.** The orchestrator's question is there with an answer box. Answer it: the orchestrator gets your answer as a message at its next request.
- **Interrupt the orchestrator** while it waits on its agents. The core refuses ("the task is waiting on its children; talk to it through an aside") and the page offers **Open an aside**: a chat with a fork of the orchestrator, whose first message is whatever is in the compose box. Talk to it, then press **Done**; its last message goes back to the orchestrator.
- **The header.** How much of the subscription is used, and when each window resets.
- **The tree, a task's context, New task**, as before.

## Real output

A run at 05:51 NZT on 2026-10-17. Lines are cut at 170 characters; request, dispatch and completion lines are dropped, except your answer. The aside and the answer were done on the page in headless Chromium. At 132.5s `roles/orchestrator.md` was edited by hand (one line added), and put back at 231.4s.

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
    0.0s    13  words delivered to orchestrator
    0.0s    14  user connected
    0.0s    15  textstats connected
    0.0s    16  orchestrator has a new root, on claude-opus-5-5
    3.5s    19  orchestrator's bash made {"command":"cat README.md; ls -R | head -30; cat check.sh; git log --oneline | head -5","grep":""}
    3.5s    20  orchestrator: responded, ToolUse, 1 calls (4 in, 0 cached, 112 out)
   20.0s    25  orchestrator's background made
   20.0s    26  orchestrator's spawn made
   20.0s    27  orchestrator: responded, ToolUse, 2 calls (2 in, 8000 cached, 1161 out)
   20.0s    29  limb textstats-wordcount defined
   20.0s    30  orchestrator-wordcount created (engineer)
   20.0s    31  limb textstats-unicode-design defined
   20.0s    32  orchestrator-unicode created (designer)
   20.0s    34  orchestrator's ask made {"question":"textstats: should a lone apostrophe (`'` or `’` on its own, e.g. in `rock ' roll`) count as a word? I've told the eng
   20.3s    36  textstats-unicode-design connected
   20.3s    37  textstats-wordcount connected
   20.3s    38  orchestrator-unicode has a new root, on claude-opus-5-5
   20.3s    39  orchestrator-wordcount has a new root, on claude-opus-5-5
   23.2s    44  orchestrator-unicode's bash made {"command":"cat AGENTS.md README.md; ls -R | head -50; cat check.sh; cat textstats/*.py 2>/dev/null; ls tests; git log --
   23.2s    45  orchestrator-unicode: responded, ToolUse, 1 calls (4 in, 0 cached, 143 out)
   23.6s    49  orchestrator-wordcount's bash made {"command":"cat README.md check.sh; ls -R textstats tests; git log --oneline|head","grep":""}
   23.6s    50  orchestrator-wordcount: responded, ToolUse, 1 calls (4 in, 0 cached, 94 out)
   28.1s    56  orchestrator-wordcount's bash made {"command":"cat > textstats/wordcount.py <<'EOF'\n\"\"\"Word counts: a word is a run of letters, digits or apostrophes 
   28.1s    57  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 8723 cached, 396 out)
   35.6s    62  orchestrator-wordcount's bash made {"command":"cat > textstats/wordcount.py <<'EOF'\n\"\"\"Word counts: a word is a run of letters, digits or apostrophes 
   35.6s    63  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 9066 cached, 812 out)
   38.2s    68  orchestrator-wordcount's bash made {"command":"git add textstats/wordcount.py tests/test_wordcount.py && git commit -qm \"Add wordcount: count_words and t
   38.2s    69  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 9474 cached, 162 out)
   44.5s    74  orchestrator-wordcount: responded, EndTurn, 0 calls (2 in, 10316 cached, 531 out)
   87.0s    75  orchestrator-unicode's skill made {"name":"design"}
   87.0s    76  orchestrator-unicode's bash made {"command":"python3 --version; python3 - <<'EOF'\nimport unicodedata as u, re\nfor s in [\"hi\\u0301\", \"\\u093f\", \"\\
   87.0s    77  orchestrator-unicode: responded, ToolUse, 2 calls (2 in, 10041 cached, 5909 out)
   95.7s    85  orchestrator-aside created (an aside of orchestrator)
   99.0s    89  orchestrator-aside: responded, EndTurn, 0 calls (4 in, 8390 cached, 136 out)
   99.7s    90  orchestrator-aside is now Autonomous
   99.7s    91  Done prompt delivered to orchestrator-aside
  102.6s    94  orchestrator-aside: responded, EndTurn, 0 calls (4 in, 9729 cached, 78 out)
  102.6s    95  the result of aside orchestrator-aside delivered to orchestrator
  113.3s    96  orchestrator-unicode's bash made {"command":"python3 - <<'EOF'\nimport unicodedata\nfrom itertools import groupby\ndef _is_word_char(c):\n    return c.isa
  113.3s    97  orchestrator-unicode: responded, ToolUse, 1 calls (2 in, 10525 cached, 2459 out)
  119.1s   102  orchestrator's ask completed
  126.2s   103  orchestrator-unicode's write made {"content":"# Non-ASCII text in `count_words`\n\n## Decision\n\nA word is a maximal run of word characters that contains
  126.2s   104  orchestrator-unicode: responded, ToolUse, 1 calls (2 in, 20855 cached, 1398 out)
  126.2s   107  file docs/unicode.md changed by orchestrator-unicode's write
  129.1s   110  orchestrator-unicode's bash made {"command":"git status --short; git add docs/unicode.md && git commit -q -m \"Decide how count_words treats non-ASCII tex
  129.1s   111  orchestrator-unicode: responded, ToolUse, 1 calls (2 in, 23416 cached, 224 out)
  131.3s   116  orchestrator-unicode's bash made {"command":"git ls-remote origin unicode-design","grep":""}
  131.3s   117  orchestrator-unicode: responded, ToolUse, 1 calls (2 in, 24837 cached, 96 out)
  132.5s   121  role roles/orchestrator.md changed
  132.5s   122  environment orchestrator defined
  143.9s   124  orchestrator-unicode: responded, EndTurn, 0 calls (2 in, 25145 cached, 1292 out)
  146.6s   128  orchestrator's bash made {"command":"git fetch -q origin && git log --oneline origin/wordcount -3 && git log --oneline origin/unicode-design -3","grep":""
  146.6s   129  orchestrator: responded, ToolUse, 1 calls (4 in, 9577 cached, 108 out)
  156.6s   134  orchestrator: responded, EndTurn, 0 calls (2 in, 11666 cached, 826 out)
```

What the orchestrator's next request carried, after the designer was done (from the journal):

```
[harness] Max talked to a fork of you meanwhile. It ended with:

Aside over. I told Max that two agents are running, on the `wordcount` and `unicode-design` branches, and that I haven't yet checked either branch on origin.
[harness] Max answered the question you asked in the background:
No, keep dropping lone apostrophes. Good call.
[harness] Your role's definition changed. Your request parameters follow it now; its prompt and tools take effect from your next `handover`.
```

The brief both agents started with:

```
[harness] The brief every agent started with you was given:

Repo: textstats (Python 3, standard library only). Read its AGENTS.md and README.md first. […] Keep it small. Max's message follows.
[harness] Max's own words, quoted verbatim (`max-47dad73d`):
Keep it small and cheap: this is a demonstration of the process, not a product. And I type curly apostrophes, so “don’t” has to count as one word.
```

The header: `used: 5h 14% (resets 10:00) · 7d 40% (resets Mon 11:00) · as of 05:52:29`.

The result, trimmed:

```
Both branches are on origin, each one commit on top of `622d13d Start textstats`: […]

**The two branches disagree.** The engineer built the plain rule before the design was in:
- `don't` and `don’t` are counted as two different words; the design says they should be one. […]

I haven't changed anything to reconcile them. **Your call:** shall I have the engineer bring `wordcount` in line with `docs/unicode.md`? […]
```
