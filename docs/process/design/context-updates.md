# Context updates and progressive disclosure

Provenance: why layer, core claims, per-element table, and the aspect list reviewed with Max 2026-08-12, line by line, with his corrections folded in (recorded in REQUIREMENTS.md §"Decisions from design review"). Aspects still to review are marked unreviewed. Interactions and summary not yet written.

Sources: `docs/source-notes/context-updates.md`, `docs/source-notes/context-and-agent-loop.md`, `docs/process/REQUIREMENTS.md` §"Decisions from design review".

## What this is about

An agent's context contains stuff that can go stale — the text of a skill it loaded, an AGENTS.md, the list of available subagents. The agent finds out via a note appended to the next request that was going to happen anyway.

That note is written at the moment the request is built, not at the moment the change happens. Max edits a skill at 3pm; the session's next request is at 6pm; what to say is worked out at 6pm.

So at 6pm, to write that note, we need to know the current state of everything that feeds the context: skill files sitting on a limb, context layers from the machine or the user, the tool set, the clock. Those are the **data sources**.

## Vocabulary

- **Context contribution** — anything that goes into a context: skill content, an AGENTS.md layer, a tool description, an option set, a notice, user activity. For example: the text of the `github` skill, as sent.
- **Data source** — anything that produces contributions. A limb is one; so are machine context, user context, face-specific context, and (probably) the user-turn stream. One data source serves many sessions.
- **Initialise** — produce a context's system section (system prompt + tools). Happens on a new session, on a compaction, and as part of a refurbishment.
- **Refurbish** — transform an existing context to reduce token count, without compacting. Code-only, possibly with utility-model calls. Re-initialises the system section as part of the job. For example: dropping notices whose content is now in place.
- **Compact** — replace a context with a fresh one built from a model-written summary. The expensive rung.
- **Notice** — an appended message telling the agent that a contribution it holds has changed. For example: "Skill `github` changed (content edit, by the user)."
- **Content versions** — the record, per context, of which contributions went in and what content each had at the time. What makes "has this session seen the old version?" answerable.
- **Utility model** — a small cheap model (e.g. Haiku) used for classification or summarisation inside harness logic, never as the session's agent.
- **Data source cut** — the consistency rule for building a request: render only when every data source has reported at or beyond the triggering point (vector-clock logic, not necessarily a literal vector clock).

## Why

**Why 1 — the user changes things while sessions are live.** Max iterates on skills, AGENTS.md files, tool schemas, and prompts constantly (the process-improver stakeholder). Sessions are long-lived and many run at once. So a live agent will routinely hold facts that have gone stale. For example: an agent loaded the `github` skill an hour ago; Max has since rewritten its merge instructions; the agent is about to merge a PR the old way. Root: **correctness** — the agent's actions should follow from current reality, not from a snapshot of it. Elapsed time is a sub-case: "the time is roughly X" is a fact that goes stale ("It can be many weeks in some cases!").

**Why 2 — the cache forbids the obvious fix.** The obvious fix for a stale fact is to edit the context in place. The cache forbids that: a cached context is append-only, and editing any earlier byte forfeits the prefix. This is not a why — it is the **constraint** that shapes the solution space (append a notice, or wait for the next initialise).

**Why 3 — a quiet session must cost nothing.** Facts change whether or not any session is active. If change notices caused API requests, every edit to a skill would bill every idle session that ever loaded it. Root: **irreducible resource pressure** — the why under piggybacking.

**Why 4 — up-front content is paid by every session.** Skill and tool descriptions in real-world cases "take up massive context paid on _every_ session" (source notes). Root: the same resource pressure as Why 3, at session start instead of mid-session. This is the why under progressive disclosure, and why this doc covers both topics: they are one economics.

## What

### Core

Three levels of context maintenance, cheapest first — the original vision: keep using the warm context; **refurbish** the existing context (incorporate notices etc.); **compact** (make a fresh context). Correctness first, then the cheapest option that is correct.

