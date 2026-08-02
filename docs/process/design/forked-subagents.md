# Forked subagents — design scoping

Provisional. Derives from `source-notes/agent-hierarchy.md`, `source-notes/handoff-improvements.md`, `source-notes/context-and-agent-loop.md`, and the walking-skeleton rulings in `REQUIREMENTS.md`. Each part is marked **settled** (the notes already decide it), **open** (a design choice this doc proposes for review), or **experiment** (only running it will tell).

## The starting point: what a subagent is

A subagent here is not a background job. It is a child in a structured concurrency scope. When a parent launches children, the parent suspends, and it resumes only when every child in the scope has finished. There is no global spawn context and no fire-and-forget: every running agent has a local owner. **Settled.**

The scope has a few deliberate visibility rules, all settled in the notes:

- A suspended parent sees nothing until the scope completes. It cannot watch its children.
- Siblings share a scope, so they can see each other's *status* — but not each other's *results*. Results go only to the parent, all together, when the scope completes.
- Any agent can launch additional siblings into its own scope, so a parent can resume to more results than it asked for.
- A child's result is the last message part of its final turn. A failed child is a *completed* child with an error result.

Two consequences are worth saying out loud. First, "the parent is suspended" is what makes the model cheap to reason about — the parent's context is frozen at the moment of the Task call, so nothing that happens during the scope can confuse it. Second, sibling-status visibility is a brain-owned tool (like Task itself), not a limb tool — status is session state, which the brain has. **Settled**, though the exact status tool shape is undesigned.

One rule from the notes doesn't fall out of structured concurrency and needs stating: whether one child failing aborts the whole scope or just delivers an error result to the parent is explicitly "needs experimenting". The structured-concurrency-pure answer is that the scope always runs to completion and failure is just a result value; the pragmatic answer is that some failures make siblings' work pointless and cancelling them saves money. **Experiment.**

## How a child gets its context: fork vs fresh

There are exactly two ways to start a child, and the choice is forced by one question: does the child need the parent's context?

- **Fork** — the child starts from a copy of the parent's conversation. Default within the parent's limb. Exists for KV-cache reuse: the provider has the parent's prefix warm, and a fork's first request replays that prefix.
- **Fresh** — the child starts from a newly built context. Required when crossing a limb boundary (a limb's context can carry load-bearing instructions, so acting in a limb means going through an agent built *from* that limb's context). Also allowed within the same limb, and preferred for narrow tasks even when a fork is possible-but-stale — a cache miss on a long context costs more than a short fresh start. **Settled** as rules; the stale-fork threshold is judgement.

### What exactly does a fork copy? (open)

The notes hedge the fork point: append-mode "possibly w.r.t the parent *as of the message before it sent the subagent tool call* - but that needs experimenting." There is a second, quieter question the notes don't address: the parent may have *accumulated but unsent* context at the fork moment — piggybacked user activity, change notices, sibling status updates queued for the next request. Do forks inherit the queue?

