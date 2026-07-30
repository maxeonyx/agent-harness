# walking-skeleton spike

Disposable spike code. Brief: `docs/process/spikes/walking-skeleton-brief.md`
(revised at Gate 1, 2026-07-30 — this is the redo: async, evented,
cancel-correct).

A toy face+brain+limb harness in one process, communicating by events. Two
select loops: the face loop (stdin in, event rendering out) and the brain's
session loop (user events, in-flight provider requests, in-flight tool
calls). The recorder is a passive consumer of the same event stream. An
event is about its emitter; the face view, the model context view, and the
recorder JSONL are all projections of the one session event log.

Two binaries:

- `skeleton` — the harness. Speaks the OpenAI-compatible chat-completions
  API over HTTP.
- `fake-provider` — a separate HTTP server serving the same API from a
  response script (optionally with per-step delays), recording every
  request it receives.

## Run against the fake provider

```bash
cd experiments/walking-skeleton
FAKE_PROVIDER_PORT=8089 FAKE_PROVIDER_SCRIPT=examples/script.json \
  cargo run --bin fake-provider &
SKELETON_BASE_URL=http://127.0.0.1:8089/v1 cargo run --bin skeleton
```

## Run against a real provider

Any OpenAI-compatible endpoint works. OpenRouter, with optional reasoning
effort (`SKELETON_REASONING_EFFORT` sends `reasoning_effort` in the request
when set; omit it for models that don't support it):

```bash
SKELETON_BASE_URL=https://openrouter.ai/api/v1 \
SKELETON_API_KEY=$OPENROUTER_API_KEY \
SKELETON_MODEL=openai/gpt-5.6-terra \
SKELETON_REASONING_EFFORT=medium \
  cargo run --bin skeleton
```

A local launcher with your own key can live at `run.ignore.sh` (gitignored).

## CLI

- `<text>` — stage a user message (appends to the session log, never
  triggers; if a request or tool call is in flight it piggybacks on the
  next request)
- `/open <path>` — simulate user file-open activity (appends, never
  triggers; the face and the model see different projections of the event)
- `/end` — end the turn (triggers inference; the agent loop runs tool calls)
- `/cancel` — cancel in-flight work: request → drain → finalize; a running
  tool's child process is killed and reaped, and the cancellation is a
  recorded outcome, not an error
- `/rebuild` — rebuild the model context view from the event log (a
  distinct operation from incremental append; no compaction policy yet)
- `/dump` — introspect the ~exact context as the model sees it: opens a
  markdown rendering of the model view in `$EDITOR` (default `nano`; a
  plain command name, no arguments), then returns to the face. Everything
  the model *cannot* see — non-wire events, piggyback annotations — is in
  HTML comments. The stdin reader parks while the editor owns the terminal.
- `/quit` — exit (drains in-flight work first)

## Scenario evidence

```bash
cargo test
```

`tests/scenario.rs` runs the three exit-condition scenarios from the brief
against the fake provider, driving the skeleton interactively through its
CLI and asserting at the face output and the fake provider's request log:
appends never trigger (observed at the wire *between* steps); user activity
during a running tool keeps the face responsive and piggybacks after the
tool exchange without splitting it; cancel during a tool call drains to a
recorded cancelled outcome and the session stays usable.
