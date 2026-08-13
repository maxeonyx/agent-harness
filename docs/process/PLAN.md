# Experiment Plan

Experiments are a **pool, not a sequence**. We pull the next one based on
what the user wants to work on next, not on an objective ordering. Only
the pulled experiment is expected to actually happen as described; the
rest are provisional and get revised when pulled.

Every experiment follows `PROCESS.md` and `EXPERIMENT_RULES.md`: brief in,
runnable evidence plus an outcome document out, user acceptance at Gate 1.
Experiment code is disposable and never becomes core by copying
(invariant). Several experiments are expected before the first core
integration; integration then happens small and continuously, alternating
with further experiments.

Buckets follow the user's soul-of-the-design weighting (see
`REQUIREMENTS.md`): experiments validating the unique soul, experiments
validating the good-taste layer, and small targeted questions.

## Done

- **walking-skeleton** — accepted 2026-07-31; see
  `experiments/walking-skeleton-outcome.md`. One process, one session:
  append-only CLI face, real provider loop with tool calls, clean
  cancellation, `/dump` introspection, journaled shared session state.

## Pool: unique soul

### user-turn

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

Also includes basic UX enhancements (user, 2026-08-03): "I want shift
enter (kitty escape seq, configured in my win terminal) to be newline,
enter to stage, enter with *no* content to submit, and control enter (if
possible) to be submit too."

Scheduling note (user, 2026-08-03): "That one requires a lot of hands on
from me tho" — pull it when the user has hands-on time available.

Exit: the user-tool contract is expressive and disciplined enough to build
around.

### forked-subagents

Purpose: validate hierarchy, blocking, fork/fresh, cache-efficient
launching, and attention semantics.

Covers: structured concurrency scopes; fork by default within a limb,
fresh across limbs; shared seed contexts and attachments
(`source-notes/handoff-improvements.md`); the main-thread pattern;
Task/Resume; whether deeply forked agents reliably end their turn at the
assigned subtask (the A.1.3 framing question in
`source-notes/agent-hierarchy.md`).

Key tests: parent suspends while children run and resumes only when all
complete; sibling status visible, sibling results hidden until the parent
resumes; user-facing child completes on `/done`; failed child returns an
error result (open experiment: abort-scope vs error-return semantics);
abandoned/stuck child visible; fresh session required across a limb
boundary, fork default within one limb; forked context is append-mode
w.r.t. the parent. Open: shared-workspace parallelism strategy.

Exit: structured concurrency is usable and understandable, not just
formally clean — and forking is demonstrably cache-cheap.

### limb-model

Purpose: validate the limb as the full context environment for a session —
not just a tool surface — including context layer composition.

Covers: session bound to exactly one limb; limb-declared tool sets;
limb-owned context injection (AGENTS.md files, skills, execution context
like output truncation); the context layer types and their composition
(`source-notes/configuration-model.md` — flagged there as needing
significant design work); how the brain stores what the limb defines.

Exit: the limb/context contract is concrete enough that "a session exists
with respect to one limb" is a designed behavior, not a slogan.

### compaction-handover

Purpose: validate the agent-owned compaction lifecycle as a first-class
mechanism.

Covers: handover as append-mode instructions at the end of the
conversation, explicit about the new situation — what changes, what stays
in the system prompt, what leaves it and must be kept
(`source-notes/context-updates.md` compaction note); two-stage handover
flow; attachments loaded immediately into the fresh context
(`source-notes/handoff-improvements.md`); cache-aware proactive timing
(`source-notes/compaction.md`); the stateful handover document aspiration.

Exit: an agent can hand over well enough that a fresh context resumes the
work without the user noticing a seam, at a cost that beats letting the
cache expire.

## Pool: good taste

### topology

Purpose: validate face/brain/limb as logical roles across co-located,
split, proxied, and multi-face configurations — the decoupled monolith.

Required configurations: `face+brain+limb`; `face+limb <-> brain`;
`face <-> brain <-> limb`; `face <-> brain <-> brain <-> limb`;
`face <-> brain+limb <-> face 2`; optional direct face↔limb stream
(brain-authorized capability, durable compressed fallback).

Key tests: same scenario in every topology; monolith still respects role
boundaries; limb never holds provider credentials; face disconnect leaves
brain/limb running (Windows and Linux, ideally without interrupting
in-flight requests); reconnect/catch-up; multiple faces see coherent
state; direct stream succeeds/falls back/revokes.

Design inputs: `experiments/event-streaming-notes.md`
(sequencer-is-substrate, replicas, proposals vs facts, peer handshake,
limb as event peer, cancellation as replicated fact) and the layered
graceful-shutdown / deadline-budget pattern (check what asupersync does).

