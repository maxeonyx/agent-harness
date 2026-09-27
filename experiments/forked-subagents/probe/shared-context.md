# Can siblings share a cached explanation? — 2026-09-28

`shared-context.py`, on Max's Claude subscription, `claude-sonnet-5`. The parent's context is ~7k tokens and ends in one `task` call. Each child's tail answers that call with a ~7k-token shared explanation, then a one-line specific one. Child `a` goes first, then `b` twice.

| where the shared explanation sits | a: read / written | b: read / written | b again |
| --- | --- | --- | --- |
| a text block inside `tool_result.content`, with its own `cache_control` | rejected, 400: "cache_control may not be specified within `tool_result.content`. Instead, place it directly on `tool_result`" | | |
| the `tool_result` itself, with `cache_control`; the specific one a text block after it in the same user message | 0 / 14,382 | **14,366 / 16** | 14,382 / 0 |
| one `tool_result` holding both, automatic caching only | 0 / 14,380 | **0 / 14,380** | 14,380 / 0 |

So on Anthropic the shared explanation can be the tool result and be cached for every sibling, but only if the specific explanation is not inside that same tool result. Automatic caching alone caches only where a request ends, so without an explicit breakpoint each sibling pays for the shared part in full.

## How many breakpoints — 2026-09-28

`breakpoints.py`: one user message of explicit `cache_control` blocks, with and without the top-level (automatic) `cache_control`.

| explicit | automatic | answer |
| --- | --- | --- |
| 3 | no | 200 |
| 3 | yes | 200 |
| 4 | no | 200 |
| 4 | yes | 400: "A maximum of 4 blocks with cache_control may be provided. Found 5." |

The automatic breakpoint takes one of the four. Every level of forking leaves one explicit breakpoint in its descendants' contexts, so with the automatic one an agent can be at most three levels below the root.

## The shared part only in the parent's call — 2026-09-28

`tool-use-breakpoint.py`: the shared explanation (~7k tokens) sits only in the parent's `task` call, as its `shared` argument. The tool result is the child's own line alone. Child `a`, then `b` twice.

| breakpoint | a: read / written | b: read / written | b again |
| --- | --- | --- | --- |
| `cache_control` on the `tool_use` block | 0 / 14,398 | **14,381 / 17** | 14,398 / 0 |
| none, automatic caching only | 0 / 14,398 | **0 / 14,398** | 14,398 / 0 |

Anthropic accepts `cache_control` on a `tool_use` block, and a sibling reads through it. Without it the sibling reads nothing.
