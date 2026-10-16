# Provider demo

```sh
cargo run -q -p harness-provider --example fork
```

Run it from the repo root, signed in to Claude Code (it reads `~/.claude/.credentials.json` and never refreshes it). An optional argument picks the model; the default is `claude-sonnet-5`.

It builds a root (the identity string plus a ledger of about 6k tokens, unique to each run) and holds a two-turn conversation on it. Then it forks the second response twice: fork 1 is sent, and fork 2 is sent when fork 1's FirstByte arrives. Every request is built from the stored entry bytes and streams its text live. Each one prints `Sent`, `FirstByte` and `Ended`, with its cache reads and writes and the billing claim. Any claim other than a plan window fails the run.

What to look for:
- turn 1 writes the root and its question; turn 2 reads it.
- Both forks read the conversation. Fork 2 also reads the fork point that fork 1 wrote, so its read is larger and its write is only its own question.
- `claim=five_hour` on every request.
