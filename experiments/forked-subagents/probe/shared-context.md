# Can siblings share a cached explanation? — 2026-09-28

`shared-context.py`, on Max's Claude subscription, `claude-sonnet-5`. The parent's context is ~7k tokens and ends in one `task` call. Each child's tail answers that call with a ~7k-token shared explanation, then a one-line specific one. Child `a` goes first, then `b` twice.

| where the shared explanation sits | a: read / written | b: read / written | b again |
| --- | --- | --- | --- |
| a text block inside `tool_result.content`, with its own `cache_control` | rejected, 400: "cache_control may not be specified within `tool_result.content`. Instead, place it directly on `tool_result`" | | |
| the `tool_result` itself, with `cache_control`; the specific one a text block after it in the same user message | 0 / 14,382 | **14,366 / 16** | 14,382 / 0 |
| one `tool_result` holding both, automatic caching only | 0 / 14,380 | **0 / 14,380** | 14,380 / 0 |

So on Anthropic the shared explanation can be the tool result and be cached for every sibling, but only if the specific explanation is not inside that same tool result. Automatic caching alone caches only where a request ends, so without an explicit breakpoint each sibling pays for the shared part in full.