1. A context is append-only while we believe it's cached. The system section (system prompt + tool schemas) changes only at an **initialise**. Re-initialising an existing context is always part of a refurbishment, never a standalone operation: replacing the system section forfeits the cached prefix, and once that cost is paid there is no reason not to reduce tokens too ("If we expect a cache miss, then there's no reason to not optimize the context somewhat" — source notes). So the three rungs are the three cost regimes, and there is no fourth: append pays 0.1× on the prefix plus a write on the delta; refurbish forfeits the prefix and pays ~1.25× on the new whole; compaction pays model output to shrink drastically.

2. A context has several cache prefixes at once, nested: the system section; everything up to any fork boundary; the whole context so far. User-facing sessions also keep a cache point ~n−2 messages back, so message undo lands on a warm prefix.

3. When a fact changes (a skill, an AGENTS.md, a tool schema, time passing): every future initialise includes it automatically — nothing to design there. Then the unique thing: a live session may additionally get an appended notice, so that the agent can know about the change. Changes can just wait for the next initialise, if waiting is safe in one of two ways: agent behavior based on the old information is kept valid (eg. the limb keeps accepting old tool calls — claim 7), or the change doesn't affect correctness of the agent's behaviour.

4. Notice decision 1 — notify at all? Only if the change could alter the agent's actions. That is what actionability means: if it would not change the agent's actions in any way, it doesn't need to know.

5. Notice decision 2 — how much does the notice carry? A spectrum: from the vaguest pointer ("something has gone stale") through a name/path, up to the information itself. The choice is economic — the same economics as claim 9, plus the agent's reaction to each form. All else equal, the minimum. Whatever the form, the agent must be able to re-discover reality reliably and cheaply — it should never have to reload everything just to be sure.

6. A notice never causes an API request. It piggybacks: appended, then carried by the next request that happens for a real reason (user message, tool result). An inactive session never pays.

7. One notable correctness example: Tool calls issued by the agent should always work. If a new version of a tool is loaded, in particular if it has a different _description_ (including schema), then limb should issue calls against the old tool version on any live session that still contains the old description. This means retaining two or more versions of the tool implementation while there is any session. Why? We don't think issuing a notice for tool description changes or tool call changes is sufficient for correct agent behaviour. Thus a fresh initialise is required for tool version changes, but we also don't want to _force_ all sessions to refurbish or compact immediately. The bound is the question "is there ever going to be another use of this tool code version, or not?" — the obligation ends when every session holding the old schema has been re-initialised or _would be re-initialised before it could be used_ (see 8.).

8. The ideal for an expired ("old cold") context: revive it as warm — re-send it exactly as it was (neither refurbished nor re-initialised; it is the event log of that session), append notices (perhaps copious), and keep going. The cost logic: compacting re-bills the whole context at input anyway; for the ~same money (cache write is ~1.25× input), pay cache write instead and don't compact. The carve-out is correctness. An old context contains old info; where that matters for correctness, notices or a refurbishment are needed. We're relatively sure tool schemas and tool presence can't be fixed via notices, and we don't want to keep old tool code versions around forever — so a refurbishment or compaction may have to be forced _if the context contains tool description content that is stale in a correctness-affecting way_. The same logic applies to any other correctness-affecting stale content — perhaps subagent description content, for example. Perhaps a user option to compact. Not fully settled.

9. The economics of notice content & frequency is based on the following. A "reference" type notice is eg. "`skill-a` has new content". A "full" notice would instead be the full new content of `skill-a`, or perhaps a diff. Choosing "reference" instead of full means: unconditionally smaller input, plus conditional billing of an extra turn (more cache read) in the branch where the agent does fetch for the full content. Content instead of a notice means: unconditionally larger input at input cost, no extra turn. Which side wins depends on how often the branch is taken. Progressive disclosure at session start is exactly this choice — descriptions up front, content on demand. Frequency of notices (ie. whether they should be debounced or not) depends on how important it is for an agent to know about the content, how likely it is to overreact to the notice, and also the raw token cost of the notices themselves.

### Constraints the code works within

