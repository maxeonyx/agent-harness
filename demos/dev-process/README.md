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

A run at 22:36 NZDT on 2026-10-17, with nobody at the page. The brain's log, with request, dispatch and completion lines dropped, paths made relative and lines cut at 170 characters:

```
    0.0s c0e3a6c19daaae21.1  brain launched, build f7a8d454f0db
    0.0s c0e3a6c19daaae21.1  limb meta defined
    0.0s c0e3a6c19daaae21.1  limb web defined
    0.0s c0e3a6c19daaae21.1  limb user defined
    0.0s c0e3a6c19daaae21.1  limb roles defined
    0.0s c0e3a6c19daaae21.2  roles connected
    0.0s aecdcc4ed5c49611.1  environment chat defined
    0.0s aecdcc4ed5c49611.1  environment designer defined
    0.0s aecdcc4ed5c49611.1  environment engineer defined
    0.0s aecdcc4ed5c49611.1  environment orchestrator defined
    0.0s aecdcc4ed5c49611.1  role roles/chat.md changed
    0.0s aecdcc4ed5c49611.1  role roles/designer.md changed
    0.0s aecdcc4ed5c49611.1  role roles/engineer.md changed
    0.0s aecdcc4ed5c49611.1  role roles/orchestrator.md changed
    0.0s c0e3a6c19daaae21.4  limb target/dev-process-run/textstats defined
    0.0s c0e3a6c19daaae21.4  tab 72fa185e50198ffc is Max's
    0.0s 72fa185e50198ffc.1  orchestrator created (orchestrator)
    2.0s c0e3a6c19daaae21.6  user connected
    2.0s c0e3a6c19daaae21.7  target/dev-process-run/textstats connected
    2.0s 97ed02c0c459ea48.1  user described: 1 tools
    2.0s 4921a4031e635d54.1  target/dev-process-run/textstats described: 5 tools
    2.0s c0e3a6c19daaae21.10  orchestrator has a new root, on claude-opus-5-5
    4.9s c0e3a6c19daaae21.13  orchestrator's bash made {"command": "cat README.md; ls -R | head -30; cat check.sh; git log --oneline | head", "grep": ""}
    4.9s c0e3a6c19daaae21.13  orchestrator: responded, ToolUse, 1 calls (4 in, 0 cached, 117 out)
   15.4s c0e3a6c19daaae21.18  orchestrator's spawn made
   15.4s c0e3a6c19daaae21.18  orchestrator: responded, ToolUse, 1 calls (2 in, 9862 cached, 1079 out)
   15.4s c0e3a6c19daaae21.19  limb target/dev-process-run/textstats-wordcount defined
   15.4s c0e3a6c19daaae21.19  orchestrator-wordcount created (engineer)
   15.4s c0e3a6c19daaae21.19  limb target/dev-process-run/textstats-unicode-design defined
   15.4s c0e3a6c19daaae21.19  orchestrator-unicode created (designer)
   15.7s c0e3a6c19daaae21.20  target/dev-process-run/textstats-unicode-design connected
   15.7s c0e3a6c19daaae21.21  target/dev-process-run/textstats-wordcount connected
   15.7s aad7265dabad7823.1  target/dev-process-run/textstats-unicode-design described: 5 tools
   15.7s 2b9502312b25d502.1  target/dev-process-run/textstats-wordcount described: 5 tools
   15.7s c0e3a6c19daaae21.24  orchestrator-wordcount has a new root, on claude-opus-5-5
   15.7s c0e3a6c19daaae21.25  orchestrator-unicode has a new root, on claude-opus-5-5
   18.7s c0e3a6c19daaae21.30  orchestrator-wordcount's bash made {"command": "ls -R | head -30; cat check.sh README* 2>/dev/null; ls tests textstats 2>/dev/null; git log 
   18.7s c0e3a6c19daaae21.30  orchestrator-wordcount: responded, ToolUse, 1 calls (4 in, 0 cached, 116 out)
   20.4s c0e3a6c19daaae21.35  orchestrator-unicode's bash made {"command": "git log --oneline -5; ls -R | grep -v '^\\.' | head -50; cat README.md; cat check.sh; ls docs 
   20.4s c0e3a6c19daaae21.35  orchestrator-unicode's skill made {"name": "design"}
   20.4s c0e3a6c19daaae21.35  orchestrator-unicode: responded, ToolUse, 2 calls (4 in, 0 cached, 385 out)
   20.4s aad7265dabad7823.2  orchestrator-unicode's bash failed
   25.8s c0e3a6c19daaae21.42  orchestrator-wordcount's write made {"path": "textstats/wordcount.py", "content": "\"\"\"Word counts: a word is a run of letters, digits or 
   25.8s c0e3a6c19daaae21.42  orchestrator-wordcount's write made {"path": "tests/test_wordcount.py", "content": "import unittest\n\nfrom textstats.wordcount import count
   25.8s c0e3a6c19daaae21.42  orchestrator-wordcount's bash made {"command": "./check.sh", "grep": ""}
   25.8s c0e3a6c19daaae21.42  orchestrator-wordcount: responded, ToolUse, 3 calls (2 in, 11180 cached, 804 out)
   25.8s 2b9502312b25d502.3  file textstats/wordcount.py changed by orchestrator-wordcount's write
   25.8s 2b9502312b25d502.4  file tests/test_wordcount.py changed by orchestrator-wordcount's write
   29.2s c0e3a6c19daaae21.51  orchestrator-wordcount's bash made {"command": "git status --short && git add textstats/wordcount.py tests/test_wordcount.py && git commit -
   29.2s c0e3a6c19daaae21.51  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 11596 cached, 225 out)
   34.3s c0e3a6c19daaae21.56  orchestrator-wordcount: responded, EndTurn, 0 calls (2 in, 12780 cached, 444 out)
  121.8s c0e3a6c19daaae21.57  orchestrator-unicode's bash made {"command": "./check.sh; echo \"exit $?\"; python3 --version; python3 - <<'EOF'\nimport re, unicodedata as 
  121.8s c0e3a6c19daaae21.57  orchestrator-unicode: responded, ToolUse, 1 calls (2 in, 12533 cached, 9235 out)
  161.8s c0e3a6c19daaae21.62  orchestrator-unicode's write made {"path": "docs/unicode.md", "content": "# How `count_words` treats non-ASCII text\n\n## Decision\n\n`count
  161.8s c0e3a6c19daaae21.62  orchestrator-unicode's bash made {"command": "cat > rule-check.ignore.py <<'EOF'\nimport unicodedata as u\nfrom collections import Counter\n
  161.8s c0e3a6c19daaae21.62  orchestrator-unicode: responded, ToolUse, 2 calls (2 in, 17216 cached, 4578 out)
  161.8s aad7265dabad7823.4  file docs/unicode.md changed by orchestrator-unicode's write
  165.4s c0e3a6c19daaae21.69  orchestrator-unicode's bash made {"command": "git add docs/unicode.md && git diff --cached --stat && git commit -q -m \"Decide how count_wor
  165.4s c0e3a6c19daaae21.69  orchestrator-unicode: responded, ToolUse, 1 calls (2 in, 26943 cached, 298 out)
  177.0s c0e3a6c19daaae21.74  orchestrator-unicode: responded, EndTurn, 0 calls (2 in, 31850 cached, 1170 out)
  180.3s c0e3a6c19daaae21.78  orchestrator's bash made {"command": "git fetch -q && git log --oneline origin/wordcount -2 && git log --oneline origin/unicode-design -2", 
  180.3s c0e3a6c19daaae21.78  orchestrator: responded, ToolUse, 1 calls (2 in, 10258 cached, 193 out)
  188.4s c0e3a6c19daaae21.83  orchestrator: responded, EndTurn, 0 calls (2 in, 12917 cached, 664 out)
  247.0s c0e3a6c19daaae21.84  brain shutting down, within 300s
  247.0s c0e3a6c19daaae21.85  roles lost
  247.0s c0e3a6c19daaae21.86  target/dev-process-run/textstats-unicode-design lost
  247.0s c0e3a6c19daaae21.87  target/dev-process-run/textstats lost
  247.0s c0e3a6c19daaae21.88  user lost
  247.0s c0e3a6c19daaae21.89  target/dev-process-run/textstats-wordcount lost
