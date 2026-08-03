# Topology — the decoupled monolith — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why, what (agent-drafted, unreviewed) · interactions, summary — not yet done.** Derives from `source-notes/tech.md`, `source-notes/agent-harness-design.md`, and `source-notes/federated-brain.md`, with deferred design inputs in `experiments/event-streaming-notes.md`.

Face, brain and limb are logical roles. This design is about what it takes for that sentence to be true in practice — co-located in one process, split across processes on one machine, or spread over machines and networks, all running the same logic.

## Why

### 1. Deployment shape is a circumstance of the user's life, not a design choice — *desire*

The story is the user's actual setup: a Windows desktop, a Linux VM on the same box, a laptop, remote servers, potentially a cloud instance. Work happens across all of them. The harness cannot sensibly have an opinion about which machine is "the" machine, because the answer changes hour to hour. So the roles have to be assignable to wherever they need to be, and the arrangement has to be configuration rather than architecture.

The failure this avoids is the obvious one: a harness that only works when everything is local, plus a bolted-on remote mode that is permanently second-class and half-broken.

### 2. Long-running work must not be hostage to a UI session — *desire*

Concretely, from the notes: the user wants to be able to detach the GUI from a running process — "re-parenting it to systemd or task scheduler. The brain keeps using it" — and have the brain carry on. Restated as the requirement: closing a laptop lid, killing a terminal, or losing a network connection must not stop an agent that is halfway through a job. Ideally it does not even interrupt an in-flight request, and it must hold on both Windows and Linux.

This is a desire with teeth, because it forces the face to be a genuinely detachable observer rather than the process that owns the work. Anything the face owns is by definition lost when it goes away, so this root determines the ownership split.

### 3. One logical model, or you end up writing the system twice — *correctness*

If in-process and split deployments run different code paths, they drift, and the split path is the one that quietly rots — it is exercised less and it fails in ways local development never sees. The decoupled monolith exists to prevent that: the same components, the same messages, the same protocol, with only the transport substituted (channel, IPC, or network).

This is the root behind invariant 10 — roles never assume co-location. No shared filesystem, environment, working directory or clock across a role boundary; data crossing a boundary travels *in the message*. That discipline is annoying precisely where it matters, because the local case would work fine without it, and only the remote case breaks later.

### 4. Centralising the agent loop avoids N copies of everything that should exist once — *resource/correctness*

The brain is the only role that drives provider requests. The reason is not primarily about protecting secrets — it is that rate limits, billing, session management and provider connection state are things there should be exactly *one* of. The notes' non-goals say it plainly: no "running a full remote copy of OpenCode/Pi/Goose per project", no "duplicating rate limits, billing, or session management per project". Distributing the agent loop means distributing all of that, and then reconciling it.

Credentials staying brain-owned (invariant 1) falls out of this as a consequence, and is worth having, but it is not the motivating force. The motivating force is that a coherent view of cost and rate limits is impossible if N machines are independently talking to providers.

### 5. Output should feel live — *quality*

The face needs tool stdout and model tokens fairly directly; the brain should not buffer a response before forwarding it. Hence the two paths in the notes: the brain proxying the stream as the safe default (because the network may simply not allow the face to reach the limb — remote limb behind SSH, different segment), and a direct face↔limb stream as a fast path when topology permits, with the brain still in the loop for the agent loop, model calls and permissions.

The important consequence is that the fast path is an *optimisation with a fallback*, never a requirement, and the fallback must be durable rather than degraded.

### 6. Transparent remote environments — *desire (inherited)*

This is limb-model's why #3, and topology is the machinery that makes it true: SSH in, copy a binary, run it, expose a port, tunnel to it, and now a remote limb is available to a local agent that knows nothing about SSH. Recorded here as inherited rather than re-drilled, because the root belongs to the limb model.

### 7. Federation gives one view of many machines, and backups for free — *desire*

