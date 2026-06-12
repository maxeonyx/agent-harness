# walking-skeleton spike

Disposable spike code. Brief: `docs/process/spikes/walking-skeleton-brief.md`.

A toy face+brain+limb harness in one process, speaking the OpenAI-compatible
chat-completions API over HTTP. Two binaries:

- `skeleton` — the harness. Append-only CLI face, brain owning the provider
  client and session context, limb owning tool execution, JSONL recorder.
- `fake-provider` — a separate HTTP server serving the same OpenAI-compatible
  API from a response script, recording every request it receives.

## Run against the fake provider

```bash
cd experiments/walking-skeleton
FAKE_PROVIDER_PORT=8089 FAKE_PROVIDER_SCRIPT=examples/script.json \
  cargo run --bin fake-provider &
SKELETON_BASE_URL=http://127.0.0.1:8089/v1 cargo run --bin skeleton
```

## Run against a real provider

Any OpenAI-compatible endpoint works. Anthropic:

```bash
SKELETON_BASE_URL=https://api.anthropic.com/v1 \
SKELETON_API_KEY=$ANTHROPIC_API_KEY \
SKELETON_MODEL=claude-sonnet-4-6 \
  cargo run --bin skeleton
```

## CLI

- `<text>` — stage a user message (appends to context, never triggers)
- `/open <path>` — simulate user file-open activity (appends, never triggers)
- `/end` — end the turn (triggers inference; agent loop runs tool calls)
- `/quit` — exit

## Scenario evidence

```bash
cargo test
```

`tests/scenario.rs` runs the exit-condition scenario from the brief against
the fake provider and asserts on the requests it recorded.
