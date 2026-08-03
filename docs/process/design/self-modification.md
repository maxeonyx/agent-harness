# Self-modification — the shell / soft-middle boundary — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why (user-involved) · what (agent-drafted, unreviewed) · interactions, summary — not yet done.** Derives from `source-notes/tech.md`.

**Terminology guard (the user corrected an error here):** *"core"* in this project means **the stuff finalized/incorporated from experiments** — a *maturity* axis, not a layer, and not "the Rust implementation". A plugin can be core; you don't know whether something is core until its experiment is done. The layering axis below (**shell** vs **soft middle**) is *orthogonal* to core-ness.

## Why

### 1. Rapid iteration and self-hosting — *desire (the north star)*

The point is rapid iteration and development *of the harness* — on whatever machine it happens to be running, no matter which one. Without bothering with source control, without deployment, without even files. Anything that is a plugin gets edited **directly in memory and immediately shared, over the same mechanism the harness already uses to share its data.** "The more's in TS, the more is boosted by self-modification." Forward: bootstrap self-modification as fast as possible (basic loop → agent edits code → relaunch onto the new code) so the harness starts building itself early.

### 2. But bounded — a thin shell must be compiled separately — *correctness/capability*

We can't do that for everything. The outer skeleton — the hosting binary, the web package / wasm module, the platform-specific "how data gets from A to B" — needs to be compiled separately. We should make that shell as **thin as we can**, and use Rust and/or JavaScript for it.

### 3. Never live-brick — *safety (top priority)*

The scar: the very first thing hit with Pi's self-modification was an agent self-edit that *immediately bricked the harness*. Really dumb. So: live editing, but **never live bricking** — exercise plugin code on load as much as possible, and revert to the old version if the new one crashes. You must be able to change yourself without dying.

### 4. Stable APIs the soft middle conforms to — *correctness (this is what makes self-mod safe *and* cache-stable)*

The shell provides stable **APIs / frameworks**; the soft middle provides **implementations that fit them**. The load-bearing example: the **context-provider API** — a plugin reload must not cause a cache break, which *implies* a stable API that context providers conform to (the system prompt, including tool defs, stays put across a reload; old tool calls remain usable until a deliberate handover/compaction or an explicit cache break). Same pattern for the replication protocol and CLI-argument contributions: the *framework* is shell, the *contributions* are soft middle.

### 5. Sandbox / secrets — *nice to have, lowest priority*

Sandboxing agent-edited plugins away from credentials (auth flow outside the plugin; hand back a pre-authenticated fetch wrapper; no arbitrary node modules) is nice, but the user does not personally prioritise it. Lowest on this list.

## What

The boundary sketched below is the spine of this design, so the "what" starts from it rather than restating it. Three things need developing on top: a *test* for classifying things the sketch does not list, since every future experiment will need to answer "shell or soft middle?"; the mechanics of changing the soft middle underneath a session that is already warm, which is where the interesting constraint lives; and the mechanics of changing the shell, which means rebuilding and relaunching the harness with something to fall back to. The meta limb ties the two together, and it turns out to divide them rather more cleanly than expected.

### The boundary — as the user sketched it

**Shell** (thin, compiled separately; provides frameworks, APIs, transport, rendering engines):

