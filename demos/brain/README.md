# brain: the harness on a root task, killed -9 mid-run, carrying on

```sh
demos/brain/run.sh
```

Then watch https://mx-hv-mj.local:4321 while it runs (another port: `PORT=4400 demos/brain/run.sh`). It takes about two minutes on `claude-sonnet-5` and costs a few dozen requests on the subscription.

What it does:

1. `scratch.sh` makes a git repository, `target/brain-demo/repo`: a Python package with a README that says what it should be, an `AGENTS.md`, and a `check.sh` that runs the tests and then a 15-second "style check".
2. Launch 1 starts the brain on a new journal with `mandate.md`. The root orchestrator reads the repository and spawns one scope of two agents: a fork of itself, and a fresh engineer. Each writes one module and its tests and runs `./check.sh`.
3. While an agent's `./check.sh` is running, the script kills the brain with `kill -9`.
4. Launch 2 is started on the same journal, with no other arguments. It folds the journal and carries on:
   - the request that was in flight is said to be lost and sent again;
   - the workspace is asked how the running `bash` call ended. That instance died with the brain, so the outcome is unknown. `bash` does not allow reruns, so the call settles as unknown and the agent is told so. The agent decides what to do; here it ran `./check.sh` again.
5. The orchestrator gets both results when the scope closes, runs `./check.sh`, commits, and reports. The script prints its result and the repository's log.

To serve a finished run again: `target/release/harness-brain --journal target/brain-demo/journal.sqlite --port 4321`. A journal whose root is done does nothing but serve.

## Real output

Trimmed only where marked `[...]`, and each line cut at 150 characters.

