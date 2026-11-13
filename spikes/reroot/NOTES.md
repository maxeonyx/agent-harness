# T9 spike: re-rooting a context on a cold cache

Never merged. Question (tool review, https://harness.maxeonyx.com/pages/tool-params/#fresh): will the provider accept a context's old messages, signed thinking blocks included, under a new root (system prompt, tool list, model, params)? And what does a re-root cost?

`node spikes/reroot/probe.mjs [MODEL]` sends 10 requests through `harness-api` on Max's login: a turn that thinks and calls `read`, its `tool_result`, then the same three messages under changed roots. Requests are shaped as the brain's are: adaptive thinking with `display: summarized`, effort, 1h breakpoints on the end of the system prompt and on the tail. Output: `run-sonnet.txt`, `run-opus.txt` (the second runs, which met cache entries the first runs wrote); the first sonnet run's output is in the result that reported this spike.

## What the provider accepts

| Old messages under | Accepted |
|---|---|
| A new system prompt and a new tool list (one description changed, one tool added) | Yes |
| A tool list without the tool the history called | Yes |
| Another effort | Yes |
| Another model (sonnet-5-5 ⇄ opus-5-5), the other model's signed thinking included | Yes |
| An edited signature (control) | No: 400 "Invalid `signature` in `thinking` block" |
| Edited thinking text (control) | Yes: with `summarized`, the text is a summary; the signature carries the thinking |
| Thinking blocks dropped (control) | Yes, at the same cached token count |

So the signature is checked, and it binds the thinking to itself, not to the system prompt, tool list or model. Nothing in a re-root needs the old messages changed.

## What it costs

Cold, first sonnet run (tokens):

```
1 start                   cache_write=10494  cache_read=0
2 continue (same root)    cache_write=151    cache_read=10494
3 re-root: system+tools   cache_write=10682  cache_read=0
4 continue on new root    cache_write=0      cache_read=10682
6 re-root: effort low     cache_write=151    cache_read=10494
```

- A new system prompt or tool list misses everything after it (tools, then system, then messages, is the prefix order): the whole context is written again. On a cold cache that write happens anyway, so a cold re-root costs what continuing costs, plus only the changed root's own tokens (10682 vs 10645 here).
- On a warm cache it costs the whole context at the write price (2x base for 1h entries) instead of the read price (0.1x): about 20x that request's input cost. Hence: never re-root warm.
- The next request on the new root reads it all (step 4): the new root is a normal cache entry from then on.
- Effort is not part of the cache key (step 6 read the old root's prefix). A change of effort alone needs no re-root at all, warm or cold: the harness can change a running context's effort on its next request.
- The root's own entry (tools + system, the breakpoint at the end of the system prompt) is shared by every context with the same root and model. In the second runs, re-roots read 10462 tokens that another context had written. So after a role change, the first agent to re-root pays for the new root's prefix and the rest read it.

## What it means for T3

- The target holds: the harness can re-root at the first request after the cache has gone cold, with the same messages, at no extra cost. No `fresh`, no carry, no summary: the agent's context continues as it was under the new system prompt and tools, and a notice says once what changed.
- "Within minutes" is wrong for this brain: its breakpoints use the 1h TTL, so a context goes cold only after an hour without a request on it, and every read refreshes it. A busy agent never goes cold; for it, the change still lands at its next compaction onto a fresh root. Cold re-root serves the idle ones: an agent waiting on a long spawn, or on Max.
- "Cold" is judged from the brain's own model-call records (the core already says warmth is inferred from them): no request in the last hour on a prefix that covers this context's. A fork or sibling reading the shared prefix keeps it warm, so the last request on any chain through the context's tail counts, not only the context's own.
- A wrong guess costs one full write, the cost being avoided, never a failure: the provider accepted every re-root.
- Model changes re-root too (they miss the cache like a system change) and were accepted across the two current models. Untested: an older-generation model, server-tool result blocks (web search's encrypted content) under another model, and redacted thinking.
