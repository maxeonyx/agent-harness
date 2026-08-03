# Context updates — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why, what (agent-drafted, unreviewed) · interactions, summary — not yet done.** Derives from `source-notes/context-updates.md` and `source-notes/context-and-agent-loop.md`. Progressive disclosure was previously implemented in the user's OpenCode fork, so that part has empirical backing.

This design covers what happens when the world changes *underneath* a running session: a skill is edited, a tool appears or vanishes, AGENTS.md changes, hours pass. The question is not whether to tell the agent — it is how to tell it without destroying the thing that makes the session cheap.

## Why

### 1. The agent's picture of the world goes stale, and acting on a stale picture is wrong — *correctness*

The story from the notes: the agent loads a skill, and then the skill is changed — by the user, by another agent, or by git moving underneath it. Nothing informs the agent, so it keeps working from content that no longer exists. The work it produces conforms to instructions that have been deliberately revised. That is a straightforwardly wrong outcome, not merely an inefficient one.

What makes this a first-order concern rather than an edge case is the rest of this harness. In a conventional harness, mid-session change is rare and mostly the user's fault for meddling. Here, the user editing files mid-session **is the designed normal** — that is the whole of user-turn. And a second agent editing shared instructions is the designed normal too, because self-modification puts the harness's own configuration under agent editing. This design is therefore load-bearing *because* of its siblings: user-turn and self-modification both manufacture exactly the situation that makes context staleness routine.

### 2. The obvious fix is illegal — the cache forbids it — *correctness under a hard constraint*

The natural response to "the context is stale" is "rebuild the context". You cannot, not in general. While the KV cache is warm the context must be treated as immutable — append-only, or at least append-only with respect to some prefix, depending on the provider's caching implementation. Rewriting the system prompt to carry the new skill content would break the cache, and the cache is what makes long sessions affordable at all (see compaction-handover's why #3).

So this design exists in a squeeze. The requirement is "correct the agent's understanding" and the constraint is "you may only add to the end". Every mechanism here is a consequence of that squeeze. This is the root that makes the design non-obvious: without the cache constraint, context updates would not be an interesting problem.

### 3. Bare-minimum notices, because injected content costs attention before it costs money — *quality, then resource*

The note is firm that new content is not included eagerly: the harness provides "the *bare minimum* for the agent to efficiently invalidate its current understanding — to know that viewing the new content is an option".

The first reason is attention, not cost. Dropping the full new text of a changed skill into the middle of a conversation actively derails the agent: it arrives as though it were the current topic, competing with the task in hand, when usually nothing about it needs acting on right now. A one-line notice leaves the agent in charge of whether the change is relevant, which is the correct division of labour — the agent knows what it is doing and the harness does not.

The second reason is genuine resource pressure, and it compounds in an unusual way: once appended, a notice sits in the cached prefix and is re-read on every subsequent request for the rest of the session. An eager content injection is not paid once, it is paid forever. This is the same arithmetic as compaction's why #3 and it points the same direction — keep appended material small, because appended material is permanent.

### 4. Some changes are load-bearing and cannot be notified at all — *correctness boundary*

Not everything can be handled by a notice. The notes list changes that are simply disallowed without a compaction or context rebuild, and the clearest is a **changed limb**: the limb determines the whole context hierarchy, and that hierarchy is load-bearing (limb-model's why #1 — a place's instructions are what let the model act correctly in that place). You cannot append "by the way, you are somewhere else now" and expect correct behaviour.

The same reasoning is why changing the **agent role / mandate** leans disallowed: "model can't be expected to respect role changes that occur later in the context." A late append does not retroactively reframe a conversation. The model's behaviour is anchored by what it was told at the start, and appending a contradiction produces an agent that is inconsistent rather than updated. Working directory and hostname are flagged as maybes on the same grounds; model change is explicitly unclear.

This root matters because it defines the design's edges. Notification is not a universal solvent — part of the deliverable is an honest classification of which context elements are notifiable and which force a rebuild.

### 5. Time passes invisibly — *correctness*

An agent has no clock. Between its last response and the user's next message, an hour or six weeks may have gone by, and nothing in the context distinguishes those cases. The failure is concrete: the agent resumes as though continuous — assuming its branch is current, that the command it ran is still meaningful, that "just now" was just now — and acts on a world that has moved. Hence the special handling: inject elapsed time past roughly an hour, below which there is no point.

### 6. The up-front context budget is contended, and it is paid on every single session — *resource, with a real tradeoff*

Progressive disclosure has its own root. The user wants a large library of skills and tools available, but skill and tool descriptions "can otherwise take up massive context paid on *every* session". Every description present up front is a permanent tax on every session that never uses it.

The tradeoff is named honestly in the notes and must not be flattened: always-up-front cost versus conditional, repeated, cached-input cost from tool calls that fetch more detail on demand. Gating skills behind more broadly applicable ones, and writing descriptions that say *when to load*, is the mechanism. Note the dependency the note draws out — this only works if the descriptions are well written, which is why it wants an information-architecture skill and a skill-writing workflow alongside. The mechanism has a documentation prerequisite.

