# Requirements

This is a derived document. The vision is the user's own notes in
`docs/source-notes/` (verbatim, never edited locally); this file is a
process-maintained distillation used at gates, and where it disagrees with
the source notes or the user, it is this file that is wrong.

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
behavioral clusters in disposable experiments before integrating anything.

Requirements here derive from three places, and each entry says which:

- **Design notes** — the user's own notes in `docs/source-notes/`
  (verbatim source material, never edited locally).
- **Experiment evidence** — behavior proven or rulings made at experiment
  gates; the history lives in the experiment outcome docs, the current truth here.
- **Process rulings** — user decisions about how the work itself is done.

## The soul of the design

The user's own weighting (2026-07-31, wording preserved). The "unique
soul" is:

- user turn
- hardcore "forking" subagent model + cache efficiency + structured
  concurrency
- limb model (a session exists with respect to one limb. a limb provides
  tools but also context etc.)
- agent-owned compaction lifecycle

The "non-unique soul" ("good taste" choices from other harnesses) is,
non-exhaustively:

- decoupled monolith for deployment & agent flexibility + composable
  config + fast in-memory "black box" testing
- self-modifying implementation (Deno, JS implementation, self-limb +
  self-deployment & auto-rollback)
- "context" model (facts, updates, skill dependencies, etc.)
- ...

## What each stakeholder needs

- **Worker** (design notes): work happens in-band; each user tool has two
  surfaces — a rich interactive UI for the user and compressed context for
  the model; the user wins on conflicting edits, and stale agent output
  never silently overwrites newer user work. Validated by the user-turn experiment.
- **Process improver** (design notes): prompts, skills, AGENTS.md files,
  tool descriptions, and schemas are rapidly and safely iterable; edits
  are recorded honestly against warm-cache reality (no pretending current
  context changed). Validated by the context-updates experiment.
- **Harness developer** (design notes): disposable experiments, safe
  tool/plugin reload, eventual safe self-modification (the harness editing
  and relaunching itself without losing sessions). Validated by walking-skeleton and self-modification.
- **Operator** (design notes): roles deploy co-located or split; safe
  updates, downgrades, protocol versioning, migrations, background
  persistence across user disconnects on Windows and Linux. Validated by
  topology and operator-lifecycle.
- **Analyst** (design notes): session data is analytics-grade and
  queryable from the start — cost, cache hit rates, tool durations,
  session classification, stuck scopes. Validated by the persistence-analytics experiment.
- **Security / authority boundaries** (design notes): provider credentials
  stay brain-owned, never reaching limbs, faces, plugins, schemas, logs,
  or model context; user tools and agent tools are framed differently;
  direct connections are capability-bound. NOT a general agent-permission
  model — personal limbs may run in YOLO mode; permission prompts and
  approval theatre are explicitly unwanted. Validated by user-turn and topology.
- **Attention / coordination** (design notes): parallel work stays
  legible — structured subagent concurrency, visible blocked states,
  explicit sibling scopes. Validated by the forked-subagents experiment.
- **Multi-client / UI state** (design notes): multiple faces share live UI
  state (drafts, open files, panes) without stale clients corrupting
  anything; eventually a real reactive TUI and web GUI over one client
  state model. Validated by the multi-client-ui experiment.

## Invariants

The non-negotiables every gate checks against. A change that violates one
of these stops and goes to the user.

1. **The brain is the only role that drives provider API requests.**
   Provider credentials never reach limbs, faces, plugins, tool schemas,
   logs, or model context. (Design notes.)
2. **Recording, appending, rebuilding, and triggering are distinct
   operations.** Passive user activity never triggers a model request;
   only turn end, tool-loop continuation, cache-nearly-expired handover,
   and explicit resume may. (Design notes; proven in the walking-skeleton experiment.)
