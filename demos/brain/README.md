# brain: the harness on a root task, killed -9 mid-run, carrying on

```sh
demos/brain/run.sh
```

Then watch https://mx-hv-mj.local:4341 while it runs, and type to a task from its compose box (another port: `PORT=4400 demos/brain/run.sh`). It takes about two minutes on Opus (the roles in `roles/`) and costs a few dozen requests on the subscription.

What it does:

1. `scratch.sh` makes a git repository, `target/brain-demo/repo`: a Python package with a README that says what it should be, an `AGENTS.md`, and a `check.sh` that runs the tests and then a 15-second "style check".
2. Launch 1 starts the brain on a new database, and Max starts an orchestrator on `mandate.md` at a tab (the script runs `examples/act`). The root orchestrator reads the repository and spawns one scope of two agents: a fork of itself, in the same directory, and a fresh engineer, in its own clone on the branch `palindromes`. Each writes one module and its tests and runs `./check.sh`; the engineer commits and pushes its branch, and the orchestrator merges it.
3. While an agent's `./check.sh` is running, the script kills the brain with `kill -9`.
4. Launch 2 is started on the same database, with no other arguments. It folds its records and carries on:
   - the request that was in flight is said to be lost and sent again, reading the prompt cache;
   - the workspaces outlive the brain. Each is a replica with its own log, and keeps running its calls while its link is down; it reconnects to the new brain with the same replica, ships what it recorded meanwhile, and the running `./check.sh` completes as if nothing happened. Had the brain stayed away 15 minutes, the workspace would have stopped its calls, recorded them as interrupted, and exited; a call is unknown only when the workspace died too, before it could record how the call ended.
5. The orchestrator gets both results when the scope closes, runs `./check.sh`, commits, reports, and waits on you: like every task you start, it is a chat with you. `done.sh` presses **Done** for you at a tab (`target/release/examples/act PORT done`), and the orchestrator's last turn writes its result. The script prints that result and the repository's log.

To serve a finished run again: `target/release/harness-brain --db target/brain-demo/brain.sqlite --port 4341`. A database whose root is done does nothing but serve.

What the agents were told, from the store (`entry.content`, cut):

The fork's first message, in the framing the fork-framing spike measured: a neutral answer to the spawn, then its part as user text.
```
[{"content":"Started wordcount, palindromes.","tool_use_id":"toolu_016w…","type":"tool_result"},{"text":"You are now `orchestrator-wordcount`, a fork of `orchestrator`. Everything above is `orchestrator`'s context; from here you do one part of its work, and only that part.\n\nYour part:\nWrite textstats/wordcount.py …
```

## Real output

A run at 16:58 NZT on 2026-10-17 (on `PORT=4397`), each line cut at 150 characters.

