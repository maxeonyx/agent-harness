# Experiment Plan

A plan is a provisional document: only the next step is expected to
actually happen, and the plan is useful even though it is expected to
change. This plan was extracted from an earlier AI-drafted planning
document (previously in `docs/source-notes/requirements.md`, removed from
the source notes and the gist because it is derived material, not source)
and has NOT yet been revised against the Spike 0 evidence. Revision is the
next planning task.

Every spike follows `PROCESS.md` and `SPIKE_RULES.md`: brief in, runnable
evidence plus an outcome document out, user acceptance at Gate 1. Spike
code is disposable and never becomes core by copying (invariant). Several
experiments are expected before the first core integration; integration
then happens small and continuously, alternating with further spikes.

## Status

| Spike | Status |
|-------|--------|
| 0. Walking skeleton | **done** — accepted 2026-07-31; see `spikes/walking-skeleton-outcome.md` |
| 1-7 below | not started, sequence provisional |

## Spike 1: user-tool context contract

Purpose: validate the central in-band collaboration thesis — the user
works (files, terminal, search) inside the shared session, the model sees
compressed useful context, and user activity never triggers requests.

Covers: worker in-band activity; user-tool compression (rich UI for the
user, compressed context for the model); user-tool framing (user activity,
never agent tool calls); user-wins conflict basics.

Key tests: file open/edit context; command/search context; large output
compression; passive activity never triggers a request; turn end carries
accumulated context; tool-loop requests carry piggyback context; user-tool
output framed as user activity; stale-edit/user-wins smoke test.

Exit: the user-tool contract is expressive and disciplined enough to build
around.

## Spike 2: actor topology / transport / lifecycle

Purpose: validate face/brain/limb as logical roles across co-located,
split, proxied, and multi-face configurations.

Required configurations: `face+brain+limb`; `face+limb <-> brain`;
`face <-> brain <-> limb`; `face <-> brain <-> brain <-> limb`;
`face <-> brain+limb <-> face 2`; optional direct face↔limb stream
(brain-authorized capability, durable compressed fallback).

Key tests: same scenario in every topology; monolith still respects role
boundaries; limb never holds provider credentials; face disconnect leaves
brain/limb running (Windows and Linux, ideally without interrupting
in-flight requests); reconnect/catch-up; multiple faces see coherent
state; direct stream succeeds/falls back/revokes.

Design inputs: `spikes/event-streaming-notes.md` (sequencer-is-substrate,
replicas, proposals vs facts, peer handshake, limb as event peer,
cancellation as replicated fact) and the layered graceful-shutdown /
deadline-budget pattern (check what asupersync does).

Exit: splitting and co-location are deployment choices over one logical
model.

## Spike 3: persistence, resume, and analytics-grade storage

Purpose: validate storage before it calcifies — durable vs transient
state, restart/resume, analytics queries. Does not own the full context
lifecycle model (Spike 5).

Key tests: restart resumes sessions (including the accepted
resumable-pending-tool-call state from Spike 0); session hierarchy
survives restart; in-flight model/tool state represented; cache-supporting
transient data survives while useful and is cleaned after expiry; cost /
cache / tool-duration / stuck-scope queries work; large blobs separate
from hot tables; schema leaves room for append/rebuild/request-trigger
state.

Exit: the storage model supports worker, operator, analyst, and future
context-lifecycle requirements without obvious rewrite.

## Spike 4: structured subagents

Purpose: validate hierarchy, blocking, fork/fresh, and attention
semantics.

Key tests: parent suspends while children run and resumes only when all
complete; sibling status visible, sibling results hidden until the parent
resumes; user-facing child completes on `/done`; failed child returns an
error result (open experiment: abort-scope vs error-return semantics);
abandoned/stuck child visible; fresh session required across a limb
boundary, fork default within one limb. Open experiments from the notes:
whether deeply forked agents reliably stay within narrow subtask scope;
shared-workspace parallelism strategy.

Exit: structured concurrency is usable and understandable, not just
formally clean.

## Spike 5: live editing, tool reload, schema stability, context lifecycle

Purpose: validate rapid iteration for tools, plugins, schemas, prompts,
and process context, including the append/rebuild/request-triggering model
and cache-aware behavior.

Key tests: warm session keeps schema v1 or receives an explicit diff;
new/rebuilt sessions receive canonical v2; failed reload quarantines
without bricking sessions; explicit cache-break/rebuild path;
request-trigger rules hold (only tool-loop continuation, turn end,
cache-nearly-expired handover, explicit resume); AGENTS.md/process edits
recorded without pretending warm context changed; rebuild does not replay
obsolete append-only notices. Open experiments from the notes: provider
cache semantics for append/rebuild/forks; cache-aware proactive handover;
two-stage handover flow.

Exit: tool iteration and process-context editing are fast without
corrupting warm sessions, model context, or request-triggering semantics.

## Spike 6: multi-client, shared UI state, streaming, and real UIs

