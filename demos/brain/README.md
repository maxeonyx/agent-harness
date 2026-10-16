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
   - the workspace is asked how the running `bash` call ended. That instance died with the brain, so the outcome is unknown. `bash` does not allow reruns, so the call settles as unknown and the agent is told so. The agent decides what to do; here it ran `./check.sh` again.
5. The orchestrator gets both results when the scope closes, runs `./check.sh`, commits, and reports. The script prints its result and the repository's log.

To serve a finished run again: `target/release/harness-brain --journal target/brain-demo/journal.sqlite --port 4321`. A journal whose root is done does nothing but serve.

What the agents were told, from the store (`entry.content`, cut):

The fork's first message, in the framing the fork-framing spike measured: a neutral answer to the spawn, then its part as user text.
```
[{"content":"Started wordcount, palindromes.","tool_use_id":"toolu_016w…","type":"tool_result"},{"text":"You are now `orchestrator-wordcount`, a fork of `orchestrator`. Everything above is `orchestrator`'s context; from here you do one part of its work, and only that part.\n\nYour part:\nWrite textstats/wordcount.py …
```

The fork's `./check.sh` after the relaunch:
```
[{"content":[{"text":"The outcome of this call is unknown: the harness stopped while it was running, and this tool is not safe to run again blind. It may or may not have had its effects. Check before relying on either.","type":"text"}],"is_error":true,"tool_use_id":"toolu_013E…","type":"tool_result"}]
```

## Real output

Trimmed only where marked `[...]`, and each line cut at 150 characters.