```
== launch 1: the orchestrator starts on demos/brain/mandate.md
    0.0s 7e4c6db7b37add13.1  brain launched, build harness-brain 0.0.0
    0.0s 7e4c6db7b37add13.1  limb meta defined
    0.0s 7e4c6db7b37add13.1  limb max defined
    0.0s 7e4c6db7b37add13.1  limb user defined
    0.0s 7e4c6db7b37add13.1  limb roles defined
    0.0s 7e4c6db7b37add13.2  roles connected
    0.0s 030c769f1e06649c.1  environment designer defined
    0.0s 030c769f1e06649c.1  environment engineer defined
    0.0s 030c769f1e06649c.1  environment orchestrator defined
    0.0s 030c769f1e06649c.1  role roles/designer.md changed
    0.0s 030c769f1e06649c.1  role roles/engineer.md changed
    0.0s 030c769f1e06649c.1  role roles/orchestrator.md changed
    0.0s 7e4c6db7b37add13.4  limb target/brain-demo/repo defined
    0.0s 7e4c6db7b37add13.4  tab f46de2785a4533a6 is Max's
    0.0s f46de2785a4533a6.1  orchestrator created (orchestrator)
face: https://mx-hv-mj.local:4397
    2.0s 7e4c6db7b37add13.6  user connected
    2.0s 7e4c6db7b37add13.7  target/brain-demo/repo connected
    2.0s 104e1a0e7a27d4f1.1  user described: 1 tools
    2.0s 801368dd9134bbe5.1  target/brain-demo/repo described: 5 tools
    2.0s 7e4c6db7b37add13.10  orchestrator has a new root, on claude-opus-5-5
    2.3s 7e4c6db7b37add13.11  orchestrator: request sent
    4.0s 7e4c6db7b37add13.12  orchestrator: first byte
    5.1s 7e4c6db7b37add13.13  orchestrator's bash made {"command": "cat README.md; ls -la; ls -R textstats tests 2>/dev/null; cat check.sh; git log --
    5.1s 7e4c6db7b37add13.13  orchestrator: responded, ToolUse, 1 calls (4 in, 8026 cached, 119 out)
    5.1s 7e4c6db7b37add13.14  orchestrator's bash dispatched
    5.1s 801368dd9134bbe5.2  orchestrator's bash completed
    5.1s 7e4c6db7b37add13.16  orchestrator: request sent
    6.8s 7e4c6db7b37add13.17  orchestrator: first byte
   13.1s 7e4c6db7b37add13.18  orchestrator's spawn made
   13.1s 7e4c6db7b37add13.18  orchestrator: responded, ToolUse, 1 calls (2 in, 8026 cached, 750 out)
   13.1s 7e4c6db7b37add13.19  orchestrator's spawn dispatched
   13.1s 7e4c6db7b37add13.19  orchestrator-wordcount created (forked from orchestrator)
   13.1s 7e4c6db7b37add13.19  limb target/brain-demo/repo-palindromes defined
   13.1s 7e4c6db7b37add13.19  orchestrator-palindromes created (engineer)
   13.4s 7e4c6db7b37add13.20  orchestrator-wordcount: request sent
   13.4s 7e4c6db7b37add13.21  target/brain-demo/repo-palindromes connected
   13.4s 5cc26dd13e92cfdb.1  target/brain-demo/repo-palindromes described: 5 tools
   13.4s 7e4c6db7b37add13.23  orchestrator-palindromes has a new root, on claude-opus-5-5
   13.7s 7e4c6db7b37add13.24  orchestrator-palindromes: request sent
   15.2s 7e4c6db7b37add13.25  orchestrator-wordcount: first byte
   15.8s 7e4c6db7b37add13.26  orchestrator-palindromes: first byte
   16.7s 7e4c6db7b37add13.27  orchestrator-palindromes's bash made {"command": "cat README.md; ls -a textstats tests; cat check.sh; git status --short
   16.7s 7e4c6db7b37add13.27  orchestrator-palindromes: responded, ToolUse, 1 calls (4 in, 8051 cached, 93 out)
   16.7s 7e4c6db7b37add13.28  orchestrator-palindromes's bash dispatched
   16.7s 5cc26dd13e92cfdb.2  orchestrator-palindromes's bash completed
   16.7s 7e4c6db7b37add13.30  orchestrator-palindromes: request sent
   18.7s 7e4c6db7b37add13.31  orchestrator-palindromes: first byte
   21.0s 7e4c6db7b37add13.32  orchestrator-wordcount's write made {"path": "textstats/wordcount.py", "content": "\"\"\"Word counts: a word is a run of
   21.0s 7e4c6db7b37add13.32  orchestrator-wordcount's write made {"path": "tests/test_wordcount.py", "content": "import unittest\n\nfrom textstats.wo
   21.0s 7e4c6db7b37add13.32  orchestrator-wordcount's bash made {"command": "./check.sh", "grep": "FAIL|Error|OK"}
   21.0s 7e4c6db7b37add13.32  orchestrator-wordcount: responded, ToolUse, 3 calls (4 in, 8962 cached, 780 out)
   21.0s 7e4c6db7b37add13.33  orchestrator-wordcount's write dispatched
   21.0s 7e4c6db7b37add13.34  orchestrator-wordcount's write dispatched
   21.0s 7e4c6db7b37add13.35  orchestrator-wordcount's bash dispatched
   21.0s 801368dd9134bbe5.3  orchestrator-wordcount's write completed
   21.0s 801368dd9134bbe5.3  file textstats/wordcount.py changed by orchestrator-wordcount's write
   21.0s 801368dd9134bbe5.4  orchestrator-wordcount's write completed
   21.0s 801368dd9134bbe5.4  file tests/test_wordcount.py changed by orchestrator-wordcount's write

** kill -9 the brain

== launch 2: the same database, nothing else
  act: tab e5a3817160f224d8 linked
  act: Max presses Done
included
    0.0s 7e4c6db7b37add13.38  brain launched, build harness-brain 0.0.0
    0.0s 7e4c6db7b37add13.39  roles connected
face: https://mx-hv-mj.local:4397
    0.0s 7e4c6db7b37add13.40  orchestrator-palindromes: request lost
    0.2s 7e4c6db7b37add13.41  target/brain-demo/repo-palindromes connected
    0.2s 7e4c6db7b37add13.42  target/brain-demo/repo connected
    0.2s 7e4c6db7b37add13.43  user connected
    0.2s 5cc26dd13e92cfdb.3  target/brain-demo/repo-palindromes described: 5 tools
    0.2s 801368dd9134bbe5.5  target/brain-demo/repo described: 5 tools
    0.2s 104e1a0e7a27d4f1.2  user described: 1 tools
    0.2s 7e4c6db7b37add13.47  orchestrator-palindromes: request sent
    2.2s 7e4c6db7b37add13.48  orchestrator-palindromes: first byte
    9.3s 7e4c6db7b37add13.49  orchestrator-palindromes's bash made {"command": "cat > textstats/palindromes.py <<'EOF'\nimport re\n\n_WORD = re.compil
    9.3s 7e4c6db7b37add13.49  orchestrator-palindromes: responded, ToolUse, 1 calls (2 in, 9564 cached, 920 out)
    9.3s 7e4c6db7b37add13.50  orchestrator-palindromes's bash dispatched
   11.0s 801368dd9134bbe5.6  orchestrator-wordcount's bash completed
   11.0s 7e4c6db7b37add13.52  orchestrator-wordcount: request sent
   15.4s 7e4c6db7b37add13.53  orchestrator-wordcount: first byte
   15.4s 7e4c6db7b37add13.54  orchestrator-wordcount: responded, EndTurn, 0 calls (2 in, 10275 cached, 309 out)
   24.3s 5cc26dd13e92cfdb.4  orchestrator-palindromes's bash completed
   24.3s 7e4c6db7b37add13.56  orchestrator-palindromes: request sent
   26.3s 7e4c6db7b37add13.57  orchestrator-palindromes: first byte
   27.2s 7e4c6db7b37add13.58  orchestrator-palindromes's bash made {"command": "git add textstats/palindromes.py tests/test_palindromes.py && git comm
   27.2s 7e4c6db7b37add13.58  orchestrator-palindromes: responded, ToolUse, 1 calls (2 in, 9564 cached, 134 out)
   27.2s 7e4c6db7b37add13.59  orchestrator-palindromes's bash dispatched
   27.2s 5cc26dd13e92cfdb.5  orchestrator-palindromes's bash completed
   27.2s 7e4c6db7b37add13.61  orchestrator-palindromes: request sent
   32.8s 7e4c6db7b37add13.62  orchestrator-palindromes: first byte
   32.8s 7e4c6db7b37add13.63  orchestrator-palindromes: responded, EndTurn, 0 calls (2 in, 10538 cached, 444 out)
   32.8s 7e4c6db7b37add13.64  orchestrator's spawn completed
   33.1s 7e4c6db7b37add13.65  orchestrator: request sent
   34.8s 7e4c6db7b37add13.66  orchestrator: first byte
   35.5s 7e4c6db7b37add13.67  orchestrator's bash made {"command": "git branch -a; git fetch origin 2>&1 | tail -2; git branch -a; git status --short"
   35.5s 7e4c6db7b37add13.67  orchestrator: responded, ToolUse, 1 calls (2 in, 9723 cached, 100 out)
   35.5s 7e4c6db7b37add13.68  orchestrator's bash dispatched
   35.5s 801368dd9134bbe5.7  orchestrator's bash completed
   35.5s 7e4c6db7b37add13.70  orchestrator: request sent
   37.3s 7e4c6db7b37add13.71  orchestrator: first byte
   38.3s 7e4c6db7b37add13.72  orchestrator's bash made {"command": "git log --oneline palindromes | head -3; git merge palindromes 2>&1 | tail -3; cat
   38.3s 7e4c6db7b37add13.72  orchestrator: responded, ToolUse, 1 calls (2 in, 10500 cached, 149 out)
   38.3s 7e4c6db7b37add13.73  orchestrator's bash dispatched
   53.4s 801368dd9134bbe5.8  orchestrator's bash completed
   53.4s 7e4c6db7b37add13.75  orchestrator: request sent
   55.4s 7e4c6db7b37add13.76  orchestrator: first byte
   57.0s 7e4c6db7b37add13.77  orchestrator's bash made {"command": "git add textstats/wordcount.py tests/test_wordcount.py && git diff --cached --stat
   57.0s 7e4c6db7b37add13.77  orchestrator: responded, ToolUse, 1 calls (2 in, 10679 cached, 207 out)
   57.0s 7e4c6db7b37add13.78  orchestrator's bash dispatched
   57.1s 801368dd9134bbe5.9  orchestrator's bash completed
   57.1s 7e4c6db7b37add13.80  orchestrator: request sent
   63.7s 7e4c6db7b37add13.81  orchestrator: first byte
   63.7s 7e4c6db7b37add13.82  orchestrator: responded, EndTurn, 0 calls (2 in, 11360 cached, 547 out)
   64.1s 7e4c6db7b37add13.83  tab e5a3817160f224d8 is Max's
   64.1s e5a3817160f224d8.1  Done prompt delivered to orchestrator
   64.1s e5a3817160f224d8.1  as seen at 7e4c6db7b37add13.82
   64.1s 7e4c6db7b37add13.85  orchestrator: request sent
   65.9s 7e4c6db7b37add13.86  orchestrator: first byte
   70.8s 7e4c6db7b37add13.87  orchestrator: responded, EndTurn, 0 calls (4 in, 11694 cached, 545 out)

== the task is done; its result:

textstats now has both modules from `README.md`, committed on `main`. After the merge `./check.sh` passes: all 12 tests pass and the style check is ok

**What was done:**
- **The fork:** wrote `textstats/wordcount.py` (`count_words` and `top`) and `tests/test_wordcount.py`, with 5 tests. I committed these as `fe1120c`.
- **The fresh engineer:** on the `palindromes` branch in its own clone, wrote `textstats/palindromes.py` (`is_palindrome` and `find_palindromes`) and 
- **Me:** read the repo first and gave the engineer the spec and the word definition the fork would use. Then I ran `git merge palindromes`, checked t

**What I learned:** this repo has no remote of its own. The engineer said it "pushed to origin", and that push arrived in this repo as the local `pali

