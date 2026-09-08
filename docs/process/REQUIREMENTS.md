# Requirements

This is the specification a builder reads in order to be aligned with the design. Everything in it is current truth, and every statement traces to a source, cited inline: the user's own notes in `docs/source-notes/` (verbatim, never edited locally), a decision in `DECISIONS.md` (cited by date where he dated it, and by its opening phrase so it can be found), or evidence and rulings from an experiment gate (`experiments/walking-skeleton-outcome.md` holds that ruling history in his original wording). Anything that cannot be traced does not belong here.

The source notes are frozen, so when a later ruling supersedes one of their statements the correction has to live here. Those places say **Supersedes** and quote the superseded sentence. Where this file disagrees with the user, this file is wrong.

Hedged material is not stated as a requirement. It sits in **Open questions** at the end, quoted with the hedge intact. Contradictions the user knows about and left open sit in **Known tensions**; they are not to be harmonised.

## Why this project exists

The agent harness is a personal system for doing real work _with_ agents in one shared context: the user opens files, runs commands, searches, and edits in-band, and the agent understands what happened without the user restating it. Existing harnesses treat the user as a prompt source outside the session; this one treats user work and agent work as two activity streams over one session, each seen by the other through appropriate projections.

"Stakeholder" below means the user in a different capacity. The design has several perpendicular requirement directions — worker UX, process improvement, harness development, operations, analytics, security boundaries, coordination, multi-client UI — and the implementation must preserve that breadth rather than building a narrow MVP and hoping it generalises. That is why the process (see `PROCESS.md`) validates risky behavioral clusters in disposable experiments before integrating anything.

## The soul of the design

The user's own weighting (2026-07-31, wording preserved). The "unique soul" is:

- user turn
- hardcore "forking" subagent model + cache efficiency + structured concurrency
- limb model (a session exists with respect to one limb. a limb provides tools but also context etc.)
- agent-owned compaction lifecycle

The "non-unique soul" ("good taste" choices from other harnesses) is, non-exhaustively:

- decoupled monolith for deployment & agent flexibility + composable config + fast in-memory "black box" testing
- self-modifying implementation (Deno, JS implementation, self-limb + self-deployment & auto-rollback)
- "context" model (facts, updates, skill dependencies, etc.)
- ...

## What each stakeholder needs

- **Worker** (design notes): work happens in-band; each user tool has two surfaces — a rich interactive UI for the user and compressed context for the model; the user wins on conflicting edits, and stale agent output never silently overwrites newer user work. Validated by the user-turn experiment.
- **Process improver** (design notes): prompts, skills, AGENTS.md files, tool descriptions, and schemas are rapidly and safely iterable; edits are recorded honestly against warm-cache reality (no pretending current context changed). Validated by the context-updates experiment.
- **Harness developer** (design notes): disposable experiments, safe tool/plugin reload, eventual safe self-modification (the harness editing and relaunching itself without losing sessions). Validated by walking-skeleton and self-modification.
- **Operator** (design notes): roles deploy co-located or split; safe updates, downgrades, protocol versioning, migrations, background persistence across user disconnects on Windows and Linux. Validated by topology and operator-lifecycle.
- **Analyst** (design notes): session data is analytics-grade and queryable from the start — cost, cache hit rates, tool durations, session classification, stuck scopes. Validated by the persistence-analytics experiment.
- **Security / authority boundaries** (design notes): provider credentials stay brain-owned, never reaching limbs, faces, plugins, schemas, logs, or model context; user tools and agent tools are framed differently; direct connections are capability-bound. NOT a general agent-permission model — personal limbs may run in YOLO mode; permission prompts and approval theatre are explicitly unwanted. Validated by user-turn and topology.
- **Unwrapped markdown, by example** (design notes, `source-notes/markdown-nowrap-lead-by-example.md`): Max prefers markdown unwrapped "because almost all viewers deal with this gracefully, and it reduces burden on editors" — and this is a product requirement, not only a docs convention: "the context and system prompts, skills etc injected by the harness should follow this and lead by example".
- **Built-in browser control** (design notes, `source-notes/browser-mcp.md`): because desktop work often happens in web apps — his examples are Outlook web, Microsoft Teams, and a browser itself — the harness should embed an MCP server that consumes a remote debugging endpoint, "so that you don't have to have a separate piece of software running just to control a Chromium-based web browser app".
- **Attention / coordination** (design notes): parallel work stays legible — structured subagent concurrency, visible blocked states, explicit sibling scopes. Validated by the forked-subagents experiment.
- **Multi-client / UI state** (design notes): multiple faces share live UI state (drafts, open files, panes) without stale clients corrupting anything; eventually a real reactive TUI and web GUI over one client state model. Validated by the multi-client-ui experiment.

## Invariants

The non-negotiables every gate checks against. A change that violates one of these stops and goes to the user.

