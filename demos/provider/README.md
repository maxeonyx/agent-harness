# Provider demo

```sh
cargo run -q -p harness-provider --example fork
```

Run it from the repo root, with the harness signed in (`harness-api login`, as `demos/home/README.md` says). An optional argument picks the model, by id or by family (`sonnet`, `opus`), read from the provider's list; the default is `haiku`, the newest Haiku, so a run costs little.

It builds a root (the identity string plus a ledger of about 6k tokens, unique to each run) and holds a two-turn conversation on it. Then it forks the second response twice: fork 1 is sent, and fork 2 is sent when fork 1's FirstByte arrives. Every request is built from the context it extends plus the bytes of the values its entries name, and streams its text live. Each response is recorded as a value and an entry, as the fold would. Each one prints `Sent`, `FirstByte` and `Ended`, with its cache reads and writes and the plan's 5h and 7d usage.

What to look for:
- turn 1 writes the root and its question; turn 2 reads it.
- Both forks read the conversation. Fork 2 also reads the fork point that fork 1 wrote, so its read is larger and its write is only its own question.
- `5h` and `7d` on every request.