**Left undone:** nothing you asked for. One small duplication: both modules define the same word pattern, `(?:[^\W_]|')+`. I left it as it is. If a th

harness-brain: SIGTERM: shutting down; requests in flight have 300s to finish (again to stop now)
   71.6s 7e4c6db7b37add13.88  brain shutting down, within 300s
limb 7e4c6db7b37add13.1.4: its link ended
   71.6s 7e4c6db7b37add13.89  roles lost
limb 7e4c6db7b37add13.19.2: its link ended
   71.6s 7e4c6db7b37add13.90  target/brain-demo/repo-palindromes lost
limb 7e4c6db7b37add13.1.3: its link ended
   71.6s 7e4c6db7b37add13.91  user lost
limb 7e4c6db7b37add13.4.0: its link ended
   71.6s 7e4c6db7b37add13.92  target/brain-demo/repo lost
harness-brain: shut down
== the repository afterwards
fe1120c Add wordcount module and tests

 tests/test_wordcount.py | 26 ++++++++++++++++++++++++++
 textstats/wordcount.py  | 15 +++++++++++++++
 2 files changed, 41 insertions(+)
232eb0e Add palindromes module and tests

 tests/test_palindromes.py | 35 +++++++++++++++++++++++++++++++++++
 textstats/palindromes.py  | 20 ++++++++++++++++++++
 2 files changed, 55 insertions(+)
0938392 Start textstats

 .gitignore            |  1 +
 AGENTS.md             |  4 ++++
 README.md             | 10 ++++++++++
 check.sh              |  7 +++++++
 tests/__init__.py     |  0
 textstats/__init__.py |  0
 6 files changed, 22 insertions(+)

To keep serving the finished run: target/release/harness-brain --roles roles --db target/brain-demo/brain.sqlite --port 4397
```
