# provider-cache-probe

Disposable evidence. Brief: `docs/process/experiments/provider-cache-probe-brief.md`.

Thesis under test: the cache model the design already assumes — nested prefixes the provider selects itself, append-only warmth, fork inheritance, and a cheap model reading an expensive model's prefix — is realisable on both the Anthropic Messages API and the OpenAI Responses API, and its whole cost model is three multipliers on the base input rate (1.25× write, 0.1× read, 1× uncached).

Each measurement asks one question with one pair of real API requests. The first writes a prefix; the second mutates exactly one thing and reports what the provider charged. A nonzero cache-read count in the response's own `usage` block means the prefix survived the mutation. The measured result sits next to the documented expectation in the run summary, so agreement and disagreement look different.

## Running it

Put your keys in `keys.ignore.env` beside this file (the `.ignore` keeps it out of git):

```
ANTHROPIC_API_KEY=sk-ant-...
OPENAI_API_KEY=sk-...
```

```bash
cd experiments/provider-cache-probe
deno task probe --dry-run      # build and price every request, send nothing
deno task probe                # both providers, no TTL waits
deno task probe --slow         # adds the TTL measurements: ~40 minutes of waiting
deno task probe --provider anthropic --only fork-with-breakpoint,fork-without-breakpoint
```

The probe prints an upper-bound cost before sending anything, and the total it actually spent at the end. A default run's bound is about US$0.35 and its real cost lands near half that. `--help` lists every option, including model overrides and the endpoint overrides used by the fake below.

Evidence goes to `results.ignore/<run-id>/`: `summary.md` is the readable record, `summary.json` the same data as data, and every request and response body verbatim under `<provider>/<measurement>/`. Request headers are never recorded, so no key can reach a result file. Pass `--out <dir>` to write a run somewhere committable once it is worth keeping.

A missing credentials file exits 2 and says what to create. A rejected credential exits 3. Everything else — a bad model id, an unsupported parameter, a rejected request shape — is recorded as that measurement's result and the run continues, because each measurement is independent and a rejection is itself evidence.

## Verifying the probe without spending anything

```bash
deno task fake      # in one terminal
deno run --allow-net --allow-read=. --allow-write=. main.ts \
  --anthropic-url http://127.0.0.1:8477/v1/messages \
  --openai-url http://127.0.0.1:8477/v1/responses \
  --keys keys.ignore.env
```

`fake_provider.ts` implements the caching rules as the documentation states them, in both APIs' shapes. It is not evidence and not a model of provider behaviour: running against it shows only that the probe builds the requests it means to, parses each provider's usage block correctly, prices the tokens correctly, reaches every verdict branch, and writes its evidence files. Every claim about how a provider actually behaves has to come from a run against the real endpoint.

## What this experiment explicitly does not support

Streaming (cache accounting arrives in the streamed `message_start` usage block either way, so it changes nothing being measured). The OpenAI `additional_tools` input item and deferred tool search — the documented append-only route for adding a tool mid-thread, which deserves its own measurement once the plain tool-array answers are in. Batch, flex and fast service tiers. Retention beyond a single TTL setting per provider. Any harness-side caching policy: this probe measures what the providers do, and what to do about it belongs to the context-updates, compaction-handover and forked-subagents experiments.