```

The orchestrator's report, which it ended its turn with and now waits on you:

```
I've confirmed both branches are on origin.

- **`wordcount`** (`69f1559`): adds `textstats/wordcount.py` and 4 tests, and `./check.sh` passes. Words can contain letters and digits from any language. “don’t” and "don't" both count as the single word `don't`.
- **`unicode-design`** (`139fdf6`): adds `docs/unicode.md`.
  - It recommends normalising text so an accented letter counts the same however it was typed (NFC normalisation), letting words contain accent marks and vowel signs, and using `lower()`.
  - Chinese, Japanese and Thai are not split into words, because the standard library has nothing that can do it.
  - It lists the rejected alternatives: ASCII-only words, the regex `[\w']+`, NFKC normalisation, `casefold()`, and one word per Chinese character. `./check.sh` on that branch says "NO TESTS RAN", which only means the branch has no tests yet; `main` gives the same result.

**Decision for you to overturn — the lone apostrophe:** I decided a lone `'` or `’` is not a word, because a word must contain at least one letter or digit. A stray apostrophe is punctuation, and counting it as a word helps nobody. The cost is that the README's "a run of letters, digits or apostrophes" now has an unwritten extra condition.

**Needs you — the two branches disagree:** the designer also recommends trimming apostrophes from the ends of words. Your curly ’ is also the closing single quote, so without trimming, ‘yes’ counts as `yes'`, separately from `yes`. Trimming costs `dogs'` becoming `dogs` and `'em` becoming `em`. The `wordcount` code doesn't trim, as the README is written. I'd adopt trimming, since you type curly quotes. That means a small follow-up on `wordcount`: the trim step, a test for it, and a change to that README line. I left it out to keep this run small; say yes and I'll have it done.
```