Exit: splitting and co-location are deployment choices over one logical
model.

### persistence-analytics

Purpose: validate storage before it calcifies — durable vs transient
state, restart/resume, analytics queries. Does not own the full context
lifecycle model.

Key tests: restart resumes sessions (including the accepted
resumable-pending-tool-call state from the walking skeleton); session
hierarchy survives restart; in-flight model/tool state represented;
cache-supporting transient data survives while useful and is cleaned after
expiry; cost / cache / tool-duration / stuck-scope queries work; large
blobs separate from hot tables; schema leaves room for
append/rebuild/request-trigger state.

Replication / backup-by-default (`source-notes/federated-brain.md`, not
previously carried into this plan): the user likes federated brains
storing *all* the data rather than each holding only its own — "that way I
get backups by default. Sync all the data in the background. keep it clear
where it came from, don't accidentally duplicate it or get it confused with
local data." Two schema demands fall out and belong here rather than in
topology: every durable row needs **provenance** (which brain it originated
on), and identity must be **globally unique** so background sync cannot
duplicate or conflate remote data with local. Whether sync itself is built
is a topology/operator question; not narrowing the schema against it is
this experiment's job. Related: `source-notes/analytics.md` wants queries
to span all connected brains.

Exit: the storage model supports worker, operator, analyst, and future
context-lifecycle requirements without obvious rewrite.

### context-updates

Purpose: validate the context model's change mechanics — append vs
rebuild, change notification as bare-minimum invalidation, progressive
disclosure.

The design is `docs/process/design/context-updates.md` (ideal state,
reviewed with the user 2026-08-12); this entry does not restate it. Scope
when pulled is a slice of that design, negotiated then.

Empirical questions the design leaves open for this experiment: whether
agents overreact to change notices (wording and frequency — also related
to the user-turn work); whether a utility model is a viable actionability
classifier and at what cost; the elapsed-time and debounce thresholds;
whether mid-session tool addition works via append at all.

Depends on provider-cache-probe for cache semantics.

Exit: context change handling is honest against warm-cache reality and
cheap in the common case.

### self-modification

Purpose: validate the self-modifying implementation bet — live editing
without live bricking, for plugins and the harness binary itself.

Covers (`source-notes/tech.md`): Deno embedding as the business-logic
layer (tools, providers, user tools, limbs?); hard plugin sandbox with
auth outside the plugin; plugins stored so warm sessions keep stable
schemas across reloads; failed reload quarantines and auto-rolls-back
without bricking sessions; the harness rebuilding and relaunching itself
onto new code, smoothly continuing sessions (self-limb + self-deployment).

Exit: an agent can edit a plugin or the harness, rebuild, reload or
relaunch, and continue — with rollback when the new version is broken.

### modular-components

