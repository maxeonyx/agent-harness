# Spike Brief: walking-skeleton

Thesis: a minimal face+brain+limb harness can run end-to-end in a single
process speaking the OpenAI-compatible chat-completions wire format, with
context appending decoupled from request triggering — and because the harness
talks to its provider over plain HTTP, the same binary works against a real
provider (any OpenAI-compatible endpoint: Anthropic's compat API, OpenAI,
OpenRouter, a local server) and against a fake provider that is a separate
local HTTP server serving the same API. The fake provider records every
request it receives, which makes the provider wire boundary directly
assertable. Falsified if: the append/trigger split cannot be expressed
cleanly (user activity ends up forcing requests), agent tool calls cannot
round-trip through limb dispatch, or the OpenAI-compatible format cannot
carry user-tool activity framed as user activity. Invariants touched: 1
(credentials live only in brain config, sourced from env), 2 (record, append,
and trigger are distinct operations — the heart of the exit scenario), 3
(simulated user file-open activity appends as user activity, never as a tool
call), 4 (face/brain/limb as logical roles co-located in one process), 8
(everything stays in `experiments/`; this is scaffold, not core).

Scope note (user direction, 2026-06-13): source-notes `requirements.md` §3
specs Spike 0 as fake-provider-only. The user expanded scope: include real
provider use ("I want to actually use it"), and the fake provider used for
tests must be a separate HTTP server serving an OpenAI-compatible API — not
an in-process stub. One adapter, two endpoints.

Exit condition: the scripted scenario passes against the fake provider —
user activity appends context without triggering a request, ending the turn
triggers exactly one request carrying the accumulated context (typed message
plus piggybacked user activity), and an agent tool call round-trips through
the limb — plus a manual smoke session against a real provider endpoint.
