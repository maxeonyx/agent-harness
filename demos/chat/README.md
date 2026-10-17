# chat: talking to the harness on its face

```sh
demos/chat/run.sh
```

Then open https://mx-hv-mj.local:4320 (another port: `PORT=4400`). It runs a real brain whose root task, `chat`, talks to you, in the brain demo's scratch repo under `target/chat/repo`. The journal persists: run it again and the chat carries on where it was; delete `target/chat` for a new one.

The root starts by reading the repo and, because `mandate.md` says what the repo should become, usually starts agents for it. Then it ends its turn and waits on you. It is done only when you press **Done**.

What to click:

1. **`chat`** on the map. Its node opens into its log, and you focus on it; the compose box says "To chat". Drag or scroll the map, or press Esc, to close it.
2. Type: the box is a staging area, kept per task across tabs and reloads, and Enter on its own is a new line. **Attach**, paste or drop images and files beside the text. **Send** (Ctrl+Enter) sends it. Your words are a delivery in the journal; the task's next request carries them, and its answer streams in at the foot of its log. Agents it starts appear on the map under it.
3. While a task works, its box offers two sends. **Send now** (Ctrl+Enter) reaches it with its next request, after the tool round in flight. **At turn end** (Ctrl+Shift+Enter) waits until it ends its turn, so it does not cut across the work; the log shows it as "it reads this when its turn ends" until then. Either way the model is told how far into its work you wrote: "Max wrote this while you were working, before your last 3 tool calls (from `toolu_…` on)".
4. **The context gauge** in an open node's header says how full its context was at its last request, of its model's window; click it for what the context holds (system prompt, tools, your words, its words, tool calls and results). Each working node on the map has the same fill as a thin bar.
5. **Dictate**, allow the microphone, and talk: the words arrive in the box as you speak, and stay editable. **Stop** ends it. On a machine with no microphone, **Dictate the sample** streams `sample.wav` (13 s of Piper speech) to the dictation endpoint at real time instead. `speak.sh` regenerates it from `sample.txt`.
6. While anything runs: **Interrupt**, **Hold/Release**, **Done** and **Stop** in an open node's header. The **needs you** pill at the top right lists your queue: ↑ ↓ reorder it, **Next** opens the top item. `demos/face-controls` drives each of these from a script.
7. Kill the brain and run it again at any point. A request in flight is lost and sent again, a tool call in flight ends unknown, and the page reconnects and loses nothing.

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
