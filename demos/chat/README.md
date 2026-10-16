# chat: talking to the harness on its face

```sh
demos/chat/run.sh
```

Then open https://mx-hv-mj.local:4320 (another port: `PORT=4400`). It runs a real brain whose root task, `chat`, talks to you, in the brain demo's scratch repo under `target/chat/repo`. The journal persists: run it again and the chat carries on where it was; delete `target/chat` for a new one.

The root starts by reading the repo and, because `mandate.md` says what the repo should become, usually starts agents for it. Then it ends its turn and waits on you. It is done only when you press **Done**.

What to click:

1. **`chat`** in the tree. You focus on it, its context opens root first, and the compose box says "To chat".
2. Type, and press **Ctrl+Enter** to send. Enter on its own is a new line. Your words are a delivery in the journal; the task's next request carries them, and its answer streams in under its context.
3. **Dictate**, allow the microphone, and talk: the words arrive in the box as you speak, and stay editable. **Stop** ends it. On a machine with no microphone, **Dictate the sample** streams `sample.wav` (13 s of Piper speech) to the dictation endpoint at real time instead. `speak.sh` regenerates it from `sample.txt`.
4. While anything runs: **Interrupt**, **Stop** and **Hold/Release** above a task's details; **Waiting on you** lists your queue, ↑ ↓ reorder it, **Next** opens the top item. `demos/face-controls` drives each of these from a script.
5. Kill the brain and run it again at any point. A request in flight is lost and sent again, a tool call in flight ends unknown, and the page reconnects and loses nothing.

The model and dictation use the claude.ai login in `~/.claude/.credentials.json`. Nothing here refreshes the token, so if it has expired, use Claude Code once.

It serves with the shared wildcard certificate in `~/.config/dev-certs/shared-local/` and accepts only this machine and the Windows host (the default route's gateway).

## A real run

The brain's log for a first launch, trimmed:

```
face: https://mx-hv-mj.local:4402
    0.0s     1  brain launched, build harness-brain 0.0.0
    0.0s     2  limb meta defined
    0.0s     3  limb workspace defined
    0.0s     4  environment dev defined
    0.0s     5  workspace connected
    0.0s     6  chat created (orchestrator)
    0.0s     7  chat: request sent
    0.9s     8  chat: first byte
    2.0s     9  chat's bash made {"command":"cat README.md; echo ---; ls -la; echo ---; cat check.sh 2>/dev/null; echo ---; find . -maxdepth 3 -not -path './.git*'","grep":""}
   ...
   68.3s    84  chat's spawn completed
   93.0s    99  chat: responded, EndTurn, 0 calls (2 in, 7546 cached, 406 out)
```

**Dictate the sample**, pressed without a browser (`node demos/chat/dictate-sample.mjs [port]` speaks the page's WebSocket), ends with these edits to the box:

```
[12.4s] {"edits":[{"at":185,"cut":0,"put":" it to"}]}
[12.6s] {"edits":[{"at":191,"cut":0,"put":" the"}]}
[12.8s] {"edits":[{"at":195,"cut":0,"put":" task."}]}
[13.1s] {"final":true}
```

Then the brain was stopped and run again, and words were sent the way the page sends them: the bytes to `/digest` for their id, then `/act` with `{face, seen, batch: {values: [the bytes], facts: [{"Delivery": {..., "content": {"Words": "<id>"}}}]}}`. The relaunched brain answered from the cache it left:

```
    0.0s   100  brain launched, build harness-brain 0.0.0
    0.0s   101  workspace connected
   24.0s   102  words delivered to chat
   24.0s   103  chat: request sent
   25.2s   104  chat: first byte
   28.3s   105  chat: responded, EndTurn, 0 calls (2 in, 9680 cached, 292 out)
```