The user likes the idea of brains connected to each other: establish persistent communication between laptop and desktop, or Windows and Linux VM, or a cloud instance, then connect any client to any brain via transparent proxying and see a merged session list. A limb has a primary brain affinity — commonly a brain per machine that hosts limbs — but the user can override it.

The freshest thinking adds a second motive: rather than each brain holding only its own data, they all store all of it, so "I get backups by default. Sync all the data in the background." With the caution attached — keep provenance clear, do not duplicate or confuse remote data with local. `source-notes/analytics.md` wants the same thing from the query side: results that span all connected brains.

This is the most speculative root here, and the notes treat it as a like rather than a requirement.

## Forward: what these roots force

- **Every component owns exactly one external world**, and the three are symmetric — face owns the TUI, brain owns the provider connection, limb owns an environment. This was already ruled from walking-skeleton evidence; roots #2 and #3 are why it matters, since ownership is exactly what determines what survives a disconnect.
- **A transport abstraction with at least three implementations** (channel, IPC, network), and a test strategy that runs the same scenario over each — otherwise root #3 is aspiration.
- **Sequencing belongs to the substrate, not a participant.** Already ruled: within a process, participants synchronise; across processes there is no total order. Whether same-machine IPC is close enough to sequence is explicitly unresolved and is a question this experiment should settle.
- **Reconnect and catch-up are first-class**, from #2: the face must be able to rejoin and be brought up to date without duplicates, which means durable ordered events and a resume point per client.
- **Version negotiation across the boundary**, since split deployment means the two sides can be different builds — this is where topology hands off to operator-lifecycle.
- **Provenance and globally unique identity on durable data**, from #7, so background sync cannot conflate machines. Recorded in `PLAN.md` under persistence-analytics, because it is a schema demand.

## What

The whys ask for one logical model over several physical arrangements. The rest of this section works out what that costs, in the order the design's own dependencies run: what a participant is, what crosses a boundary, what a transport is allowed to promise, what ordering exists over it, how we stop co-location from hiding our mistakes — and only then the configurations that put all of it under test, the two streaming paths, detach and catch-up, and federation last, because it is the least settled thing here.

### What a participant is

This part is already ruled from walking-skeleton evidence, and it is worth restating precisely because everything downstream leans on it. Each of face, brain and limb is an {inbox + select loop + owned in-flight work} participant, and each owns exactly one external world: the face owns the TUI, the brain owns the provider connection, the limb owns an environment (filesystem, processes, tools). The three are symmetric — no participant is privileged, and in particular the brain is not a sequencer, it is another participant.

The useful consequence is that a participant is defined by what it owns, not by where it runs. "The same logic in any topology" means the select loop and the owned world stay put while only the participant's edges change substrate. A face whose renderer is a websocket to a browser rather than a terminal is still the same face, because rendering is an output port and not loop logic. A limb in the brain's own process is still a limb, because it still owns an environment nobody else may touch.

### What actually crosses each boundary

Discipline about boundaries is unenforceable until you can say what a message is, so it is worth being concrete. From the face to the brain go the user's intents: submit a turn, cancel, resume, update a draft, and whatever approval surface exists. From the brain to the face go projections of session facts for that interface — never the brain's provider socket state, which the face learns about only through facts like "a request was sent" and "a response began". From the brain to the limb go dispatches: execute this tool call, cancel that one, build context for this session. The notes are explicit that the brain does no limb tool-call *processing*, only dispatching. From the limb to the brain come execution facts (started, output chunk, exited with this status), environment facts (hostname, working directory, git root), context contributions, and — because the limb is an event peer with its own select loop — unsolicited things like file-watch events.

Nothing world-specific crosses. No file handles, no PIDs the other side is expected to act on, no clock reading treated as "now" on the far side, and no assumption that a path means the same thing to both ends. Hostname is the example already ruled: it is a fact the limb contributes, not something the brain reads. Credentials cross nowhere at all (invariant 1).

