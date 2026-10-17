# brain: the harness on a root task, killed -9 mid-run, carrying on

```sh
demos/brain/run.sh
```

Then watch https://mx-hv-mj.local:4321 while it runs, and type to a task from its compose box (another port: `PORT=4400 demos/brain/run.sh`). It takes about two minutes on Opus (the roles in `roles/`) and costs a few dozen requests on the subscription.

What it does:

1. `scratch.sh` makes a git repository, `target/brain-demo/repo`: a Python package with a README that says what it should be, an `AGENTS.md`, and a `check.sh` that runs the tests and then a 15-second "style check".
2. Launch 1 starts the brain on a new journal with `mandate.md`. The root orchestrator reads the repository and spawns one scope of two agents: a fork of itself, in the same directory, and a fresh engineer, in its own clone on the branch `palindromes`. Each writes one module and its tests and runs `./check.sh`; the engineer commits and pushes its branch, and the orchestrator merges it.
3. While an agent's `./check.sh` is running, the script kills the brain with `kill -9`.
4. Launch 2 is started on the same journal, with no other arguments. It folds the journal and carries on:
   - the request that was in flight is said to be lost and sent again;
   - the workspace is asked how the running `bash` call ended. When the brain died, its line to the workspace closed, and the workspace took that as a shutdown: it stopped the call's processes and recorded the call as interrupted, with the output it had so far. The agent is told that, and decides what to do; here it ran `./check.sh` again. A call is unknown only when the workspace died too, before it could record how the call ended.
5. The orchestrator gets both results when the scope closes, runs `./check.sh`, commits, reports, and waits on you: like every task you start, it is a chat with you. `done.sh` presses **Done** for you, the way the page does, and the orchestrator's last turn writes its result. The script prints that result and the repository's log.

To serve a finished run again: `target/release/harness-brain --journal target/brain-demo/journal.sqlite --port 4321`. A journal whose root is done does nothing but serve.

What the agents were told, from the store (`entry.content`, cut):

The fork's first message, in the framing the fork-framing spike measured: a neutral answer to the spawn, then its part as user text.
```
[{"content":"Started wordcount, palindromes.","tool_use_id":"toolu_016w…","type":"tool_result"},{"text":"You are now `orchestrator-wordcount`, a fork of `orchestrator`. Everything above is `orchestrator`'s context; from here you do one part of its work, and only that part.\n\nYour part:\nWrite textstats/wordcount.py …
```

The fork's `./check.sh` after the relaunch:
```
{"content":[{"text":"test_empty (tests.test_wordcount.CountWordsTest.test_empty) ... ok\n[…]\nRan 7 tests in 0.000s\n\nOK\nstyle: checking...\n[interrupted: the harness shut down]","type":"text"},{"text":"[interrupted; it may have had partial effects]","type":"text"}],"is_error":true,"tool_use_id":"toolu_01DX…","type":"tool_result"}
```

## Real output

A run at 16:05 NZT on 2026-10-17. Trimmed only where marked `[...]`, and each line cut at 150 characters.