The limb model contributes to the same goal from a different direction: a subagent given a specific limb gets a context-specific tool set, so those tools need not exist in every session at all. Progressive disclosure and the limb model are two solutions to one pressure.

## Forward: what these roots force

Chaining forward from the roots, before detailing anything:

- **Two modes, and a prediction.** Because of #2, the harness must operate in either append mode or rebuild mode, and it must *decide which* by predicting cache state. That prediction becomes a first-class piece of machinery — and it is shared with compaction, which needs the same judgement.
- **Rebuild is free exactly when you were already paying.** If a cache miss is expected anyway, there is no reason not to optimise: refresh AGENTS.md and skill content, truncate old tool output harder, canonicalise everything. So rebuild is not a fallback, it is an *opportunity* that arrives on a schedule set by cache expiry.
- **Every context element needs a declared update policy.** From #4: notifiable, notifiable-with-full-injection (the schema-changed-tool case, where a bare notice would leave the agent calling a tool wrongly), or rebuild-only. This classification is the core deliverable.
- **Change detection is required and is not free.** Something must notice that a skill file, an AGENTS.md, a tool set, or an option set changed. That is a watcher, and watchers live in the limb — which means the limb reports change and the brain decides what to do about it.
- **Notices must not trigger requests.** Invariant 2 and `context-and-agent-loop.md` are explicit: appending context is not the same as asking the model. A change notice piggybacks on the next real request. Getting this wrong turns a file save into a paid API call.
- **Rebuild must not replay history.** A rebuild produces the canonical *current* context; it must not carry forward the append-only notices that described how the old context got out of date, since they are now noise describing a superseded state.

## What

### The rule that does the classifying: can the agent act on it?

The core deliverable is an honest classification of every context element, and the classification needs a principle rather than a list of precedents, or it will not survive the first element nobody thought of.

The principle is **actionability**: a change is worth appending a notice about exactly when the agent has an action available, *within append mode*, that resolves the staleness. A changed skill is notifiable because the agent can re-read the skill. A changed AGENTS.md is notifiable because the agent can re-read the file. A changed limb is not notifiable because there is no action — you cannot read your way into being somewhere else.

This principle also absorbs the one apparent exception cleanly. Why #3's rule is "bare minimum", and the notes carve out changed tool schemas as needing "full content injection", which looks like a violation. It is not: for a schema, the new schema *is* the action-enabling content, because the action the agent needs to take is "call this tool correctly" and there is nothing to go and read that would tell it how. So the rule is not "the shortest possible string". It is **the minimum that makes the corrective action possible** — which for a skill is a path and for a schema is the schema. One rule, no exceptions, and it sharpens why #3 rather than contradicting it.

The third category, rebuild-only, is then just the residue: changes where no append can produce a correct agent. That is a smaller set than it first looks, and it has a shape, treated below.

### First, what the context is even made of

You cannot classify elements without enumerating them, and the enumeration itself turns out to be the useful part, because it splits along a line that decides everything: **is this element baked into the cached prefix, or was it fetched by a tool call?**

Baked into the prefix, and therefore frozen while the cache is warm: the agent's role and mandate; the limb's identity and instructions; the AGENTS.md set and the machine, user, and project context layers; the *descriptions* of available skills; the tool schemas, including every enumerated option set inside them; the working directory and hostname; and whatever date or time the prompt states. These are the system prompt and the tools array. Changing any of them is a wire-level change to the prefix.

Fetched by tool call, and therefore sitting in the conversation body: loaded skill *content*; file reads; command output; search results; subagent results; the user's messages and the user's in-band activity; earlier change notices; earlier handover briefings. These are all appends, and they are already immutable by construction — nothing in this design edits them.

The line matters because the two halves fail differently. A stale *prefix* element cannot be corrected on the wire at all without a rebuild; the best a notice can do is tell the agent to stop believing it. A stale *body* element can be corrected by appending a newer version, because appending is what put it there in the first place. Almost every entry in the classification below is an instance of that distinction.

### The classification

Status letters follow `README.md`: `S` settled by the notes, `P` proposed here, `E` needs an experiment.

