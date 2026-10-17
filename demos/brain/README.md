# brain: the harness on a root task, killed -9 mid-run, carrying on

```sh
demos/brain/run.sh
```

Then watch https://mx-hv-mj.local:4341 while it runs, and type to a task from its compose box (another port: `PORT=4400 demos/brain/run.sh`). Every role runs on the scripted model, which costs nothing: the orchestrator and its two agents answer from `script.md`, whose lines say each response, and the run takes about 30 seconds. `MODEL=sonnet demos/brain/run.sh` runs real agents on `mandate.md` instead: about two minutes and a few dozen requests on the subscription.

What it does:

1. `scratch.sh` makes a git repository, `target/brain-demo/repo`: a Python package with a README that says what it should be, an `AGENTS.md`, and a `check.sh` that runs the tests and then a 15-second "style check".
2. Launch 1 starts the brain on a new database, and Max starts an orchestrator on `script.md` (or `mandate.md`) at a tab (the script runs `examples/act`). The root orchestrator reads the repository and spawns one scope of two agents: a fork of itself, in the same directory, and a fresh engineer, in its own clone on the branch `palindromes`. Each writes one module and its tests and runs `./check.sh`; the engineer commits and pushes its branch, and the orchestrator merges it.
3. While an agent's `./check.sh` is running, the script kills the brain with `kill -9`.
4. Launch 2 is started on the same database, with no other arguments. It folds its records and carries on:
   - the request that was in flight is said to be lost and sent again, reading the prompt cache;
   - the workspaces outlive the brain. Each is a replica with its own log, and keeps running its calls while its link is down; it reconnects to the new brain with the same replica, ships what it recorded meanwhile, and the running `./check.sh` completes as if nothing happened. Had the brain stayed away 15 minutes, the workspace would have stopped its calls, recorded them as interrupted, and exited; a call is unknown only when the workspace died too, before it could record how the call ended.
5. The orchestrator gets both results when the scope closes, runs `./check.sh`, commits, reports, and waits on you: like every task you start, it is a chat with you. `done.sh` presses **Done** for you at a tab (`target/release/examples/act PORT done`), and the orchestrator's last turn writes its result. The script prints that result and the repository's log.

To serve a finished run again: `target/release/harness-brain --db target/brain-demo/brain.sqlite --port 4341 --model scripted`. A database whose root is done does nothing but serve.

What the agents were told, from the store (`entry.content`, cut):

The fork's first message, in the framing the fork-framing spike measured: a neutral answer to the spawn, then its part as user text.
```
[{"content":"Started wordcount, palindromes.","tool_use_id":"toolu_016w…","type":"tool_result"},{"text":"You are now `orchestrator-wordcount`, a fork of `orchestrator`. Everything above is `orchestrator`'s context; from here you do one part of its work, and only that part.\n\nYour part:\nWrite textstats/wordcount.py …
```

## Real output

A run on the scripted model at 04:05 NZDT on 2026-10-18 (on `PORT=4375`), each line cut at 150 characters. The last turn's result is the scripted model echoing Max's Done prompt, since `script.md` cannot say what comes after Done.