User-requested (2026-07-30: "an experiment which is focused on clean,
modular components especially with regard to testing and config").

Thesis: the harness's components can be ordinary objects constructed from
composable typed config with all I/O injectable at construction — so the
black-box scenarios that today spawn two binaries can also run fully
in-process, deterministic and fast (whole suite well under a second),
while the out-of-process wiring stays just another composition of the same
components. Config composition follows the user's `deconfuse` Python
library in Rust terms (define a typed schema once; explicit ordered
sources; recursive merge for nested components; parent→child propagation
without globals; injectable environ/argv); testing follows the user's
guidelines (black-box first through public surfaces; injected
implementations, never mock/patch).

Falsified if: in-process tests can't reach the same assertion surfaces
without asserting internals, or config forces components to know their
construction context, or determinism needs mock/patch seams.

Exit: the walking-skeleton scenario suite running fully in-process AND
still composable into the two-binary CLI form, plus a written comparison
with deconfuse. Touches invariants 4, 8, 10.

Open user decision (hedged): "Perhaps this would be its own library in the
agent-tools ecosystem though, actually - it's useful for all my projects."
— standalone library vs harness infrastructure; the experiment should
produce evidence for that call, not presume it. Overlaps limb-model's
layer-composition work — candidates to merge when either is pulled.

### multi-client-ui

Purpose: validate the hardest part of the client state model. Before this
experiment, prototypes use an append-only CLI; after it, a real reactive
TUI and a real web GUI share one underlying client state model.

Key tests: two clients converge on the same ordered durable events; stale
client sends are represented causally and cannot silently overwrite newer
draft/tool state; reconnect catches up without duplicates; shared editable
UI state (draft buffers, file open/edit state) converges (CRDT or
equivalent — mechanism is exploratory); face-local state stays local;
shared UI state does not become model context by accident; TUI and web GUI
attach to the same session with the same state model. TUI explorations
(`source-notes/tui-styling.md`, `source-notes/throbber-design.md`) attach
here.

Exit: the UI/client state model is credible for real multi-client use.

### operator-lifecycle

Purpose: validate deployment and operational lifecycle — version
negotiation, staged updates, activation/verification, downgrade, safe
migrations, smooth relaunch (locally or remotely) without unnecessarily
interrupting in-flight work.

Exit: operational lifecycle assumptions are credible enough for core
design.

## Pool: targeted questions

- **provider-cache-probe** (identified by the stage-3 interaction pass): what are the provider's actual cache
  semantics? What counts as a cached prefix, whether append-only means
  append-only with respect to the whole context or some smaller unit, what
  a forked child inherits, the read-discount and write-multiplier prices,
  and how the OpenAI responses API and the Anthropic messages API differ.
  Small and cheap, but three designs — compaction-handover, context-updates
  and forked-subagents — each depend on the answer and none can settle it
  alone. The notes are explicit that this is a precondition rather than a
  detail: "we need to *very* correctly use OpenAI responses API & Anthropic
  messages API w.r.t. caching for this all to work." User-confirmed
  2026-08-04: "Yes, definitely. That is a great, well-scoped experiment."

  Prior knowledge to **verify, not re-discover** (from the Anthropic
  prompt-caching docs, 2026-08-12 — documentation, not yet observed
  against the real API):
  - Cache write is 1.25× base input at 5-minute TTL; cache read is 0.1×;
    breakpoints themselves cost nothing.
  - Cache write is charged when content newly enters the cache, and that
    includes the previous assistant turn: "input tokens represent the new
    user message, cache creation input tokens account for new assistant
    and user turns, cache read input tokens reflect the conversation
    history up to the previous turn." So model output is not cached at
    generation time — it is charged again at 1.25× on the next request if
    the breakpoint sits after it.
  - "The system automatically identifies and utilizes the longest
    previously cached sequence" — supports the nested-prefix model
    (context-updates claim 2): prefixes need not be selected.
  - An automatic caching mode exists (top-level `cache_control`) where the
    breakpoint advances as history grows.
  Still open and squarely this experiment's job: whether anything about
  tools can change without involving the cached prefix; what a fork
  inherits; real TTL behavior and observability; late system parts;
  whether a *different* model (eg. Haiku as a utility model) can read a
  prefix cached by a larger model at 0.1× — believed not, since the cache
  key is expected to include the model, but cheap to test; and the same
  questions on the OpenAI responses API.
- **meta-agent-tuning** (candidate, from user feedback 2026-08-04, hedges
  his): compaction, cancellation and forking economics "all feel like
  empirical domains", and he "would prefer a mechanism for agents to run
  these experiments or perform observational tuning. For example, a
  background meta-agent could tune global harness settings via A/B
  testing. If we can run a scheduled meta-agent, it could also tune
  handover instructions and other parameters over time." Not scoped into
  an experiment yet; recorded so the economics experiments design their
  parameters as *tunable settings with recorded outcomes* rather than
  constants, which is the property the meta-agent would need.
- **cancellation-economics** (`source-notes/analytics.md`): does
  cancelling after first byte avoid the charge? "Probably worth
  experiment."
- **oauth-credentials**
  (`source-notes/anthropic-oauth-references.md`): principled credential
  handling; third-party Anthropic OAuth approaches; Claude sub in this
  harness.
- **layered-shutdown** (user, 2026-07-31): every layer shuts down what it
  owns; the layer above holds a timeout backstop; a descending deadline
  budget is an idea, not a ruling; check what asupersync does. May fold
  into topology or stay a pattern note.

## Provisional core integration expectations

Core is a fresh design integrating experiment-proven behavior — never
copied experiment code. It should likely include: face/brain/limb role
boundaries over co-located and split topologies; SQLite-backed
session/event storage with an explicit
durable/transient/shared-UI/disposable data lifecycle; explicit context
lifecycle (record, append, rebuild, trigger); the user-tool and agent-tool
contracts; append and rebuild modes with cache-aware metadata; structured
subagent hierarchy; a multi-client-safe shared UI state model; plugin/tool
reload with schema stability across warm sessions; analytics-grade
metadata from the start; a protocol/version/migration story; operator
lifecycle hooks; and authority boundaries keeping provider credentials
brain-owned.

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
context; no experiment-code cargo culting; no security boundary erosion;
shared UI state explicitly modeled; the experiment outcome documents
reviewed.