Some of what crosses deserves naming as a *proposal* rather than a fact, in the sense that `event-streaming-notes.md` sets out: a face's send is a proposal until the deployment's sequencing mechanism, where one exists, turns it into a fact that replicas consume. Cancellation is the same shape and is the one that bites, because a cancel cannot be an in-memory token handed across a role boundary — it has to be a replicated fact, which is what makes it observable, idempotent, and resumable. The already-ruled case of a proposed-but-unexecuted tool call is the other side of that boundary: the response is a fact, the call it proposes is not, and no replica may invent an outcome to make the exchange look finished.

### What a transport is allowed to promise

There are at least three implementations to build: an in-process channel (or, under invariant 10's explicit exception, shared session state used directly as the substrate), same-machine IPC, and a network link. The interesting design question is not how each is built — that is a few days of ordinary work — but what a participant is permitted to assume about any of them, because that assumption is what determines whether the logic is genuinely one logic.

The proposal here is that a transport promises exactly two things: messages on a single link arrive in the order they were sent, and a message is delivered at least once, with a sender-assigned identity so that at-least-once is safe to receive. It promises nothing across links, nothing about latency, and nothing about liveness. Everything else — total order, deduplication, resume — is built above it out of sequence numbers. The source notes are silent on this contract, so per-link ordering is a proposal, chosen because all three plausible implementations already give it for free (a channel, a stream socket, a websocket) and because taking it away buys nothing.

### Sequencing, and the fact everything else rests on

The ruling is settled and this design does not reopen it: sequencing belongs to the deployment substrate, not to any participant. Within one process, participants synchronise — appends are synchronous calls under a lock, and a total order exists. Across processes there is no total order and no synchronisation; asynchrony is accepted.

The consequence for the code is sharper than it first looks. Participant logic has to be written against the *weaker* model — no total order, asynchronous, a peer may act on information you have not seen yet — and a substrate that happens to offer sequencing is an optimisation the logic may benefit from but must never require. Read that way, the walking skeleton's shared-mutable-state design stops looking like a different architecture and starts looking like what it was: the strongest substrate available in that deployment, correctly exploited.

That reframing is also where the detailed work strains against why #3's phrasing that only the transport is substituted. What is substituted is the *substrate*, and substrates differ in the guarantees they offer, not just in how bytes move. The honest version of the claim is: one logic, correct under the weakest substrate, faster under stronger ones. That is a finding rather than a fix, and it is recorded below.

### Deep dive: settling the same-machine IPC question

Whether same-machine IPC is close enough to sequence is explicitly unresolved, and it is the fact every other ordering guarantee in the system rests on, so this experiment should be the one that settles it rather than inheriting the ambiguity.

"Close enough to sequence" needs an operational meaning before it can be measured. The one this design proposes: a participant can await acknowledgement that its append has been sequenced before it proceeds, and the wait is small enough that nothing the user can perceive gets slower. That makes it a latency question with an interactive bar rather than an aesthetic one. The measurement is a round-trip append over a Unix domain socket and over a Windows named pipe, under a load that resembles a busy session (a long response arriving while a chatty tool is producing output), with keystroke-to-echo on the face as the thing that must not degrade — and it has to be measured on both platforms, because the answer being different on Windows and Linux is a real possibility that would itself be the finding.

Three verdicts are possible and they are not symmetric. If yes, the same-machine split configurations may use synchronous append, and `face+limb ↔ brain` becomes materially simpler to reason about. If no, IPC is treated exactly as the network is, and every same-machine split inherits full asynchrony. If it is ambiguous — good enough usually, occasionally not — we treat it as the network. The asymmetry is the point: wrongly assuming a total order you do not have produces bugs that are invisible during local development and appear only in the deployment you can least easily debug, whereas wrongly assuming you have none costs only performance. So in the absence of evidence, the answer is no.

### Making co-location hostile

Invariant 10 is annoying exactly where it is free, which is why it needs enforcement rather than good intentions. Two mechanisms, and they compose.

The first is that participants get no ambient access to anything: no reading the environment, the working directory, the clock, or argv from inside the loop. Everything arrives at construction. That is the same requirement the modular-components design derives from the testing side, which is a strong hint the two are one design seen from two ends.

The second is that the co-located configuration is deliberately run *hostile*. In one process, on one machine, each participant is given a different working directory, a different environment map, and a deliberately skewed clock. Anything that quietly worked because the three shared a process fails on the spot instead of a year later against a remote limb. There is one deliberate exception to respect: invariant 10 says a face and limb commonly *do* share an environment, the user's machine, and co-located deployments may share session state directly as the substrate. So the hostile-local rule is mandatory for anything crossing to the brain, and optional between face and limb, where sharing is licensed by design rather than by accident.

On top of that, the same scenario runs over every transport and every configuration, and the assertion is that the durable record comes out the same — the same facts with the same causal relationships, setting aside the timings and identities that are legitimately different between two runs. If it does not, the claim that these are deployment choices over one logical model is simply false, and better to know.

### The six configurations, and what each one is for

The list of required configurations is not a checklist someone wrote down; it is close to a covering set, and it reads better if you know what it covers. Each of the three role boundaries should appear both co-located and split at least once; each multiplicity that the protocol must support should appear once; and one entry adds a *link* rather than moving a role.

`face+brain+limb` — everything in one process. This is the reference: the scenario suite's baseline behaviour, the latency floor, and the configuration where the hostile-local discipline above does its work. It is also the one that proves the monolith still respects role boundaries, which is the whole reason the monolith is called decoupled.

`face+limb ↔ brain` — the configuration the notes call out as common: launch as both client and limb, connect to the brain for the agent loop. It exercises exactly one process boundary, and it is the case invariant 10's exception was written for, since face and limb share the user's machine on purpose. It is also where the same-machine IPC question is decided in practice, and where credentials must demonstrably exist only on the far side.

`face ↔ brain ↔ limb` — the three-way split, and the demanding case for streaming, because the face may have no route to the limb at all. This is where a remote limb over SSH plus a tunnel appears, with its own lifecycle: it survives an ungraceful disconnect in case of reconnect, and shuts down if nobody reconnects within a timeout.

`face ↔ brain ↔ brain ↔ limb` — the minimum federation shape. It exercises transparent proxying, a merged session list, a limb's overridable primary brain affinity, and two protocol hops rather than one, which makes it the configuration where version negotiation matters most — and version negotiation is where topology hands off to operator-lifecycle.

`face ↔ brain+limb ↔ face 2` — two faces on one session, with the brain and limb co-located, which is the "a brain server running on every machine that hosts limbs" arrangement from the federation note, seen from the other side. This is the configuration where topology and multi-client-ui are the same problem: per-client resume points, causally positioned sends, and two faces that must not see incoherent state.

The optional direct face↔limb stream is the sixth entry and the only one that adds a link instead of moving a role, so it is treated separately below.

Two honest gaps. Multiple limbs on one brain is described in the notes as possible but not expected, and no configuration above covers it. And detaching the face — the notes' hedged "If possible, we should support transitioning the process to being only a limb ... maybe re-parenting it to systemd or task scheduler" — is not a configuration at all, it is a *transition* between two of them, which makes it a distinct class of test that the list does not currently contain.

### Streaming: two paths, one durable record

The default path is the brain proxying the limb's output to the face, because network topology may simply not permit anything else. The fast path is the face connecting directly to the limb where topology allows, with the brain still in the loop for the agent loop, model calls and permissions. The fast path is a capability the brain authorises and can revoke.

What makes this safe rather than a second code path is how "durable compressed fallback" is read. The reading this design proposes: the durable, compressed record always travels the brain path, and the direct stream carries only disposable live data. Then losing, refusing, or revoking the direct connection costs liveness and never correctness, revocation is cheap because nothing depends on the link, and the assertion for the experiment is easy to state — the durable record carries the same facts whether the fast path was used or not. That is an interpretation of a phrase from `PLAN.md` rather than something the notes spell out, so it wants confirmation.

One collision with an existing ruling should be noted rather than worked around: streaming responses are deferred by explicit ruling. Tool stdout streaming can be exercised now and needs no provider support; model-token streaming cannot be exercised until that deferral is lifted. So as things stand this experiment can test the two paths for tool output only, which is enough to prove the routing and the fallback but not enough to prove the latency claim for model tokens.

### Detach, reconnect, catch-up

The requirement from why #2 is that the face is a detachable observer, so catch-up has to be ordinary rather than exceptional. Each client holds its own replica of session state — the face does not treat the brain's memory as its query surface — and keeps a resume point, which is the highest sequence it has consumed. Reconnect replays from there. When the resume point has fallen outside whatever the substrate retains, or provenance says the record is not the one this replica was following, ordinary catch-up escalates to a full resync; where exactly that line falls is one of the things the experiment has to find rather than assume.

A joining peer declares its contributions, catches up, then signals ready. What existing peers may do in the window before that signal is not something the notes settle, and it matters — a face that is still catching up should not be sent a prompt it cannot render, and a limb that is still catching up should not be dispatched work it may duplicate. If two writers race to resolve the same operation, the outcome is idempotent and first-wins, so at-least-once delivery stays harmless.

The detach test itself is the strongest single piece of evidence for the ownership rule: kill the face, on Windows and on Linux, and the brain and limb carry on, ideally without interrupting an in-flight request. That is nearly free given the brain owns the provider connection — which is precisely the point of insisting each component owns exactly one external world.

Storage of the ordered record is not this design's problem. Topology needs per-link ordering, a sequence, and a resume point; how any of that is persisted, indexed or queried belongs to persistence-analytics.

### Federation, in proportion

The user likes this idea; he does not require it, and this section is deliberately short in proportion. The minimum content that belongs to topology is that a brain can proxy a face to another brain transparently, that the session list the user sees is merged across brains, and that a limb has a primary brain affinity — commonly a brain per machine that hosts limbs — which the user can override.

The freshest note's motive, that every brain stores all the data so "I get backups by default", is mostly a schema demand rather than a topology one: every durable row needs provenance, and identity has to be globally unique so background sync cannot conflate remote data with local. Those live in persistence-analytics, which already carries them, and the caution the user attached carries with them — keep it clear where the data came from, don't accidentally duplicate it or get it confused with local data.

One thing genuinely is unresolved and belongs here. In the `face ↔ brain ↔ brain ↔ limb` chain, which brain owns the session and drives the agent loop? The reading that fits the notes best is that the brain holding the limb's affinity owns the session and the loop, and the near brain is a transparent proxy for the face — so the chain is really face ↔ proxy ↔ session-brain ↔ limb. The alternative, where the near brain drives the loop and treats the far brain as a route to a remote limb, is also coherent. The notes do not choose, and the choice changes what "transparent proxying" has to carry.

### What this experiment does not own

Face↔brain identity and auth is explicitly deferred by the notes to existing solutions — how OpenCode, Goose or ACP handle it. ACP is named as the plausible face↔brain shape and MCP as a plausible brain↔limb shape, with two caveats preserved: limbs must not make direct provider calls even if MCP would allow routing completions back through the brain, and whether any of this is needed at all is unclear. Version negotiation and staged update across a split boundary go to operator-lifecycle. Durable storage, provenance and cross-brain queries go to persistence-analytics. The layered graceful-shutdown pattern — every layer shuts down what it owns, the layer above holds a timeout backstop, with a descending deadline budget as an idea and not a ruling — stays its own targeted question, and the note to check what asupersync does before briefing it still stands.

### Putting it back together

Read as one thing, the design is smaller than the list of parts suggests. There is one participant shape, repeated three times with three different owned worlds. There is one message set, and the rule that anything crossing a boundary travels inside it. There is a transport contract thin enough that three implementations can honestly satisfy it, and a sequencing story that lives underneath that contract rather than inside any participant, which is what lets the same logic run over a lock, a pipe, or a wire. Reconnect is not a recovery mode but the ordinary way a participant starts. Streaming has two routes and one record, so the fast route can fail without consequence. Federation is one more hop of the same protocol, and the parts of it that are hard are schema problems living somewhere else.

The thesis that falls out is that splitting and co-locating are deployment choices over one logical model. It is falsified by any of the following, all of which are observable: a scenario that passes co-located and fails split, or vice versa; a component that needs something from another component's external world; a durable record whose facts or causal relationships differ between two configurations running the same scenario; a face disconnect that stops or corrupts the work; a reconnect that produces duplicates or gaps; the presence or absence of the direct stream changing the durable record; provider credentials observable anywhere but the brain; or any code path that exists only in the monolith or only in the split. The invariants in play are 1 (credentials brain-owned), 3 (per-consumer projections), 4 (logical roles), 9 (cancellation as a recorded, replicated fact) and 10 (no co-location assumptions), with 5 and 7 touched at the edges where the durable record and multiple faces come in.

## Parked for later stages

**Interactions flagged for stage 3:** limb-model (remote limbs are the demanding case; brain and limb are usually the same process, so the split is logical before it is physical); multi-client-ui (multiple faces on one session is a topology configuration *and* a state-model problem — the two experiments meet exactly at "two faces, one session"); operator-lifecycle (version negotiation and staged update across a split boundary); persistence-analytics (durable ordered events, provenance, cross-brain queries); self-modification (the replication protocol is shell, its data formats may be soft middle); modular-components (in-process composition is the same wiring problem seen from the testing side — strong merge candidate).

## Questions for review

- Federation (#7) is the one root drawn from a "like the idea" note rather than lived friction. Should it be scoped out of this experiment entirely and left as a schema-compatibility constraint only?
- Is the direct face↔limb stream worth carrying at this stage, or is brain-proxied streaming sufficient until latency is measured and found wanting?
- The notes leave same-machine IPC sequencing unresolved. Should this experiment be the one that settles it, given it is the fact everything else's ordering guarantees rest on?
- **Contradiction found while drilling the what, recorded rather than fixed:** why #3 says the same components run "with only the transport substituted". The detailed work says substrates differ in the *guarantees* they offer, not only in how bytes move, so what is substituted is the substrate and the logic must be written to be correct under the weakest one. Should the why be reworded, or is "transport" already meant loosely enough?
- **A second contradiction, between two whys:** #4 says rate limits, billing and session management are things there should be exactly *one* of, and cites the notes' non-goal against duplicating them. Federation (#7) puts a provider-calling brain on every machine, which reintroduces N of them — per machine rather than per project, but N nonetheless. Does federation therefore need one designated billing/rate-limit owner, or is per-machine duplication accepted as the price?
- The transport contract proposed here — per-link ordering plus at-least-once with sender-assigned identity, and nothing else — is agent-proposed; the notes are silent. Is that the contract you want participants written against?
- Is the reading of "durable compressed fallback" right: the durable compressed record always travels the brain path, the direct stream carries only disposable live data, so the durable record carries the same facts whether the fast path ran or not?
- In `face ↔ brain ↔ brain ↔ limb`, does the brain holding the limb's affinity own the session and drive the loop while the near brain is a pure proxy, or does the near brain drive the loop and use the far brain as a route to the limb?
- Streaming responses are deferred by explicit ruling, so the fast path can only be exercised for tool stdout, not model tokens. Lift the deferral for this experiment, or accept partial evidence?
- What may existing peers do in the window after a peer joins and before it signals ready? The notes do not say, and it decides whether catch-up needs to block dispatch.
- Two gaps in the required-configuration set: multiple limbs on one brain (notes: possible, not expected) is uncovered, and detaching the face is a *transition* between configurations rather than a configuration. Should either be added, given the notes hedge the detach case with "If possible"?