```
== launch 1: the orchestrator starts on demos/brain/script.md, every role on scripted
harness-brain 65a3d6fdc6dd+dirty
    0.0s d6228c41c2363fd8.1  brain launched, build 279bba386ad5
    0.0s d6228c41c2363fd8.1  limb meta defined
    0.0s d6228c41c2363fd8.1  limb web defined
    0.0s d6228c41c2363fd8.1  limb user defined
    0.0s d6228c41c2363fd8.1  limb browser defined
    0.0s d6228c41c2363fd8.1  limb roles defined
face: https://mx-hv-mj.local:4375
    0.0s d6228c41c2363fd8.2  roles connected
    0.0s 08d6b4382b98796b.1  environment browser defined
    0.0s 08d6b4382b98796b.1  environment chat defined
    0.0s 08d6b4382b98796b.1  environment designer defined
    0.0s 08d6b4382b98796b.1  environment engineer defined
    0.0s 08d6b4382b98796b.1  environment orchestrator defined
    0.0s 08d6b4382b98796b.1  role roles/browser.md changed
    0.0s 08d6b4382b98796b.1  role roles/chat.md changed
    0.0s 08d6b4382b98796b.1  role roles/designer.md changed
    0.0s 08d6b4382b98796b.1  role roles/engineer.md changed
    0.0s 08d6b4382b98796b.1  role roles/orchestrator.md changed
    0.0s d6228c41c2363fd8.4  tab 9d1b4f3042e871c8 is Max's
    0.0s 9d1b4f3042e871c8.1  limb target/brain-demo/repo defined
    0.0s 9d1b4f3042e871c8.1  Build textstats created (orchestrator)
    0.0s 9d1b4f3042e871c8.1  as seen at d6228c41c2363fd8.3
    0.1s d6228c41c2363fd8.6  target/brain-demo/repo connected
    0.1s 8ac9e033a1a7a47d.1  target/brain-demo/repo described: 5 tools
    0.1s d6228c41c2363fd8.8  user connected
    0.1s 80510eb772c726a3.1  user described: 1 tools
    0.1s d6228c41c2363fd8.10  Build textstats has a new root, on scripted
    0.4s d6228c41c2363fd8.11  Build textstats: request sent
    0.4s d6228c41c2363fd8.12  Build textstats: first byte
    0.4s d6228c41c2363fd8.13  Build textstats's bash made {"command": "cat README.md AGENTS.md; git log --oneline"}
    0.4s d6228c41c2363fd8.13  Build textstats: responded, ToolUse, 1 calls (5990 in, 0 cached, 16 out)
    0.4s d6228c41c2363fd8.14  Build textstats's bash dispatched
    0.4s 8ac9e033a1a7a47d.2  Build textstats's bash completed
    0.4s d6228c41c2363fd8.16  Build textstats: request sent
    0.4s d6228c41c2363fd8.17  Build textstats: first byte
    0.4s d6228c41c2363fd8.18  Build textstats's spawn made
    0.4s d6228c41c2363fd8.18  Build textstats: responded, ToolUse, 1 calls (6316 in, 0 cached, 630 out)
    0.4s d6228c41c2363fd8.19  Build textstats's spawn dispatched
    0.4s d6228c41c2363fd8.19  Build textstats-wordcount created (forked from Build textstats)
    0.4s d6228c41c2363fd8.19  limb target/brain-demo/repo-palindromes defined
    0.4s d6228c41c2363fd8.19  Build textstats-palindromes created (engineer)
    0.6s d6228c41c2363fd8.20  Build textstats-wordcount: request sent
    0.6s d6228c41c2363fd8.21  Build textstats-wordcount: first byte
    0.6s d6228c41c2363fd8.22  Build textstats-wordcount's bash made {"command": "cat > textstats/wordcount.py <<'PY'\nimport re\nfrom collections imp…
    0.6s d6228c41c2363fd8.22  Build textstats-wordcount: responded, ToolUse, 1 calls (8211 in, 0 cached, 165 out)
    0.6s d6228c41c2363fd8.23  Build textstats-wordcount's bash dispatched
    0.7s 8ac9e033a1a7a47d.3  Build textstats-wordcount's bash completed
    0.7s d6228c41c2363fd8.25  Build textstats-wordcount: request sent
    0.7s d6228c41c2363fd8.26  Build textstats-wordcount: first byte
    0.7s d6228c41c2363fd8.27  Build textstats-wordcount's bash made {"command": "./check.sh"}
    0.7s d6228c41c2363fd8.27  Build textstats-wordcount: responded, ToolUse, 1 calls (8439 in, 0 cached, 8 out)
    0.7s d6228c41c2363fd8.28  Build textstats-wordcount's bash dispatched
    0.7s d6228c41c2363fd8.29  target/brain-demo/repo-palindromes connected
    0.7s d8e5a532552c6999.1  target/brain-demo/repo-palindromes described: 5 tools
    0.7s d6228c41c2363fd8.31  Build textstats-palindromes has a new root, on scripted
    1.0s d6228c41c2363fd8.32  Build textstats-palindromes: request sent
    1.0s d6228c41c2363fd8.33  Build textstats-palindromes: first byte
    1.0s d6228c41c2363fd8.34  Build textstats-palindromes's bash made {"command": "cat > textstats/palindromes.py <<'PY'\nimport re\n\n\ndef is_palin…
    1.0s d6228c41c2363fd8.34  Build textstats-palindromes: responded, ToolUse, 1 calls (6063 in, 0 cached, 216 out)
    1.0s d6228c41c2363fd8.35  Build textstats-palindromes's bash dispatched
    1.0s d8e5a532552c6999.2  Build textstats-palindromes's bash completed
    1.0s d6228c41c2363fd8.37  Build textstats-palindromes: request sent
    1.0s d6228c41c2363fd8.38  Build textstats-palindromes: first byte
    1.0s d6228c41c2363fd8.39  Build textstats-palindromes's bash made {"command": "./check.sh"}
    1.0s d6228c41c2363fd8.39  Build textstats-palindromes: responded, ToolUse, 1 calls (6353 in, 0 cached, 8 out)
    1.0s d6228c41c2363fd8.40  Build textstats-palindromes's bash dispatched

** kill -9 the brain

== launch 2: the same database, nothing else
  act: tab 2e260c57b36bc10f linked
  act: Max presses Done
included
harness-brain 65a3d6fdc6dd+dirty
    0.0s d6228c41c2363fd8.41  brain launched, build 279bba386ad5
face: https://mx-hv-mj.local:4375
    0.0s d6228c41c2363fd8.42  target/brain-demo/repo-palindromes connected
    0.0s d8e5a532552c6999.3  target/brain-demo/repo-palindromes described: 5 tools
    0.0s d6228c41c2363fd8.44  target/brain-demo/repo connected
    0.0s 8ac9e033a1a7a47d.4  target/brain-demo/repo described: 5 tools
    0.0s d6228c41c2363fd8.46  user connected
    0.0s 80510eb772c726a3.2  user described: 1 tools
   10.6s 8ac9e033a1a7a47d.5  Build textstats-wordcount's bash completed
   10.6s d6228c41c2363fd8.49  Build textstats-wordcount: request sent
   10.6s d6228c41c2363fd8.50  Build textstats-wordcount: first byte
   10.6s d6228c41c2363fd8.51  Build textstats-wordcount: responded, EndTurn, 0 calls (8573 in, 0 cached, 30 out)
   11.0s d8e5a532552c6999.4  Build textstats-palindromes's bash completed
   11.0s d6228c41c2363fd8.53  Build textstats-palindromes: request sent
   11.0s d6228c41c2363fd8.54  Build textstats-palindromes: first byte
   11.0s d6228c41c2363fd8.55  Build textstats-palindromes's bash made {"command": "git add textstats/palindromes.py tests/test_palindromes.py && git …
   11.0s d6228c41c2363fd8.55  Build textstats-palindromes: responded, ToolUse, 1 calls (6489 in, 0 cached, 44 out)
   11.0s d6228c41c2363fd8.56  Build textstats-palindromes's bash dispatched
   11.0s d8e5a532552c6999.5  Build textstats-palindromes's bash completed
   11.0s d6228c41c2363fd8.58  Build textstats-palindromes: request sent
   11.0s d6228c41c2363fd8.59  Build textstats-palindromes: first byte
   11.0s d6228c41c2363fd8.60  Build textstats-palindromes: responded, EndTurn, 0 calls (6601 in, 0 cached, 30 out)
   11.0s d6228c41c2363fd8.61  Build textstats's spawn completed
   11.3s d6228c41c2363fd8.62  Build textstats: request sent
   11.3s d6228c41c2363fd8.63  Build textstats: first byte
   11.3s d6228c41c2363fd8.64  Build textstats's bash made {"command": "git merge -q palindromes && ./check.sh && git add textstats tests && git commi…
   11.3s d6228c41c2363fd8.64  Build textstats: responded, ToolUse, 1 calls (7082 in, 0 cached, 35 out)
   11.3s d6228c41c2363fd8.65  Build textstats's bash dispatched
   26.3s 8ac9e033a1a7a47d.6  Build textstats's bash completed
   26.3s d6228c41c2363fd8.67  Build textstats: request sent
   26.3s d6228c41c2363fd8.68  Build textstats: first byte
   26.3s d6228c41c2363fd8.69  Build textstats: responded, EndTurn, 0 calls (7294 in, 0 cached, 41 out)
   26.6s d6228c41c2363fd8.70  tab 2e260c57b36bc10f is Max's
   26.6s 2e260c57b36bc10f.1  Done prompt delivered to Build textstats
   26.6s 2e260c57b36bc10f.1  as seen at d6228c41c2363fd8.69
   26.6s d6228c41c2363fd8.72  Build textstats: request sent
   26.6s d6228c41c2363fd8.73  Build textstats: first byte
   26.6s d6228c41c2363fd8.74  Build textstats: responded, EndTurn, 0 calls (7399 in, 0 cached, 66 out)

== the task is done; its result:

(scripted) You said: [harness] Max has marked this chat done. Write your result: what you did, what you learned, and what is left undone. This is you…

Start a line with one of say, tool, long, vary, error, limit, fail, drop to script a reply.

harness-brain: SIGTERM: shutting down; requests in flight have 300s to finish (again to stop now)
   27.2s d6228c41c2363fd8.75  brain shutting down, within 300s
harness-brain: shut down
== the repository afterwards
a56174a Add wordcount

 tests/test_wordcount.py | 11 +++++++++++
 textstats/wordcount.py  | 10 ++++++++++
 2 files changed, 21 insertions(+)
7662b32 Add palindromes

 tests/test_palindromes.py | 12 ++++++++++++
 textstats/palindromes.py  | 14 ++++++++++++++
 2 files changed, 26 insertions(+)
b514f3b Start textstats

 .gitignore            |  1 +
 AGENTS.md             |  4 ++++
 README.md             | 10 ++++++++++
 check.sh              |  7 +++++++
 tests/__init__.py     |  0
 textstats/__init__.py |  0
 6 files changed, 22 insertions(+)

To keep serving the finished run: target/release/harness-brain --roles roles --db target/brain-demo/brain.sqlite --port 4375 --model scripted
```

A run on Sonnet (`MODEL=sonnet`) at 04:00 NZDT on 2026-10-18 did the same with real agents. The brain was killed while the fork's `./check.sh` ran; launch 2 sent the engineer's lost request again, and the fork's `./check.sh` completed in its workspace. The orchestrator merged `palindromes`, committed, and reported 14 tests passing. Done was pressed 42 s after the relaunch.
