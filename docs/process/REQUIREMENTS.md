# Requirements

## Why this project exists

The agent harness is a personal system for doing real work *with* agents
in one shared context: the user opens files, runs commands, searches, and
edits in-band, and the agent understands what happened without the user
restating it. Existing harnesses treat the user as a prompt source outside
the session; this one treats user work and agent work as two activity
streams over one session, each seen by the other through appropriate
projections.

"Stakeholder" below means the user in a different capacity. The design has
several perpendicular requirement directions — worker UX, process
improvement, harness development, operations, analytics, security
boundaries, coordination, multi-client UI — and the implementation must
preserve that breadth rather than building a narrow MVP and hoping it
generalises. That is why the process (see `PROCESS.md`) validates risky
behavioral clusters in disposable spikes before integrating anything.

Requirements here derive from three places, and each entry says which:

- **Design notes** — the user's own notes in `docs/source-notes/`
  (verbatim source material, never edited locally).
- **Spike evidence** — behavior proven or rulings made at spike gates;
  the history lives in the spike outcome docs, the current truth here.
- **Process rulings** — user decisions about how the work itself is done.

## What each stakeholder needs

- **Worker** (design notes): work happens in-band; each user tool has two
  surfaces — a rich interactive UI for the user and compressed context for
  the model; the user wins on conflicting edits, and stale agent output
  never silently overwrites newer user work. Validated by Spike 1.
- **Process improver** (design notes): prompts, skills, AGENTS.md files,
  tool descriptions, and schemas are rapidly and safely iterable; edits
  are recorded honestly against warm-cache reality (no pretending current
  context changed). Validated by Spike 5.
- **Harness developer** (design notes): disposable spikes, safe
  tool/plugin reload, eventual safe self-modification (the harness editing
  and relaunching itself without losing sessions). Validated by Spikes 0/5.
- **Operator** (design notes): roles deploy co-located or split; safe
  updates, downgrades, protocol versioning, migrations, background
  persistence across user disconnects on Windows and Linux. Validated by
  Spikes 2/7.
- **Analyst** (design notes): session data is analytics-grade and
  queryable from the start — cost, cache hit rates, tool durations,
  session classification, stuck scopes. Validated by Spike 3.
- **Security / authority boundaries** (design notes): provider credentials
  stay brain-owned, never reaching limbs, faces, plugins, schemas, logs,
  or model context; user tools and agent tools are framed differently;
  direct connections are capability-bound. NOT a general agent-permission
  model — personal limbs may run in YOLO mode; permission prompts and
  approval theatre are explicitly unwanted. Validated by Spikes 1/2.
- **Attention / coordination** (design notes): parallel work stays
  legible — structured subagent concurrency, visible blocked states,
  explicit sibling scopes. Validated by Spike 4.
- **Multi-client / UI state** (design notes): multiple faces share live UI
  state (drafts, open files, panes) without stale clients corrupting
  anything; eventually a real reactive TUI and web GUI over one client
  state model. Validated by Spike 6.

## Invariants

The non-negotiables every gate checks against. A change that violates one
of these stops and goes to the user.

1. **The brain is the only role that drives provider API requests.**
   Provider credentials never reach limbs, faces, plugins, tool schemas,
   logs, or model context. (Design notes.)
2. **Recording, appending, rebuilding, and triggering are distinct
   operations.** Passive user activity never triggers a model request;
   only turn end, tool-loop continuation, cache-nearly-expired handover,
   and explicit resume may. (Design notes; proven in Spike 0.)
3. **All activity has multiple views.** An event is about its emitter, not
   *for* anyone; consumers (or a helpful middle layer) project it — to the
   model (possibly per-model), to the user (possibly per-interface), for
   rebuild vs append. User-tool activity framed as user activity rather
   than agent tool calls is one projection of this. (Design notes,
   reworded on Spike 0 evidence.)
4. **Face, brain, and limb are logical roles**; co-location versus
   splitting is a deployment choice over the same logical model. (Design
   notes.)
5. **Durable session data is analytics-grade and queryable.** Durable,
   cache-supporting-transient, shared-UI, and disposable-stream data are
   explicitly distinguished. (Design notes.)
6. **Subagent concurrency is structured**: parents block on children;
   sibling results stay hidden until the parent resumes. (Design notes.)
7. **Multi-client UI state is explicitly modeled.** Stale clients cannot
   silently overwrite newer state; the user wins on conflicting edits.
   (Design notes.)
8. **Spike code never becomes core by copying.** Core integration is a
   fresh design from evidence. (Process ruling.)