| Element | Policy | Reasoning |
|---|---|---|
| Loaded skill content | Notifiable `S` | It got there by tool call; the agent can re-read it. |
| Skill *description* (not loaded) | Ignore until rebuild `S` | The note is explicit: "If it's in the system prompt, it's there with a desc only. We ignore desc changes until the context is rebuilt." Nothing is stale in the agent's understanding, because it never read it. |
| New skill appearing | Notifiable `S` | The agent can load it by name; the description's absence from the prefix costs discovery, not capability. |
| AGENTS.md, global / machine / user / project context | Notifiable `S` | Prefix content, but readable as a file — so the corrective action exists. |
| Tool **removed** | Notifiable `S` | See below: the dead schema can stay on the wire and the call can fail informatively. |
| Tool **added** | Not notifiable — rebuild-only `P` | See below. This departs from the note, which lists new and missing tools together. |
| Tool schema **changed** | Notifiable with full injection `S` | The schema is the action-enabling content. |
| Option sets inside a schema (agent types, limbs, other enums) | Notifiable *if* the parameter is not a frozen enum `P` | See below — this turns into a design constraint on how such tools are declared. |
| Elapsed time | Notifiable above roughly an hour `S` | The agent has an action: re-verify what it assumed was current. |
| Limb | Rebuild-only `S` | The note: load-bearing, "shan't change without a compaction / context re-build". |
| Agent role / mandate | Rebuild-only, leaning `S` | The user's own reasoning: "model can't be expected to respect role changes that occur later in the context." |
| Working directory, hostname | Not a change at all — a different limb `P` | See below. This sharpens the note's "maybe". |
| Model | Not a context change; it forces a free rebuild `P` | See below. This resolves the note's "Unclear". |
| User in-band activity | Notified by construction — it *is* an append | user-turn's business, not this design's. |
| Prior notices | Dropped on rebuild `S` | Why #6's forward drill: they describe a superseded state. |

#### Adding a tool is not the same shape of problem as removing one

The note lists "Tool availability (new tools, missing tools)" as one category. Drilling in, they are opposites, and I think the note flattens a real asymmetry.

**Removal works.** The tool is gone from the limb but its schema is still in the tools array on the wire — and it can stay there, because removing it would change the prefix and break the cache. The agent may still call it; the call fails with a result that explains the tool is no longer available. So a notice is honest and useful: it stops the agent wasting a turn, and if it does waste one, the failure is informative. Nothing about the wire has to change.

**Addition does not work.** A model cannot call a tool that is not in the tools array, and putting it there is precisely the prefix change the cache forbids. So a notice saying "a new tool is available" would be a *false* notice — the tool is not available to this session. The most it could honestly say is "a tool exists that this session cannot use until its context is rebuilt", which is nearly useless as an instruction but is genuinely useful as one input to the agent's decision to hand over. So my proposal is: additions are rebuild-only, and if they are surfaced at all it is as a *fact about the value of rebuilding*, not as a capability notice. This is a departure from the note and it is in Questions for review.

The same asymmetry explains the schema case, and it is worth being precise because it is easy to design wrong. When a schema changes, the wire array still advertises the old shape while the injected content describes the new one. That only works if **execution validates against the limb's current schema, not against the advertised one** — otherwise a model faithfully following the injected new schema gets its call rejected for not matching a stale advertisement. So the wire array is best understood as a possibly-stale *advertisement*, and the limb's current schema as the truth at execution time. A call shaped to the old advertisement fails with a corrective result; a call shaped to the injected new schema succeeds. The user's own instinct in `context-and-agent-loop.md` points the same way — "maybe update tool call schemas (although I think that's confusing, because the chat likely contains tool calls - appending is probably still correct here)" — and the hedge is worth keeping: he is not certain, and neither am I. `E`.

#### Option sets are frozen sets, and that is a declaration problem

The note wants notices for "Avaialble agent types for subagent tool, available limbs for subagent tool, other tool option sets etc." Follow that through and it collides with the prefix rule: if those options are declared as JSON-schema enums, the enum is in the tools array, the array is frozen, and a new agent type simply cannot be selected — the same wall as adding a tool. Even documenting the options in the parameter *description* does not help, because the description is in the array too.

The way out is not a notice mechanism, it is a declaration convention: **sets that can change during a session must not be baked into the schema.** Declare the parameter as a free-form string, keep the valid values discoverable through a listing tool (or through the same limb that will validate the call), and validate at execution time. Then a notice about a new agent type is immediately actionable, because the parameter already accepts it.

That generalises into a rule worth stating on its own, because it is the same rule that governs progressive disclosure: any *set* baked into the prefix is frozen until rebuild, and any set fetched by tool call is live. Proposed here; `P`.

#### Why role and limb cannot be appended, but cwd and model are different questions

The user's reasoning about role is the sharpest thing in the note: "model can't be expected to respect role changes that occur later in the context." A late append does not retroactively reframe a conversation — it produces an agent holding two mandates, which is worse than one stale mandate. Limb fails for the same reason plus a stronger one: the limb determines the whole context hierarchy, and appending "you are somewhere else now" leaves every instruction above it describing the wrong place.

The distinction underneath is between changes that are **referential** (a fact the agent used is now different) and changes that are **behavioural** (who the agent is, or where it is acting, has changed). Referential changes append fine, because models handle "that fact has been updated" well. Behavioural changes do not, because they do not update anything — they contradict.

That distinction lets me push on the two entries the notes leave unsettled.