1. **The brain is the only role that drives provider API requests.** Provider credentials never reach limbs, faces, plugins, tool schemas, logs, or model context. (Design notes.)
2. **Recording, appending, refurbishing, and triggering are distinct operations.** Passive user activity never triggers a model request; only turn end, tool-loop continuation, cache-nearly-expired handover, and explicit resume may. (Design notes; proven in the walking-skeleton experiment.)
3. **All activity has multiple views.** An event is about its emitter, not _for_ anyone; consumers (or a helpful middle layer) project it — to the model (possibly per-model), to the user (possibly per-interface), for refurbish vs append. User-tool activity framed as user activity rather than agent tool calls is one projection of this. (Design notes, reworded on walking-skeleton evidence.)
4. **Face, brain, and limb are logical roles**; co-location versus splitting is a deployment choice over the same logical model. (Design notes.)
5. **Durable session data is analytics-grade and queryable.** Durable, cache-supporting-transient, shared-UI, and disposable-stream data are explicitly distinguished. (Design notes.)
6. **Subagent concurrency is structured**: parents block on children; sibling results stay hidden until the parent resumes. (Design notes.)
7. **Multi-client UI state is explicitly modeled.** Stale clients cannot silently overwrite newer state; the user wins on conflicting edits. (Design notes.)
8. **Experiment code never becomes core by copying.** Core integration is a fresh design from evidence. (Process ruling.)
9. **Cancellation is baked in from the start** — request → drain → finalize; anything in flight ends with a recorded outcome; cancelled is distinct from error; four-valued outcomes (ok / error / cancelled / panicked); a drain structurally cannot start new work. Completed work that ties with a cancel is kept and recorded — it cost money and is probably good — while the turn still finalizes cancelled. (Walking-skeleton gate ruling; modeling inspiration: Dicklesworthstone/asupersync.)
10. **Roles never assume co-location.** No shared filesystem, environment, working directory, or clock is assumed across role boundaries; data crossing a boundary travels in the message, never by reference to role-local state. Everything the model sees must be derivable from the session record by any consumer. Exception, by design: a face and limb commonly DO share an environment (the user's machine), and co-located deployments may share the session state directly as the substrate. (Walking-skeleton gate ruling.)

## Face, brain, and limb

Three roles, connected as `clients (faces) <-> control server (brain) <-> workspace runtimes (limbs)` (`source-notes/agent-harness-design.md`):

- **Face** — a client: TUI, web UI, desktop app, IDE plugin.
- **Brain** — owns the agent loop, model calls, session storage, provider credentials, billing, rate limits, and compaction.
- **Limb** — the context and execution environment for one project: filesystem, shell, git, grep, formatters, linters, tests, diagnostics, possibly LSP, and context curation.

The reason for the split is memory. N projects each running a full harness costs ~400–500 MB per busy instance. One brain at moderate fixed cost, plus N lightweight limbs at a <100 MB target, plus lazy evictable LSPs, makes memory scale with active work instead of with every open project (`source-notes/agent-harness-design.md`).

"Limb" is a placeholder name. The recorded candidates are workspace, runtime, context, environment and sandbox; not yet decided (`source-notes/agent-harness-design.md`, `source-notes/agent-hierarchy.md`).

Each of the three owns exactly one external world, and the three are symmetric: each is an {inbox + select loop + owned in-flight work} participant. The face owns the TUI, the brain owns the provider connection, the limb owns an environment. Nothing world-specific crosses a component boundary — ephemeral provider state is the brain's exactly as ephemeral UI state is the face's (walking-skeleton gate ruling).

The TUI is the face's external world, not its innards: rendering is an output port rather than loop logic ("rendering != face innards"). Synchronous tty takeover — the user's editor — is owned in-flight work, so the face loop keeps selecting and is never blocked blind (walking-skeleton gate ruling).

### What the limb is authoritative over

The limb is not just a tool surface. It aggregates context from multiple sources and presents a unified environment to the agent (`source-notes/agent-harness-design.md`):

- **The tool set.** The limb declares it. The brain may add its own tools, and may filter if the user configures that, but by default does not filter or redefine limb tools. The brain does no limb tool-call processing at all, only dispatching.
- **What context reaches the model from its domain** — in-repo AGENTS.md files, per-machine AGENTS.md files, checked-in agent and skill definitions, and execution-related context such as truncating large tool outputs or appending LSP info to tool calls.
- **How execution happens** — the limb server owns the filesystem, shell, git, and all local execution.

The limb does not store the session context or the tool set; the brain does. A limb is currently identified by `ssh_host` (empty for same-machine) and `directory`, both stored in the brain, and that is likely all that is needed to connect, start and reconnect one. Display name, which agent types may use it, and resource limits are candidate per-limb config, not yet decided (`source-notes/agent-harness-design.md`). A limb is also not one thing: "it's tools, context, cwd, and more" (decision 2026-08-12, per-element decisions).

The limb owns and cleans up its own process trees — group lifetime equals operation lifetime, on every resolution path. No process-table scanning anywhere in the harness or its tests. Kernel-enforced ownership (PID namespaces or cgroups — "some kind of container maybe") is a hedged later idea (walking-skeleton gate ruling).

### Limb types

Examples, not exhaustive (`source-notes/agent-harness-design.md`). A **full local limb** is a subprocess in a git repo on the brain's machine, living as long as it is connected or until the brain exits. A **remote limb** has the same capabilities over SSH plus a tunnel; its lifecycle is related to but distinct from the SSH connection, so it survives an ungraceful disconnect in case of reconnect and shuts down if not reconnected within a timeout. An **in-process meta limb** lives inside the brain process and provides meta/global tools — cross-session search, agent status, config edits — with no project-local filesystem. A **remote limb-as-a-service** is a permanently available authenticated endpoint, for example with database-backed tools. A **read-only limb** offers only read, grep and glob. A **pure model limb** has no tools and no injected context beyond the prompt; a zero-tool limb is a valid type. A limb with no brain connection does nothing and might as well not exist — this is fine.

### Deployment

Face, brain and limb are logical roles; co-location versus splitting is a deployment choice over the same logical model (invariant 4), and no role assumes co-location (invariant 10). One binary runs in any mode — client (TUI or GUI), brain server (optionally with a tray icon and management GUI), limb server — and can run as all three at once while splitting out additional limbs and accepting additional clients. Components in one process communicate over a channel; separate processes on one machine over IPC; remote over TCP, HTTP, WebSockets, wireguard or SSH. A common configuration is client plus limb locally, connected to a brain elsewhere (`source-notes/tech.md`).

Sequencing belongs to the deployment substrate, not to any participant. The brain is not a sequencer. Within one process, participants synchronize — appends are synchronous calls under a lock. Across processes there is no total order and no synchronization; asynchrony is accepted. Whether same-machine IPC is close enough to sequence is explicitly unresolved, and the async-append question begins at a process boundary (walking-skeleton gate ruling; deferred to the event-streaming experiment).

Communication is evented with causal consistency, and each actor behaves as sequential — "eg. each session, including the UI in that somehow, probably". The limb server, the brain's agent loop and the user's UI are expected to see things in different orders; what matters is that the harness can represent that state of affairs and handle it correctly — the user sends a message without having seen the latest tool result, and that is a representable, handleable situation (`source-notes/tech.md`). Event streaming implies snapshotting: "while we have event streaming, we should also have roll ups, and we should deliver snapshots, not just event streams. I would ideally like that baked into the model from the very start... every thing that implements event streaming should ideally implement snapshotting" — the reason given is cost, "otherwise it gets really, like, really expensive". Because they are _delivered_, this is a protocol requirement as much as a storage one: a joining consumer can be sent a snapshot plus subsequent events rather than a replayed history (decision, event streaming implies snapshotting — note "ideally" in both halves).

Streaming output — tool stdout, model tokens — must reach the face fairly directly, and the brain must not buffer everything before forwarding. The default path is the brain proxying the limb's stream to the face, because the topology may not allow the face to reach the limb (a remote limb behind SSH, a different network segment). The fast path, where topology permits, is the face connecting to the limb directly, leaving the brain in the path only for the agent loop, model calls and permissions (`source-notes/agent-harness-design.md`).

Shutdown is one pattern at every scale. A layer always has kill authority over its children — "it is its children for all intents and purposes, whilst it's blocked on its children" — exercised by command over the protocol. Only the kill command crosses a boundary, carrying a time budget that is passed down again at each level, and each layer kills what it locally owns. The remote limb's orphan timeout is the fallback for a _vanished_ owner, not a second form of shutdown (decision, shutdown is one pattern at every scale).

### Brains, credentials and storage

Provider API calls happen only from the brain. Limbs hold no model credentials and run no agent loop (invariant 1; `source-notes/agent-harness-design.md`).

A brain owns one provider, billing and data-access domain, and "exactly one" is per domain rather than global: "home data access, work data access, home billing, work billing should be separate", while within a domain one brain is enough — he is "quite happy for one home brain". Multi-brain is therefore a requirement of his real setup, not a speculative feature. The harness "should be able to act as its own brain if it has its own provider setup and the [OAuth] stuff setup", and "connecting to another brain is ideal too"; both are ordinary configurations and neither is privileged (decision, a brain owns one domain).

Credentials live inside the session database — "credentials should be treated like everything else we treated". Replication of credential rows is scoped by brain profile: replicas of the same profile share credentials, and brains in other domains never receive them. That settles the home of record only; the OS keychain remains available as a _security root_, a key encrypting the rows at rest. Credentials differ from code in one way: "they become invalid through external actions", so there is no auto-rollback, and recovery is re-authentication (decision, credentials live inside the session database).

Storage is SQLite from the start: one indexed database the brain sees all of, rather than a file per session. His stated reason is that it is "Better for a brain managing many projects" — discovery becomes an indexed query rather than a filesystem glob (`source-notes/tech.md`; `source-notes/agent-harness-design.md`, "Preference"). Message contents, large text and images sit apart from the event tables, and the schema stays as normalized as performance allows. Beyond conversation history it must represent thread relationships (parent, child, sibling), blocked parent state, dynamic sibling sets, user-facing session lifecycle state, resume targets, compaction and handover continuity, and per-message API metadata — cost, token usage, model, provider (`source-notes/agent-harness-design.md`).

Because the brain works out of that database, on restart it picks up where it left off without requiring user interaction to resume threads. Interrupted while waiting on a model response: safe to resend and continue. Interrupted mid-tool-call: the tool reports something like "tool call interrupted by harness crash", and the agent reasons about state and safety itself. Limbs that were connected reconnect or are restarted as needed. Graceful shutdown is a priority — crashes should be rare, not designed around (`source-notes/agent-harness-design.md`).

**Supersedes `source-notes/agent-harness-design.md`:** the brain owning "permission decisions", and "Permissions / safety model — Not yet designed. Principle: strict and principled, or not at all." The live position is that this is not a general agent-permission model at all: personal limbs may run in YOLO mode, permission prompts and approval theatre are explicitly unwanted (stakeholder list above), and stricter permissions are a limb implementation concern (`PLAN.md`, core non-goals). The one surviving permission is the user's approval to launch a user-facing subagent, which is itself a tentative rule (see Open questions).

## Sessions, contexts and contributions

A **session** is one agent's conversation. A session is bound to exactly one main limb; there are no multi-limb sessions. Switching to another project's context means launching a fresh subagent in that limb, because a limb may carry load-bearing instructions in its AGENTS.md files — "so to do something in that context, you MUST do it via an agent in that context" (`source-notes/agent-harness-design.md`). His own phrasing of the soul item is the same claim from the other side: "a session exists with respect to one limb. a limb provides tools but also context etc."

A **context** is what actually gets sent to the provider for that session: a **system section** — system prompt plus tool schemas (decision 2026-08-12, "rebuild" veto) — followed by messages. A session keeps a fixed context and tool set for as long as the cache is valid, and the context is persisted so the harness can restart without losing it (`source-notes/agent-harness-design.md`).

A **context contribution** is anything that goes into a context: skill content, an AGENTS.md layer, a tool description, an option set, a notice, user activity. "Identity of a context contribution" is his term for what a notice or a content-version record points at (decision, identity of a context contribution).

Contributions come from **data sources**, of which a limb is one. There are also machine context, user context, maybe face-specific context, and probably the user-turn stream. One data source serves many sessions, forked sessions especially. So a skill can have several possible sources, and limb-local content is just one case (decision 2026-08-12, contributions come from many data sources).

There is a computation graph over those sources: "there's a computation graph. we ask it for the 6pm context. it gets built for us." Nothing hands the graph a view of the world; the graph fetches what it needs. Demand stands for the length of a turn — "while the agent turn is going, there's constant demand - we're streaming live updates so that the latest notice set is immediately ready to piggy back on the next request" — so nothing waits at request-assembly time, and a session with no turn running generates no demand and therefore no cost (decision 2026-08-12, computation graph and standing demand).

Consistency at request build is a cut, not eventual consistency. Render and send only once the derived data covers a point at or beyond the trigger across every data source, so a request never mixes one source's 3pm view with another's 1pm view. This is vector-clock logic; "We don't necessarily have to literally implement that... but that's the logic behind what we want to do." The tool-call loop counts as another data source (decision 2026-08-12, consistency is a vector-clock cut).

## The context lifecycle

### Three operations

The word "rebuild" is vetoed, because "rebuild is not 'build a new context'!! rebuild is 'transform an existing context'". The three operations, in his wording (decision 2026-08-12, "rebuild" veto):

- **Initialise** — "this refers to the system prompt only - and happens on new sessions, on compactions, and yes, on refurbishments."
- **Refurbish** — "transform existing to reduce token count, but NOT compact - the messy one". Done with only regular code, and maybe utility model calls. Old notices get rolled in, "ideally even without a compaction".
- **Compact** — make a fresh context, paying model output to shrink drastically.

Those are also the three levels of context maintenance, cheapest first — "the original vision": "keep using warm context; rebuild [refurbish] existing context (incorporate notices etc.); compact (make fresh context)." The rungs escalate by who does the work: nothing, then code (with an optional utility model), then main-model output. Build for correctness first, then choose the cheapest option within that (decision 2026-08-12, hedge his: "we build the harness for correctness, then choose the cheapest option within that?").

Two consequences of the veto. The system section can only change by an initialise. And initialising an existing context happens only as part of a refurbishment — otherwise the change waits for a compaction into a fresh context. Two unrelated uses of "rebuild" survive and are not this concept: the walking skeleton's `/rebuild` of in-memory state from its journal, and the harness rebuilding its own binary.

A **utility model** is a small cheap model — his example is Claude Haiku — used inside harness logic for classification or summarisation. Refurbishment may call one (decision 2026-08-12, utility model may classify and summarise; refurbishment is code-only).

### Cache prefixes

A context has several cache prefixes at once, nested. "Prefix" is not just the system section. His list: "system section (system prompt, tools, etc); system section + messages _up to the last fork boundary_; system section + messages _up to .._ + all subsequent messages." A forked session has many. For user-facing messages there is additionally a prefix at "_everything up to n-2 messages ago_ (or something like that)" so that message undo lands on a warm prefix (decision 2026-08-12, several nested cache prefixes).

While the context is believed cached it is append-only: editing any earlier byte forfeits the prefix, which is why a stale fact cannot simply be corrected in place. Conversely, "If we expect a cache miss, then there's no reason to not optimize the context somewhat" — which is what makes refurbishment the natural companion of re-initialising (`source-notes/context-and-agent-loop.md`).

Superseded contexts are stored directly: "in practice we will probably just store the context directly. That is much simpler than trying to reconstruct it deterministically from raw events. While full determinism is a nice aspiration, it feels overly ambitious and not important enough to justify the complexity." There is one stored context per warm cache point. The append-only cache is "effectively a branching structure" — suffixes may be discarded where a cache point can be predicted, and "That is why forked sub-agents work at all" (decisions 2026-08-12, superseded contexts stored directly; stored state per warm cache point).

Prices to design against, from the Anthropic prompt-caching documentation and **not yet observed against a real API**: cache write 1.25× base input at the 5-minute TTL, cache read 0.1×, and breakpoints themselves free (`PLAN.md`, provider-cache-probe). Model output is ~5× — his figure, "admittedly output so 5.0x" (decision 2026-08-12, the fork model subsumes tail pruning). The provider also "automatically identifies and utilizes the longest previously cached sequence", so the harness places breakpoints but never selects a prefix. Verifying all of this is the `provider-cache-probe` experiment's job.

### What triggers a request

Only four things, and this is invariant 2: the agent tool-call loop continuing, the user ending a turn, cache-nearly-expired proactive handover or compaction, and explicit resume (`source-notes/context-and-agent-loop.md`).

Everything else piggybacks — appended now, carried by the next request that was going to happen anyway. His own list of piggyback-only events: user opens a file, user edits a file, user searches, user terminal output arrives, tool schema changes, process config changes, the client app reconnects, sibling or child agent status changes. If a request is already in flight, the activity goes out with it, alongside the tool result. If not, it stays queued until the user submits their turn. A quiet session therefore costs nothing (`source-notes/context-and-agent-loop.md`).

### Notices

A notice is an appended message telling the agent that a contribution it holds has changed. It carries the bare minimum needed for the agent to invalidate its current understanding — to know that viewing the new content is an option, or to explain a missing tool (`source-notes/context-updates.md`).

**Whether to notify at all** is decided by actionability: "if it would not change the agent's actions in any way, then it doesn't need to know!" Content the agent was never exposed to gets no notice — a first load simply gets the new version. Actionability is the logical condition, but "there's a practical constraint on actually knowing whether free-text content update X affects session Y or not" (decisions 2026-08-12, actionability; change notices are economised; per-element).

**How much the notice carries** is a separate decision, and its name is "detail". It "trades off against distraction (task quality) & economics" — so a bigger notice is not merely more expensive, it can degrade the agent's work on its actual task. All else equal, the minimum, but "not necessarily - it still depends on the economics & the agent's reaction." A reference is not literal and may carry less than a name: "It could even be something like 'one or more skills have gone stale.' (although that is too un-specific probably, it gets across the idea - we don't neccessarily need to list 10 skills that have updates - we might elide even that info as long as we give the agent a way to re-discover reality reliably & cheaply ie. it shouldn't have to reload everything just to be sure)." Whichever form is chosen, "there always needs to be a clear, reliable path to that information" (decisions 2026-08-12, "detail" trades three ways; "a reference" is not literal; why small notices are safe).

**The economics is the fundamental**, and it is a contingent choice: a reference means unconditionally smaller input plus conditional billing of an extra turn in the branch where the agent fetches the detail; the content itself means unconditionally larger input at input cost with no extra turn. Which side wins depends on how often that branch is taken. Progressive disclosure at session start is exactly the same choice — descriptions up front, content on demand — though he calls the connection "a connection, not a fundamental". Economics decisions of this kind typically require empirical measurement or an experiment rather than derivation (decisions 2026-08-12, notices are a contingent economic choice; economics needs measurement; `source-notes/context-updates.md`).

**Notices are rendered at request build, not appended at detection.** Generalised by him: "we compute + lock notices & other piggybacked contributions as we build the request" — so the rule covers every piggybacked contribution, not just change notices. Detection therefore records only change _facts_ (element, actor, when) as durable events, and the request builder compares current sources against the session's content versions and renders the notice block then (decision 2026-08-12, notices rendered at request build). Four things fall out for free: repeated edits coalesce into one notice describing the latest state; an edit reverted before the next request produces nothing; elapsed time is computed at delivery by construction; and there is no pending-notice queue to recover after a restart.

**Detection compares content, not timestamps.** "not just time, hash (or frankly just equality - hashes are for when you don't want to keep the content itself around) is better." And "you don't always need to actually say anything" (decision 2026-08-12, change thresholds for content notices).

**Provenance is worth carrying.** The harness reliably knows only three buckets — changes it caused itself, git state, and unattributable external edits — and giving the model that information is "potentially _very_ useful" (decision, provenance buckets are useful to the model).

**Voice matters.** Harness voice carries ground truth: the time, or "the AGENTS.md contains this content". But content the harness is _showing_ is not in the harness's voice; it stays quoted content (decision 2026-08-12, harness voice carries ground truth — hedges his).

**Option sets absolutely get notices.** They live in the context but deliberately outside the JSON schema proper, precisely so they can change without a schema change: "that's the whole _point_ of not putting them in the schema proper - and so yes they absolutely get notices!" His examples: "skill names are options for the skill tool! subagent names are options for the task tool!" (decision 2026-08-12, option sets absolutely get notices).

**Tool schemas sit outside all of this and have their own correctness logic.** "changed tool schema never gets a notice, as discussed." Either the tools change underneath, in which case a notice is not wise or possible and a refurbish or compaction is forced, or they do not change underneath — which is achieved by keeping the old ones around until the next initialise. "Tool schemas" always included the tool set: "Tool removal is a breaking change to the tool schema", so removal almost certainly requires a refurbishment or compaction rather than a notice (decisions 2026-08-12, tool schemas are outside notice decision 2; "tool schemas" always included the tool set). **Supersedes `source-notes/context-updates.md`:** "Tools with changed schema need full content injection."

The limb must therefore continue to accept old tool calls "until all sessions that used those old tools have reached cache expiry (and so would be rebuilt [re-initialised])", which means retaining two or more versions of the tool implementation while any such session exists. The only bound on the obligation: "is there ever going to be another use of this tool code version, or not?" (decisions 2026-08-12, the limb must continue to accept old tool calls; the only bound on honouring old tool schemas).

Waiting for the next initialise is safe in exactly one of two ways: "to deal with correctness, we _keep the old tools working_. or it might be stuff that doesn't really affect correctness" (decision 2026-08-12, waiting for the next initialise is safe in one of two ways).

That logic generalises past tools. An old context contains old info; where that matters for correctness it needs notices or a refurbish/compact. Because tool schemas and tool presence cannot be fixed by notices, and because old tool code versions cannot be kept forever, a refurbishment or compaction may have to be forced "_if the context contains tool description content that is stale in a correctness-affecting way_. Importantly, the same logic would apply to any _other_ things, perhaps for example _subagent_ description content?" (decision 2026-08-12, the cold-context logic generalised — hedges his).

Per-element, in his words (decision 2026-08-12, per-element decisions): skill content notifies "only if loaded". Skill description does not — no is "a safe general rule", because "skill descriptions changing is unusual without skill content changes too, and descriptions are not usually load bearing". A new skill notifies "only if it would be available". AGENTS.md and other limb context is "almost certainly yes" — "that feels more like a contract to me. It's technically the same though." Limb identity requires a fresh initialise. Elapsed time is its own case: it needs a threshold, because every request differs, and past that threshold the notice carries the value itself — "This is important to have an agent understand how long between its response and the user message. It can be many weeks in some cases!" (`source-notes/context-updates.md`). The threshold is in Open questions.

### Compaction and handover

Call it a **handover**. The word "naturally implies that all relevant information must be passed forward — goals, progress, decisions, blockers, next steps", and in practice that framing produces better results than "summarise" or "compact". Compaction is lossy, so the quality of what is kept and discarded matters enormously; the most important thing to preserve is the goal and the current place in the overall work (`source-notes/compaction.md`).

Handover happens, in general, by instructions appended to the end of the conversation — "ideally system parts - depends on provider / model support" — so no cache break is required. It is structured rather than a free-text summary: the agent can include files and resources that load immediately into the fresh context, saving turns and their cache-read round trips at the start. And it must be clear about the new situation: what is changing, what will still be in the system prompt and so does not need repeating, and what will not be there anymore and so does need keeping (`source-notes/context-updates.md`; `source-notes/handoff-improvements.md`).

Compaction has four trigger kinds, three of them forcible (decision, compaction has four trigger kinds — replacing an earlier invite-only model). The agent triggers at a milestone. The harness forcibly triggers on the context-window limit — "something like 80-85% (or better, a fixed token threshold like 100k-200k tokens)" — and on cache expiry while the agent is idle, where an in-flight tool call is "rewritten as still in progress, and execution continues seamlessly when it completes". The user can also forcibly trigger.

Compact-and-continue and compact-and-report-back are the only two situations, and report-back-to-user and report-back-to-parent are the same flow. The predecessor writes the report: "lets the first agent build the report as well as their compaction summary. And then the compaction summary deals with the initial context of the new agent, and the report is given as if it was its first message. I guess. something like that." (decision, compact-and-report-back — hedge his).

An uncached compaction flow is also needed: "we can't always assume the original model is available or desirable to do a compaction. We may want to have an old-style compaction flow - an 'uncached compaction' with a traditional 'compaction agent' system prompt as well as our additional context." (decision 2026-08-12, uncached compaction).

The fork model subsumes tail pruning. Bulk that would otherwise need pruning is generated in a child context and returned as a report, so the parent never carries it: "I think the forked agent design gets around this by subbing the tail with a (admittedly output so 5.0x) task summary / report (kinda a compaction)." A per-tool-call summary field was considered and priced out — "The summary is not input, but output. Which is even more expensive! So yeah it seems pretty unlikely to be net positive, actually." (decisions 2026-08-12, the fork model subsumes tail pruning; refurbish-time coalescing and a tool-call summary field).

### Recording and introspection

Tool facts are recorded by both brain and limb, split by ownership. The brain records context facts: a call detected in a response, a result entering the model view. The limb is in charge of the actual execution (or not) of tool calls and records the execution facts; environment facts like hostname come from the limb (walking-skeleton gate ruling).

A proposed-but-unexecuted tool call is valid, resumable state. On cancel, wait for the in-flight response and keep it, but do not execute its proposed calls. Unexecuted calls get no fabricated outcome, are omitted from the wire — the model never sees a call that never ran — remain visible to introspection, and may be executed on a later resume. Executed-then-cancelled calls do get their cancelled result on the wire, preserving exchange adjacency (walking-skeleton gate ruling).

Introspection is first-class: an easy way to see the ~exact text as the model sees it (`/dump`), with everything the model cannot see marked as such. The request builder and the dump share one projection, so they cannot diverge (walking-skeleton gate ruling). `model` and `reasoning_effort` are request facts, not context facts — refined by him afterwards: mechanically that is right, but "I think we do want to tell the model which model it is. But yes, changing it invalidates cache so we do rebuild [initialise]." (decision 2026-08-12, model: he challenges "not a context fact").

Analytics is a retention question, not a queryability claim. Recording events does not make analytics free: "analytics should be about _what_ do we keep, _for how long_, and especially _why_. 'keep everything' is an option but not an answer." (decision 2026-08-12, analytics is a retention question). What is wanted from it early: token usage across all API requests by session, searchable session messages and data, a read-only tool surface or limb for meta-work, timesheet-grade data, and queries that span all connected brains — "I'm ok with it just being sql queries" (`source-notes/analytics.md`).

## Subagents

The hierarchy uses structured concurrency (invariant 6; `source-notes/agent-hierarchy.md` throughout this section unless noted). A parent agent blocks when it launches children and resumes only when all children have finished.

A **scope** is created when a parent blocks. Every task has a local owner — the parent — and there is no global spawn context. There is no "launching into a child scope": you either block, creating a scope with children, or launch a sibling into the current scope. Agents can launch siblings into their own scope, so a parent can gain a child it never explicitly anticipated. Agents can see the _status_ of their siblings, because they share a scope, but not their _results_ — results reach the parent only when the whole scope completes. Agents cannot see the status of their own children at all, because they are suspended until every child completes.

A **result** is the last message part in a turn. Prompting decides what goes in it, ideally "what the parent needs to know". Failure counts as completion with an error result.

**Fork versus fresh.** Fork by default: children start from a copy of the parent's conversation context, which is good for KV cache reuse, and stay in the same limb. Fresh is required when crossing a limb boundary, and is also valid within one limb — for example spinning out a meta or global session in the home directory context. So: different limb always fresh; same limb fork by default, fresh allowed. Fresh wins over forked-but-stale when the task is narrow and focused, because that is cheaper than a long-context cache miss if the new agent ends up needing less context. Forked subagents are trying to be append-mode subagents with respect to the parent, possibly as of the message before it sent the subagent tool call (`source-notes/context-and-agent-loop.md`) — exactly what prefix a fork inherits is an experiment.

**The Task tool** launches a subagent. Parameters: `task`, the prompt or instructions; `agent_type`, which agent persona to use; `user_facing`, whether the session is user-facing or autonomous, kept as a separate orthogonal parameter even though it is informally conflated with agent type; and `context`, which limb to run in — omitted or `"self"` forks in the same limb, an explicit limb id or type gives a fresh session in that limb (including `"global"` for a home-dir or no-project context), and the same limb id given explicitly gives a fresh session in the same limb.

**The Resume tool** continues a previous agent session as a new subagent, taking the `id` a previous Task call returned on success or on error.

**Attachments** are how a launch avoids repeated work (`source-notes/handoff-improvements.md`). The agent calling `task` can attach tool calls — file reads, skills, searches, maybe command output. These are all executed in one "init" step, appear to the subagent as normal tool calls, and are shared across all parallel subagents. That is also what makes the shared-seed path work: start one subagent with only the shared context and tell it to wait, so the first API request establishes a shared cache, then send different instructions to different branches so they all share the prefix. The handover tool takes attachment arguments for the same reason — attaching a file beats spending output tokens re-summarising it, and beats every parallel subagent reading it separately. Three paths result: a forked task (forks the parent), a fresh task (shared seed context established by an initial API request, with forked instructions as follow-ups), and compaction/handover (the same structure as the fresh variant — context, attachments, task). The task tool's prompt should make the parent act as a good router between fork and fresh: is the task a natural continuation, is the parent's context bloated, do the children need only some of the context, or a different limb?

**The main-thread pattern** is how a parent keeps talking to the user while parallel work runs. The parent forks a user-facing child, so from the user's perspective the conversation simply continues. The parent also launches one or more autonomous siblings into the same scope. The user-facing child runs the interactive part while the autonomous siblings run in parallel. The user calls `/done` when satisfied, and that child completes. Siblings may still be running; the scope completes when all of them are done. The parent then resumes with all results, and does not see intermediate content — only final responses.

**User-facing versus autonomous sessions.** A user-facing session completes when the user signals `/done`, after which the agent writes its response; an autonomous session completes automatically at the end of its turn. Launching a user-facing session requires user permission; autonomous siblings can be launched freely by any agent. Multiple user-facing sessions are supported and appear in a session switcher, and the user has a button to launch one into a scope that does not currently have one.

**Naming.** Auto-generated short descriptive names, for example `tk-prodsync-findfiles` and then `tk-prodsync-findfiles-refactor-imports`, so names lengthen with nesting naturally. The user can edit a name when spinning out a user-facing scope.

**Brain-owned tools** are exposed to agents regardless of limb, because they are not limb-provided: view session history, view the status of sibling agents, Task, and Resume.

**A forked agent must end its turn at its assigned subtask.** A deeply forked agent is given task A, then told to focus only on A.1, then only on A.1.3. "But importantly, it must not continue on to complete A.1.4 or the full A.1. It must end its turn after A.1.3 - this must be reliable for forked agents to work well." Highly forked agent threads are not empirically tested and will likely need careful model-facing framing.

## The user turn

The thesis (`source-notes/user-turn.md` throughout this section unless noted): instead of the user sending messages and asking the agent to show things, the user works in-band — opens files, edits them, runs commands, searches — and the outputs attach to the conversation exactly as if an agent had used a tool. "This way I can actually work too! And the agent gets to see what I've done. I don't have to worry about the overhead of telling the agent that I made 'out of band' changes."

**Two projections, and the tool owns both.** Every tool has two outputs: the UI and actions for the user, and a transcript, written live, that shows the agent what the user is doing, framed clearly as useful context on what the user is doing. It is the mirror image of the view the user already has of the agent's tool calls. The corollary runs the other way too — agent tools should provide UI implementations, for both web and TUI. "The tools own both projections - so the user tool owns both the UI and the context compression / projection. Same for agent tools." This is one instance of invariant 3.

**The context projection is not the UI.** It should carry all of the same important information, "including what the user saw but _didn't_ use", and can exclude purely visual or irrelevant information, or intermediate states the agent does not need. More strongly: "All of the information that the user used to make their decision should ideally remain available to the agent, albeit in as minimal a form as possible, NOT just the decision outcome." The goal is that "the user and the agent should be on the same page about the history, not that they should see exactly the same stuff."

**User tools are not agent tools.** "we should make sure that the context is clear to the agent that it has a different tool set to the user (we don't want the agent trying to use the user's tools)."

**None of this triggers a request** (invariant 2). If a request is already in flight the activity is sent with it; otherwise it stays queued until the user submits their turn (`source-notes/context-and-agent-loop.md`). User tool calls are multiple message parts, and multiple user message parts do go to the model rather than being merged into one — he kept that distinction because the model "probably sees them differently".

**How much user context to carry is a sizing question, not a choice between two positions.** "this is not about a versus b. It's about how much a versus how much b." Input is cheap compared to output, and compared to repeated tool calling — and it is the second comparator that justifies carrying looked-at context at all. He also observed that user activity piggybacks on turns that would have happened anyway rather than creating new ones, while noting he had not fully settled the point (decision, on carrying user-turn context).

**The tools.** A **file tool**: opens a file explorer, selecting a file opens an interactive editor, tracks changes as a diff and — importantly — tracks what the user _looked at_, including the file explorer and any find commands; files open fully collapsed. A **terminal tool**: run a command in a terminal, persistent rather than ephemeral ("probably"), and ideally not an _actual_ bash or fish terminal so that it can be forked and undone along with the message history — that last part is "totally a stretch goal". A **REPL tool** works the same way. A **search tool**: shows the history of what the user searched for and what they found. A **GitHub tool**, "probably": an interactive `gh` terminal for PR description, comments, reviews and diff, ideally an existing tool integrated — though tracking what happens inside it may mean forking that tool, which goes for other integrations too. And a **subagent tool** — "one obvious user tool is a subagent tool!!", for example "find me that nix issue where XYZ", with the user's prompt and the subagent's response both included; it supports forked and fresh agents, and warns when the cache has likely expired while leaving the judgment to the user, "note that forked can still be cheaper even if cache expired, if the model would have to do many sequential tool calls to get back up to speed". For that tool the context carries what the user saw, not what the subagent saw. The general bar: "We need to support as many tools as needed to allow the user to fully make decisions 'in band'."

**Entering a tool.** The main harness view stays a chat UI, and a hotkey drops the user into a tool: `$` immediately opens the terminal view ready to type a command, and `esc` back to the harness view leaves the `$` in the terminal so the user can still type `$` normally. `@` for file. Nothing chosen for search.

**Keybindings** (user, 2026-08-03, recorded in `PLAN.md` under user-turn): "I want shift enter (kitty escape seq, configured in my win terminal) to be newline, enter to stage, enter with _no_ content to submit, and control enter (if possible) to be submit too." **Supersedes `source-notes/user-turn.md`:** "In this harness, pressing enter would not automatically send the message. it would be ctrl+enter. Not sure whether there should be a distinction between enter (send message?) and shift+enter (new line in existing message)."

**Conflicts: the user wins**, and stale agent output never silently overwrites newer user work (invariant 7). Beyond that, restraint: "we might reject an agent's updates to a file if the user currently has it open, or has edited it since the user last did so. I don't think we should be too eager about that. Maybe only if the updates actually conflict. We don't want to overprescribe live collaboration. The fact that the agent gets to observe the user is already a massive win."

**The activity trail's order is the face's own recorded order.** Every face event carries a front-end and a back-end time anchor — "It's after this time on the front end. It's after that time on the back end... a partial order. Sure. But that's not to say that the face doesn't have its own total order and that we can't remember that" — so "in the order things happened" is an ordinary recorded fact, primarily a representation question (decision, the activity trail's order).

**Not just a TUI.** "This concept is NOT just for a TUI - it's for a harness which could later also be a GUI. We build GUI/web support in from the start, even if unimplemented. This would be more natural in fact as the user may commonly want to use a web browser."

## The box: how harness code is written

Doctrine for all harness code, not one feature's constraints. Its purpose is that an implementer has no room for a major wrong decision: "there's stuff inside the box, then there's building that box. But we should put as much as possible in the former." (decision 2026-08-12, the "what" stage defines the box).

The bar for the box being defined at all: "an implementer could write the code without leeway for major wrong decisions on important behavioural aspects. It obviously still needs to invent stuff. But we should have broad guidance about how the data should be modeled, how the code should look & feel, and especially _defining the box that the implementer should work within_ - if the implementer uses too many capabilities, they go outside the box... Eg. did they write it as a pure dataflow? or did they write it as imperative logic with I/O? Does write data? or does it leave that to be done by some other kind of layer? Does it read the current time? or does it take it as a parameter?... Truly good code is written for the minimal capabilities & runtime." And: "'Well behaved' code is the goal, and it may be highly counter-intuitive." (decision 2026-08-12, the "what" stage defines the box).

**The ideal module**, his words: "takes a snapshot + a batch of events over each input dimension and produces its own snapshot + events. It is pure, has no I/O, cannot see anything except its declared inputs, and does nothing except produce its declared outputs." (decision 2026-08-12, the "what" stage defines the box). Purity is why "elapsed time is computed at delivery" is not a rule anyone has to remember: at detection there is no clock to read.

**Everything I/O has an in-memory implementation.** "We should never _have_ to read a file from an _actual_ filesystem, or send a packet across a TCP socket. We should be able to use some kind of hash map backed tree structure, and a lightweight channel (with maybe some delayer implementation for network robustness tests)." This applies to the layer that builds the box as well as the logic inside it: "the box-building layer still gets in-memory e2e tests! The point is to remove system call overhead & blocking calls. The box-building layer should still accept I/O interfaces with in-memory implementations." (decision 2026-08-12, every feature gets a scenario test).

**No ad-hoc boundary breaking.** The in-memory property only survives if code never reaches around an abstraction for convenience. A strict requirement for core, "but not necessarily for experiments" (decision 2026-08-12, fast end-to-end test definition).

**Structured values until the last step.** Rendering to text is a separate projection, shared with `/dump` so the two cannot diverge (walking-skeleton gate ruling).

**Thresholds are recorded tunables, not designed constants.** The ~1h elapsed-time threshold is "discoverable, that's my naive guess" — a tunable (decision 2026-08-12, the ~1h elapsed-time threshold).

**Language split.** "the data source / data flow framework is rust I think. but everything within it should be deno but TS not JS." So the framework — sources, cut, transport, request assembly — is Rust, and the logic inside it — notice policy, projections, ladder choice — is TypeScript on Deno (decision 2026-08-12, language split). The wider bet behind that: Rust for performant async, safe multithreading, structured concurrency, easy binary wire formats and native compilation; Deno embedded for almost all business logic via plugins — tools, providers, user tools, and maybe limbs — kept in a hard sandbox with authentication implemented outside the plugin, so a provider plugin gets a pre-authenticated fetch wrapper rather than the credentials (`source-notes/tech.md`).

**The box is the convex hull, not only the pure core.** "the point is to nail down the convex hull of the feature across many, many dimensions. Some of those are the 'inner box', but there's more." Real I/O has to be built — reading the time, reading context files for an ordinary filesystem limb, emitting new versions. Topology is part of it: "it shouldn't just be one node - first of all, it's one reader per limb, one notifier per session / context, etc." So is data lifecycle: "what's stored & what's ephemeral (if anything), what the data lifecycle is (when do we drop the data, if ever), and more." (decision 2026-08-12, the what covers the convex hull).

**Experiments have latitude.** "experiments do not have to behave to all the constraints - that's probably too hard, and is what core integration is for. Experiments should still try to behave where doing so would be _hard_ - up-front the difficult innards while validating the important surface." (decision 2026-08-12, experiments need not obey all box constraints).

### The two kinds of test

Defined by him, 2026-08-12:

- **Scenario test** — "a fully black box end-to-end test including setup, probably UI eventually. Uses real I/O as the boundary. Proves real usage, forces that the harness is itself harnessable for tests (eg. forces observability features useful for testing), can be slow (but should never be flaky) and is a happy path test mostly." Every feature gets one.
- **Fast end-to-end test** — "uses in-memory I/O, a whole distributed system in one process, relying on solid abstractions and requires we never do any ad-hoc boundary breaking when programming the harness code (note this is a strict requirement for core, but not necessarily for experiments)."

Timing is virtual — "although the scenario test can take real time, but should not have dumb waiting/polling still - everything should have an event it waits for". The delayer channel exists for fake network conditions in in-process distributed-systems tests, not for reordering robustness: "re-ordered data should NOT be a problem, a data source should NOT do this."

These two definitions and the purity doctrine above are workspace-level doctrine shared by every tool ("yes, excellent!"), so they belong in the `agent-tools` workspace docs and should be referenced rather than restated. TODO: place them there (decisions 2026-08-12, every feature gets a scenario test; convex-hull definitions imply further shared docs).

Durable tests assert only at the product-public surfaces — CLI and UI behaviour, the provider wire via the fake provider, the durable storage and query surface, and eventually the transport protocol — never on internals (process ruling). The fake provider is a separate HTTP server serving the same OpenAI-compatible API, so real versus fake is only a base URL, and real provider use is in scope for experiments: "I want to actually use it". A flake is a bug: races are structurally excluded, not made unlikely or retried away ("test flake is a bug. make sure to make it impossible." — walking-skeleton gate ruling).

### Structured lifecycle

No detached tasks or threads, and no `process::exit` escape hatches. In-flight work is owned — identity, cancellation and join handle together — and always joined. Participants return Results, and failures fold into the exit code. Every layer shuts down what it owns gracefully; parent-held timeout backstops and descending deadline budgets are a recorded pattern, deferred (walking-skeleton gate ruling).

Anything the user has to read is written decompressed, not short: "word count is not expensive because I have a very fast reading speed, but word depth is expensive because I actually have quite a slow mental speed." (decision, write decompressed, not short.)

## Open questions

What is not settled. Quoted with the hedge intact, because the hedge is the information — nothing here becomes a line above without him settling it first. Where an experiment answers a question, the experiment is named; the pool is in `PLAN.md`.

**Provider and cache behaviour** — all `provider-cache-probe`, pre-approved 2026-08-04 ("Yes, definitely. That is a great, well-scoped experiment."):

- Whether anything about tools can change _without_ involving the cached prefix. He calls this the real question and the reverse of the one usually asked: "is it ever possible to change anything about tools _without_ involving the cached prefix. That's unanswered."
- Whether mid-session tool addition works at all via append: "I'm unsure about whether tool addition works robustly at all via append (ie. without breaking prefix)". Separately, on the notify decision for it: "I'm not sure if new tools work yet or not. Seems fine to me?"
- Whether providers validate tool arguments against the advertised schema: "no, highly doubt it? but yes, unsure."
- What prefix a forked child actually inherits, and whether it is the parent as of the message before the subagent call (`source-notes/context-and-agent-loop.md`: "that needs experimenting too").
- Whether a utility model can read a prefix cached by a larger model at 0.1×: "it's potentially possible that a utility model (eg. claude haiku) can re-use the same prefix at 0.1x cost. This should be confirmed empirically - if true, it's useful." His recorded derivation: the fallback — a one-shot task with a cached system prompt, just enough context, and a one-word answer — is ~two orders of magnitude cheaper than a 0.1× read of a 100k prefix on a large model, so the one-shot is the design regardless of how this resolves.
- Real TTL behaviour and observability, whether late system parts are supported per provider, and the same questions again on the OpenAI responses API (`PLAN.md`).
- Whether cancelling an API request after the first byte avoids the charge (`source-notes/analytics.md`: "probably worth experiment - does long thinking process actually get interrupted on the providers server or do they charge for the whole thing?"). `cancellation-economics`.

**Context lifecycle:**

- Expired ("old cold") contexts. Constant across his takes: on load it is neither refurbished nor re-initialised, and it is the event log of that agent session. His current position and his own verdict on it: "I think ideally we just keep an 'old cold' context and make it an 'old warm context', append some (perhaps copious, but oh well) notices, and keep going. That is the ideal state btw. it's minimum cost - we get re-billed at input to compact, we might as well up it to cache write & _not_ compact? Perhaps an option for the user? I don't think this is settled. Tool schemas for tools that will no longer work is a great reason to force the compaction, though."
- The elapsed-time threshold: "discoverable, that's my naive guess". Also the debounce window and any utility model's confidence bar.
- Whether a refurbishment is always "compact immediately prior": "tbc if rebuild [refurbishment] is always 'compact immediately prior' or not. I think it is, if our compaction is good."
- Refurbishment's design: "this needs to be designed still because it's a bit odd, because it mixes 'event stream' and 'rollup' in a messy way." Including what it coalesces: "when rebuilding [refurbishing] a context we can coalesce notices into the system prompt & elide edits where we have a later read, etc? I think that's reasonable. TBH I haven't thought about context rebuild [refurbishment] much here."
- Where pruning may happen: "pruning is for either a rebuild [refurbishment], or rewriting pre-cache, in a fixed-size suffix maybe (fixed number of messages or token size whichever is larger)." And on the suffix variant: "I'm not confident on 'prune within suffix', it implies re-sending uncached content over and over, but on the other hand it also implies much smaller context & cache write due to tool call pruning / coalescing. Again, an economic decision."
- Rename of a contribution. Under the identity model a rename is a delete plus an add, so the agent sees "skill gone" plus "new skill". Genuinely confusing; open (decision 2026-08-12, identity of a context contribution).
- cwd and hostname: "hostname change can be legit. maybe a limb can relocate too? not sure... also - not every limb has a cwd."
- Whether agents overreact to change notices — wording and frequency. Needs empirical testing, and is "also related to the user-turn stuff" (decision 2026-08-12, overreaction to notices needs empirical testing). `context-updates`.
- Whether a utility model is a viable actionability classifier, and at what cost: "there's one more thing to consider - again only if the economics justifies it - passing these things through a 'small model' or 'utility model' for better quality classification & summary." (decision 2026-08-12, a utility model may classify and summarise — hedge his.)
- Whether notice framing is a security concern: "not sure about this, but yes... tbh I think this is all relatively obvious to the agent. but the channels or roles do matter, but also I just want the agent to do what I want. I don't have an authority model across multiple users or whatever." (decision 2026-08-12, harness voice carries ground truth — hedges his.)
- The compact-and-report-back shape: "I guess. something like that." (decision, compact-and-report-back — hedge his.)
- Restart cache affinity beyond reproducing the same prefix — "any other surrogate ids or whatever that refer to cache affinity or cache points" — noted as a dependency, not designed (decision, restart cache affinity is a separate doc's problem).

**Subagents and shared state:**

- Shared mutable state under concurrency. Fork-by-default plus parallel siblings plus one limb means several agents on one filesystem, and "experience with multiple agents working on the same codebase simultaneously is poor". Possible directions: clear instructions telling forked agents what scope of the workspace they own, or "Some kind of borrow-checker-style rule on mutable workspace regions (tentative, not designed)". "Not resolved. Needs careful thought." (`source-notes/agent-harness-design.md`, `source-notes/open-questions.md`.)
- Whether one child failing aborts the scope or just propagates an error to the parent — "needs experimenting" (`source-notes/agent-hierarchy.md`). `forked-subagents`.
- Whether a session can be resumed as a different agent type, and whether that breaks cache badly enough to matter, or whether behaviour should depend on whether cache has already expired. Open (`source-notes/agent-hierarchy.md`).
- The stale-cache case, where a fresh subagent wants forked siblings but the parent and all forks are stale: "open question, tentatively lean toward fresh for narrow tasks" (`source-notes/agent-hierarchy.md`).
- Agent types are loosely OpenCode-style personas and are informal: "Not yet fully specified." (`source-notes/agent-hierarchy.md`.)
- Whether only user-facing agents may launch user-facing sessions is a "Tentative rule", along with its escape hatch: "permission requests could expire after a timeout, so even a user-facing agent's request doesn't block indefinitely if the user is absent" (`source-notes/agent-hierarchy.md`).
- A stuck child or abandoned user-facing session blocks the parent scope indefinitely. Intentional that the user is responsible, but the permission prompt preceding it is a known UX pain point, and "Expiry as escape hatch is noted but not decided" (`source-notes/open-questions.md`).
- Main-thread child naming: inherits the parent name with a suffix such as `'` or `*` — "not yet decided" (`source-notes/agent-hierarchy.md`).
- The optional pre-step of a temp fork whose only job is to write a good task prompt: "idea, not decided", because it adds latency and cost to every Task call (`source-notes/agent-hierarchy.md`).

**User turn:**

- Which hotkey opens search: "Not sure what for search." (`source-notes/user-turn.md`.)
- The forkable, undoable terminal and REPL: "This is totally a stretch goal though." (`source-notes/user-turn.md`.)
- Recording and transcribing the user's voice while they work, so talk-while-working is attached too: "Ideally we might also record/transcribe" (`source-notes/user-turn.md`).

**Roles, limbs and topology:**

- The name "limb" itself. Candidates: workspace, runtime, context, environment, sandbox. "Not yet decided." (`source-notes/agent-harness-design.md`.)
- Context layer composition. The provisional layer types are user-specific, user-and-machine-specific, machine-specific, project-specific and user-and-project-specific. Open: how layers are discovered and composed, precedence and merge order on conflict, how the brain knows what the limb injected, whether layers are declared to the brain or opaque, how this interacts with skills and extensions, and whether a "brain-local limb" for user config blurs the brain/limb boundary problematically. "This area needs significant design work. Deferred." (`source-notes/configuration-model.md`.) `limb-model`.
- Limb-local context opacity. The limb owns what reaches the model from its domain; the brain owns cost, reliability and debugging. "Can the brain manage those without visibility into what the limb injected or filtered? Not yet resolved." (`source-notes/open-questions.md`.)
- Whether same-machine IPC is close enough to sequence, and therefore whether appends across a process boundary are synchronous or asynchronous. Explicitly unresolved (walking-skeleton gate ruling); deferred to the event-streaming experiment.
- Session storage blast radius: claimed not to be deeply entangled with the agent loop, but the hierarchy model makes storage semantics central to concurrency, blocking, resume and compaction. "May be larger work than assumed." (`source-notes/open-questions.md`.)
- Extension compatibility: stock Pi extensions may break behaviourally even with an identical API surface, because tools now execute remotely, state lives elsewhere, and context is shaped by the limb. "Risk acknowledged, not mitigated." (`source-notes/open-questions.md`.)
- Principled credential handling and Anthropic OAuth for a Claude subscription (`source-notes/anthropic-oauth-references.md`). `oauth-credentials`.

**Process and packaging:**

- The eventual user-facing command name (`HANDOFF.md`).
- Whether the modular-components work is its own library: "Perhaps this would be its own library in the agent-tools ecosystem though, actually - it's useful for all my projects." The experiment should produce evidence for that call rather than presume it (`PLAN.md`).
- A mechanism for agents to tune the harness empirically. Hedges his: "compaction economics, cancellation economics... and forking economics all feel like empirical domains. I am not very strong in this area, so I would prefer a mechanism for agents to run these experiments or perform observational tuning. For example, a background meta-agent could tune global harness settings via A/B testing. If we can run a scheduled meta-agent, it could also tune handover instructions and other parameters over time." A capability want, not a committed design. `meta-agent-tuning`.
- The layered-shutdown deadline budget: "give a global timeout budget... and then basically hand down a slightly shorter budget at each level. I'm not sure exactly how that should work. I don't know what asupersync does here. We should look." An idea, not a ruling.
- Whether `opentui_rust` is the right TUI bet: "We'll try to use opentui rust" (`source-notes/tui-styling.md`). `multi-client-ui`.
- Whether the OpenAI-compat tool-call encoding holds beyond OpenRouter, which is the only provider it has been verified against (`HANDOFF.md`).

## Known tensions

Contradictions he knows about and has deliberately left open. Do not harmonise them.

**Storing context inputs and outputs, versus reloading sources.** The narrow position: "we need to preserve enough information to produce exactly the same prefix in the next API request so that we can keep cache across restarts. So we store notices (naturally, as part of the context storage), but we don't actually need to store all data sources to produce a new context from scratch. That can & should be reloaded all the time, and so neither that nor the derived data need to be persisted." The wider position, from the same review: "I meant we might have to store both input (for diffing in the right place) and output (rendered API request for caching purposes / close to it). Yes, this contradicts my earlier statement about not storing data, just reloading it." Both are in `DECISIONS.md`, both dated 2026-08-12 (persistence approach for context updates; resuming a weeks-old session may require storing both input and output). His ruling on the conflict itself: the tension is recorded, not resolved.

## Deferred from early experiments by explicit ruling

Streaming responses; provider error taxonomy in the fake provider; principled credential handling; SQLite storage design (persistence-analytics); subagents (forked-subagents); compaction and per-model/per-interface view variation; tool-calling robustness and improvements; the event-streaming / replication protocol and everything in `experiments/event-streaming-notes.md`; configuration/context-layer composition (see PLAN.md, modular components); "the brain is in charge of configuration changes, I believe" — left for later.

Validation sequencing and status live in `PLAN.md`.