Proposed answer, for review: **a fork copies the wire-visible context — what the model has actually seen — and nothing else.** The pending queue stays with the parent (it describes the parent's session, and the parent will still send it after resume). This keeps a fork an honest copy of the parent model's actual state, which is also exactly the property that makes the cache warm. If some queued item matters to the child's task, it belongs in the task prompt or an attachment, explicitly. **Open**, and the fork-point question ("as of which message") stays **experiment**.

### The agent_type tension (open)

Task takes an `agent_type`. But a fork exists for cache reuse, and provider prefix caches require the prefix to be byte-identical — *including the system prompt and tool definitions*. A forked child with a different persona in its system prompt is a cache miss by construction, which deletes the reason to fork.

The notes don't confront this for Task (they do flag the same problem for Resume: "does it break cache badly enough to matter?"). Options, roughly in order of how much they preserve the fork's purpose:

1. Forked children keep the parent's agent_type. Wanting a different persona implies fresh. Simple, honest, slightly restrictive.
2. Forked children keep the parent's *system prompt*, and the persona arrives as an appended instruction inside the task message. The cache prefix survives; the persona is weaker (appended text, not role framing). Whether an appended persona is behaviorally good enough is a model question.
3. Accept the cache break and warn, as the user-side subagent tool already plans to ("forked should warn if cache likely expired").

Option 2 is attractive and testable cheaply. Which of 1/2 should be the default is **experiment**; that the tension exists and Task's docs must be honest about it is **settled by arithmetic**.

## Launching several children well: seeds and attachments

From `handoff-improvements.md`, all **settled in intent, experiment in the details**:

- The `task` context prompt is generic across all children launched together; per-child instructions differ.
- **Attachments**: the launching agent can attach tool calls — file reads, skills, searches — which execute once, in one init step, and appear to every parallel child as ordinary tool calls already in context. This is the cheap way to give N children the same background without N sets of read-tool round-trips.
- **Seed contexts**: for fresh parallel children, start one request carrying only the shared context ("wait for further instructions"), establishing a warm shared prefix; then send each child its own instructions as a follow-up, so all children share the cached seed. Cost: one extra round trip and the discipline that all children share the seed's system prompt and tool defs — the same arithmetic as the agent_type tension above. At most one seed per limb per launch.

The notes also sketch a more radical shape: task launching as *actual code* — create seed contexts, attach tasks, await subagents, compose their outputs. The user's own judgement is kept verbatim: "Well, that's complicated but we can certainly consider it." It stays a considered alternative, not the mainline. **Open, parked.**

Getting any of this to actually hit the cache requires being, in the notes' words, *very* correct with the OpenAI Responses API and Anthropic Messages API caching semantics. That correctness work is shared with the compaction-handover and context-updates experiments and is a big part of what this experiment exists to prove. **Experiment.**

## The tool surface

`Task` parameters, **settled**: `task` (the prompt), `agent_type` (see tension above), `user_facing` (orthogonal to agent_type, deliberately), `context` (omitted/`"self"` = fork in place; a limb id = fresh in that limb; the same limb id explicitly = fresh in place).

`Resume` continues a previous session as a new child: `id` from a prior Task result, `agent_type` optional and question-marked in the notes. Resume interacts with a walking-skeleton ruling: a cancelled session may hold proposed-but-unexecuted tool calls, which are valid resumable state — a Resume may execute them. **Settled** that Resume exists; **open** whether resuming-as-a-different-type is allowed at all before the cache question is answered.

An optional pre-step — a temp fork whose only job is writing a good task prompt — is in the notes as "idea, not decided", with a real latency/cost tradeoff. **Open, parked.**

### User-facing children and the main-thread pattern

**Settled.** A `user_facing` child blocks on the user and completes when the user says `/done`; an autonomous child completes at end of turn. Only user-facing agents may request launching new user-facing sessions (an autonomous agent must not create surprise blocking dependencies on a human); permission-request expiry as an escape hatch is noted as *possible*, not decided. Multiple user-facing sessions appear in a session switcher, and the user can launch a user-facing session into a scope that lacks one.

The main-thread pattern is the composition that makes all this feel natural: a parent forks a user-facing child (the conversation "just continues" from the user's point of view), launches autonomous siblings beside it, and resumes when the user is done *and* the siblings are. From the user's perspective nothing happened except that work got done in the background while they kept talking.

Naming: auto-generated hierarchical names (`tk-prodsync-findfiles-refactor-imports`), user-editable when spinning out; the main-thread child inherits the parent's name plus an undecided suffix. **Settled/cosmetic-open.**

## Cancellation meets scopes (open — proposed here)

Invariant 9 was ruled for a single session: cancel → drain → finalize, four-valued outcomes, a drain structurally cannot start new work, completed work that ties with a cancel is kept ("it cost us money"). Scopes need the composed version, which the notes don't give. Proposal, for review:

- Cancelling a suspended parent cancels the scope: every child receives cancel and drains by its own session rules.
- A draining scope cannot accept new siblings — that is the scope-level meaning of "a drain cannot start new work."
- The scope finalizes when every child has finalized, with mixed outcomes preserved per child (this child ok, that child cancelled). The parent's Task result carries all of them; completed children's results are kept and delivered even though the scope was cancelled.
- A cancelled-mid-scope parent is itself resumable state: Resume on the parent should be able to continue with the delivered results.

This is the smallest design that composes invariant 9 without new concepts. **Open**, needs the user's eye; the abort-on-child-failure question above should be decided in the same breath, since "abort scope" is just "cancel scope" triggered by a result.

## The two honest risks

**Turn-ending discipline (the A.1.3 problem).** From the notes' own status header: a deeply forked agent is given task A, narrowed to A.1, narrowed to A.1.3 — and "importantly, it must not continue on to complete A.1.4 or the full A.1. It must end its turn after A.1.3 - this must be reliable for forked agents to work well." A forked child carries the parent's entire momentum in its context; the whole design leans on a model reliably *stopping* against that momentum. This is empirically untested, purely a model-behavior question, and the single biggest risk in the design. The experiment should treat it as a first-class thesis: measure how reliably forked children stay in scope under realistic momentum, and what framing (system-side instructions, task phrasing, structural end-of-turn signals) makes it reliable. **Experiment, priority.**

**Shared mutable workspace.** Fork-by-default plus parallel siblings plus one limb means several agents on one filesystem. The notes call the known experience poor and offer two candidate directions — clear scope-ownership instructions in each child's context, and "some kind of borrow-checker-style rule on mutable workspace regions (tentative, not designed)". No solution is proposed here either; the experiment can generate evidence with deliberately overlapping tasks, but design work remains. **Open risk, unresolved.**

## Interactions with other experiments

- **compaction-handover.** Forked task, fresh task, and handover share one structure — context, attachments, task — and the notes lean toward making them "essentially the same". If that holds, this experiment and compaction-handover are two views of one mechanism, and whichever runs first should design the shared shape with the other in mind.
- **limb-context.** Fresh-across-limbs is forced by limb-owned load-bearing context; seeds are per-limb. What "building a fresh context in limb X" actually assembles is limb-context's question; this design only requires that it exists.
- **user-turn.** The user gets a subagent tool too — same fork/fresh choice, fork warning when the cache is likely stale, and only what the user saw (not what the subagent saw) needs attaching back.
- **persistence-analytics.** Scope state is exactly the storage stress case the notes list: blocked parents, dynamic sibling sets, resume targets, per-child results and costs. Whatever this experiment learns about scope state becomes schema requirements there.
- **multi-client-ui.** The session switcher, visible blocked/stuck states, and the launch-user-facing-into-scope button are UI surface over this model.
- **ts-vs-rust.** Undecided where scope orchestration lives. Instinct from the boundary doc's principle: scope bookkeeping (ownership, blocking, cancellation composition) smells like substrate (Rust, invariant-adjacent), while task-prompt shaping and router judgement ("should this be forked or fresh?") smells like iterable business logic (TS). Flagged, not resolved.

## What the experiment must actually test

On paper (settle by review, no code needed): the fork-copies-wire-visible-context rule; the cancellation-composition proposal; Task/Resume parameter semantics; the user_facing launch rules.

Only by running (the experiment's actual content):

1. A.1.3 turn-ending reliability under momentum — the priority thesis.
2. Fork cache economics: measured cache hits for forks, the agent_type options (parent-type vs appended persona), the fork-point question, seed-context round-trip vs savings.
3. Abort-scope vs error-result on child failure, tried both ways.
4. Scope cancellation composed with invariant 9's drain semantics, including keep-completed-work.
5. Sibling status visibility: what status granularity is useful without leaking results.
6. Shared-workspace collisions with deliberately overlapping sibling tasks — evidence-gathering, not solution-proving.

Exit (from PLAN.md): structured concurrency is usable and understandable, not just formally clean — and forking is demonstrably cache-cheap.

## The matrix

Levels, statuses, and aspect definitions per `README.md`. The Why column is the motivating story. Blank = not addressed.

| Aspect | Why (the story) | Behavior | Mechanics | Verified | Interacts with |
|---|---|---|---|---|---|
| Model framing | a forked child inherits the parent's full momentum; told "only A.1.3", it completes A.1.4 anyway and the decomposition collapses | | E persona-as-appended-instruction | E A.1.3 reliability (priority) | |
| Wire & cache | launching N children today re-reads the same files N times at full input price; cache reuse is what makes forking cheap enough to be the default | P fork copies wire-visible context only | E fork point; E seeds/attachments; P agent_type options | | compaction-handover, context-updates |
| Tool surface | delegation needs one obvious verb, or context gets retyped into every child prompt | S Task/Resume parameters; O resume-as-different-type | O pre-step fork (parked); O task-calls-as-code (parked) | | user-turn (user subagent tool) |
| UX & input | delegating currently stops the conversation — the user waits; the main-thread pattern lets talking and background work overlap | S main-thread pattern; S `/done`; S session switcher | O naming suffix | | multi-client-ui |
| Ownership & placement | | | O scope orchestration Rust vs TS | | ts-vs-rust |
| Lifecycle | fire-and-forget children get orphaned and stall unnoticed; scopes make every agent someone's responsibility | S scope visibility rules | P cancellation×scopes; E abort-vs-error | | |
| Storage | a blocked parent with dynamic siblings must survive a brain restart, or long delegation is fragile | O scope state shape | | | persistence-analytics |
| Economics | "fresh wins over forked-but-stale" is a money decision made constantly; unmeasured, it's vibes | | E seed round trip vs savings; E fork hit rates | | cancellation-economics |
| Security | an autonomous agent launching a user-facing child creates a surprise blocking dependency on an absent human | S only user-facing launches user-facing | O permission expiry escape hatch | | |
| Testing & verification | parallelism plus timing is where flakes breed, and flakes are bugs here | O black-box surfaces for scope semantics | E A.1.3 needs real models, not the fake provider | | |
| Code shape | | P children as owned in-flight work of the brain's loop (walking-skeleton pattern extended) | | | |
| Dev workflow & references | | S consult the OpenCode fork's subagent behavior and oh-my-pi before inventing | | | |
| Core migration | | S never copy; scope semantics become storage schema requirements first | | | persistence-analytics |