- website package, wasm module, hosting binary
- the CLI — though CLI *arguments* may have contributions from the soft middle
- getting data from place to place; the **replication protocol** (enforced by the shell); the framework one must write replication-protocol and CLI-arg contributions within
- the **provider API** — the interface provider implementations must fit
- the **rendering framework** for the TUI and for the web
- stable APIs generally (e.g. the context-provider API in #4)

**Soft middle** (plugins, edited live, rapid iteration):

- provider *implementations*
- tools, user tools, business logic
- **pretty much all UI content — even the TUI content** (not the rendering framework), because UI especially wants rapid iteration
- UI elements for a provider's usage/cost/etc — given a place in the UI, kept as flexible as possible
- exact data *formats* may be provided or extended by the soft middle (the transport and protocol are shell)

Principle: make the shell **as thin as possible**; push everything you want to iterate rapidly into the soft middle.

**How the editing happens: the meta limb.** Self-editing happens via the **"meta limb"** — the agent operates on the harness itself from that limb, rather than on a project. This is the concrete home of #1: the plugin-editing-in-memory that gets shared over the harness's own data mechanism is work done in the meta limb.

### A test for what belongs in the shell

The sketch is a list of examples, and lists do not classify new cases. Reading across it, what the shell items have in common is not "compiled" and not "important" — it is that **other things depend on their shape**. A protocol is depended on by a peer process. An API is depended on by whatever implements it. A rendering framework is depended on by the content drawn into it. A tool-definition schema is depended on by a conversation that already contains calls made against it. The soft-middle items are, uniformly, things that *fit into* a shape somebody else defines: a provider implementation fits the provider API, a tool fits the tool framework, TUI content fits the rendering framework, a CLI argument contribution fits the CLI framework.

So the test: **if changing it changes a contract that a warm session, a peer process, or a stored row depends on, it is shell.** Otherwise it is soft middle, and should be, since the principle is to make the shell as thin as possible. There is a second, duller reason something can be shell — it physically cannot be loaded at runtime, like the hosting binary or the wasm module — but that is a platform fact rather than a design judgement, and it applies to a small and shrinking list.

The test predicts the sketch's own classifications, with one apparent failure that turns out to be the most important thing in this design. Tools are soft middle, but a tool's *schema* goes into the system prompt, and the system prompt is exactly a contract a warm session depends on. By the test, tools should be shell. They are not, and they should not be — tools are the single thing most worth iterating rapidly. The resolution is that the contract is not "the schema is fixed forever"; it is "**a session's schema snapshot is fixed for that session's lifetime**". Versioning rescues the classification. Which means the test needs a rider: soft-middle things whose shape leaks into a warm contract must be *versioned and retained*, not frozen. That rider is precisely what "we store plugins in the DB, perhaps?" buys, and it is why that storage question is load-bearing rather than incidental.

The same rider explains the sketch's line about data formats — that exact formats may be provided or extended by the soft middle while the transport and protocol are shell. An extension is additive inside an envelope the shell owns, so old rows stay readable; the envelope is the contract and the contents are not.

Two things this test deliberately does not decide. It says nothing about *maturity*: a plugin can be core, and shell code can be entirely unproven. That is the terminology guard at the top of this doc, and the test is orthogonal to it. And it says nothing about language: the sketch says use Rust and/or JavaScript for the shell, so "shell" does not mean "the Rust part" any more than "core" does.

### Changing the soft middle underneath a warm session

This is where the design has an actual constraint rather than a preference. From `source-notes/tech.md`: we want to keep the context the same for as long as the KV cache is still valid, so if we relaunch or reload plugins we still want the system prompt to remain the same — but the system prompt contains the tool call definitions, so "we still want the old tool calls to be usable by the agent until the next handover/compaction or the next cache break". A cache break should be *available* as an option for a plugin reload, but explicitly not the normal path: "Cache break should be an option for plugin reloads though. but not normal path, it should rather be explicit."

Making that true requires three things. A session **pins** a plugin version set at the moment its context is built. The pinned versions must remain **addressable** afterwards — you cannot delete a version while some warm session's system prompt still describes it, which is exactly the retention property that storing plugins in the database makes easy and that storing them as files on disk makes awkward. And **dispatch must be version-aware**, so that a call arriving against a pinned definition is handled in a way consistent with that definition rather than with whatever was loaded thirty seconds ago.

That last one hides a genuine tension between two of the whys, and I do not think it resolves by itself. Why #1 wants rapid iteration: the agent edits code and wants to see the effect *now*. Why #4 wants the warm session's contract stable. If pinning means the warm session keeps running the *old code*, then a bug fix does not take effect until the next compaction — which is close to the opposite of what the north star asks for, and it would be maddening in exactly the situation self-modification exists for, where the session doing the editing is the session that wants to test the change.

The reading that dissolves it is that the note asks for the old *calls* to remain usable, not for the old *code* to keep running. So: **pin the schema, run the newest implementation.** The system prompt stays byte-identical, the cache stays warm, the agent's existing calls stay valid — and the behaviour behind them is current. Three cases fall out, and they are worth naming because a reload has to classify itself into one of them before deciding what it is allowed to do:

A **schema-identical** change — the implementation changed, the declared interface did not — is adopted immediately by every session, warm or not, with no cache consequence. This should be the overwhelmingly common case, and it is the one that makes rapid iteration feel rapid.

A **schema-additive** change — a new optional parameter, a new tool — is safe to adopt for the same reason, with the caveat that a warm session simply cannot use the addition, because its system prompt does not mention it. New sessions get the new definition. If the addition matters to the warm session, that is what the explicit cache break is for; and this is the same shape as the change-notice problem in context-updates, where a tool with a changed schema is the one case that needs full content injection rather than a bare notice.

A **schema-breaking** change — a removed or renamed parameter, a removed tool — cannot be adopted by a warm session without lying to it. Either the old version stays runnable for the sessions pinned to it (retention doing its job), or the reload takes the explicit cache break. This is the case that justifies keeping old versions addressable at all, and it should be rare.

Classifying the change requires comparing declared schemas across versions, which is mechanical and cheap, and it means a reload is a decision rather than an event. It also means the reload path needs a way to say "this one needs a cache break" and have that be a deliberate, user-or-agent-visible act rather than a silent cost.

### Never live-bricking

The scar in why #3 is an agent self-edit that immediately bricked the harness. The notes' instinct is right and modest: "I guess we probably want to exercise plugin code on plugin launch as much as we can", and if the new version crashes, revert to the old one. "While we want live editing, we don't want live bricking."

That is a ladder rather than a single mechanism, and the rungs have very different strengths.

Load-time validation is the cheap rung and catches the majority of real self-edit disasters: does the module parse, does it evaluate, does it export what it claims, do its declared schemas validate. A syntax error taking down a running harness is the stupid failure the scar was made of, and this rung ends it.

Exercising the code is the rung the notes ask for without saying how. The honest options are plugin-declared self-checks — a plugin ships something the harness can run on load — and a synthetic invocation of its entry points. Both help and neither is sufficient, because no amount of load-time exercise catches a plugin that only breaks under real use. This is a proposal rather than a ruling, and its limits should be stated openly so nobody mistakes a green load for a working plugin.

Quarantine is what happens on failure: the failing version is marked bad and does not become active, the previous version stays active, and the agent that made the edit is told what went wrong in enough detail to fix it. Quarantine is strictly better than rollback because nothing was ever swapped in.

Auto-rollback covers failures that only appear after activation, and it has a prerequisite the notes do not mention: you have to know **what to blame**. A fault raised inside a tool invocation is attributable to that plugin trivially. A fault that surfaces later, or in the event loop, or as corrupted state, may not be attributable at all — and rolling back the wrong thing is its own outage. The mechanism that makes attribution possible is running each plugin somewhere a fault has a name, which is what the Deno isolate and the hard sandbox from `source-notes/tech.md` provide. That is worth noticing because the user rates sandboxing lowest of the whys, as a security concern — but its value here is not security at all. It is blast radius and attribution: knowing which plugin faulted, and being sure it cannot have damaged the rest of the process on its way out. Those are the properties auto-rollback needs. So the lowest-priority why turns out to be infrastructure for the highest-priority one, which is a reason to build it earlier than its ranking suggests. That reframing is mine.

And the ladder needs one rung the plugin system cannot provide. The flow that must never break is *the flow that lets you fix a break*. If the tools that edit and reload plugins are themselves plugins — which the sketch's classification implies, since tools are soft middle — then a sufficiently bad edit removes your ability to repair it. The way to keep both the sketch and the guarantee is to make **recovery a shell capability that does not go through the model at all**: a CLI path that starts the harness in a safe mode with only the shell plus the last-known-good plugin set, and a way to roll a plugin back from outside. The CLI is already shell. This costs nothing, requires no exception to the boundary, and means the worst case is a manual command rather than a lost harness. Proposal, not in the notes.

### Changing the shell: rebuild and relaunch

The compiled path is the other half, and the notes describe the intended flow: implement the basic loop, then the agent can edit the code and probably use a tool call to relaunch onto the newer code — for the binary itself as well as for plugins — with the binary supporting "some kind of state scheme so that we can launch back into the same session". Backend server cache ids and similar must not be ephemeral; they are tracked in the database by session, so a relaunch can seamlessly continue. The brain may be running the agent loop for many sessions at once, and on relaunch it should simply continue them. Graceful shutdown means waiting for all in-flight API requests to complete, remembering that we were about to run tool calls, and then running those and continuing after the relaunch.

The resume-after-a-long-gap question is left open in the notes with the hedging intact, and it is worth preserving exactly: "Perhaps if it's been more than an hour, then (if interactive) we ask the user whether we should continue other agents. If it's the brain in server mode, it should definitely just continue. Actually - I don't think that's so clear. Probably this should be optional too. The brain relaunching within an hour can continue but beyond an hour, first client would have to decide whether or not to resume other agents."

Two design points need adding. First, a relaunch has a gate: the new build must have compiled, must start, and must be able to open the database and satisfy its migrations before it becomes the thing that runs. Migrations themselves are operator-lifecycle's subject; the requirement here is only that a relaunch does not proceed on hope.

Second, rollback for a binary is structurally harder than rollback for a plugin, and the difference deserves stating. A plugin rollback happens inside a running process that is still there to do it. A binary rollback happens when the *new process failed to start*, at which point there is nothing running to notice. Something outside must hold the fallback: either an external supervisor (`source-notes/tech.md` already contemplates re-parenting to systemd or Task Scheduler when detaching a GUI, so the supervisor is not a new idea in this project), or a small launcher that keeps the previous binary and restarts it if the new one does not reach a healthy state within a window. The notes do not specify which, so this doc does not either — but the requirement is unavoidable: **never-live-brick for the shell needs an actor that survives the shell.**

One small thing that is easy to omit and annoying to retrofit: a deliberate self-relaunch has to be *distinguishable in the record* from a crash, or the analytics surface will report the harness as unstable every time the agent improves it.

### The meta limb, and the two workflows the boundary actually divides

The sketch names the meta limb as where self-editing happens: the agent operates on the harness itself from that limb rather than on a project. Because a session is bound to exactly one limb, and because crossing a limb boundary is always fresh, self-modification is necessarily a *separate session* from the project work that provoked it. That has a pleasant consequence nobody had to design: you cannot accidentally self-modify from a project session. Getting at the harness's own guts requires deliberately spawning into the meta limb, and the safety comes free from the limb model rather than from a permission check — which matters in a project that explicitly does not want permission prompts.

Developing this reveals that there are really *two* self-modification workflows, and the shell/soft-middle boundary is the line between them. The in-memory workflow is why #1 in its purest form: the agent edits a plugin in the meta limb, in memory, "without bothering with source control, without deployment, without even files", and the change is shared immediately over the same mechanism the harness already uses to share its data. There is no build step and no restart; the reload machinery above is the whole story. The compiled workflow is different in kind: editing the shell means a checkout, a build, and a relaunch, which means source control and files are back, and the meta limb is the wrong place for it — that work happens in an ordinary project limb pointed at the harness's own repository, and the only meta-limb part is the tool call that relaunches onto the result.

The notes do not draw that distinction explicitly; `source-notes/tech.md` treats "edit a plugin or the harness implementation itself, build and reload autonomously" as one capability. I think separating them clarifies a lot: it explains why "make the shell as thin as possible" is not an aesthetic preference but a direct measure of how much of the harness can be improved in the fast loop, it gives the phrase "the more's in TS, the more is boosted by self-modification" a precise meaning, and it tells the experiment that there are two distinct things to demonstrate with two distinct risk profiles. It also means the meta limb's tool set is smaller than it first appears: edit, validate, reload, roll back, and relaunch. It does not need a compiler.

The last piece is that plugin edits propagate. If a plugin edit is shared over the same replication mechanism as session data, then it reaches other faces and other brains the way events do, which makes "the harness edits itself" a distributed change rather than a local one. That is mostly a stage-3 concern with topology and persistence, but it has one consequence worth stating now: a version pin has to mean the same thing everywhere, so plugin versions need globally unique identity for the same reason durable rows do.

### Putting it back together

The design is one boundary and one constraint. The boundary: anything whose *shape* other things depend on — a protocol, an API, a framework, a schema — is shell, and everything that merely fits into such a shape is soft middle, which is nearly everything, which is the point. The constraint: the soft middle can be swapped underneath a session whose system prompt already describes it, so a session pins the *schema* it was built with, keeps running the *newest implementation*, and only takes a cache break when a change is genuinely breaking and someone asks for it explicitly. Around those two sit the safety mechanisms, which exist because the failure mode is losing the ability to fix yourself: validate and exercise on load, quarantine rather than swap, roll back with attribution good enough to blame the right plugin, and keep a recovery path in the shell that never goes through the model. The shell's own version of all this is a gated rebuild-and-relaunch with something outside the process holding the previous binary. And the meta limb is where the fast half happens, while the slow half happens in an ordinary project limb pointed at the harness's own source — the boundary being, in the end, a boundary between two editing workflows.

The thesis is that an agent can edit the harness and continue working, without bricking it and without breaking the sessions that are already warm. Falsifiers, in roughly descending order of how much they would hurt: schema pinning turns out not to preserve the cache in practice, so every reload costs a cache break and self-modification stops being free; running the newest implementation behind a pinned schema produces behaviour the warm session cannot make sense of, forcing the old-code reading and with it the slow-iteration cost; fault attribution proves too weak for auto-rollback to be safe, meaning the only honest response to a fault is a full rollback of everything changed since the session started; or the relaunch path cannot resume many concurrent sessions cleanly, which turns every shell improvement into an interruption and pushes the shell/soft-middle line for reasons of pain rather than design.

It touches invariant 1 most directly of all the invariants here — a plugin is agent-edited code, and the auth flow stays outside it with a pre-authenticated wrapper handed back, so credentials never reach the thing the model wrote. Invariant 2 matters in a way that is easy to miss: a plugin reload is a context change, not a reason to call the model, so a reload must never trigger a request. Invariant 5 carries the version retention: plugin versions are durable data with a lifetime tied to the sessions pinned to them. Invariant 8 and the terminology guard sit together — shell-versus-soft-middle is a layering question and core-ness is a maturity question, and a plugin can be core. Invariant 9's drain semantics are what a relaunch's graceful shutdown is made of. And invariant 4 keeps the meta limb honest: it lives inside the brain process and must still behave like a limb.

## Parked for later stages

**Interactions flagged for stage 3:** the **limb model** (the meta limb is where self-modification is performed — self-modification is, in effect, an application of the limb model to the harness itself); compaction-handover & context-updates (the context-provider stable API and cache stability across reloads); persistence-analytics (plugins stored/versioned — "in the DB, perhaps" — so an old schema stays addressable while the cache is valid); and *every* experiment (whether its result lands in the shell or the soft middle, and separately whether it becomes core).

## Questions for review

- **My classification test.** "If changing it changes a contract that a warm session, a peer process, or a stored row depends on, it is shell" is derived from your sketch's examples, not from anything you said. It predicts your list, but it initially misclassifies tools as shell, and I rescued that with the rider that soft-middle things whose shape leaks into a warm contract must be versioned and retained. Is the test the right one to hand to future experiments?
- **Pin the schema, run the newest code.** This is the biggest call in the doc. `tech.md` says the old tool *calls* must remain usable; I read that as being about the declarations rather than the implementations, because the alternative means a warm session keeps running stale code, which fights why #1. If you meant the stronger thing — old code for warm sessions — say so, because it changes the retention and dispatch design substantially.
- **The sandbox reframing.** You rate sandboxing lowest, as a security concern. I argue its real value here is fault attribution and blast radius, which auto-rollback depends on, and that this is a reason to build it earlier than its priority suggests. That contradicts the priority you gave it, though not the reasoning behind it.
- **Recovery in the shell.** I have proposed a CLI safe-mode path (shell plus last-known-good plugins) and out-of-band plugin rollback, so that bricking the editing tools cannot cost you the harness. It keeps your sketch intact — the editing tools stay soft middle — but it adds a shell surface you did not list.
- **Who holds the fallback binary?** Never-live-bricking the shell needs something that survives the shell: an external supervisor (systemd / Task Scheduler, which you already contemplate for detaching) or a small launcher that keeps the old binary. The notes do not choose; I have stated the requirement without choosing either.
- **Two workflows, not one.** I have split in-memory plugin editing in the meta limb from compiled shell editing in an ordinary project limb pointed at the harness repo. `tech.md` treats them as one capability. If you agree, the meta limb needs no compiler and the experiment has two demonstrations rather than one.
- **Load-time exercise is weaker than it sounds.** Plugin self-checks and synthetic invocation are my proposals for "exercise plugin code on plugin launch as much as we can", and neither catches a plugin that only breaks under real use. Is quarantine-plus-rollback carrying more of the safety weight than you intended?
- **A structural question about this doc.** Your boundary sketch reads as the natural opening of this "what", and I have left it in place above rather than folding it in, since you wrote it. It may want merging at summary stage.