Max's mental model is a dataflow graph. Whether the implementation is *explicitly* a dataflow graph is open — "that need not be explicitly a dataflow graph, but also, maybe it should be" — so the constraints below must hold either way. They exist to leave an implementer no room for a major wrong decision, and they are mostly restrictions on what the code is *allowed to know and do*.

**There is a computation graph, and we ask it for a context.** "there's a computation graph. we ask it for the 6pm context. it gets built for us." Nothing hands the graph a view of the world; the graph fetches what it needs. So no component exists whose job is to hold the whole current world on behalf of the notice logic.

**Derived by demand, and demand stands for the length of a turn.** "if there's demand, it gets computed" — and "while the agent turn is going, there's constant demand - we're streaming live updates so that the latest notice set is immediately ready to piggy back on the next request." Three consequences: nothing waits at request-assembly time, because the notice set is already current; a session with no turn running generates no demand and so costs nothing; and there is no path by which producing a notice causes a request.

**A data source presents a view at a point in time.** It "presents a view of various data at a given time, that can then be used in downstream computation". Downstream asks what a source says as of some point; it does not replay a change log. This is what makes a consistent cut expressible.

**How a source learns of a change is invisible downstream.** A source may be a file watcher, a read-on-request, or a combination — it "may or may not always listen to changes". No downstream logic may branch on which. This is also why this design is not "eventually consistent": consistency comes from the cut, not from a source pushing promptly.

**One cut per request.** A request is rendered only from a set of source views that are consistent as of the triggering point — vector-clock logic, though a hand-rolled equivalent is acceptable. A request must never mix one source's view from 3pm with another's from 1pm.

**The derivation is pure.** No I/O, no clock, no storage, no network; everything it uses arrives as a declared input, and it produces only its declared outputs. This is what makes "compute elapsed time at delivery, never at detection" unbreakable rather than merely intended.

**Structured values until the last step.** Notices are data, not text. Rendering to text is a separate projection, shared with `/dump` so the two cannot diverge.

**Policy and tunables are data.** The per-element decisions and the thresholds are values the code reads, not branches the code hard-codes — that is what makes them tunable, and what a meta-agent would tune.

**Everything has an in-memory implementation.** No code path may *require* a real filesystem or a real socket: a hash-map-backed tree and a lightweight channel must be able to stand in for them, so the whole distributed system can run in one process. Strict for core; experiments have latitude.

**No ad-hoc boundary breaking.** The above only holds if code never reaches around an abstraction for convenience. Strict requirement for core.

**Language split.** The data-source / dataflow framework is Rust; the logic within it is TypeScript on Deno.

### Per-element decisions

Each context element, against the two notice decisions (claims 4 and 5).

| Element | Notify? | Notice carries | Basis |
| --- | --- | --- | --- |
| Skill content | Only if this session loaded it | Name — or less, batched ("skills have gone stale") | Stale instructions alter actions. Never-loaded content just gets its new version at first load |
| Skill description (content never loaded) | No, as a safe general rule | — | Descriptions rarely change without content changes too, and are not usually load bearing. The next initialise gets the new version |
| New skill | Only if it would be available to this session | Name (maybe its one-line desc) | Actionability is the logical condition, but there is a practical constraint: we can't actually know whether free-text update X affects session Y |
| AGENTS.md / other limb context | Almost certainly yes | Which file/layer | More like a contract (technically the same actionability logic) |
| Tool removed | No — ~almost certainly a refurbishment or compaction | — | A breaking change to the tool schema ("tool schema" includes the tool set) |
| Tool added | Mechanism unsettled — the uncertainty is not the notify decision but whether tool addition works robustly via append at all, without breaking the prefix | — | See the reversed prefix question (questions section) |
| Tool schema changed | No — waits for a fresh initialise; the limb retains the old tool version (claim 7) | — | A notice isn't sufficient for correct agent behaviour |
| Option set inside a tool | Yes | The changed options (minimum per claim 5) | Option sets live in the context but outside the JSON schema precisely so they can change without a schema change. For example: skill names are options for the skill tool; subagent names are options for the task tool |
| Elapsed time | Yes, past a threshold — ~1h naive guess, tunable | The elapsed time itself | No retrieval path. Computed at delivery, never at detection |
| Limb identity | No — requires a fresh initialise | — | A limb is not one thing — tools, context, cwd, and more; load bearing |
| Agent role / mandate | No — requires a fresh initialise | — | The model can't be expected to respect role changes that occur later in the context |
| cwd / hostname | Low confidence: probably a different limb, therefore a different session. But a hostname change can be legit; maybe a limb can relocate; not every limb has a cwd | — | Unsettled |
| Model | Not a notice matter — changing it invalidates the cache, so a fresh initialise happens anyway | — | Mechanically a request fact; but we do want to tell the model which model it is |