3. **All activity has multiple views.** An event is about its emitter, not
   *for* anyone; consumers (or a helpful middle layer) project it — to the
   model (possibly per-model), to the user (possibly per-interface), for
   rebuild vs append. User-tool activity framed as user activity rather
   than agent tool calls is one projection of this. (Design notes,
   reworded on walking-skeleton evidence.)
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
8. **Experiment code never becomes core by copying.** Core integration is a
   fresh design from evidence. (Process ruling.)
9. **Cancellation is baked in from the start** — request → drain →
   finalize; anything in flight ends with a recorded outcome; cancelled is
   distinct from error; four-valued outcomes (ok / error / cancelled /
   panicked); a drain structurally cannot start new work. Completed work
   that ties with a cancel is kept and recorded — it cost money and is
   probably good — while the turn still finalizes cancelled. (Walking-skeleton
   gate ruling; modeling inspiration: Dicklesworthstone/asupersync.)
10. **Roles never assume co-location.** No shared filesystem, environment,
    working directory, or clock is assumed across role boundaries; data
    crossing a boundary travels in the message, never by reference to
    role-local state. Everything the model sees must be derivable from the
    session record by any consumer. Exception, by design: a face and limb
    commonly DO share an environment (the user's machine), and co-located
    deployments may share the session state directly as the substrate.
    (Walking-skeleton gate ruling.)

## Architecture requirements from walking-skeleton evidence

These were ruled during the walking-skeleton review loop and are current truth; the
ruling history with the user's original wording is in
`experiments/walking-skeleton-outcome.md`.

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

## Decisions from design review (2026-08-04 onward)

What the user decided while reviewing design docs, in his wording, dated.
Separate from the walking-skeleton section because these come from review
rather than experiment evidence, and kept out of the invariants list
because several are hedged — hedging is information. Design docs fold
these into their content and point here; "

From the 2026-08-12 review of the context-updates rewrite, to be carried
by the rewritten doc:

- **A context has several cache prefixes at once, nested — "prefix" is
  not just the system section.** His list: "system section (system
  prompt, tools, etc); system section + messages *up to the last fork
  boundary*; system section + messages *up to ..* + all subsequent
  messages." A forked session has many. And new: "For user-facing
  messages, we also need a prefix which is *everything up to n-2 messages
  ago* (or something like that) to support message undo."
- **Actionability is the notify-at-all test, not the content rule.** "if
  it would not change the agent's actions in any way, then it doesn't
  need to know!" The second, separate decision is **reference versus the
  information itself**: a reference is less injected content ("less cost,
  less confusion") but "the agent actually has to be able to retrieve the
  new info somehow if it thinks it *is* relevant."
- **The limb must continue to accept old tool calls** "until all sessions
  that used those old tools have reached cache expiry (and so would be
  rebuilt)."
- On mid-session tool additions: "I'm not sure if new tools work yet or
  not. Seems fine to me?" — waiting reads as acceptable; hedge kept.
- **Piggybacking needs explaining** in the doc — it is important.
- **"A reference" is not literal — a notice carries the minimum, possibly
  less than a name.** "It could even be something like 'one or more skills
  have gone stale.' (although that is too un-specific probably, it gets
  across the idea - we don't neccessarily need to list 10 skills that have
  updates - we might elide even that info as long as we give the agent a
  way to re-discover reality reliably & cheaply ie. it shouldn't have to
  reload everything just to be sure)."