```
== launch 1: the orchestrator starts on demos/brain/mandate.md
    0.0s     1  brain launched, build harness-brain 0.0.0
[...]
    0.0s    15  /home/maxeonyx/agent-harness-workspace/ah-cancel/target/brain-demo/repo connected
    0.0s    16  /home/maxeonyx/agent-harness-workspace/ah-cancel/target/brain-demo/repo described: 5 tools
    0.0s    17  orchestrator has a new root, on claude-opus-5-5
    0.3s    18  orchestrator: request sent
    1.8s    19  orchestrator: first byte
[...]
    2.9s    24  orchestrator: request sent
    4.5s    25  orchestrator: first byte
   10.3s    26  orchestrator's spawn made
   10.3s    27  orchestrator: responded, ToolUse, 1 calls (2 in, 8025 cached, 733 out)
   10.3s    28  orchestrator's spawn dispatched
   10.3s    29  orchestrator-wordcount created (forked from orchestrator)
   10.3s    30  limb /home/maxeonyx/agent-harness-workspace/ah-cancel/target/brain-demo/repo-palindromes defined
   10.3s    31  orchestrator-palindromes created (engineer)
   10.6s    32  orchestrator-wordcount: request sent
   10.6s    33  /home/maxeonyx/agent-harness-workspace/ah-cancel/target/brain-demo/repo-palindromes connected
   10.6s    34  /home/maxeonyx/agent-harness-workspace/ah-cancel/target/brain-demo/repo-palindromes described: 5 tools
[...]
   13.9s    42  orchestrator-palindromes's bash completed
   13.9s    43  orchestrator-palindromes: request sent
   18.9s    44  orchestrator-wordcount's write made {"path": "textstats/wordcount.py", "content": "\"\"\"Word counts.\"\"\"\n\nimport re\nfrom collect
   18.9s    45  orchestrator-wordcount's write made {"path": "tests/test_wordcount.py", "content": "import unittest\n\nfrom textstats.wordcount import
   18.9s    46  orchestrator-wordcount's bash made {"command": "./check.sh", "grep": "FAIL|Error|OK|style"}
   18.9s    47  orchestrator-wordcount: responded, ToolUse, 3 calls (4 in, 8965 cached, 938 out)
   18.9s    48  orchestrator-wordcount's write dispatched
   18.9s    49  orchestrator-wordcount's write dispatched
   18.9s    50  orchestrator-wordcount's bash dispatched
   18.9s    51  orchestrator-wordcount's write completed
   18.9s    52  file tests/test_wordcount.py changed by orchestrator-wordcount's write
   18.9s    53  orchestrator-wordcount's write completed
   18.9s    54  file textstats/wordcount.py changed by orchestrator-wordcount's write
   20.4s    55  orchestrator-palindromes: first byte

** kill -9 the brain

== launch 2: the same journal, nothing else
    0.0s    56  brain launched, build harness-brain 0.0.0
face: https://mx-hv-mj.local:4400
    0.0s    57  orchestrator-palindromes: request lost
    0.0s    58  user connected
    0.0s    59  /home/maxeonyx/agent-harness-workspace/ah-cancel/target/brain-demo/repo-palindromes connected
    0.0s    60  /home/maxeonyx/agent-harness-workspace/ah-cancel/target/brain-demo/repo-palindromes described: 5 tools
    0.0s    61  user described: 1 tools
    0.0s    62  orchestrator-palindromes: request sent
    0.0s    63  /home/maxeonyx/agent-harness-workspace/ah-cancel/target/brain-demo/repo connected
    0.0s    64  /home/maxeonyx/agent-harness-workspace/ah-cancel/target/brain-demo/repo described: 5 tools
    0.0s    65  orchestrator-wordcount's bash dispatched
    0.0s    66  orchestrator-wordcount's bash interrupted
    0.0s    67  orchestrator-wordcount: request sent
    1.5s    68  orchestrator-wordcount: first byte
    2.3s    69  orchestrator-palindromes: first byte
    2.4s    70  orchestrator-wordcount's bash made {"command": "./check.sh 2>&1 | tail -4", "grep": "FAIL|Error|OK|style"}
    2.4s    71  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 10246 cached, 127 out)
    2.4s    72  orchestrator-wordcount's bash dispatched
   10.7s    73  orchestrator-palindromes's bash made {"command": "cat > textstats/palindromes.py <<'EOF'\n\"\"\"Palindrome detection.\"\"\"\n\nimport 
[...]
   58.1s   101  orchestrator's bash dispatched
   73.2s   102  orchestrator's bash completed
   73.2s   103  orchestrator: request sent
   74.7s   104  orchestrator: first byte
   76.1s   105  orchestrator's bash made {"command": "git add textstats/wordcount.py tests/test_wordcount.py textstats/palindromes.py && git diff --ca
   76.1s   106  orchestrator: responded, ToolUse, 1 calls (2 in, 11367 cached, 182 out)
   76.1s   107  orchestrator's bash dispatched
   76.1s   108  orchestrator's bash completed
   76.1s   109  orchestrator: request sent
   77.6s   110  orchestrator: first byte
   81.7s   111  orchestrator: responded, EndTurn, 0 calls (2 in, 11729 cached, 459 out)
   82.1s   112  Done prompt delivered to orchestrator
   82.1s   113  orchestrator: request sent
   83.4s   114  orchestrator: first byte
   88.8s   115  orchestrator: responded, EndTurn, 0 calls (4 in, 12081 cached, 619 out)

== the task is done; its result:

The package is built and committed on `main`, and `./check.sh` passes: all 14 tests and the style check.

**What was done**
- **The fork** wrote `textstats/wordcount.py` and `tests/test_wordcount.py`, with 7 tests. A word is a run of letters, digits or apostrophes, accented
- **The engineer**, on branch `palindromes`, wrote `textstats/palindromes.py` and `tests/test_palindromes.py`, with 7 tests, in commit `4491413`. I me
- **My fix:** `find_palindromes` only took plain ASCII letters (`[a-z0-9']+`), so an accented word like "été" was split apart instead of being found. 

**What I learned**
- This repository has no `origin` remote. The engineer said it pushed its branch, but it can't have; the merge used the local `palindromes` branch.
- My mandate to the engineer offered two word patterns, ASCII-only or `\w`-based. That left room for the mismatch I then had to fix, so I should have 

**Left undone**
- Nothing is pushed, because there is no remote to push to.
- No test covers accented words in `find_palindromes`.
- `palindromes.py` imports `_WORD`, which is private to `wordcount.py` (its leading underscore marks it as internal). If that matters, rename it to `W

harness-brain: SIGTERM: shutting down; requests in flight have 300s to finish (again to stop now)
   89.7s   116  brain shutting down, within 300s
limb b67edba1110bddcf74a4c88127c4489c: its process ended
   89.7s   117  /home/maxeonyx/agent-harness-workspace/ah-cancel/target/brain-demo/repo-palindromes lost
limb b8020dcc19c2c2b2b62558b3d34b7571: its process ended
   89.7s   118  /home/maxeonyx/agent-harness-workspace/ah-cancel/target/brain-demo/repo lost
limb 0361bc18ee3fbc1c2a5722ca9c38611d: its process ended
   89.7s   119  user lost
harness-brain: shut down
== the repository afterwards
514aeea Add wordcount; share its word pattern with palindromes

 tests/test_wordcount.py  | 32 ++++++++++++++++++++++++++++++++
 textstats/palindromes.py |  4 ++--
 textstats/wordcount.py   | 16 ++++++++++++++++
 3 files changed, 50 insertions(+), 2 deletions(-)
4491413 Add palindromes module and tests

 tests/test_palindromes.py | 39 +++++++++++++++++++++++++++++++++++++++
 textstats/palindromes.py  | 18 ++++++++++++++++++
 2 files changed, 57 insertions(+)
8b5b8e2 Start textstats

 .gitignore            |  1 +
 AGENTS.md             |  4 ++++
 README.md             | 10 ++++++++++
 check.sh              |  7 +++++++
 tests/__init__.py     |  0
 textstats/__init__.py |  0
 6 files changed, 22 insertions(+)

To keep serving the finished run: target/release/harness-brain --roles roles --journal target/brain-demo/journal.sqlite --port 4400
```