### Aspects

Unordered. The point is that the approach is written down, so an implementer has no leeway for major wrong decisions. Aspects not yet reviewed with Max are marked **(unreviewed)**.

**identity of a context contribution** — A notice has to point at something, and "the `github` skill" must mean the same thing across versions, across a rename, and across two data sources that each provide a `github` skill. So an id is `(data source, kind, name-or-path)`. Consequence: a rename is a delete plus an add, so the agent sees "skill gone" + "new skill" rather than "renamed". That is genuinely confusing and unresolved — open.

**change thresholds for content notices** — Not every difference deserves a notice. Compare content by equality: the harness holds the content as contributed, so it can compare directly; hashes are only for when you don't want to keep the content around. Content-inequality is necessary but not sufficient — a whitespace-only edit clears that bar and shouldn't notify. Elapsed time needs its own threshold (~1h) because every request differs. A utility model is a candidate classifier here, for quality of both classification and summary, if the economics justify it.

**content versions** — To decide whether to notify a session, the harness must know what that session has in its context. Max edits the `github` skill at 3pm: session A loaded it this morning (holds old text — notify), session B never loaded it (holds only the description — no notice), session C started at 4pm (already has new text — no notice). Those three different calls are only possible if the harness recorded, per context, which contributions went in and what content each had at the time. This is also what the compaction briefing's diff reads: what this context believed versus what is now true.

**detection** — Sources live in a data source's environment, so a reader per data source observes them and reports current content to the brain. Detection produces facts only; it decides nothing about notices, and couldn't — a reader doesn't know what any session has seen.

**rendering** — Notices and other piggybacked contributions are computed and locked as the request is built, never at detection. `(content versions, current sources, now) → (notice block, updated content versions)`. Elapsed time is the sharpest case: its value doesn't exist until delivery.

**purity** — That render function is pure: no file reads, no clock, no storage, no network; everything arrives as an argument. This is why "compute elapsed time at delivery" stops being a rule to remember and becomes impossible to violate — there is no clock to read at detection time, because the function isn't called then.

**provenance** — Who changed it, because the right response differs: a user edit may be an instruction, a git checkout may mean the workspace moved, another agent's edit may mean coordination is needed. What is reliably knowable is three buckets: changes the harness caused itself (its own tool calls), observable git state, and unattributable external edits. Not a general audit trail. Potentially very useful to the model.

**option sets** — Ordinary contributions with identity and versions, deliberately kept out of the JSON schema so they can change without a schema change. For example: skill names are the skill tool's option set; subagent names are the task tool's. They absolutely do get notices.

**notice content** — What changed, the kind of change, who changed it, and the available action. For example: "Skill `github` changed (content edit, by the user). Reload it with the skill tool if relevant."

**actionability** — Notify only if the change could alter what the agent does. Mechanically undecidable in general — a free-text skill edit against a session halfway through unrelated work. Three implementations, increasing cost: per-element policy (the table above), default-yes, or a utility-model classifier.