- **Notices are a contingent economic choice, and progressive disclosure
  is the same choice.** "the actual economics is the fundamental here
  (that the notices are actually a contingent choice - unconditional input
  smaller, conditional billing an extra turn (so more cache read in that
  branch), vs unconditional billing of larger number of tokens at 'input'
  cost but no extra turn. And yes, this is exactly the same as progressive
  disclosure." The PD connection is "a connection, not a fundamental."
- **Waiting for rebuild is safe in one of two ways.** "to deal with
  correctness, we *keep the old tools working*. or it might be stuff that
  doesn't really affect correctness."
- **The only bound on honouring old tool schemas:** "I don't see any other
  constraint? The question is - is there ever going to be another use of
  this tool code version, or not?"
- **The ~1h elapsed-time threshold is "discoverable, that's my naive
  guess"** — a tunable, not a designed constant. (Hedge his.)
- **Expired ("old cold") contexts: not settled.** Constant across his
  takes: on load it is not rebuilt — "just leave it purely as it was? I
  think the latter - much easier"; it is the event log of that agent
  session. First take was compact-only ("I think we only ever need to
  load it up in order to compact it"), then revised same day: "I think
  ideally we just keep an 'old cold' context and make it an 'old warm
  context', append some (perhaps copious, but oh well) notices, and keep
  going. That is the ideal state btw. it's minimum cost - we get
  re-billed at input to compact, we might as well up it to cache write &
  *not* compact? Perhaps an option for the user? I don't think this is
  settled. Tool schemas for tools that will no longer work is a great
  reason to force the compaction, though." (Hedges his. Also touches the
  future compaction-handover doc.)
- **Notice decision 2 is itself economic.** On reference-vs-content: "not
  necessarily - it still depends on the economics & the agent's reaction.
  but *all else equal*, the minimum."

- **Event streaming implies snapshotting.** Wording preserved: "while we
  have event streaming, we should also have roll ups, and we should deliver
  snapshots, not just event streams. I would ideally like that baked into
  the model from the very start... every thing that implements event
  streaming should ideally implement snapshotting." Reason given: cost —
  "otherwise it gets really, like, really expensive." Note "ideally" in
  both halves. The word *deliver* makes this a protocol requirement as much
  as a storage one: a joining consumer can be sent a snapshot plus
  subsequent events rather than a replayed history.md`.
- **A brain owns one provider, billing and data-access domain, and
  "exactly one" is per domain rather than global.** The user's domains must
  stay separate: "home data access, work data access, home billing, work
  billing should be separate." Within a domain, one brain is enough — he is
  "quite happy for one home brain". Multi-brain is therefore a requirement
  of his real setup, not a speculative feature; what remains speculative is
  background sync, backup-by-default and cross-brain querying. Also ruled:
  the harness "should be able to act as its own brain if it has its own
  provider setup and the [OAuth] stuff setup", and "connecting to another
  brain is ideal too" — both are ordinary configurations, neither
  privileged. This refines rather than contradicts invariant 1, which keeps
  provider credentials brain-owned.
- **On carrying user-turn context: it is a sizing question, not a
  trade-off between two positions.** "this is not about a versus b. It's
  about how much a versus how much b." Input is cheap "compared to output"
  and "compared to repeated tool calling" — the second comparator is what
  justifies carrying looked-at context at all. The user also observed that
  user activity piggybacks on turns that would have happened anyway rather
  than creating new ones, while noting he had not fully settled the point;
  his working-through is in git history (first-generation INTERACTIONS.md).
- **Credentials live inside the session database.** "credentials should
  live inside the database... credentials should be treated like
  everything else we treated." Replication of credential rows is scoped by
  **brain profile**: replicas of the same profile share credentials;
  brains in other domains never receive them. Two decoupled decisions —
  this settles the home of record only; the OS keychain remains available
  as a *security root* (a key encrypting the rows at rest), not precluded.
  Credentials differ from code in one way: "they become invalid through
  external actions. So you can't... do the auto rollback... But that's
  fine" — recovery is re-authentication. This reverses the design docs'
  earlier keep-them-outside proposal and closes the "fourth durable store"
  gap.
- **Compact-and-report-back: the predecessor writes the report.** "lets
  the first agent build the report as well as their compaction summary.
  And then the compaction summary deals with the initial context of the
  new agent, and the report is given as if it was its first message. I
  guess. something like that." (Hedge his.) Report-back-to-user and
  report-back-to-parent are the same flow; compact-and-continue and
  compact-and-report-back are the only two situations.
- **Shutdown is one pattern at every scale.** A layer always has kill
  authority over its children — "it is its children for all intents and
  purposes, whilst it's blocked on its children" — exercised by command
  over the protocol: only the kill command (with its time budget, passed
  down again) crosses a boundary, and each layer kills what it locally
  owns. The remote limb's orphan timeout is the fallback for a *vanished*
  owner, not a second form of shutdown. This rejects the design docs'
  earlier two-forms framing.
- **The activity trail's order is the face's own recorded order.** Every
  face event carries a front-end and a back-end time anchor ("It's after
  this time on the front end. It's after that time on the back end... a
  partial order. Sure. But that's not to say that the face doesn't have
  its own total order and that we can't remember that") — so "in the order
  things happened" is an ordinary recorded fact, primarily a
  representation question. Rejects a derived cross-clock-scrambling
  concern.
- **Change notices are economised.** No notice for content the agent was
  never exposed to (first load simply gets the new version); some elements
  may warrant no notice even when exposed — "for example, the skill
  description. Assumably, that's not changing too much... maybe debatable,
  but I think we need to draw these lines. Otherwise, we'll get too much
  change notifications coming into the event stream." Notices only need to
  say something changed; the agent reloads at will. And a context rebuild
  "is basically the new snapshot" — notices are events that get rolled in.
- **Compaction has four trigger kinds, three of them forcible.**
  Agent-at-milestone is one kind; the harness forcibly triggers on the
  context-window limit ("something like 80-85% (or better, a fixed token
  threshold like 100k-200k tokens)") and on cache expiry while the agent
  is idle (the in-flight tool call "rewritten as still in progress, and
  execution continues seamlessly when it completes"); the user can also
  forcibly trigger. Replaces the earlier invite-only model.
- **Superseded contexts are stored directly, not reconstructed.** "in
  practice we will probably just store the context directly. That is much
  simpler than trying to reconstruct it deterministically from raw
  events. While full determinism is a nice aspiration, it feels overly
  ambitious and not important enough to justify the complexity." Also
  recorded as fact: the append-only cache is "effectively a branching
  structure" — suffixes may be discarded where a cache point can be
  predicted, "That is why forked sub-agents work at all."md`.
- **Write decompressed, not short.** A process requirement rather than a
  product one, recorded here because it governs every doc these gates
  read: "word count is not expensive because I have a very fast reading
  speed, but word depth is expensive because I actually have quite a slow
  mental speed. So decompressed is much better than compressed, much,
  much better than compressed... go through the examples. Go through the
  story. Go through the the entire logic chain, and I'll read that much
  faster than I'll read one sentence of compressed language." 
- **Harness economics are an empirical domain, and he wants agent-run
  tuning.** Hedges his: "compaction economics, cancellation economics...
  and forking economics all feel like empirical domains. I am not very
  strong in this area, so I would prefer a mechanism for agents to run
  these experiments or perform observational tuning. For example, a
  background meta-agent could tune global harness settings via A/B
  testing. If we can run a scheduled meta-agent, it could also tune
  handover instructions and other parameters over time." A capability
  want, not a committed design; flagged in `PLAN.md`.

## Engineering discipline (process rulings)

- **Test flakes are bugs.** Races are structurally excluded, not made
  unlikely or retried away.
- Black-box tests only, at product-public surfaces (CLI/UI, provider wire
  via fake provider, storage/query surface, and eventually the transport
  protocol). No asserting internals.
- Real provider use is in scope for experiments ("I want to actually use it");
  the fake provider is a separate HTTP server serving the same
  OpenAI-compatible API, so real vs fake is just a base URL.

## Deferred from early experiments by explicit ruling

Streaming responses; provider error taxonomy in the fake provider;
principled credential handling; SQLite storage design (persistence-analytics); subagents
(forked-subagents); compaction and per-model/per-interface view variation;
tool-calling robustness and improvements; the event-streaming /
replication protocol and everything in `experiments/event-streaming-notes.md`;
configuration/context-layer composition (see PLAN.md, modular components);
"the brain is in charge of configuration changes, I believe" — left for
later.

Validation sequencing and status live in `PLAN.md`.
