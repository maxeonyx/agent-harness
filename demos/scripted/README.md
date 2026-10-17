# Scripted model demo

```sh
demos/scripted/run.sh
```

It lists the models on offer, read from the provider with `harness-api` (one call on Max's login, no model call), then starts a brain at https://mx-hv-mj.local:4370/ (`PORT` to change) whose every role runs on the scripted model (`--model scripted`): no model call, no cost. Its first chat runs `mandate.md`, a script of three responses: a `bash` call, a `spawn` of a fork that streams its reply a word at a time, and a 60-line reply. Ctrl-C ends it and the limbs it started.

What to look for:
- The chat's tool call and its result, the fork's reply, then the long reply streaming in.
- The model pill lists the newest model of each family the provider returned, and `Scripted (no cost)`.
- Send the chat a script of your own. Each line starting with a verb is one response; other text is echoed back:
  - `say TEXT`
  - `tool NAME JSON`
  - `long N`
  - `error STATUS TEXT`
  - `limit SECONDS`
  - `fail TEXT`
  - `drop`
  
  A verb may follow `slow MS` (stream each piece after MS) or `flaky STATUS` (refused once, then answered). For example `flaky 529 slow 300 say retried, then streamed slowly`, which you can stop mid-stream.

The verbs are documented in `crates/provider/src/scripted.rs`. Any brain can choose the model too: `model: scripted` in a role file, or the pill on one chat.