**detail** — How much the notice carries: "something changed" → "the `github` skill changed" → the diff → the full content. Trades three ways: economics, distraction (a bigger notice can degrade the agent's work on its actual task), and the agent's reaction. All else equal, the minimum.

**re-discovery** — Smaller notices are cheaper when the agent doesn't need the detail, but mean it must go and get more when it does. So there always needs to be a clear, reliable path to that information. The reverse doesn't follow automatically: elapsed time could be made retrievable by a clock tool, but minimising to "time has passed" would still be wrong, because the value is a few tokens and retrieval costs a whole turn. The rule is minimise when retrieval is cheaper than carrying.

**overreaction** — Agents respond too strongly to notices: abandoning plans, re-reading everything, or refusing to use a new enum value they've been told is valid. Wording and frequency are empirical and need testing; related to the user-turn work.

**placement** — All contributions the build-time comparison finds go in one block, not one per element. The block never separates a tool call from its result. Proposed: notices before the user's message, so the user's words are the last thing read.

**channel** — Harness voice, distinct from user and agent, so a notice cannot be mistaken for the user giving an instruction. System-reminder-style is fine; a real provider channel would be better.

**debouncing** — Not a cost mechanism: render-at-build already collapses repeated edits. Its only job is behavioral — while Max is actively editing a skill, a notice on every request may destabilise the agent.

**thresholds** — Elapsed time ~1h (his naive guess, discoverable), the debounce window, a utility model's confidence bar. Recorded tunables with recorded outcomes, not constants, so a meta-agent could tune them.

**triggering** — Nothing here ever triggers a request (invariant 2). Piggyback only, so a quiet session never pays.

**data sources** — Not just limbs. A limb is one data source, serving many sessions (forked sessions especially); there is also machine context, user context, possibly face-specific context, and probably the user-turn stream. So skills have several possible sources and limb-local content is one case.

**no notifier** — There is no notifier entity. Because notices are rendered at request build, nothing needs owning between requests: notification is a step inside request assembly. A consequence of the render-at-build decision, and an example of what defining the box is for.

**data source cut** — Consistency at request build is a cut, not eventual consistency: render and send only once derived data covers a point at or beyond the trigger across every data source — vector-clock logic, though a hand-rolled equivalent is acceptable. The tool-call loop counts as a data source.

**source resolution** — The "same" skill can come from different data sources, so resolution and precedence between them is a real question. Overlaps context-layer composition (`source-notes/configuration-model.md`, flagged there as needing significant design work). **(unreviewed)**

**persistence** — Preserve enough to produce exactly the same prefix in the next API request, so cache survives restarts. Notices are stored naturally, as part of context storage.

**data lifecycle** — Resuming a weeks-old session may require storing both the input (source content, so the comparison happens at the right level) and the output (the rendered API request, for cache purposes). Max has noted this contradicts the narrower "reload sources, don't store them" position; the tension is recorded, not resolved.

**stored contexts per cache point** — Historic context state exists: one stored context per warm cache point. Superseded contexts are stored directly rather than reconstructed deterministically.

**restarts** — A relaunch must not cost warm caches: reproducing the same prefix is necessary, and so are any surrogate ids referring to cache affinity or cache points — those belong to another doc.

**prefixes** — Nested cache points per context. The provider uses the longest previously cached sequence automatically, so the harness places breakpoints but never selects a prefix.

**ladder** — Keep warm → refurbish → compact. Build for correctness first, then choose the cheapest option that is correct.

**refurbish** — A re-projection of session history into a transformed context: current sources in place, notices dropped because their content is now in place, body optionally coalesced. Not a compaction — regular code, possibly with utility-model calls. Still needs design: it mixes event stream and rollup in a messy way.

**coalescing at delivery** — Free, by construction. Ten edits between requests produce one notice describing the latest state; an edit reverted before the next request produces none. No dedup logic, no notice-expiry logic.

**coalescing at refurbish** — Different mechanism: notices roll into the system section, and edits can be elided where a later read supersedes them.

**pruning** — Considered and rejected. Inside the cached region, pruning _is_ a refurbishment. In the uncached tail, it is only profitable if the content is pruned within roughly one or two turns — and that is the freshest content, which is what you least want to prune. The forked-agent design subsumes it anyway: bulk is generated in a child context and returned as a report, so the parent never carries it. A per-tool-call summary field is unattractive because a model-written summary is output-priced.

**old tool versions** — The limb retains every tool version any live context still holds, because a notice is not sufficient for correct tool-calling behaviour. The obligation ends when no session could use it: "is there ever going to be another use of this tool code version, or not?"

**forced refurbishment** — Correctness-affecting stale description content forces a refurbishment or compaction rather than a notice — tool schemas and the tool set for certain, perhaps subagent descriptions too.

**cold contexts** — Reviving an expired context as warm is the ideal, at ~the same cost as compacting it; it is re-sent exactly as it was, neither refurbished nor re-initialised. Not settled where that stops being possible.

**undo** — The ~n−2 cache point exists so that message undo lands on a warm prefix.

**forks** — A child inherits content versions at the fork point. What prefix a fork actually inherits is an experiment.

**economics** — The contingent-choice arithmetic behind nearly every decision here: a notice instead of content is unconditionally smaller input plus a conditional extra turn in the branch where the agent fetches; content instead of a notice is unconditionally larger input and no extra turn. Frequency of the branch decides. The prices are known — cache write 1.25×, cache read 0.1×, model output ~5× — but which side wins typically requires empirical measurement or an experiment, not derivation.

**progressive disclosure** — The same economic decision at session start: descriptions up front, content on demand. Up-front content is paid by every session forever; fetched content only by the sessions that need it. Fork-proven for skills; the same trick for tools is unproven.

**analytics** — Not "we get queryability free because everything is an event". The question is what we keep, for how long, and above all why. "Keep everything" is an option, not an answer. Concretely, this feature's candidates are change facts and rendered notices, and the reason to keep them is that the overreaction question and the tunables cannot be answered without them.

**authority** — No permission model over who may change sources; personal limbs run YOLO and approval theatre is explicitly unwanted. Provenance is recorded, not gated. There is no authority model across multiple users.

**harness voice** — The harness's voice carries ground truth: the time, or "the AGENTS.md contains this content". Content being *shown* by the harness is not in the harness's voice — it stays quoted content. Channels and roles matter for that reason, and this is probably fairly obvious to the agent; the goal is simply that the agent does what Max wants.

**scenario test** — A fully black-box end-to-end test including setup, probably UI eventually, with real I/O as the boundary. It proves real usage and forces the harness to be harnessable — which in turn forces observability features that are useful for testing. It may be slow but must never be flaky, and is mostly a happy-path test.

**fast end-to-end tests** — In-memory I/O: a whole distributed system in one process. They rely on solid abstractions and require that harness code never does ad-hoc boundary breaking — a strict requirement for core, though not necessarily for experiments.

**fake network conditions** — The delayer channel implementation exists to simulate network conditions for in-process distributed-systems testing. It is not for reordering robustness: reordered data should not be a problem because a data source should not do that.

**flakes** — Virtual time, so no sleeps. The scenario test may take real time, but still no dumb waiting or polling — everything waits on an event. A flake is a bug; races are structurally excluded rather than made unlikely.

**shared doctrine** — The two testing definitions above and the purity/box doctrine are workspace-level constraints shared by every tool, so they belong in `agent-tools` workspace docs and should be referenced from here rather than restated. TODO: place them there.

## Interactions

TODO once all docs are written.

## Questions for review / needs experiment

- Is it ever possible to change anything about tools _without_ involving the cached prefix? Unanswered. For example: does mid-session tool addition work robustly via append, without breaking the prefix? (Experiment.)
- Do providers validate tool arguments against the advertised schema? Highly doubted, but unsure. (Experiment.)
- Are late system parts supported, per provider? (Experiment.)
- cwd / hostname: different limb ⇒ different session, or can a limb legitimately relocate? (Low confidence.)
- Cold-context revival (claim 8): when is the revive-as-warm ideal not possible or practical? (Not fully settled.)
- Is a refurbishment always "compact immediately prior"? ("I think it is, if our compaction is good" — see uncached compaction, REQUIREMENTS.)
