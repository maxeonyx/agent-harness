# Spike Outcome: walking-skeleton

Spike: walking-skeleton
Status: evidence complete; awaiting Gate 1 (user acceptance). Real-provider
smoke run pending — no API key was available in the build environment, so it
is a one-command check for the user (see `experiments/walking-skeleton/README.md`).
Requirements tested: none by itself (Spike 0 is the shared substrate per
`REQUIREMENTS.md`); exercises invariants 1, 2, 3, 4, 8.

## What The Spike Proved

- The append/trigger split is expressible and cheap: typed user messages and
  simulated file-open activity append to brain-held context and send nothing;
  `/end` triggers exactly one request carrying everything accumulated. The
  scripted scenario asserts this at the wire (request count and contents from
  the fake provider's log) and at the recorder (append events strictly before
  `trigger_inference`).
- The OpenAI-compatible chat-completions format works as the single adapter
  boundary: one blocking HTTP client, base URL + optional bearer key from env.
  The fake provider is a genuinely separate HTTP server serving the same API,
  so tests assert what was actually sent over HTTP, not what internals claim.
- Agent tool calls round-trip: assistant `tool_calls` → limb executes
  (`list_files`, `read_file`, `bash`) → `tool` role result → follow-up request
  → final text. Demonstrated in the test and a manual run using `bash`.
- User-tool activity frames as user activity on the wire (a `user` role
  message prefixed `[user activity]`), never as a tool call, with the dual
  surface visible: rich face output, compressed model context.
- Face/brain/limb hold as logical roles in one process: the face never
  touches the provider, the limb never sees credentials, the brain owns the
  loop, context, and key.

## What The Spike Failed To Prove

- Real-provider behavior (tool-call quirks, auth, model naming across
  OpenAI-compatible endpoints) — smoke run pending an API key.
- Streaming, multi-turn cache semantics, append-vs-rebuild, compaction.
- Any persistence beyond an append-only JSONL event log; no querying.
- Concurrency of any kind (single blocking thread throughout).

## What Should Be Integrated

Shapes, not code (fresh design per invariant 8):

- Provider adapter = OpenAI-compatible HTTP client configured by base URL +
  key, with the fake provider as a separate server being the canonical test
  double. "Real vs fake is just a base URL" is worth keeping.
- The recorder's event taxonomy (`append_*`, `trigger_inference`,
  `request_sent`, `tool_call`, `tool_result`) as a seed for the storage/event
  design — it made the invariant-2 distinction directly observable.
- Context entries as a typed structure distinct from wire messages, with the
  wire built per-request from entries.
- The scenario-test pattern: drive the harness through its CLI, assert at the
  fake provider's request log. This is the durable black-box test shape for
  the provider wire surface and should become the first promoted test
  primitive.

## What Must Not Be Integrated

- Any of this code by copying — it is scaffold (invariant 8).
- The blocking single-threaded loop, the unrestricted `bash` tool, the
  panic-on-bad-env error handling, or env-var-only configuration.
- A commitment that OpenAI-compat is the only or final adapter — it is one
  adapter behind a boundary; native Anthropic Messages may be wanted later.

## Tests To Promote Or Preserve

`tests/scenario.rs` (spike-local) expresses the exit-condition scenario at
the public surfaces (CLI in, provider wire out). Re-derive it as a durable
black-box test when the first core slice integrates; the fake provider should
be re-implemented as the shared test primitive at that point.

## Requirements Pressure

- Scope change (user direction, 2026-06-13, recorded in the brief): spikes may
  use a real provider; the fake provider used for tests must be a separate
  HTTP server serving an OpenAI-compatible API. Source-notes `requirements.md`
  §3 still says fake-only for Spike 0 — fold into the next gist sync.
- No invariant changes suggested.

## New Risks Or Open Questions

- Streaming is absent and will be needed for real interactive use; it will
  reshape the face↔brain interface and possibly the recorder.
- OpenAI-compat dialect drift across providers (tool-call `arguments` string
  encoding, system role handling) is unverified until the real smoke run.
- The `/open` simulation reads files in the face; a real design must decide
  where user-tool compression lives (notes say the limb curates context).

## Invariants Check

1. Upheld — the key is read once into brain config from `SKELETON_API_KEY`;
   face and limb code have no access path to it.
2. Upheld — the core of the scenario; asserted at wire and recorder.
3. Upheld — user activity appears only as a `[user activity]` user message.
4. Upheld by construction — separate modules with narrow interfaces,
   co-located in one process as a deployment choice.
8. Upheld — everything lives in `experiments/walking-skeleton/`; nothing
   imported from `src/`.

## Review Result

Pending (fresh-context review optional at Gate 1, user's call).

## User Acceptance

Pending.
