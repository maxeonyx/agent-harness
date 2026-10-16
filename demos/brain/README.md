# brain: the harness on a root task, killed -9 mid-run, carrying on

```sh
demos/brain/run.sh
```

Then watch https://mx-hv-mj.local:4321 while it runs, and type to a task from its compose box (another port: `PORT=4400 demos/brain/run.sh`). It takes about two minutes on `claude-sonnet-5` and costs a few dozen requests on the subscription.

What it does:

1. `scratch.sh` makes a git repository, `target/brain-demo/repo`: a Python package with a README that says what it should be, an `AGENTS.md`, and a `check.sh` that runs the tests and then a 15-second "style check".
2. Launch 1 starts the brain on a new journal with `mandate.md`. The root orchestrator reads the repository and spawns one scope of two agents: a fork of itself, and a fresh engineer. Each writes one module and its tests and runs `./check.sh`.
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
    1.2s    10  orchestrator: first byte
    2.2s    11  orchestrator's bash made {"command":"cat README.md; echo ---; ls -la; echo ---; find . -type f -not -path './.git/*' | sort; echo ---;
    2.2s    12  orchestrator: responded, ToolUse, 1 calls (1 in, 2214 cached, 137 out)
    2.2s    13  orchestrator's bash dispatched
    2.2s    14  orchestrator's bash completed
    2.2s    15  orchestrator: request sent
    3.3s    16  orchestrator: first byte
    4.4s    17  orchestrator's bash made {"command":"echo ---init textstats; cat textstats/__init__.py; echo ---init tests; cat tests/__init__.py; ech
    4.4s    18  orchestrator: responded, ToolUse, 1 calls (2 in, 2587 cached, 143 out)
    4.4s    19  orchestrator's bash dispatched
    4.4s    20  orchestrator's bash completed
    4.4s    21  orchestrator: request sent
    5.4s    22  orchestrator: first byte
   16.5s    23  orchestrator's spawn made
   16.5s    24  orchestrator: responded, ToolUse, 1 calls (2 in, 3543 cached, 1363 out)
   16.5s    25  orchestrator's spawn dispatched
   16.5s    26  orchestrator-wordcount created (orchestrator, forked from orchestrator)
   16.5s    27  orchestrator-palindromes created (engineer)
   16.5s    28  orchestrator-wordcount: request sent
   16.5s    29  orchestrator-palindromes: request sent
[...]
   28.3s    43  orchestrator-wordcount's write completed
   28.3s    44  orchestrator-wordcount: request sent
   29.7s    45  orchestrator-wordcount: first byte
   31.4s    46  orchestrator-wordcount's bash made {"command":"cd /home/user/textstats 2>/dev/null || cd \"$(dirname \"$0\")\"; pwd; ls; ./check.sh","
   31.4s    47  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 6119 cached, 125 out)
   31.4s    48  orchestrator-wordcount's bash dispatched
   33.9s    49  orchestrator-palindromes's write made {"content":"\"\"\"Palindrome detection utilities.\"\"\"\nimport re\n\n_WORD_RE = re.compile(r\"[
   33.9s    50  orchestrator-palindromes: responded, ToolUse, 1 calls (2 in, 0 cached, 1990 out)
   33.9s    51  orchestrator-palindromes's write dispatched
   33.9s    52  textstats/palindromes.py changed
   33.9s    53  orchestrator-palindromes's write completed
   33.9s    54  orchestrator-palindromes: request sent
   34.8s    55  orchestrator-palindromes: first byte

** kill -9 the brain

== launch 2: the same journal, nothing else
face: https://mx-hv-mj.local:4321
    0.0s    56  brain launched, build harness-brain 0.0.0
    0.0s    57  workspace connected
    0.0s    58  orchestrator-palindromes: request lost
    0.0s    59  orchestrator-palindromes: request sent
    0.0s    60  orchestrator-wordcount's bash ended unknown
    0.0s    61  orchestrator-wordcount: request sent
    0.9s    62  orchestrator-palindromes: first byte
    1.1s    63  orchestrator-wordcount: first byte
    1.4s    64  orchestrator-wordcount's bash made {"command":"pwd; ls","grep":""}
    1.4s    65  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 7155 cached, 72 out)
    1.4s    66  orchestrator-wordcount's bash dispatched
    1.5s    67  orchestrator-wordcount's bash completed
    1.5s    68  orchestrator-wordcount: request sent
    2.4s    69  orchestrator-wordcount: first byte
    4.1s    70  orchestrator-wordcount's bash made {"command":"cd /home/maxeonyx/agent-harness-workspace/ah-brain/target/brain-demo/repo && ./check.sh
    4.1s    71  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 7347 cached, 205 out)
[...]
   42.9s   125  orchestrator's bash made {"command":"cd /home/maxeonyx/agent-harness-workspace/ah-brain/target/brain-demo/repo && ./check.sh","grep":"
   42.9s   126  orchestrator: responded, ToolUse, 1 calls (2 in, 5739 cached, 148 out)
   42.9s   127  orchestrator's bash dispatched
   58.0s   128  orchestrator's bash completed
   58.0s   129  orchestrator: request sent
   59.0s   130  orchestrator: first byte
   60.2s   131  orchestrator's bash made {"command":"cd /home/maxeonyx/agent-harness-workspace/ah-brain/target/brain-demo/repo && git add -A && git st
   60.2s   132  orchestrator: responded, ToolUse, 1 calls (2 in, 6708 cached, 171 out)
   60.2s   133  orchestrator's bash dispatched
   60.3s   134  orchestrator's bash completed
   60.3s   135  orchestrator: request sent
   61.3s   136  orchestrator: first byte
   65.4s   137  orchestrator: responded, EndTurn, 0 calls (2 in, 8092 cached, 532 out)

== the root is done; its result:

Committed cleanly (pycache files were excluded via `.gitignore`).

## Report

**wordcount agent (fork)** built `textstats/wordcount.py`:
- `count_words(text)` — regex `[A-Za-z0-9']+`, lower-cased, via `Counter`.
- `top(text, n)` — sorted by count descending, ties broken alphabetically, sliced to `n`.
- `tests/test_wordcount.py` — 14 cases: basics, case-insensitivity, apostrophes, digits, punctuation splitting, empty/no-words text, tie-breaking, `n`

**palindromes agent (fresh engineer)** built `textstats/palindromes.py`:
- `is_palindrome(s)` — strips to letters/digits, lower-cases, empty result is not a palindrome.
- `find_palindromes(text, min_len=3)` — extracts words, cleans/lower-cases, dedupes, filters by `min_len`, preserves first-appearance order.
- `tests/test_palindromes.py` — 14 cases: simple/mixed-case words, non-alphanumeric stripping, empty/single-char strings, example text, `min_len` filt

**My verification:** ran `./check.sh` from the repo root — all 28 tests pass, style check ok. No fixes needed. Committed all new files (`textstats/wor

== the repository afterwards
31ebe25 Implement textstats.wordcount and textstats.palindromes with tests

 tests/test_palindromes.py | 67 +++++++++++++++++++++++++++++++++++++
 tests/test_wordcount.py   | 84 +++++++++++++++++++++++++++++++++++++++++++++++
 textstats/palindromes.py  | 45 +++++++++++++++++++++++++
 textstats/wordcount.py    | 22 +++++++++++++
 4 files changed, 218 insertions(+)
b9fc98f Start textstats

 .gitignore            |  1 +
 AGENTS.md             |  4 ++++
 README.md             | 10 ++++++++++
 check.sh              |  7 +++++++
 tests/__init__.py     |  0
 textstats/__init__.py |  0
 6 files changed, 22 insertions(+)

To keep serving the finished run: target/release/harness-brain --journal target/brain-demo/journal.sqlite --port 4321
```
