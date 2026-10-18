# brain: the harness on a root task, killed -9 mid-run, carrying on

```sh
demos/brain/run.sh
```

It starts the test brain `brain` (`dev/drive.mjs`) and prints its link: watch it while it runs, and type to a task from its compose box. Every role runs on the scripted model, which costs nothing, and the run takes about 30 seconds. `MODEL=sonnet/low demos/brain/run.sh` runs real agents on `mandate.md` instead: about two minutes and a few dozen requests on the subscription.

What it does:

1. `scratch.sh` makes a git repository, `target/brain-demo/repo`: a Python package with a README that says what it should be, an `AGENTS.md`, and a `check.sh` that runs the tests and then a 15-second "style check".
2. Launch 1 starts the brain on a new database, and Max starts an orchestrator on `script.md` (or `mandate.md`) at a tab (the script runs `examples/act`). The root orchestrator reads the repository and spawns one scope of two agents: a fork of itself, in the same directory, and a fresh engineer, in its own clone on the branch `palindromes`. Each writes one module and its tests and runs `./check.sh`; the engineer commits and pushes its branch, and the orchestrator merges it.
3. While an agent's `./check.sh` is running, the script kills the brain with `kill -9 $(cat .drive.ignore/brain/pid)`.
4. Launch 2 (`node dev/drive.mjs up --brain brain` again) starts the brain on the same database, its limbs still running. It folds its records and carries on:
   - the request that was in flight is said to be lost and sent again, reading the prompt cache;
   - the workspaces outlive the brain. Each is a replica with its own log, and keeps running its calls while its link is down; it reconnects to the new brain with the same replica, ships what it recorded meanwhile, and the running `./check.sh` completes as if nothing happened. Had the brain stayed away 15 minutes, the workspace would have stopped its calls, recorded them as interrupted, and exited; a call is unknown only when the workspace died too, before it could record how the call ended.
5. The orchestrator gets both results when the scope closes, runs `./check.sh`, commits, reports, and waits on you: like every task you start, it is a chat with you. `done.sh` presses **Done** for you at a tab (`target/release/examples/act PORT done`), and the orchestrator's last turn writes its result. The script prints that result and the repository's log.

To serve a finished run again: `node dev/drive.mjs up --brain brain`. A database whose root is done does nothing but serve. `node dev/drive.mjs down --brain brain` ends it.

What the agents were told, from the store (`entry.content`, cut):

The fork's first message: a neutral answer to the spawn, then its part as user text.
```
[{"content":"Started wordcount, palindromes.","tool_use_id":"toolu_016w…","type":"tool_result"},{"text":"You are now `orchestrator-wordcount`, a fork of `orchestrator`. Everything above is `orchestrator`'s context; from here you do one part of its work, and only that part.\n\nYour part:\nWrite textstats/wordcount.py …
```

## Real output

A run on the scripted model at 04:25 NZDT on 2026-10-18, each line cut at 150 characters. The last turn's result is the scripted model echoing Max's Done prompt, since `script.md` cannot say what comes after Done.