Purpose: validate the hardest part of the client state model. Before this
spike, prototypes use an append-only CLI; after it, a real reactive TUI
and a real web GUI share one underlying client state model.

Key tests: two clients converge on the same ordered durable events; stale
client sends are represented causally and cannot silently overwrite newer
draft/tool state; reconnect catches up without duplicates; shared editable
UI state (draft buffers, file open/edit state) converges (CRDT or
equivalent — mechanism is exploratory); face-local state stays local;
shared UI state does not become model context by accident; TUI and web GUI
attach to the same session with the same state model. TUI explorations
(styling, throbber/status designs) attach here.

Exit: the UI/client state model is credible for real multi-client use.

## Spike 7: operator update/relaunch/protocol lifecycle

Purpose: validate deployment and operational lifecycle — version
negotiation, staged updates, activation/verification, downgrade, safe
migrations, smooth relaunch (locally or remotely) without unnecessarily
interrupting in-flight work.

Exit: operational lifecycle assumptions are credible enough for core
design.

## Unnumbered experiment candidates

Not placed in the sequence; some may fold into numbered spikes during plan
revision.

- **Modular components** (user-requested, 2026-07-30: "an experiment which
  is focused on clean, modular components especially with regard to
  testing and config"). Thesis: the harness's components can be ordinary
  objects constructed from composable typed config with all I/O injectable
  at construction — so the black-box scenarios that today spawn two
  binaries can also run fully in-process, deterministic and fast (whole
  suite well under a second), while the out-of-process wiring stays just
  another composition of the same components. Config composition follows
  the user's `deconfuse` Python library in Rust terms (define a typed
  schema once; explicit ordered sources; recursive merge for nested
  components; parent→child propagation without globals; injectable
  environ/argv); testing follows the user's guidelines (black-box first
  through public surfaces; injected implementations, never mock/patch).
  Falsified if in-process tests can't reach the same assertion surfaces
  without asserting internals, or config forces components to know their
  construction context, or determinism needs mock/patch seams. Exit: the
  walking-skeleton scenario suite running fully in-process AND still
  composable into the two-binary CLI form, plus a written comparison with
  deconfuse. Touches invariants 4, 8, 10. Open user decision (hedged):
  "Perhaps this would be its own library in the agent-tools ecosystem
  though, actually - it's useful for all my projects." — standalone
  library vs harness infrastructure; the spike should produce evidence
  for that call, not presume it. Overlaps the context-layer composition
  design work in `source-notes/configuration-model.md` ("needs
  significant design work") — candidates to merge at plan revision.
- **Event-streaming / replication protocol** (user, 2026-07-31): innards —
  unlocks cleaner capabilities and deployment flexibility but is not
  user-facing, so not a good first experiment. Design inputs curated in
  `spikes/event-streaming-notes.md`; largely a Spike 2 concern.
- **Provider cancellation economics** (`source-notes/analytics.md`):
  does cancelling after first byte avoid the charge? "Probably worth
  experiment."
- **Credentials / OAuth** (`source-notes/anthropic-oauth-references.md`):
  principled credential handling; third-party Anthropic OAuth approaches.
- **Layered graceful shutdown / deadline budgets** (user, 2026-07-31):
  every layer shuts down what it owns; the layer above holds a timeout
  backstop; a descending deadline budget is an idea, not a ruling; check
  what asupersync does.

## Provisional core integration expectations

Core is a fresh design integrating spike-proven behavior — never copied
spike code. It should likely include: face/brain/limb role boundaries over
co-located and split topologies; SQLite-backed session/event storage with
an explicit durable/transient/shared-UI/disposable data lifecycle;
explicit context lifecycle (record, append, rebuild, trigger); the
user-tool and agent-tool contracts; append and rebuild modes with
cache-aware metadata; structured subagent hierarchy; a multi-client-safe
shared UI state model; plugin/tool reload with schema stability across
warm sessions; analytics-grade metadata from the start; a
protocol/version/migration story; operator lifecycle hooks; and authority
boundaries keeping provider credentials brain-owned.

Core should NOT prematurely include: full GUI polish; full browser
integration; perfect terminal undo/fork semantics; federated-brain UX
beyond validated routing basics; highly forked shared-workspace automation
as a default; elaborate permission prompts or a general harness-owned
permission model for limb operations (personal-use limbs may run in YOLO
mode; stricter permissions are a limb implementation concern).

Integration acceptance (Gate 2 in `PROCESS.md`) checks, beyond the
invariants: behavioral coverage across the stakeholder breadth; would we
choose the design fresh; no accidental narrowing of future GUI /
multi-client / topology / analytics / reload requirements; every stored
datum knows its lifecycle class; append stays separate from triggering;
warm append-mode deltas distinguishable from rebuild-mode canonical
context; no spike-code cargo culting; no security boundary erosion; shared
UI state explicitly modeled; the spike outcome documents reviewed.