9. **Cancellation is baked in from the start** — request → drain →
   finalize; anything in flight ends with a recorded outcome; cancelled is
   distinct from error; four-valued outcomes (ok / error / cancelled /
   panicked); a drain structurally cannot start new work. Completed work
   that ties with a cancel is kept and recorded — it cost money and is
   probably good — while the turn still finalizes cancelled. (Spike 0
   gate ruling; modeling inspiration: Dicklesworthstone/asupersync.)
10. **Roles never assume co-location.** No shared filesystem, environment,
    working directory, or clock is assumed across role boundaries; data
    crossing a boundary travels in the message, never by reference to
    role-local state. Everything the model sees must be derivable from the
    session record by any consumer. Exception, by design: a face and limb
    commonly DO share an environment (the user's machine), and co-located
    deployments may share the session state directly as the substrate.
    (Spike 0 gate ruling.)

## Architecture requirements from Spike 0 evidence

These were ruled during the Spike 0 review loop and are current truth; the
ruling history with the user's original wording is in
`spikes/walking-skeleton-outcome.md`.

- **Sequencing belongs to the deployment substrate, not to any
  participant.** The brain is not a sequencer. Within one process,
  participants synchronize (appends are synchronous calls under a lock);
  across processes there is no total order and no synchronization —
  asynchrony is accepted. Whether same-machine IPC is close enough to
  sequence is explicitly unresolved. The async-append question begins at a
  process boundary and is deferred to the event-streaming experiment.
- **Every component owns exactly one external world**, and the three are
  symmetric — each an {inbox + select loop + owned in-flight work}
  participant: the face owns the TUI, the brain owns the provider
  connection, the limb owns an environment (filesystem, processes,
  tools). Ephemeral provider state is the brain's, as ephemeral UI state
  is the face's. Nothing world-specific crosses a component boundary.
- **The TUI is the face's external world, not its innards.** Rendering is
  an output port, not loop logic ("rendering != face innards");
  synchronous tty takeover (an editor) is owned in-flight work; the face
  loop keeps selecting and is never blocked blind.
- **Tool facts are recorded by both brain and limb, split by ownership.**
  The brain records context facts: a call detected in a response, a result
  entering the model view. The limb is in charge of the actual execution
  (or not) of tool calls and records the execution facts; environment
  facts like hostname come from the limb.
- **A proposed-but-unexecuted tool call is valid, resumable state.** On
  cancel, wait for the in-flight response and keep it, but do not execute
  its proposed calls. Unexecuted calls get no fabricated outcome, are
  omitted from the wire (the model never sees a call that never ran),
  remain visible to introspection, and may be executed on a later resume.
  Executed-then-cancelled calls do get their cancelled result on the wire
  (exchange adjacency).
- **Process cleanup happens by ownership, never by global observation.**
  The limb owns and cleans up its process trees (group lifetime ==
  operation lifetime, on every resolution path); no process-table
  scanning in the harness or its tests. Kernel-enforced ownership (PID
  namespaces / cgroups — "some kind of container maybe") is a hedged
  later idea.
- **Structured lifecycle throughout.** No detached tasks or threads, no
  process::exit escape hatches; in-flight work is owned (identity +
  cancellation + join handle together) and always joined; participants
  return Results and failures fold into the exit code; every layer shuts
  down what it owns gracefully (parent-held timeout backstops and
  descending deadline budgets are a recorded pattern, deferred).
- **Introspection is first-class.** An easy way to see the ~exact text as
  the model sees it (/dump), with everything the model cannot see marked
  as such; the request builder and the dump share one projection so they
  cannot diverge. `model` and `reasoning_effort` are request facts, not
  context facts.

## Engineering discipline (process rulings)

- **Test flakes are bugs.** Races are structurally excluded, not made
  unlikely or retried away.
- Black-box tests only, at product-public surfaces (CLI/UI, provider wire
  via fake provider, storage/query surface, and eventually the transport
  protocol). No asserting internals.
- Real provider use is in scope for spikes ("I want to actually use it");
  the fake provider is a separate HTTP server serving the same
  OpenAI-compatible API, so real vs fake is just a base URL.

## Deferred by explicit ruling

Streaming responses; provider error taxonomy in the fake provider;
principled credential handling; SQLite storage design (Spike 3); subagents
(Spike 4); compaction and per-model/per-interface view variation;
tool-calling robustness and improvements; the event-streaming /
replication protocol and everything in `spikes/event-streaming-notes.md`;
configuration/context-layer composition (see PLAN.md, modular components);
"the brain is in charge of configuration changes, I believe" — left for
later.

Validation sequencing and status live in `PLAN.md`.