```
== launch 1: the orchestrator starts on demos/brain/script.md, every role on scripted
brain brain on https://mx-hv-mj.local:4440/ (pid 2424397; every role on scripted)
log …/.drive.ignore/brain/brain.log
end it with: node dev/drive.mjs down --brain brain
chat cc2115f03aebe96f.1.1
harness-brain 57c11971892a+dirty
    0.0s e548841920b622e7.1  brain launched, build df5b07bc35da
    0.0s e548841920b622e7.1  limb meta defined
    0.0s e548841920b622e7.1  limb web defined
    0.0s e548841920b622e7.1  limb user defined
    0.0s e548841920b622e7.1  limb browser defined
    0.0s e548841920b622e7.1  limb roles defined
face: https://mx-hv-mj.local:4440
    0.0s e548841920b622e7.2  roles connected
    0.0s 1c747feae08f8ceb.1  environment browser defined
    0.0s 1c747feae08f8ceb.1  environment chat defined
    0.0s 1c747feae08f8ceb.1  environment designer defined
    0.0s 1c747feae08f8ceb.1  environment engineer defined
    0.0s 1c747feae08f8ceb.1  environment orchestrator defined
    0.0s 1c747feae08f8ceb.1  role …/roles/browser.md changed
    0.0s 1c747feae08f8ceb.1  role …/roles/chat.md changed
    0.0s 1c747feae08f8ceb.1  role …/roles/designer.md changed
    0.0s 1c747feae08f8ceb.1  role …/roles/engineer.md changed
    0.0s 1c747feae08f8ceb.1  role …/roles/orchestrator.md changed
    0.2s e548841920b622e7.4  tab cc2115f03aebe96f is Max's
    0.2s cc2115f03aebe96f.1  limb …/target/brain-demo/repo defined
    0.2s cc2115f03aebe96f.1  Build textstats created (orchestrator)
    0.2s cc2115f03aebe96f.1  as seen at e548841920b622e7.3
    0.2s e548841920b622e7.6  …/target/brain-demo/repo connected
    0.2s 485c059a0d6f116c.1  …/target/brain-demo/repo described: 5 tools
    0.2s e548841920b622e7.8  user connected
    0.2s 25dce67c44c3d396.1  user described: 1 tools
    0.2s e548841920b622e7.10  Build textstats has a new root, on scripted
    0.5s e548841920b622e7.11  Build textstats: request sent
    0.5s e548841920b622e7.12  Build textstats: first byte
    0.5s e548841920b622e7.13  Build textstats's bash made {"command": "cat README.md AGENTS.md; git log --oneline"}
    0.5s e548841920b622e7.13  Build textstats: responded, ToolUse, 1 calls (6488 in, 0 cached, 16 out)
    0.5s e548841920b622e7.14  Build textstats's bash dispatched
    0.5s 485c059a0d6f116c.2  Build textstats's bash completed
    0.5s e548841920b622e7.16  Build textstats: request sent
    0.5s e548841920b622e7.17  Build textstats: first byte
    0.5s e548841920b622e7.18  Build textstats's spawn made
    0.5s e548841920b622e7.18  Build textstats: responded, ToolUse, 1 calls (6827 in, 0 cached, 630 out)
    0.5s e548841920b622e7.19  Build textstats's spawn dispatched
    0.5s e548841920b622e7.19  Build textstats-wordcount created (forked from Build textstats)
    0.5s e548841920b622e7.19  limb …/target/brain-demo/repo-palindromes defined
    0.5s e548841920b622e7.19  Build textstats-palindromes created (engineer)
    0.8s e548841920b622e7.20  Build textstats-wordcount: request sent
    0.8s e548841920b622e7.21  Build textstats-wordcount: first byte
    0.8s e548841920b622e7.22  Build textstats-wordcount's bash made {"command": "cat > textstats/wordcount.py <<'PY'\nimport re\nfrom collections impo
    0.8s e548841920b622e7.22  Build textstats-wordcount: responded, ToolUse, 1 calls (8713 in, 0 cached, 165 out)
    0.8s e548841920b622e7.23  Build textstats-wordcount's bash dispatched
    0.8s 485c059a0d6f116c.3  Build textstats-wordcount's bash completed
    0.8s e548841920b622e7.25  Build textstats-wordcount: request sent
    0.8s e548841920b622e7.26  Build textstats-wordcount: first byte
    0.8s e548841920b622e7.27  Build textstats-wordcount's bash made {"command": "./check.sh"}
    0.8s e548841920b622e7.27  Build textstats-wordcount: responded, ToolUse, 1 calls (8953 in, 0 cached, 8 out)
    0.8s e548841920b622e7.28  Build textstats-wordcount's bash dispatched
    0.8s e548841920b622e7.29  …/target/brain-demo/repo-palindromes connected
    0.8s 2f3c8386ca7ffdc0.1  …/target/brain-demo/repo-palindromes described: 5 tools
    0.9s e548841920b622e7.31  Build textstats-palindromes has a new root, on scripted
    0.9s e548841920b622e7.32  Build textstats-palindromes: request sent
    0.9s e548841920b622e7.33  Build textstats-palindromes: first byte
    0.9s e548841920b622e7.34  Build textstats-palindromes's bash made {"command": "cat > textstats/palindromes.py <<'PY'\nimport re\n\n\ndef is_palind
    0.9s e548841920b622e7.34  Build textstats-palindromes: responded, ToolUse, 1 calls (6565 in, 0 cached, 216 out)
    0.9s e548841920b622e7.35  Build textstats-palindromes's bash dispatched
    0.9s 2f3c8386ca7ffdc0.2  Build textstats-palindromes's bash completed
    0.9s e548841920b622e7.37  Build textstats-palindromes: request sent
    0.9s e548841920b622e7.38  Build textstats-palindromes: first byte
    0.9s e548841920b622e7.39  Build textstats-palindromes's bash made {"command": "./check.sh"}
    0.9s e548841920b622e7.39  Build textstats-palindromes: responded, ToolUse, 1 calls (6868 in, 0 cached, 8 out)
    0.9s e548841920b622e7.40  Build textstats-palindromes's bash dispatched

** kill -9 the brain

== launch 2: the same database and limbs, nothing else
brain brain on https://mx-hv-mj.local:4440/ (pid 2424795; every role on scripted)
log …/.drive.ignore/brain/brain.log
end it with: node dev/drive.mjs down --brain brain
  act: tab fb068ee6eba43672 linked
  act: Max presses Done
included
harness-brain 57c11971892a+dirty
    0.0s e548841920b622e7.41  brain launched, build df5b07bc35da
face: https://mx-hv-mj.local:4440
    0.0s e548841920b622e7.42  …/target/brain-demo/repo connected
    0.0s 485c059a0d6f116c.4  …/target/brain-demo/repo described: 5 tools
    0.0s e548841920b622e7.44  …/target/brain-demo/repo-palindromes connected
    0.0s 2f3c8386ca7ffdc0.3  …/target/brain-demo/repo-palindromes described: 5 tools
    0.0s e548841920b622e7.46  user connected
    0.0s 25dce67c44c3d396.2  user described: 1 tools
    5.6s 485c059a0d6f116c.5  Build textstats-wordcount's bash completed
    5.6s e548841920b622e7.49  Build textstats-wordcount: request sent
    5.6s e548841920b622e7.50  Build textstats-wordcount: first byte
    5.6s e548841920b622e7.51  Build textstats-wordcount: responded, EndTurn, 0 calls (9088 in, 0 cached, 30 out)
    5.6s 2f3c8386ca7ffdc0.4  Build textstats-palindromes's bash completed
    5.6s e548841920b622e7.53  Build textstats-palindromes: request sent
    5.6s e548841920b622e7.54  Build textstats-palindromes: first byte
    5.6s e548841920b622e7.55  Build textstats-palindromes's bash made {"command": "git add textstats/palindromes.py tests/test_palindromes.py && git c
    5.6s e548841920b622e7.55  Build textstats-palindromes: responded, ToolUse, 1 calls (7003 in, 0 cached, 44 out)
    5.6s e548841920b622e7.56  Build textstats-palindromes's bash dispatched
    5.7s 2f3c8386ca7ffdc0.5  Build textstats-palindromes's bash completed
    5.7s e548841920b622e7.58  Build textstats-palindromes: request sent
    5.7s e548841920b622e7.59  Build textstats-palindromes: first byte
    5.7s e548841920b622e7.60  Build textstats-palindromes: responded, EndTurn, 0 calls (7116 in, 0 cached, 30 out)
    5.7s e548841920b622e7.61  Build textstats's spawn completed
    6.0s e548841920b622e7.62  Build textstats: request sent
    6.0s e548841920b622e7.63  Build textstats: first byte
    6.0s e548841920b622e7.64  Build textstats's bash made {"command": "git merge -q palindromes && ./check.sh && git add textstats tests && git commit
    6.0s e548841920b622e7.64  Build textstats: responded, ToolUse, 1 calls (7608 in, 0 cached, 35 out)
    6.0s e548841920b622e7.65  Build textstats's bash dispatched
   21.1s 485c059a0d6f116c.6  Build textstats's bash completed
   21.1s e548841920b622e7.67  Build textstats: request sent
   21.1s e548841920b622e7.68  Build textstats: first byte
   21.1s e548841920b622e7.69  Build textstats: responded, EndTurn, 0 calls (7820 in, 0 cached, 41 out)
   21.5s e548841920b622e7.70  tab fb068ee6eba43672 is Max's
   21.5s fb068ee6eba43672.1  Done prompt delivered to Build textstats
   21.5s fb068ee6eba43672.1  as seen at e548841920b622e7.69
   21.5s e548841920b622e7.72  Build textstats: request sent
   21.5s e548841920b622e7.73  Build textstats: first byte
   21.5s e548841920b622e7.74  Build textstats: responded, EndTurn, 0 calls (7926 in, 0 cached, 51 out)

== the task is done; its result:

(scripted) Noted: [harness] Max has marked this chat done. Write your result: what you did, what you learned, and what is left undone. This is your la


(scripted) The script is used up.

== the repository afterwards
c8115f2 Add wordcount

 tests/test_wordcount.py | 11 +++++++++++
 textstats/wordcount.py  | 10 ++++++++++
 2 files changed, 21 insertions(+)
a138730 Add palindromes

 tests/test_palindromes.py | 12 ++++++++++++
 textstats/palindromes.py  | 14 ++++++++++++++
 2 files changed, 26 insertions(+)
b97e8b1 Start textstats

 .gitignore            |  1 +
 AGENTS.md             |  4 ++++
 README.md             | 10 ++++++++++
 check.sh              |  7 +++++++
 tests/__init__.py     |  0
 textstats/__init__.py |  0
 6 files changed, 22 insertions(+)

To serve the finished run again: node dev/drive.mjs up --brain brain scripted
```

A run on Sonnet (`MODEL=sonnet`) at 04:00 NZDT on 2026-10-18 did the same with real agents. The brain was killed while the fork's `./check.sh` ran; launch 2 sent the engineer's lost request again, and the fork's `./check.sh` completed in its workspace. The orchestrator merged `palindromes`, committed, and reported 14 tests passing. Done was pressed 42 s after the relaunch.
