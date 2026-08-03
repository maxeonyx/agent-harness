# Multi-client UI state — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why (agent-drafted, unreviewed) · what, interactions, summary — not yet done.** Derives from the multi-client and GUI passages of `source-notes/tech.md`, plus `source-notes/tui-styling.md` and `source-notes/throbber-design.md` for the UI content that attaches here.

Before this experiment, prototypes use an append-only CLI. After it, a real reactive TUI and a real web GUI share one underlying client state model.

## Why

### 1. The user wants to move between devices mid-work — *desire*

Stated directly: "I should be able to seamlessly transition between my phone (web app?) and a TUI on my desktop." The GUI should be usable as a web app and not only as a desktop app. This is the motivating story — not two people collaborating, but one person changing where he is standing while the work continues.

That framing matters because it sets the bar at *continuity* rather than at concurrent editing. The common case is sequential handover between the user's own clients; simultaneous use is possible but secondary.

### 2. Losing the user's work silently is the worst failure available — *correctness*

Invariant 7: stale clients cannot silently overwrite newer state, and the user wins on conflicting edits. The story is a phone that has been asleep with an old draft in its buffer, reconnecting and pushing that draft over something newer. Nothing warns anyone, and typed text is gone.

Silent loss is worse than a visible error because there is no signal to react to. So stale sends must be *representable* — the model has to be able to say "this arrived from a client that had not seen X yet" and act on that fact, rather than applying writes in arrival order.

### 3. Some UI state is genuinely shared and editable, which is the hard part — *correctness*

Draft buffers and file open/edit state are the difficult case: they are edited by the user, live, from possibly more than one place. The notes suggest CRDTs for the draft buffer, with the mechanism explicitly exploratory, and offer the observation that an event-based architecture is already most of the way there — "By being event based, we are essentially implementing CRDTs, and that's OK. Mostly for the live state though - as we do have a session-authoritative brain server."

That last clause is the useful constraint. There is an authoritative server, so this is not peer-to-peer convergence in the general case; it is convergence of live state under a sequencer that already exists.

### 4. Shared UI state must not become model context by accident — *correctness*

This one is easy to overlook and expensive to get wrong. Draft buffers, cursor positions and open-pane state exist for the human. If they leaked into the model's context, the agent would be reading half-typed thoughts and abandoned phrasings as though they were instructions.

This is invariant 3 applied here — an event is about its emitter, and consumers project it — but the specific requirement is a *negative* one: there must be a class of state that is shared between faces and explicitly never projected to the model. Note the deliberate tension with user-turn, which exists precisely to give the agent visibility into user activity. The line between them needs drawing exactly: what the user *did* is context; what the user is *in the middle of typing* is not.

### 5. Not everything should be shared — *resource/correctness*

Face-local state stays face-local. Scroll position, focus, transient visual state: replicating it would be both wasteful and wrong, since two clients on different screen sizes do not want the same view. So the model needs at least three classes — face-local, shared-live, and durable — which lines up with the lifecycle classes invariant 5 demands of storage.

### 6. Build the state model before committing to a reactive UI — *timing, to avoid sunk cost*

The reason this is its own experiment rather than part of building the TUI: a reactive TUI and a web GUI are both large pieces of work, and both encode assumptions about where state lives. Building either on a wrong state model means rewriting the UI, not just the model. The append-only CLI is deliberately kept until this is settled, because it makes almost no state assumptions.

## Forward: what these roots force

- **An explicit taxonomy of client state** — face-local, shared-live, durable — with every piece of UI state assigned, and a rule that shared-live is never projected to the model.
- **Causal representation of client sends.** From #2: a send carries what the client had seen, so a stale send is a recognisable fact rather than an indistinguishable write. This is the same causal-consistency machinery topology needs, applied to faces.
- **Reconnect and catch-up without duplicates**, which means an ordered durable event stream and a per-client resume point — again shared with topology.
- **A convergence mechanism for the draft buffer**, chosen empirically. CRDT is the named candidate; the event log plus an authoritative server may be sufficient. This is the experiment's central open question.
- **Two real front-ends, or the claim is untested.** A TUI and a web GUI attaching to the same session with the same state model is the exit condition; one front-end proves nothing about a shared model.
- **UI content belongs in the soft middle.** Self-modification's boundary puts "pretty much all UI content — even the TUI content" in the rapidly-iterable layer, with only the rendering framework in the shell. So this experiment should produce UI *content* that is replaceable without recompiling, which is a constraint on how the state model is exposed.

## Parked for later stages

**UI content that attaches here:** the TUI styling principles from `source-notes/tui-styling.md` — OpenCode's TUI as the reference with an Anthropic-web colour scheme, 1 row = 2 cols everywhere, half-block transitions for density, the scrollbar wanting configurable half-block end caps, opentui-rust as the intended library, and Windows-style persistent select with right-click copy/paste (never achieved in the OpenCode fork). Also the throbber design from `source-notes/throbber-design.md`: show *what the harness is waiting on* rather than generic motion — request sent but not started, model streaming, tool call running, harness internals, and user input as a distinct near-still state — with the visual approach explicitly unresolved and wanting exploration.

**Interactions flagged for stage 3:** topology (two faces on one session is simultaneously a topology configuration and a state-model problem; ordered events, reconnect and causal sends are shared machinery — these two meet exactly at "multiple faces see coherent state"); user-turn (the line in why #4 between user activity that *is* context and draft state that is not; also every user tool owns a UI projection, so user-turn generates most of the shared UI state); self-modification (UI content as soft middle); persistence-analytics (shared-UI state as its own lifecycle class, possibly a separate store); modular-components (two faces in one test process).

## Questions for review

- Where exactly is the line in why #4? A file the user has open is arguably both shared UI state *and* context that user-turn wants the agent to see. Same datum, two classes — or two different events?
- Is a CRDT worth it for the draft buffer given a session-authoritative brain already exists, or is last-writer-wins-with-causal-rejection enough for a single user moving between his own devices?
- Should the throbber and TUI styling work happen here, or wait until the state model is proven and be its own pass? They are appealing to do and not load-bearing.