```
== launch 1: the orchestrator starts on demos/brain/mandate.md
face: https://mx-hv-mj.local:4321
    0.0s     1  brain launched, build harness-brain 0.0.0
    0.0s     2  limb meta defined
    0.0s     3  limb workspace defined
    0.0s     4  environment dev defined
    0.0s     5  workspace connected
    0.0s     6  orchestrator created (orchestrator)
    0.0s     7  orchestrator: request sent
    0.0s     8   changed
    0.0s     9  AGENTS.md changed
    1.1s    10  orchestrator: first byte
    1.9s    11  orchestrator's bash made {"command":"cat README.md; echo ---; find . -not -path './.git*' | sort; echo ---; cat check.sh 2>/dev/null",
    1.9s    12  orchestrator: responded, ToolUse, 1 calls (1 in, 0 cached, 123 out)
    1.9s    13  orchestrator's bash dispatched
    1.9s    14  orchestrator's bash completed
    1.9s    15  orchestrator: request sent
    2.9s    16  orchestrator: first byte
    3.7s    17  orchestrator's bash made {"command":"cat textstats/__init__.py; echo ---; cat tests/__init__.py","grep":""}
    3.7s    18  orchestrator: responded, ToolUse, 1 calls (2 in, 2587 cached, 109 out)
    3.7s    19  orchestrator's bash dispatched
    3.7s    20  orchestrator's bash completed
    3.7s    21  orchestrator: request sent
    4.8s    22  orchestrator: first byte
   19.1s    23  orchestrator's spawn made
   19.1s    24  orchestrator: responded, ToolUse, 1 calls (2 in, 3150 cached, 1381 out)
   19.1s    25  orchestrator's spawn dispatched
   19.1s    26  orchestrator-wordcount created (orchestrator, forked from orchestrator)
   19.1s    27  orchestrator-palindromes created (engineer)
   19.1s    28  orchestrator-wordcount: request sent
   19.1s    29  orchestrator-palindromes: request sent
[...]
   32.2s    58  orchestrator-wordcount: first byte
   32.9s    59  orchestrator-wordcount's bash made {"command":"cd /home/user 2>/dev/null; cd \"$(pwd)\" ; true","grep":""}
   32.9s    60  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 5756 cached, 94 out)
   32.9s    61  orchestrator-wordcount's bash dispatched
   32.9s    62  orchestrator-wordcount's bash completed
   32.9s    63  orchestrator-wordcount: request sent
   34.0s    64  orchestrator-wordcount: first byte
   35.0s    65  orchestrator-wordcount's bash made {"command":"pwd; ls; ./check.sh","grep":"FAIL|Error|OK|ok"}
   35.0s    66  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 6742 cached, 116 out)
   35.0s    67  orchestrator-wordcount's bash dispatched
   37.5s    68  orchestrator-palindromes's write made {"content":"import unittest\n\nfrom textstats.palindromes import is_palindrome, find_palindromes
   37.5s    69  orchestrator-palindromes: responded, ToolUse, 1 calls (2 in, 3799 cached, 1163 out)
   37.5s    70  orchestrator-palindromes's write dispatched
   37.5s    71  tests/test_palindromes.py changed
   37.5s    72  orchestrator-palindromes's write completed
   37.5s    73  orchestrator-palindromes: request sent
   38.4s    74  orchestrator-palindromes: first byte

** kill -9 the brain

== launch 2: the same journal, nothing else
face: https://mx-hv-mj.local:4321
    0.0s    75  brain launched, build harness-brain 0.0.0
    0.0s    76  workspace connected
    0.0s    77  orchestrator-palindromes: request lost
    0.0s    78  orchestrator-palindromes: request sent
    0.0s    79  orchestrator-wordcount's bash ended unknown
    0.0s    80  orchestrator-wordcount: request sent
    0.9s    81  orchestrator-palindromes: first byte
    1.1s    82  orchestrator-wordcount: first byte
    1.5s    83  orchestrator-wordcount's bash made {"command":"pwd; ls","grep":""}
[...]
    3.2s    93  orchestrator-wordcount: first byte
    3.5s    94  orchestrator-palindromes: first byte
    3.6s    95  orchestrator-wordcount's bash made {"command":"cd /home/maxeonyx/agent-harness-workspace/ah-brain/target/brain-demo/repo && ./check.sh
    3.6s    96  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 7031 cached, 124 out)
    3.6s    97  orchestrator-wordcount's bash dispatched
    4.6s    98  orchestrator-palindromes's bash made {"command":"cd textstats_repo 2>/dev/null || true; ls; ./check.sh 2>&1 | tail -40","grep":"FAIL|E
    4.6s    99  orchestrator-palindromes: responded, ToolUse, 1 calls (2 in, 5725 cached, 148 out)
    4.6s   100  orchestrator-palindromes's bash dispatched
   18.6s   101  orchestrator-wordcount's bash completed
   18.6s   102  orchestrator-wordcount: request sent
   19.6s   103  orchestrator-palindromes's bash completed
   19.6s   104  orchestrator-palindromes: request sent
   19.7s   105  orchestrator-wordcount: first byte
   20.6s   106  orchestrator-palindromes: first byte
   21.3s   107  orchestrator-wordcount: responded, EndTurn, 0 calls (2 in, 7176 cached, 162 out)
   23.6s   108  orchestrator-palindromes: responded, EndTurn, 0 calls (2 in, 6734 cached, 359 out)
   23.6s   109  orchestrator's spawn completed
   23.8s   110  orchestrator: request sent
   24.9s   111  orchestrator: first byte
   26.1s   112  orchestrator's bash made {"command":"cd /home/user 2>/dev/null; cd \"$(pwd)\" ; ls; cat textstats/wordcount.py; echo ---; cat textstat
   26.1s   113  orchestrator: responded, ToolUse, 1 calls (2 in, 4637 cached, 149 out)
   26.1s   114  orchestrator's bash dispatched
   26.1s   115  orchestrator's bash completed
   26.1s   116  orchestrator: request sent
   27.1s   117  orchestrator: first byte
   27.6s   118  orchestrator's bash made {"command":"cd . && ./check.sh","grep":"OK|FAIL|Error|style"}
   27.6s   119  orchestrator: responded, ToolUse, 1 calls (2 in, 5182 cached, 88 out)
   27.6s   120  orchestrator's bash dispatched
   42.7s   121  orchestrator's bash completed
   42.7s   122  orchestrator: request sent
   43.7s   123  orchestrator: first byte
   44.8s   124  orchestrator's bash made {"command":"cd . && git add -A && git -c user.email=\"agent@example.com\" -c user.name=\"Agent\" commit -m \"
   44.8s   125  orchestrator: responded, ToolUse, 1 calls (2 in, 6021 cached, 148 out)
   44.8s   126  orchestrator's bash dispatched
   44.8s   127  orchestrator's bash completed
   44.8s   128  orchestrator: request sent
   45.8s   129  orchestrator: first byte
   49.4s   130  orchestrator: responded, EndTurn, 0 calls (2 in, 7547 cached, 436 out)

== the root is done; its result:

```
