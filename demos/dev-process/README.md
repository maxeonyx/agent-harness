# dev-process: the dev process, run by the harness

```sh
demos/dev-process/run.sh
```

Then open https://mx-hv-mj.local:4325 and watch the tree (another port: `PORT=4400`; a new run: `FRESH=1`). It takes about two and a half minutes on Opus and spends a few dozen requests of the subscription. The face keeps serving the finished run until you stop it.

What it does:

1. `scratch.sh` makes `target/dev-process/textstats.git`, a bare repository standing in for GitHub, and `textstats`, your clone of it: a Python package whose README says what `wordcount` should be.
2. The brain reads `roles/*.md`: orchestrator, designer and engineer, each with its model, effort, limbs, places and the roles it can start. They are the same files Claude Code reads through `.claude/agents/`.
3. The command line starts an orchestrator in your clone, as you, with `mandate.md`: an engineer builds `wordcount` on the branch `wordcount`, and in parallel a designer decides how it should treat non-ASCII text, on `unicode-design`.
4. The orchestrator `spawn`s both in one call. Each is given its own clone, made from `textstats.git` before its first request: `textstats-wordcount` and `textstats-unicode-design`. Its root says the directory and branch are its own, and includes your `CLAUDE.md` and skills index. The designer loads the `design` skill.
5. Each commits and pushes its branch to `textstats.git`. When both are done, the orchestrator's `spawn` returns both results; it checks the branches on origin and reports.

What to look at on the page:

- **The tree.** Each task shows its role and its clone (`orchestrator-wordcount engineer in textstats-wordcount`). While a clone is made, the task shows `making textstats-wordcount…`. Then each row shows what its task is doing now: `model streaming`, `tool: bash: ./check.sh`, `1 of 2 children running` on the orchestrator, `✓ done`.
- **Click a task.** Its context opens beside the tree and follows it as it grows; a running command prints its output there as it comes.
- **New task.** Starts a task in your scope from a role, a directory and a mandate, the same thing the command line does.
- **The header.** How much of the subscription is used, from the latest response.

## Real output

A run at 05:18 NZT on 2026-10-17. Lines are cut at 170 characters; request, dispatch and completion lines are dropped.

```
    0.0s     1  brain launched, build harness-brain 0.0.0
    0.0s     2  limb meta defined
    0.0s     3  limb user defined
    0.0s     4  environment designer defined
    0.0s     5  environment engineer defined
    0.0s     6  environment orchestrator defined
    0.0s     7  limb textstats defined
    0.0s     8  orchestrator created (orchestrator)
    0.0s     9  textstats connected
    0.0s    10  user connected
    0.0s    11  orchestrator has a new root, on claude-opus-5-5
    2.2s    14  orchestrator's bash made {"command":"cat README.md; ls -R | head -30; cat check.sh; git log --oneline | head","grep":""}
    8.6s    20  orchestrator's spawn made
    8.6s    23  limb textstats-wordcount defined
    8.6s    24  orchestrator-wordcount created (engineer)
    8.6s    25  limb textstats-unicode-design defined
    8.6s    26  orchestrator-unicode created (designer)
    8.9s    27  textstats-unicode-design connected
    8.9s    28  textstats-wordcount connected
    8.9s    29  orchestrator-wordcount has a new root, on claude-opus-5-5
    8.9s    30  orchestrator-unicode has a new root, on claude-opus-5-5
   12.6s    42  orchestrator-unicode's skill made {"name":"design"}
   19.5s    50  orchestrator-wordcount's bash made {"command":"cat > textstats/wordcount.py <<'EOF'\n\"\"\"Word counts for plain text.\"\"\"\n\nimport re\nfrom collection
   25.0s    56  orchestrator-wordcount: responded, EndTurn, 0 calls (2 in, 8411 cached, 465 out)
   87.1s    57  orchestrator-unicode's bash made {"command":"python3 --version; python3 - <<'EOF'\nimport re, unicodedata as u\nfor s in [\"cafe\\u0301\", \"café\", \"हिन्दी\", …
  117.2s    63  orchestrator-unicode's write made {"content":"# Unicode in `count_words`\n\n## Decision\n\n1. **Word characters.** A word is a maximal run of characters that …
  117.2s    67  file docs/unicode.md changed by orchestrator-unicode's write
  124.2s    76  orchestrator-unicode's bash made {"command":"git push -u origin unicode-design 2>&1 | tail -3; ./check.sh; echo exit=$?","grep":""}
  134.4s    82  orchestrator-unicode: responded, EndTurn, 0 calls (2 in, 25083 cached, 1009 out)
  134.4s    83  orchestrator's spawn completed
  136.2s    86  orchestrator's bash made {"command":"git fetch -q origin && git log --oneline origin/wordcount -3 && git log --oneline origin/unicode-design -3","grep":""}
  143.3s    92  orchestrator: responded, EndTurn, 0 calls (2 in, 9663 cached, 722 out)

== the task is done; its result:

Both branches are on origin. I fetched and checked each one's log.

- **`wordcount`** (`b911573`): the engineer added `textstats/wordcount.py` with `count_words` and `top`, and 6 unittest cases. `./check.sh` passes, 6/6. […]
- **`unicode-design`** (`04b9c45`): the designer wrote the decision to `docs/unicode.md`.
  - **What counts as a word:** a run of letters or numbers (`isalnum`), combining marks, or the apostrophes `'` and `’`.
  - **How words are stored:** lower-cased, with `’` turned into `'`, then NFC-normalised, so the two ways of typing `café` count as one word.
  - **Chinese, Japanese and Thai** are not split: a run between punctuation counts as one word.
  - **Rejected:** stripping accents, NFKC, `casefold`, one token per Chinese character, among others. Each has a one-line reason in the doc.
  […]
Decisions for you:
- Should I run that task?
- Will anyone count Chinese, Japanese or Thai text? If so, the designer's next step is letting the caller supply a word splitter.
- The README's definition makes a lone `'` count as a word. Do you want that?
```

The origin afterwards:

```
$ git -C target/dev-process/textstats.git log --oneline --all
04b9c45 Decide how count_words treats non-ASCII text
b911573 Add wordcount: count_words and top
dd24de0 Start textstats
```
