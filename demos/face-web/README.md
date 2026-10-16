# face-web: the task tree, live from the journal

```sh
cargo run -p harness-face-web --example scripted
```

Then open https://mx-hv-mj.local:4320 (another port: append it, `-- 4400`). The terminal prints each record as the script writes it; the page shows the same journal folded.

The script writes a realistic night of work through `State::apply`, each agent from its own thread: the orchestrator spawns two designers that share one attachment, then two engineers and a fork of itself. `engineer-face-web` asks you a question and sits in "Waiting on you" for 15 seconds before the script answers as you. `engineer-store` fails after three overloaded requests. The orchestrator ends its turn and waits on you. The whole run takes about a minute and a half, then the page keeps serving.

What to watch:

- each task's throbber: the model before its first byte, the model streaming, a tool, its children, the harness preparing a request, or you;
- the queue, in the order the ranking gives, with its reason;
- click a task to read its context, root first, its head marked;
- reload the page, or drop the network, at any point: the stream resumes from the last journal position the page saw and loses nothing.

The journal is a real `harness-store` file, `target/face-web-demo.sqlite`, recreated each run; the face reads it on its own connection.

It serves with the shared wildcard certificate in `~/.config/dev-certs/shared-local/` and accepts only this machine and the Windows host (the default route's gateway).