**Working directory and hostname.** The note says "Maybe likewise" — i.e. maybe disallowed. Following the limb model through makes this more than a maybe: a limb is identified by `ssh_host` plus `directory` (`source-notes/agent-harness-design.md`), and a session is bound to exactly one limb. So a changed working directory or hostname is not a change to a session's context; it *is a different limb*, and therefore a different session. There is nothing to notify because the situation cannot arise: you do not move a session, you start one elsewhere. (The genuine edge case is the directory being renamed or deleted underneath a live limb, which is not a context update either — it is a limb that has lost its identity, and the honest response is limb failure.) This makes the user's "maybe" a "yes, and for a stronger reason than he gave", which is the kind of sharpening that wants his ruling; it is in Questions for review.

**Model.** The note says "Changed model? Unclear." Current truth already resolves this from an unexpected direction: the walking-skeleton ruling recorded in `REQUIREMENTS.md` is that `model` and `reasoning_effort` are **request facts, not context facts**. So changing model does not mutate the context at all, and there is nothing to notify. What it does do is invalidate the cache, because caches are per-model — which means a model change lands the session in rebuild mode automatically, and rebuild there is free in exactly the sense of the next section. So the answer is "rebuild-only", but not because appending would be wrong; because you are already paying. Worth checking against invariant 3's parenthetical that projections may be per-model: if the projection differs per model, the rebuild is not merely free, it is required. `P`.

### What a notice actually says

A notice is one line where one line will do, and its content is fixed by the actionability rule: it must carry enough for the agent to decide whether to act, and enough to act if it decides to.

Four fields do that work. **What** — the element, identified the way the agent would refer to it (a skill name, a file path). **What kind of change** — changed, appeared, disappeared. **Who or what changed it** — and this is not decoration: the note's own story names three sources, "by the user or another agent or by git", and they imply different responses. The user editing a skill mid-session is a deliberate instruction to work differently; git moving underneath you is a branch change that probably invalidates more than the one file; another agent editing shared instructions is a coordination event. **What action is available** — usually implicit in the element type, worth being explicit where it is not.

What a notice must *not* contain is the new content, except in the schema case. Why #3's first reason is attention, not cost: content arriving mid-conversation reads as the current topic and competes with the task in hand.

Two mechanical points. Notices are **batched and placed at the next request**, appended as a single block after any tool result rather than between a tool call and its result — the walking skeleton established that tool-call/result adjacency must not be split on the wire, and notices are the obvious thing that would split it. And notices carry the **harness's** voice: they are facts about the harness's observation, not the user speaking. If a provider forces them onto the user channel, that is a projection choice, and the recorded fact must still be attributed correctly (invariant 3: an event is about its emitter). This is the same problem compaction-handover has with its briefing, and it looks like one mechanism.

#### Notices are a diff, not an event log

This is the mechanism I would most want reviewed, because it makes several otherwise-awkward problems disappear.

The obvious implementation is a queue: something notices a file changed, pushes a notice, and the queue drains at the next request. That implementation has three unpleasant behaviours. A file saved forty times produces forty notices to coalesce. A file changed and then changed *back* produces notices about nothing. And a session left idle for a week accumulates a queue whose size is a function of how much unrelated activity happened on the machine.

Compute the notice set instead as a **diff at flush time**: compare the world as it is now against what this context actually contains. Then forty saves produce one notice, a change-and-revert produces silence, and idle time produces nothing at all because there is nothing to accumulate. Coalescing is not a feature, it is the absence of a problem.

This does impose a storage requirement, and it is the same one the note implies anyway when it says notification is warranted only "if the agent has loaded the skill": the context must know **what version of what it contains**. Every append that carried config-ish content — a skill load, an AGENTS.md read, a schema — records what it was and a content hash. That is the left-hand side of the diff. It is also, satisfyingly, the same machinery compaction-handover needs for its old→new briefing diff: one is a diff between the context and the world, the other a diff between the context and its successor.

### A notice is never a reason to call the model

Invariant 2 and `context-and-agent-loop.md` are unambiguous, and the note lists "tool schema changes" and "process config changes" among the things that only piggyback. So: change detection never triggers a request. Notices ride on a request that was going to happen anyway — a tool-loop continuation, a user turn ending, a proactive handover.

The consequence that is easy to miss, and pleasant once seen: if the session never sends another request, the notices are **never paid for**. Under the diff formulation they are never even constructed. A file save in a project whose session has gone quiet costs nothing, which is exactly right, and it is the difference between a design where the user can edit freely and one where saving a file costs money.

The failure mode to test for is the trivial one and it is worth a black-box test rather than an argument: save a file, observe zero requests on the fake provider's log; then end a turn, observe one request carrying the notice.

### Time

The rule is the note's: inject elapsed time past roughly an hour, "less no point". The action it enables is re-verification — the agent's branch may have moved, its running command may be long dead, its "just now" may have been six weeks ago.

One mechanical point that the diff formulation makes obvious: elapsed time is a fact about *now*, so it must be computed at flush, never at queue. A notice constructed when the gap was two hours and delivered when the gap is three days would be actively misleading. Under the diff formulation this is automatic, which is a small piece of evidence that the formulation is right.

