# watch: a real tree of agents, live on the face

```sh
demos/watch/run.sh
```

Then open https://mx-hv-mj.local:4324 (another port: `PORT=4400`). It runs the brain demo's mandate in a fresh scratch repo under `target/watch`: an orchestrator reads the repo, spawns a fork and a fresh engineer in parallel, each writes its module and runs `./check.sh` (its tests, then a 15 s "style check"), and the orchestrator checks and commits. It takes about two minutes, and the face keeps serving the finished run until you stop it.

What to look at:

1. **The header**: how much of the subscription is used, `used: 5h 94% · 7d 38% · as of 03:50:18`, from the latest response. If a response was billed outside the plan, or the account is limited, it says so there in red.
2. **The tree**: each row says what its task is doing now. A running tool shows its command, `tool: bash: cd . && ./check.sh · 11s`; a task whose last turn failed says how often and why, under its row.
3. **Click a task.** Its context opens at the bottom and stays there as it grows. A tool use is one line, `→ write: tests/test_wordcount.py`; click it for the input. While the model streams, its text appears under the context; while a command runs, its output prints there as it comes, `running bash: cd . && ./check.sh`, then the tests, then `style: checking...` for 15 s. Scroll up and it stops following you.

Everything shown comes from the journal and the live feed. The command output is the workspace's `Live::Call` stream, which the brain forwards to `/live?call=<id>`:

```
$ curl -sN https://mx-hv-mj.local:4402/live?call=815fcd83a69f233de2b2e92733711341
retry: 1000

id: 1
data: "test_basic_usage (tests.test_palindromes.TestFindPalindromes.test_basic_usage) ... "

id: 2
data: "ok\n"
```

Each piece is a JSON string, so a carriage return or a trailing newline arrives as it was; the page draws carriage returns the way a terminal does.