```
== launch 1: the orchestrator starts on demos/brain/mandate.md
face: https://mx-hv-mj.local:4400
    0.0s     1  brain launched, build harness-brain 0.0.0
    0.0s     2  limb meta defined
    0.0s     3  limb workspace defined
    0.0s     4  environment dev defined
    0.0s     5  workspace connected
    0.0s     6  orchestrator created (orchestrator)
    0.0s     7  orchestrator: request sent
    1.0s     8  orchestrator: first byte
    1.8s     9  orchestrator's bash made {"command":"cat README.md; echo ---; find . -not -path './.git*' -type f | sort; echo ---; cat check.sh 2>/de
    1.8s    10  orchestrator: responded, ToolUse, 1 calls (1 in, 2624 cached, 126 out)
    1.8s    11  orchestrator's bash dispatched
    1.8s    12  orchestrator's bash completed
    1.8s    13  orchestrator: request sent
    2.6s    14  orchestrator: first byte
    3.5s    15  orchestrator's bash made {"command":"cat textstats/__init__.py; echo ---; cat tests/__init__.py","grep":""}
    3.5s    16  orchestrator: responded, ToolUse, 1 calls (2 in, 3179 cached, 121 out)
    3.5s    17  orchestrator's bash dispatched
    3.5s    18  orchestrator's bash completed
    3.5s    19  orchestrator: request sent
    4.4s    20  orchestrator: first byte
   18.6s    21  orchestrator's spawn made
   18.6s    22  orchestrator: responded, ToolUse, 1 calls (2 in, 3179 cached, 1600 out)
   18.6s    23  orchestrator's spawn dispatched
   18.6s    24  orchestrator-wordcount created (forked from orchestrator)
   18.6s    25  orchestrator-palindromes created (engineer)
   18.6s    26  orchestrator-wordcount: request sent
   18.6s    27  orchestrator-palindromes: request sent
   19.4s    28  orchestrator-palindromes: first byte
   19.5s    29  orchestrator-wordcount: first byte
   20.5s    30  orchestrator-palindromes's bash made {"command":"cd /home/maxeonyx/agent-harness-workspace/ah-migrate-integrate/target/brain-demo/repo
[...]
   24.4s    51  orchestrator-palindromes's write dispatched
   24.4s    52  orchestrator-palindromes's write completed
   24.4s    53  file textstats/palindromes.py changed by orchestrator-palindromes's write
   24.4s    54  orchestrator-palindromes: request sent
   24.8s    55  orchestrator-wordcount: first byte
   25.3s    56  orchestrator-palindromes: first byte
   31.5s    57  orchestrator-wordcount's write made {"content":"import unittest\n\nfrom textstats.wordcount import count_words, top\n\n\nclass CountWo
   31.5s    58  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 5928 cached, 921 out)
   31.5s    59  orchestrator-wordcount's write dispatched
   31.5s    60  orchestrator-wordcount's write completed
   31.5s    61  file tests/test_wordcount.py changed by orchestrator-wordcount's write
   31.5s    62  orchestrator-wordcount: request sent
   32.7s    63  orchestrator-wordcount: first byte
   33.0s    64  orchestrator-wordcount's bash made {"command":"cd . && ./check.sh","grep":"test_wordcount|OK|FAIL|Error"}
   33.0s    65  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 6108 cached, 93 out)
   33.0s    66  orchestrator-wordcount's bash dispatched

** kill -9 the brain

== launch 2: the same journal, nothing else
face: https://mx-hv-mj.local:4400
    0.0s    67  brain launched, build harness-brain 0.0.0
    0.0s    68  workspace connected
    0.0s    69  orchestrator-palindromes: request lost
    0.0s    70  orchestrator-wordcount's bash dispatched
    0.0s    71  orchestrator-palindromes: request sent
    0.0s    72  orchestrator-wordcount's bash ended unknown
    0.0s    73  orchestrator-wordcount: request sent
    0.9s    74  orchestrator-palindromes: first byte
    1.3s    75  orchestrator-wordcount: first byte
    2.0s    76  orchestrator-wordcount's bash made {"command":"cd . && python3 -m unittest tests.test_wordcount -v 2>&1 | tail -30","grep":"FAIL|ERROR
    2.0s    77  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 7055 cached, 112 out)
    2.0s    78  orchestrator-wordcount's bash dispatched
    2.1s    79  orchestrator-wordcount's bash completed
    2.1s    80  orchestrator-wordcount: request sent
    2.9s    81  orchestrator-wordcount: first byte
    4.1s    82  orchestrator-wordcount's bash made {"command":"cd . && ./check.sh 2>&1 | tail -40","grep":"FAIL|ERROR|OK|style"}
    4.1s    83  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 7215 cached, 159 out)
[...]
   64.1s   126  orchestrator: responded, ToolUse, 1 calls (2 in, 5598 cached, 174 out)
   64.1s   127  orchestrator's bash dispatched
   64.1s   128  orchestrator's bash completed
   64.1s   129  orchestrator: request sent
   65.3s   130  orchestrator: first byte
   66.4s   131  orchestrator's bash made {"command":"cd /home/maxeonyx/agent-harness-workspace/ah-migrate-integrate/target/brain-demo/repo && git add 
   66.4s   132  orchestrator: responded, ToolUse, 1 calls (2 in, 7231 cached, 168 out)
   66.4s   133  orchestrator's bash dispatched
   66.4s   134  orchestrator's bash completed
   66.4s   135  orchestrator: request sent
   68.3s   136  orchestrator: first byte
   72.2s   137  orchestrator: responded, EndTurn, 0 calls (2 in, 8060 cached, 503 out)

== the root is done; its result:

## Report

Two agents worked in parallel on separate files, no conflicts:

**wordcount agent** (fork) wrote `textstats/wordcount.py`:
- `count_words(text)`: tokenizes with regex `[A-Za-z0-9']+`, lower-cases, counts.
- `top(text, n)`: sorts by `(-count, word)` for alphabetical tie-breaking, slices to `n`.
- `tests/test_wordcount.py`: 14 tests (basics, case-folding, punctuation, apostrophes, digits, empty text, whitespace, ties, `n=0`, `n` > vocab size).

**palindromes agent** (fresh engineer) wrote `textstats/palindromes.py`:
- `is_palindrome(s)`: strips non-alphanumerics, lower-cases, compares to reverse; empty (after cleaning) → `False`.
- `find_palindromes(text, min_len=3)`: tokenizes the same way as wordcount, lower-cases, filters by `min_len`, dedups preserving first-appearance orde
- `tests/test_palindromes.py`: 17 tests (simple/mixed-case, punctuation/spacing ignored, empty string, single chars, numeric palindromes, varying `min

Both ran `./check.sh` independently and it passed each time. I then ran `./check.sh` myself with both modules present — all **31 tests pass**, style c

== the repository afterwards
844b285 Implement wordcount and palindromes modules with tests

 tests/test_palindromes.py | 94 +++++++++++++++++++++++++++++++++++++++++++++++
 tests/test_wordcount.py   | 62 +++++++++++++++++++++++++++++++
 textstats/palindromes.py  | 35 ++++++++++++++++++
 textstats/wordcount.py    | 27 ++++++++++++++
 4 files changed, 218 insertions(+)
62ed3b9 Start textstats

 .gitignore            |  1 +
 AGENTS.md             |  4 ++++
 README.md             | 10 ++++++++++
 check.sh              |  7 +++++++
 tests/__init__.py     |  0
 textstats/__init__.py |  0
 6 files changed, 22 insertions(+)

To keep serving the finished run: target/release/harness-brain --journal target/brain-demo/journal.sqlite --port 4400
```