Two things the notes do not settle, recorded rather than invented. What form the injection takes — elapsed duration, absolute timestamp, or both — where "both" is probably right, since the agent may care that it is now Monday as well as that six days passed. And whether the *stated date* in the system prompt is itself a notifiable element for a session that spans midnight; by the actionability rule it is (the agent can update its belief and there is a real action: stop asserting yesterday's date), but nobody has ruled on it.

### Rebuild is the opportunity, not the fallback

Why #6's forward drill already names this and it is worth holding onto because it inverts the intuition. Rebuild is not what you do when notices fail. It is what you do when you were **already going to pay for a cache miss**, and at that moment there is no reason not to take everything: refresh the AGENTS.md and skill content, refresh the schemas, canonicalise the option sets, truncate old tool output harder. The note's phrasing: "If we expect a cache miss, then there's no reason to not optimize the context somewhat."

Which makes the interesting property of rebuild its *schedule*: it arrives whenever the cache lapses, whenever the model changes, whenever a handover happens, whenever a fresh session starts. The design job is to make sure those moments are used fully rather than to make rebuild rare.

Two rules constrain what a rebuild produces.

**A rebuild produces the canonical current context, and canonical means it looks as though the session had started now.** In particular it must not replay the append-only notices, which described how the *old* context became stale; carried forward, they are noise describing a superseded state, and they would accumulate across successive rebuilds. Nor should it carry a notice's *effects* as a special case: if a skill changed and the agent re-read it, the rebuild contains the current skill once, not the old version plus a notice plus the new version.

**Rebuild has to be deterministic and repeatable**, because compaction-handover needs to run it as a dry run in order to describe it, and a rebuild that produced different output the second time would make that briefing a lie. This is a shared requirement rather than a coincidence, and both designs depend on it.

The one thing the notes explicitly leave open is what rebuild does with the conversation *body*. "Truncate old tool calls harder" is named, but how hard, and whether truncation is reversible, is not. Nor is whether a rebuild may re-order or drop body content at all — my reading is that it may truncate but may not re-order or reinterpret, because the body is the record of what happened, but that is a proposal. `P`.

### Progressive disclosure

Progressive disclosure arrives from a different root (why #6) but lands in the same machinery, which is the point of the closing section below.

The mechanism is the note's: not everything is available up front; skills carry descriptions rather than content; skills can be **gated behind other, strictly more broadly applicable skills** being loaded first; and descriptions "need to say when to load" rather than what the skill contains. The limb model contributes from the other side — a subagent with a specific limb gets a context-specific tool set, so tools irrelevant to that limb need not exist in that session at all.

The tradeoff must not be flattened, and the note states it exactly: "a careful balance between always up-front input cost and conditional repeated cached-input cost from tool calling to get more info". Both sides are real. Up-front cost is paid on every session including the ones that never use the thing. On-demand cost is paid per fetch, plus a round trip, plus permanence — a fetched skill sits in the cached prefix for the rest of the session, so an unnecessary load is a permanent tax, the same arithmetic as why #3. Progressive disclosure is therefore not strictly cheaper; it moves cost from *certain and universal* to *conditional and permanent-once-incurred*, and it wins when the conditional probability is low. The design consequence is that the balance is a measurement, not a principle, and the experiment should measure it rather than assert it.

The note also names a **prerequisite that is not a mechanism at all**: this only works if the descriptions are well written, which is why it wants "an info architecture skill and a skill writing workflow that helps motivate & get this correct". That is worth taking seriously as a design fact rather than a nice-to-have: a gating mechanism whose gates are described badly is worse than no gating, because the agent neither loads what it needs nor knows what it is missing. So the deliverable here has a documentation half, and pretending otherwise would be dishonest about why the mechanism works in the fork.

`source-notes/context-updates.md` records that this was "previously implemented in my opencode fork", so the mechanism has empirical backing and the fork is where to look before inventing (`F`, per `source-notes/open-code-inspiration.md`). What has *not* been validated is the same trick for tools, which the note says "can & should be done" — that is the newer half.

### The two halves of this design are one axis

The source note has two sections, "context changes" and "progressive disclosure", and they read as separate concerns. They are not. They are the same axis seen from two ends.

Everything baked into the cached prefix is **cheap to have and frozen once you have it**. Everything fetched by tool call is **expensive per fetch and live**. Progressive disclosure is the decision about which side of that line each piece of content starts on. Context updates is the machinery for the consequences: things on the frozen side can only be *pointed at* by a notice, things on the live side can be *replaced* by an append, and the only way to move something across the line is a rebuild.

That is why the same three mechanisms keep appearing. The **diff** — between context and world for a notice, between context and successor for a handover, between old and new prefix for a rebuild — is one mechanism, and it is the only way to know what is stale without replaying history. **Cache-state prediction** is one mechanism, and it decides append versus rebuild here exactly as it decides when to compact and whether to fork. And the **actionability rule** is one policy, and it decides notice content, notice existence, and how tools and option sets must be declared in the first place.

Read that way, the design has a single sentence: *the context is a frozen prefix plus a live tail, and every mechanism here is either a diff against the frozen part, a decision about when the freeze lifts, or a rule about what to put where.* Everything above is detail on those three.

### The cache is genuinely unknown, and that is the experiment

This design rests on cache semantics nobody here knows, and the honest move is to mark them as questions rather than decide them. `context-and-agent-loop.md` already hedges the central one: the context must be treated as "append only (or at least, append only with respect to some prefix - this depends on the provider's caching implementation and needs experimenting)".

What needs finding out, none of which this doc decides:

- **What append mode is append-only with respect to.** Whether a provider's cache tolerates appends at all without an explicit breakpoint, and if breakpoints are explicit and limited in number, where they should go and what that costs in flexibility.
- **Whether the tools array participates in the cached prefix**, which is the assumption the whole added-versus-removed asymmetry above rests on. If it does not, additions become notifiable after all and that part of the classification changes.
- **Whether providers validate tool call arguments against the advertised schema.** The changed-schema policy assumes execution-time validation against the limb's current schema is possible; if the provider rejects first, the policy needs rethinking.
- **How long entries actually live, and how that is observable.** Feeding the prediction machinery described in compaction-handover.
- **What append mode means for a fork.** The note's own guess is "possibly the parent *as of the message before it sent the subagent tool call* - but that needs experimenting too."
- **Whether late system parts are supported**, per provider, since that decides the channel for both notices and compaction briefings.

Every one of these is cheap to test against real providers and impossible to settle by reasoning, which makes them the natural first work of the experiment rather than a risk to it.

### What this makes falsifiable

The thesis: an agent in a warm session can be kept honest about a changing world using nothing but small appended notices plus a rebuild taken opportunistically when the cache lapses — and doing so is cheap in the common case and does not derail the work.

It is falsified if: an agent given a notice fails to act on a change that mattered, or acts on one that did not and derails; notices accumulate to a size that matters, which would mean the diff formulation failed; a notice triggers a request, directly contradicting invariant 2; a rebuild replays obsolete notices or otherwise fails to be canonical; the appended-schema policy produces calls the provider or the limb rejects; or progressive disclosure measurably costs *more* than always-up-front for a realistic skill and tool library, which is the outcome the note's own honest framing of the tradeoff admits is possible.

Invariants touched: **2** (the whole no-trigger rule, and the piggyback path), **3** (a notice and a rebuild are two projections of the same change facts; the per-model parenthetical bears on the model question), **5** (content versions and hashes must be durable and queryable, since the diff is computed against them), and **10** (everything the model sees must be derivable from the session record — a notice whose meaning depends on limb-local state would violate this, which is why change facts travel in the message).

## Interactions

### What this design owns, and what it assumes

This design owns the policy layer: the actionability rule, the classification of every context element by update policy, what a notice says and when it is flushed, the diff-at-flush formulation, the rule that a notice never triggers a request, what a rebuild produces, and progressive disclosure. It also owns one rule that reaches outward into somebody else's tool surface — that a set which can change during a session must not be baked into a schema — and one mechanism that a sibling consumes: the harness voice, developed here rather than in compaction-handover because notices are the high-frequency case and the late-system-part provider probe is already on this design's list of unknowns. That placement is a call rather than a ruling; it is in Questions for review.

What it assumes rather than tests is short and specific. Cache-state prediction is shared machinery (`INTERACTIONS.md`); this design needs it only as a two-way decision — append or rebuild — with the honest caveat that the decision is an expected-value bet. The durable record of what-version-of-what a context contains is persistence-analytics' schema; this design assumes content hashes and epoch anchoring exist and tests the *behaviour* the diff produces from them. The dry-run rebuild is compaction-handover's consumer, not this design's feature; what this design owes it is determinism and canonicality, which are already stated above. And change detection, though required here, is hosted by the limb: the limb reports change and the brain decides what to do about it, so limb-model owns the message surface and this design owns the requirement that lands on it.

### Self-modification proposes the same mechanism from the other side

This is the interaction that changed the most, and it changed by agreement rather than by conflict.

Self-modification classifies every plugin reload into schema-identical, schema-additive, or schema-breaking, and those three land on rows in the classification above. A schema-identical change needs no notice at all, because nothing the agent can observe changed — the wire is untouched and the behaviour behind an existing call is simply current. A schema-additive change — a new tool, or a new optional parameter on an existing one — has the shape of the tool-added row: the addition is not on the wire, so a warm session cannot use it, which is the rebuild-only conclusion this design reaches and the one self-modification reaches independently. A schema-breaking change is either the changed-schema row with full content injection, or the explicit cache break self-modification reserves for exactly this case.

More striking is that its central call — pin the schema, run the newest implementation — is the same policy as this design's: the wire's tools array is a possibly-stale advertisement, and the truth at execution time is the limb's current schema. Two designs arriving at one mechanism from unrelated roots is the strongest evidence either has for it. It also means the risk is shared and correlated: if a provider validates tool call arguments against the advertised schema, both designs lose their central move at the same moment. That is the single most valuable thing the provider probe can tell us, and it should be probed before either commits.

Invariant 2 lands identically on both. A plugin reload is a context change, so it must never trigger a request — the same rule as a file save, for the same reason.

### User-turn: two facts about one edit, and why the diff formulation makes them compose

Why #1 says this design is load-bearing *because* user-turn makes mid-session change routine. Developing that turns out to narrow the interaction rather than widen it, which is the useful result.

User activity is notified by construction: it *is* an append, and user-turn owns projecting it. So the agent already learns that the user edited a file. What this design adds is orthogonal — that the edited file may also be a context element the agent's understanding depends on, which is a different fact about the same edit. The user editing a loaded skill produces one activity projection (he edited it, here is the diff) and would otherwise also produce one notice (the skill you loaded has changed, re-read it).

The diff formulation makes those compose without any deduplication logic, and that is not a coincidence so much as a consequence. A notice is computed at flush time by comparing the world against what the context actually contains. If user-turn's activity projection has already carried the new content into the context, the comparison is empty and no notice fires. If it carried only a summary or a partial diff, the comparison is non-empty and a notice fires correctly. Neither design has to know about the other; the shared left-hand side does the work. A queue-based implementation would have needed an explicit rule here, which is a small piece of evidence for the formulation on top of the arguments already given.

### Forked-subagents: a rebuild must drop notices, and a fork must not

Fork is append mode with respect to an ambiguous baseline, and the ambiguity is forked-subagents' to resolve. But one consequence belongs here, and it is sharp enough to be worth stating plainly, because the naive reading gets it backwards.

A rebuild must drop prior notices, because they describe how a superseded context went stale. A fork must *not* drop them, because the entire economic point of forking is prefix identity with the parent — and a child whose prefix differs from the parent's by the removal of some notices has no shared prefix at all. So the same content is noise in one operation and load-bearing in the other, and the distinction is not about the content's usefulness but about whether the operation is allowed to change bytes. Rebuild is the only operation that may.

A second consequence follows for the diff's left-hand side. A forked child inherits the parent's conversation, so it must also inherit the parent's record of what-version-of-what that context contains, or the child's first flush will re-notice every skill and AGENTS.md the parent already knew about. The baseline forks with the context.

The declaration rule also lands here rather than staying local. Agent types, limbs and other option sets appear as parameters on forked-subagents' Task tool, and the rule above says those parameters cannot be schema enums if the sets can change during a session. So this design owns the rule and that design's tool declaration has to conform. That crossing is already in Questions for review; what stage 3 adds is which side owns which half.

### Limb-model: the watcher's home, and one row that cannot fire

Change detection lives in the limb, which suits the limb's existing role — it owns an environment and already contributes unsolicited facts about it. Limb-model's proposal that limb-contributed context arrives as labelled blocks naming the layer that produced it is what makes a notice able to say *which* thing to re-read, so the actionability rule depends on that labelling existing.

The other consequence is a tidy piece of subtraction. A changed limb is the canonical rebuild-only change, but a limb is identified by host plus directory and a session is bound to exactly one limb, so the situation cannot arise inside a session. That row documents a boundary rather than a case, and the honest reading is that the rebuild-only category is smaller than the notes imply. The role row goes nearly the same way: a session's agent type is fixed at launch, and the only route to a different one is resuming as a different agent type, which forked-subagents treats as creating a new session rather than mutating a running one. That leaves tool additions as the only rebuild-only change that routinely happens inside a live session.

### Compaction-handover, and the schedule that makes rebuild-only tolerable

The reciprocal requirements are developed in that doc and not repeated here. What belongs on this side is a dependency that is easy to miss: rebuild-only is only an acceptable answer because a rebuild is a *scheduled* event. Compaction is what schedules it. If handover quality has to be demonstrated before the proactive cache-driven trigger is enabled — which compaction-handover argues — then rebuild arrives less predictably than this design assumes, and the rebuild-only rows are stale for longer.

### The cells that are empty

Multi-client-ui and this design barely meet. Draft buffers and pane state are not context elements, so there are no notices about them, and the negative requirement runs the other way: shared-live state must never reach a projection, which is a constraint on where that state lives rather than on what this design says about it. Two faces means the user can edit from two places, but that is still just a file change.

Topology contributes nothing beyond an invariant already honoured: change facts travel in the message rather than by reference to limb-local state, which is what invariant 10 requires and what the diff formulation already does. Oauth-credentials, cancellation-economics and layered-shutdown have no relationship with this design at all.

Two thin ones are worth a line each because both are counterintuitive. Operator-lifecycle: a relaunch is *not* a rebuild boundary. The provider-side cache handle is durable, so a session resumes with its context intact and its prefix still warm — restarting the harness does not entitle it to rebuild. And modular-components: the no-trigger rule's central assertion — save a file, observe zero requests; end a turn, observe one carrying the notice — depends on being able to inspect the provider wire *between* steps, which is a property that design has to preserve when the suite moves in-process.

### The unknown this design sits under

Everything above rests on cache semantics nobody here has measured, and `INTERACTIONS.md` records that as the portfolio's largest shared unknown rather than as a risk local to this doc. The pieces this design specifically cannot proceed without are whether the tools array participates in the cached prefix, which the added-versus-removed asymmetry rests on entirely, and whether providers validate arguments against the advertised schema, which the changed-schema policy rests on. Both are cheap to test and impossible to reason out.

## Questions for review

- Is the attention-before-cost framing of why #3 right, or is the honest root simply resource? The notes emphasise cost; the attention argument is mine.
- Why #1 claims this design is load-bearing *because* user-turn and self-modification create routine mid-session change. That reframes context-updates from a good-taste item to something closer to soul. Do you agree, and if so should it move bucket in `PLAN.md`?
- Should cache-state prediction be its own experiment? It is now required by context-updates, compaction-handover, and forked-subagents alike, and all three are blocked on the same unknown provider cache semantics.
- **Adding a tool is rebuild-only, in my analysis, and the note treats additions and removals together.** A removed tool can keep its schema on the wire and fail informatively; a new tool cannot be called at all without changing the frozen tools array. Do you want additions surfaced anyway as an input to the decision to hand over, or suppressed entirely until rebuild?
- **Option sets must not be declared as schema enums** if they can change mid-session — otherwise a notice about a new agent type or limb is unactionable. That is a constraint on how the subagent tool is declared, arriving from this design into another one, and it wants your ruling before it becomes assumed.
- **Working directory and hostname: I have turned your "maybe" into a "yes, and it isn't even a change".** A limb is identified by `ssh_host` + `directory` and a session is bound to one limb, so a moved session is a different session. Is that the reading you intended, or is there a case where a session's directory legitimately changes?
- **Model change: I have resolved your "Unclear" using the walking-skeleton ruling** that `model` is a request fact, not a context fact — so it is not a context update at all, just an automatic free rebuild via cache invalidation. Worth confirming, and worth checking against invariant 3's "possibly per-model" projections, which would make the rebuild required rather than merely free.
- **Notices as a diff rather than a queue.** This is the mechanism I would most want you to look at. It makes coalescing, revert-to-original, and long idle periods all disappear as problems, but it requires the context to record content hashes for everything config-ish that it contains.
- **The appended-schema policy assumes execution-time validation against the limb's current schema**, treating the wire's tools array as a possibly-stale advertisement. Your own note hedges here ("appending is probably still correct here") and I have kept the hedge; this is the piece most likely to break against a strict provider.
- **The harness voice.** Same question as compaction-handover: notices and compaction briefings are both the harness speaking, and both need a channel that does not attribute them to you. One mechanism or two?
- **Rebuild and the conversation body.** I have proposed that a rebuild may truncate body content but may not re-order or reinterpret it, because the body is the record of what happened. The notes only say "truncate old tool calls harder".
- **Progressive disclosure has a documentation prerequisite**, per your own note. I have treated that as a design fact rather than an adjacent nice-to-have — the mechanism does not work with badly written descriptions. Does the experiment therefore need the info-architecture skill and skill-writing workflow in scope, or does it validate the mechanism on hand-tuned descriptions and defer the workflow?
- **Is the frozen-prefix/live-tail synthesis the right way to unify the note's two halves?** I have argued context changes and progressive disclosure are one axis rather than two topics. If you agree, this doc's structure and the experiment's shape both follow from it; if not, they should be pulled apart again.
- **A rebuild drops prior notices; a fork must keep them.** The same content is noise in one operation and load-bearing in the other, because a fork's whole economic point is byte-identical prefix with the parent. I am confident in the reasoning but it is mine, and it puts a constraint on forked-subagents' fork encoding: whichever encoding wins, it cannot tidy the parent's context on the way in.
- **The harness voice is developed here rather than in compaction-handover.** Both designs need it; I have put the mechanism in this one because notices are the high-frequency case and the late-system-part probe is already on this design's list, with compaction consuming it. That is a placement call on top of the earlier one-mechanism-or-two question.
- **The rebuild-only category may be almost empty.** Limb and role changes are both structurally unreachable inside a session, which leaves tool additions as the only rebuild-only change that actually occurs. If that is right, the classification's third category is a boundary statement rather than a working policy, and the experiment has much less to demonstrate there than the notes imply.
- **Rebuild-only is only tolerable because compaction schedules rebuilds.** Compaction-handover argues the proactive cache-driven trigger should wait until handover quality is proven. If that holds, rebuilds arrive less predictably than this design assumes and stale-for-longer becomes a real cost — which may mean the rebuild-only policy needs a fallback rather than just a schedule. Worth your ruling on whether that is acceptable as it stands.
