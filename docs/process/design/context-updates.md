# Context updates — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why, what, interactions, summary (agent-drafted, unreviewed). (Style rewrite 2026-08-04, content unchanged.)** Derives from `source-notes/context-updates.md` and `source-notes/context-and-agent-loop.md`. Progressive disclosure was previously implemented in the user's OpenCode fork, so that part has empirical backing.

This design covers what happens when the world changes *underneath* a running session. A skill gets edited. A tool appears or vanishes. AGENTS.md changes. Hours pass.

The question is not whether to tell the agent. It is how to tell it without destroying the thing that makes the session cheap.

## Vocabulary

Seven terms get used throughout. They are defined here so that no sentence later on assumes you already know them.

**Context.** Everything the model receives on a request. That is the system prompt, the tool schemas, and the whole conversation so far.

**Prefix.** The front part of the context. It is byte-for-byte identical from one request to the next. In practice it is the system prompt plus the tools array. Providers cache it, which is why it matters.

**Body.** Everything after the prefix. The conversation itself: messages, tool calls, tool results.

**Warm cache.** The provider is still holding its cached copy of our prefix, so re-sending that prefix is cheap. **Cold**, or **lapsed**, means it is not holding it any more, and the next request pays full price for those tokens.

**Append mode.** Adding to the end of the context and changing nothing before it. This keeps the cache warm.

**Rebuild.** Constructing the context again from scratch. This loses the cache. When we choose to do it on purpose, the cost is called a **cache break**.

**Notice.** A short block that the harness appends in order to tell the agent that something it was relying on has changed.

**Flush.** The moment just before a request is sent. Pending notices are assembled and appended at flush.

## Summary

### The shape of a context explains everything else

A context is a **frozen prefix plus an append-only body**.

The prefix is frozen because the cache is warm. Baked into it are the agent's role, the limb's instructions, the AGENTS.md set, the skill *descriptions*, the tool schemas, and every enumerated option inside those schemas. While the cache is warm, none of that can be touched.

The body got there by being appended. It can be added to. It cannot be edited or removed.

Three consequences follow, and they are the whole design in miniature:

1. Prefix content can be **pointed at** but not corrected.
2. Body content can be **superseded** by appending a newer version.
3. Only a rebuild removes anything, or moves content across the line between prefix and body.

### Why any machinery is needed at all

There is a squeeze.

On one side, the world changes underneath a running session. The user edits a loaded skill. Another agent rewrites shared instructions. Git moves. Hours pass. An agent acting on a stale picture is straightforwardly wrong, not merely inefficient.

In this harness that is the designed normal rather than an edge case. Two sibling designs manufacture it deliberately: user-turn makes the user editing files mid-session the point, and self-modification puts the harness's own configuration under agent editing.

On the other side, the obvious fix is expensive. Rewriting the prefix to carry the new content breaks the cache, and the cache is what makes long sessions affordable.

So the requirement is "correct the agent's understanding". The working constraint is "you may only add to the end".

### The rule that resolves the squeeze

The rule is **actionability**. Append a notice exactly when the agent has an action available, within append mode, that resolves the staleness.

The notice carries the minimum that makes the corrective action possible. For a changed skill that is a path. For a changed tool schema it is the whole new schema, because there is nothing else the agent could go and read that would tell it how to call the tool.

The rule has one parameter: whether an element is worth notifying about at all. Where that line sits is deliberately debatable.

Running alongside it, and independent of it, is one more question: does this change also raise the value of **rebuilding sooner**?

Those two together — one policy, one orthogonal axis — are the deliverable. Not a three-way classification.

### The furthest-reaching output is a declaration rule

Any *set* baked into the prefix is frozen along with the prefix.

So a set that can change mid-session must not be declared as a schema enum. Declare it free-form. Keep the valid values discoverable by tool call. Validate at execution time.

That same policy is what the changed-schema case needs. The wire's tools array is a possibly-stale **advertisement**. The limb's current schema is the truth.

One level up, it is also why a newly added tool is a cost decision rather than a wall.

### Two mechanisms make notices behave well

The first is that notices are computed as a **diff at flush time**. We compare the world as it is now against what this context actually contains. We do not queue change events.

That makes three problems disappear rather than needing to be handled. Forty saves of one file produce one notice. A change followed by a change back produces silence. An idle week accumulates nothing.

The second is that a notice is **never a reason to call the model**. It rides on a request that was going to happen anyway. This is the difference between a design where the user can edit freely and one where saving a file costs money.

The diff formulation has a price. The context must record what version of what it contains. That is the same record compaction's briefing diff reads.

### Rebuild is an opportunity, not a fallback

A rebuild is not what you do when notices fail.

It is what you do when you were **already going to pay for a cache miss**. At that moment there is no reason not to take everything.

So the interesting property of a rebuild is its *schedule*. And what it produces must be **canonical** — it should look as though the session had started now, which in particular means it must not replay the notices that described how the superseded context went stale.

That schedule is what makes "wait for the rebuild" an acceptable answer at all. Which leaves this design quietly dependent on compaction firing on time.

### Progressive disclosure lands in the same machinery

Progressive disclosure arrives from a different root, and that is why it belongs in this doc rather than its own.

The up-front context budget is paid on *every* session, including the sessions that never touch the skill in question. So skills carry descriptions rather than content, descriptions say *when to load*, and skills can be gated behind more broadly applicable ones.

The tradeoff must not be flattened. A fetched skill then sits in the prefix for the rest of the session. So this moves cost from certain-and-universal to conditional-and-permanent. It wins only when the condition is rare, which makes it a measurement rather than a principle.

It also has a prerequisite that is not a mechanism at all. The gates only work if the descriptions are well written.

### The largest risk

All of this rests on cache semantics that nobody here has measured.

Two matter most. Whether the tools array participates in the cached prefix — the added-versus-removed asymmetry and the declaration rule both rest on it. And whether providers validate tool arguments against the advertised schema — the changed-schema policy rests on that.

Both are cheap to test against a real provider. Both are impossible to settle by reasoning. That makes them the first work rather than a risk to the work.

## Why

### 1. The agent's picture of the world goes stale, and acting on a stale picture is wrong — *correctness*

Here is the story from the notes, step by step.

The agent loads a skill. The skill is then changed — by the user, by another agent, or by git moving underneath it. Nothing informs the agent. So the agent keeps working from content that no longer exists. The work it produces conforms to instructions that have been deliberately revised.

That is a wrong outcome. Not an inefficient one.

Now, why is this a first-order concern rather than an edge case?

In a conventional harness, mid-session change is rare, and mostly it is the user's fault for meddling.

Here it is not rare. The user editing files mid-session **is the designed normal** — that is the whole of user-turn. And a second agent editing shared instructions is also the designed normal, because self-modification puts the harness's own configuration under agent editing.

So this design is load-bearing *because* of its siblings. User-turn and self-modification both manufacture exactly the situation that makes context staleness routine.

### 2. The obvious fix costs the cache — *correctness under a priced constraint*

The natural response to "the context is stale" is "rebuild the context".

You can. It is never impossible. But it is not free, and therefore it cannot be routine.

While the KV cache is warm, the context has to be *treated as* immutable. That means append-only — or at least append-only with respect to some prefix, which depends on the provider's caching implementation.

Rewriting the system prompt to carry the new skill content breaks the cache. And the cache is what makes long sessions affordable at all. Compaction-handover's why #3 is the same arithmetic seen from its side.

So this design lives in a squeeze. The requirement is "correct the agent's understanding". The working constraint is "you may only add to the end". Every mechanism below is a consequence of that squeeze.

This is also the root that makes the design non-obvious. Without the cache constraint, context updates would not be an interesting problem at all.

One word in the heading is doing important work: **priced**.

The alternative reading — that the cache *forbids* things — generates fake impossibilities. It is worth holding onto the correct reading, because that mistake is easy to make and expensive.

Nothing in this design is forbidden by the cache. Things are *expensive*. Every "you cannot append your way out of this" below is shorthand for "the only way out is a cache break, and here is why that is or is not worth it".

Where the answer is that it is worth it, the design should say so.

### 3. Bare-minimum notices, because injected content costs attention before it costs money — *quality, then resource*

The note is firm that new content is not included eagerly. The harness provides "the *bare minimum* for the agent to efficiently invalidate its current understanding — to know that viewing the new content is an option".

There are two reasons for that, and the order matters.

The first reason is attention, not cost.

Consider what dropping the full new text of a changed skill into the middle of a conversation actually does. It arrives as though it were the current topic. It competes with the task in hand. And usually nothing about it needs acting on right now.

A one-line notice leaves the agent in charge of whether the change is relevant. That is the correct division of labour, because the agent knows what it is doing and the harness does not.

The second reason is genuine resource pressure, and it compounds in an unusual way.

Once appended, a notice sits in the cached prefix of every subsequent request. It is re-read for the rest of the session. So an eager content injection is not paid once. It is paid forever.

This is the same arithmetic as compaction's why #3, and it points the same direction. Keep appended material small, because appended material is permanent.

### 4. Some changes are load-bearing and cannot be notified at all — *correctness boundary*

Not everything can be handled by a notice.

The notes list changes that are simply disallowed without a compaction or a context rebuild. The clearest is a **changed limb**.

The reasoning: the limb determines the whole context hierarchy, and that hierarchy is load-bearing. Limb-model's why #1 is that a place's instructions are what let the model act correctly in that place. You cannot append "by the way, you are somewhere else now" and expect correct behaviour, because every instruction above that append still describes the old place.

The same reasoning is why changing the **agent role or mandate** leans disallowed. In the user's words: "model can't be expected to respect role changes that occur later in the context."

Unpack that. A late append does not retroactively reframe a conversation. The model's behaviour is anchored by what it was told at the start. Appending a contradiction produces an agent that is inconsistent, not an agent that is updated.

Working directory and hostname are flagged as maybes on the same grounds. Model change is explicitly unclear. Both are pushed on later in the What.

This root matters because it defines the design's edges. Notification is not a universal solvent. Part of the deliverable is an honest classification of which context elements are worth notifying about, and which changes are better answered by rebuilding sooner.

### 5. Time passes invisibly — *correctness*

An agent has no clock.

Between its last response and the user's next message, an hour may have gone by. Or six weeks. Nothing in the context distinguishes those two cases.

The failure is concrete. The agent resumes as though no time passed. It assumes its branch is still current. It assumes the command it ran is still meaningful. It assumes "just now" was just now. Then it acts on a world that has moved.

Hence the special handling: inject elapsed time past roughly an hour. Below an hour there is no point.

### 6. The up-front context budget is contended, and it is paid on every single session — *resource, with a real tradeoff*

Progressive disclosure has its own root, separate from everything above.

The user wants a large library of skills and tools available. But skill and tool descriptions "can otherwise take up massive context paid on *every* session".

That is the pressure. Every description present up front is a permanent tax on every session that never uses it.

The tradeoff is named honestly in the notes, and it must not be flattened: always-up-front cost, versus conditional and repeated cached-input cost from tool calls that fetch more detail on demand.

The mechanism has two parts. Gate skills behind more broadly applicable ones. And write descriptions that say *when to load*, rather than what the skill contains.

The note also draws out a dependency, and it is worth taking seriously. This only works if the descriptions are well written. That is why the note wants an information-architecture skill and a skill-writing workflow alongside. The mechanism has a documentation prerequisite.

The limb model contributes to the same goal from a different direction. A subagent given a specific limb gets a context-specific tool set, so those tools need not exist in every session at all. Progressive disclosure and the limb model are two solutions to one pressure.

## Forward: what these roots force

Before detailing anything, here is what chains forward from the roots.

- **Two modes, and a prediction.** Because of #2, the harness must operate in either append mode or rebuild mode. And it must *decide which*, by predicting cache state. That prediction becomes a first-class piece of machinery. It is also shared with compaction, which needs the same judgement.
- **Rebuild is free exactly when you were already paying.** If a cache miss is expected anyway, there is no reason not to optimise: refresh AGENTS.md and skill content, truncate old tool output harder, canonicalise everything. So rebuild is not a fallback. It is an *opportunity* that arrives on a schedule set by cache expiry.
- **Every context element needs a declared update policy.** From #4: notify or don't, carrying the minimum the corrective action needs. For a changed tool schema that minimum is the whole schema, since a bare notice would leave the agent calling the tool wrongly. Orthogonally: does this change also raise the value of rebuilding sooner? That pair is the core deliverable.
- **Change detection is required, and it is not free.** Something must notice that a skill file, an AGENTS.md, a tool set, or an option set changed. That something is a watcher. Watchers live in the limb. So the limb reports change and the brain decides what to do about it.
- **Notices must not trigger requests.** Invariant 2 and `context-and-agent-loop.md` are explicit: appending context is not the same as asking the model. A change notice piggybacks on the next real request. Getting this wrong turns a file save into a paid API call.
- **Rebuild must not replay history.** A rebuild produces the canonical *current* context. It must not carry forward the append-only notices that described how the old context got out of date, because those are now noise describing a superseded state.

## What

### First, what a context is actually made of

You cannot classify elements without enumerating them. And the enumeration turns out to be the useful part, because it splits along a line that decides everything else.

The line is: **is this element baked into the cached prefix, or was it appended to the body?**

```
┌──────────────────────────── the context ────────────────────────────┐
│                                                                     │
│  PREFIX — frozen while the cache is warm                            │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ system prompt                                                 │  │
│  │   · agent role and mandate                                    │  │
│  │   · limb identity and instructions                            │  │
│  │   · AGENTS.md set; machine / user / project context layers    │  │
│  │   · skill DESCRIPTIONS  (not skill content)                   │  │
│  │   · working directory, hostname                               │  │
│  │   · whatever date or time the prompt states                   │  │
│  │ tools array                                                   │  │
│  │   · every tool schema                                         │  │
│  │   · every enumerated option set inside every schema           │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                     │
│  BODY — append-only; nothing is ever edited or removed              │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │ · the user's messages                                         │  │
│  │ · the user's in-band activity                                 │  │
│  │ · loaded skill CONTENT                                        │  │
│  │ · file reads, command output, search results                  │  │
│  │ · tool calls and their results                                │  │
│  │ · subagent results                                            │  │
│  │ · earlier change notices                                      │  │
│  │ · earlier handover briefings                                  │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                              ← new appends go here  │
└─────────────────────────────────────────────────────────────────────┘
```

Some body entries arrived by tool call and some did not. A notice is the harness appending. A user message is the user appending. The mechanism is the same, and it is the only mechanism this design has.

The line matters because the two halves fail differently.

A stale **prefix** element cannot be corrected on the wire without a rebuild. The best a notice can do is tell the agent to stop believing it.

A stale **body** element can be superseded by appending a newer version. Appending is what put it there in the first place.

Almost every entry in the classification below is an instance of that one distinction.

There is one thing the line does *not* mean, and it is worth killing here, because the economics depend on it.

**The body is not the cheap half.**

Appended content joins the cached prefix of every *subsequent* request. It is re-read for the rest of the session. So nothing is ever really replaced. The old version stays, and it keeps costing.

"Supersede" means the agent stops believing the earlier copy. It does not mean the earlier copy goes away.

That is why why #3's bare-minimum rule applies to the body just as hard as it applies to the prefix.

### The rule that does the classifying: can the agent act on it?

The core deliverable is an honest classification of every context element. That is one policy plus one orthogonal axis, both below.

The policy needs a principle rather than a list of precedents. A list of precedents will not survive the first element nobody thought of.

The principle is **actionability**. A change is worth appending a notice about exactly when the agent has an action available, *within append mode*, that resolves the staleness.

Three examples, to make it concrete:

- A changed skill **is** notifiable. The agent can re-read the skill.
- A changed AGENTS.md **is** notifiable. The agent can re-read the file.
- A changed limb **is not** notifiable. There is no action available. You cannot read your way into being somewhere else.

This principle also absorbs the one apparent exception cleanly, and it is worth walking through that, because at first glance it looks like a contradiction.

Why #3's rule is "bare minimum". But the notes carve out changed tool schemas as needing "full content injection". A whole schema is not a bare minimum, so the two look incompatible.

They are not. For a schema, the new schema *is* the action-enabling content. The action the agent needs to take is "call this tool correctly". There is nothing it could go and read that would tell it how.

So the rule is not "the shortest possible string". The rule is **the minimum that makes the corrective action possible**. For a skill that minimum is a path. For a schema it is the schema.

One rule, no exceptions. And it sharpens why #3 rather than contradicting it.

The rule has one parameter: whether the element is worth notifying about at all.

That is not the same question as whether an action exists. A skill description the agent never read has nothing stale to correct. And some elements are simply not worth the noise even when the agent has been exposed to them.

Where that line sits is a judgement per element, and the user has ruled it deliberately debatable.

### The second axis: does this raise the value of rebuilding sooner?

Running alongside the policy, and independent of it, is a second question. **Does this change raise the value of rebuilding sooner?**

Rebuild is not a third policy value. It is not the residue left over when notices fail. It is a scheduled event whose timing this design can influence but does not own.

Three cases exist:

1. Some changes are fully answered by a notice, and say nothing about rebuild timing.
2. Some are answered by a notice *and* argue for rebuilding earlier than the schedule would otherwise.
3. Some cannot be acted on at all until the next rebuild. Those are the strongest such argument.

Keeping the two axes separate is what stops "the agent cannot act on this yet" being mistaken for "this can never be acted on".

### The classification

Status letters follow `README.md`: `S` settled by the notes, `P` proposed here, `E` needs an experiment. The **rebuild** column is the orthogonal axis, not a third policy value.

| Element | Notice policy | Raises rebuild value? | Reasoning |
|---|---|---|---|
| Loaded skill content | Notify `S` | no | It is body content; the agent can re-read it. |
| Skill *description* (not loaded) | No notice `S` | no | The note is explicit: "If it's in the system prompt, it's there with a desc only. We ignore desc changes until the context is rebuilt." Nothing is stale in the agent's understanding, because it never read it. |
| New skill appearing | Notify `S` | no | The agent can load it by name; the description's absence from the prefix costs discovery, not capability. |
| AGENTS.md, global / machine / user / project context | Notify `S` | no | Prefix content, but readable as a file — so the corrective action exists. |
| Tool **removed** | Notify `S` | no | See below: the dead schema can stay on the wire and the call fails informatively. |
| Tool **added** | Notify, honestly — not yet callable `P` | **yes, strongly** | See below. The addition is not on the wire, so using it needs a cache break or an escape hatch. This sharpens the note, which lists new and missing tools together. |
| Tool schema **changed** | Notify, with the full new schema `S` | no | The schema is the action-enabling content. |
| Option sets inside a schema (agent types, limbs, other enums) | Notify `P` | only if the set was declared as an enum | See below — this turns into a declaration constraint on how such tools are written, and the constraint removes the rebuild dependency. |
| Elapsed time | Notify above roughly an hour `S` | no | The agent has an action: re-verify what it assumed was current. |
| Limb | Cannot arise in a session `S`/`P` | — | The note: load-bearing, "shan't change without a compaction / context re-build". And structurally unreachable — see below. |
| Agent role / mandate | Cannot arise in a session `S`/`P` | — | The user's own reasoning for why a late change would be wrong: "model can't be expected to respect role changes that occur later in the context." Also fixed at launch — see below. |
| Working directory, hostname | Not a change at all — a different limb `P` | — | See below. This sharpens the note's "maybe". |
| Model | Not a context change; it forces a free rebuild `P` | n/a — the rebuild already happened | See below. This resolves the note's "Unclear". |
| User in-band activity | Notified by construction — it *is* an append | no | user-turn's business, not this design's. |
| Prior notices | Dropped on rebuild `S` | no | Why #6's forward drill: they describe a superseded state. |

#### Adding a tool is a different shape of problem from removing one

The note lists "Tool availability (new tools, missing tools)" as one category. Drilling in, they turn out to be opposites. The note flattens a real asymmetry.

**Removal is free.**

Walk it through. The tool is gone from the limb. But its schema is still sitting in the tools array on the wire. It can stay there — removing it would change the prefix and cost the cache, for no benefit.

The agent may still call it. The call fails, with a result explaining that the tool is no longer available.

So a notice here is honest and useful. It stops the agent wasting a turn. And if the agent does waste one, the failure is informative. Nothing about the wire has to change.

**Addition is not free.**

A model cannot call a tool that is not in the tools array. Putting it there is a prefix change. A prefix change means a cache break.

So the notice has to be honest about a two-part state: the tool exists, *and* this session cannot call it yet.

There are exactly three ways forward. The design has all three. It is not stuck.

*Option one: wait for the next rebuild.* This is the default, and usually the right answer. One new tool rarely justifies discarding a warm context. And a rebuild is a scheduled event rather than a hope — see §Rebuild is the opportunity. Here the notice's value is as an input to the decision to hand over, not as a capability announcement.

*Option two: pay the cache break now.* This is legitimate, and the design already reserves the mechanism elsewhere. Self-modification breaks the cache deliberately for schema-breaking plugin changes. Nothing distinguishes a badly-needed new tool from that case except how much the agent wants it. There is no reason to grant the mechanism to one and deny it to the other. So this is a judgement — expected value of the tool against the cost of the miss — made with the same cost model everything else here uses. It should be available to the agent rather than reserved to the harness.

*Option three: declare an escape hatch up front.* This is the declaration rule below, applied one level higher. Suppose a single generic dispatch tool sits in the prefix from the start: it takes a tool name and an argument object. Then a tool added mid-session becomes callable immediately. Its schema is injected into the body, as with any changed schema. The name goes in as a free-form string. The limb validates at execution time.

The reasoning behind option three is worth making explicit, because it generalises. The set of tools is itself a set that can change mid-session. The rule below says such sets must not be frozen enums. The tools array is the largest frozen enum in the context.

Option three is not free either, and its costs are the reason it is not simply the answer:

- Models are trained on native tool calling. They are likely worse at a dispatch indirection.
- The provider's own argument validation stops helping.
- Every call pays the injected schema rather than the prefix's cached copy.

So the honest position is a narrow escape hatch for the case that matters, rather than a replacement for the tools array. And which of the three is right for a given change is a measurement, not a deduction. `E`.

#### The same asymmetry explains the changed-schema case

This is worth being precise about, because it is easy to design wrong.

When a schema changes, two things are true at once. The wire array still advertises the old shape. The injected content describes the new one.

That only works under one condition: **execution must validate against the limb's current schema, not against the advertised one.**

Here is what goes wrong otherwise. A model faithfully follows the injected new schema. It makes a call in the new shape. The call gets rejected, for not matching a stale advertisement. The model did everything right and was punished for it.

So the wire array is best understood as a possibly-stale **advertisement**. The limb's current schema is the truth at execution time.

Then the two cases behave correctly. A call shaped to the old advertisement fails with a corrective result. A call shaped to the injected new schema succeeds.

The user's own instinct in `context-and-agent-loop.md` points the same way: "maybe update tool call schemas (although I think that's confusing, because the chat likely contains tool calls - appending is probably still correct here)".

The hedge is worth keeping. He is not certain, and neither am I. `E`.

#### Option sets are frozen sets, and that is a declaration problem

The note wants notices for "Avaialble agent types for subagent tool, available limbs for subagent tool, other tool option sets etc."

Follow that through and it collides with the prefix rule. Step by step:

1. Those options are declared as JSON-schema enums.
2. The enum lives in the tools array.
3. The tools array is frozen while the cache is warm.
4. Therefore a new agent type cannot be selected without a cache break.

That is the same wall as adding a tool.

Documenting the options in the parameter *description* instead does not help. The description is in the array too.

The way out is not a notice mechanism. It is a declaration convention: **sets that can change during a session must not be baked into the schema.**

Concretely: declare the parameter as a free-form string. Keep the valid values discoverable through a listing tool, or through the same limb that will validate the call. Validate at execution time.

Then a notice about a new agent type is immediately actionable, because the parameter already accepts it.

That generalises into a rule worth stating on its own, because it is the same rule that governs progressive disclosure: **any set baked into the prefix is frozen until rebuild, and any set discoverable by tool call is live.**

Proposed here; endorsed by the user 2026-08-04 with two reasons of his own added. The first: "the agent's gonna be quite reticent to put an invalid enum value into a tool call even if it's been told that enum value is now valid" — so even a notice cannot rescue a frozen enum. The second: schema enums carry no documentation anyway ("you can't attach documentation to them... it's not quite what we wanted anyway"), so free-form strings with documented discovery are better on their own terms, not just cache-compatible.

Note how far the rule reaches once stated that way. The set of *tools* is a set baked into the prefix. So by this rule it should not be frozen either. That is the escape-hatch option in the previous section, and it is why the added-tool case is a trade rather than a wall.

His first reason cuts against the escape hatch as well as for it. An agent reticent about an unfamiliar enum value may be equally reticent about dispatching a tool by name. That is an argument for measuring it, not for pretending the option does not exist.

#### Why role and limb cannot be appended, but cwd and model are different questions

The user's reasoning about role is the sharpest thing in the note: "model can't be expected to respect role changes that occur later in the context."

A late append does not retroactively reframe a conversation. It produces an agent holding two mandates. That is worse than one stale mandate.

Limb fails for the same reason, plus a stronger one. The limb determines the whole context hierarchy. Appending "you are somewhere else now" leaves every instruction above it describing the wrong place.

The distinction underneath these two is worth naming, because it is what lets us push on the unsettled entries.

A **referential** change means a fact the agent used is now different. A **behavioural** change means who the agent is, or where it is acting, has changed.

Referential changes append fine. Models handle "that fact has been updated" well.

Behavioural changes do not append. They do not update anything — they contradict.

Now the two entries the notes leave unsettled.

**Working directory and hostname.** The note says "Maybe likewise", meaning maybe disallowed.

Following the limb model through makes this more than a maybe. A limb is identified by `ssh_host` plus `directory` (`source-notes/agent-harness-design.md`). A session is bound to exactly one limb. So a changed working directory or hostname is not a change to a session's context. It *is a different limb*, and therefore a different session.

There is nothing to notify, because the situation cannot arise. You do not move a session. You start one elsewhere.

There is a genuine edge case, and it is not a context update either: the directory being renamed or deleted underneath a live limb. That is a limb that has lost its identity, and the honest response is limb failure.

This makes the user's "maybe" into a "yes, and for a stronger reason than he gave". That kind of sharpening wants his ruling, so it is in Questions for review.

**Model.** The note says "Changed model? Unclear."

Current truth already resolves this, from an unexpected direction. The walking-skeleton ruling recorded in `REQUIREMENTS.md` is that `model` and `reasoning_effort` are **request facts, not context facts**.

So changing model does not mutate the context at all. There is nothing to notify.

What it does do is invalidate the cache, because caches are per-model. Which means a model change lands the session in rebuild mode automatically. And rebuild there is free, in exactly the sense of the next section.

So the rebuild happens either way. Not because appending would be wrong, but because you are already paying.

One thing worth checking against invariant 3, which says projections may be per-model: if the projection differs per model, then the rebuild is not merely free, it is required. `P`.

### What a notice actually says

A notice is one line where one line will do. Its content is fixed by the actionability rule: it must carry enough for the agent to decide whether to act, and enough to act if it decides to.

Four fields do that work.

**What** — the element, identified the way the agent would refer to it. A skill name. A file path.

**What kind of change** — changed, appeared, disappeared.

**Who or what changed it** — and this is not decoration. The note's own story names three sources, "by the user or another agent or by git", and they imply different responses. The user editing a skill mid-session is a deliberate instruction to work differently. Git moving underneath you is a branch change that probably invalidates more than the one file. Another agent editing shared instructions is a coordination event.

**What action is available** — usually implicit in the element type. Worth being explicit where it is not.

What a notice must *not* contain is the new content, except in the schema case. Why #3's first reason is attention, not cost: content arriving mid-conversation reads as the current topic and competes with the task in hand.

Two mechanical points.

Notices are **batched and placed at the next request**. They are appended as a single block, after any tool result, rather than between a tool call and its result. The walking skeleton established that tool-call/result adjacency must not be split on the wire, and notices are the obvious thing that would split it.

Notices carry the **harness's** voice. They are facts about the harness's observation, not the user speaking. If a provider forces them onto the user channel, that is a projection choice — and the recorded fact must still be attributed correctly, per invariant 3 (an event is about its emitter). This is the same problem compaction-handover has with its briefing, and it looks like one mechanism.

#### Notices are a diff, not an event log

This is the mechanism I would most want reviewed, because it makes several otherwise-awkward problems disappear.

Start with the obvious implementation, so the contrast is clear. A queue: something notices that a file changed, pushes a notice onto a queue, and the queue drains at the next request.

That implementation has three unpleasant behaviours.

1. A file saved forty times produces forty notices, which then have to be coalesced.
2. A file changed and then changed *back* produces notices about nothing.
3. A session left idle for a week accumulates a queue whose size is a function of how much unrelated activity happened on the machine.

Now the alternative. Compute the notice set as a **diff at flush time**: compare the world as it is now against what this context actually contains.

```
   what the context contains                 what the world contains now
   (recorded at the time it was appended)    (read at flush time)

   ┌────────────────────────────┐            ┌────────────────────────────┐
   │ skill "git-merge"          │            │ skill "git-merge"          │
   │   hash abc123              │──compare──▶│   hash def456        ✗     │
   │                            │            │                            │
   │ AGENTS.md                  │            │ AGENTS.md                  │
   │   hash 111aaa              │──compare──▶│   hash 111aaa        ✓     │
   │                            │            │                            │
   │ tool "grep" schema         │            │ tool "grep" schema         │
   │   hash 222bbb              │──compare──▶│   hash 222bbb        ✓     │
   └────────────────────────────┘            └────────────────────────────┘
                                                          │
                                            one difference found
                                                          │
                                                          ▼
                                    notice: skill "git-merge" has changed,
                                            re-read it at <path>
```

Now re-run the three problems against this formulation.

1. Forty saves produce one notice. The diff only sees the current state.
2. A change-and-revert produces silence. The hashes match again.
3. Idle time produces nothing at all, because there is nothing accumulating.

Coalescing is not a feature here. It is the absence of a problem.

This does impose a storage requirement. It is the same one the note implies anyway when it says notification is warranted only "if the agent has loaded the skill": the context must know **what version of what it contains**.

Concretely: every append that carried config-ish content — a skill load, an AGENTS.md read, a schema — records what it was, plus a content hash. That is the left-hand side of the diff.

It is also, satisfyingly, the same machinery compaction-handover needs for its old→new briefing diff. One is a diff between the context and the world. The other is a diff between the context and its successor. Same record.

The user re-derived the exposure rule independently when reviewing (2026-08-04), which is some confirmation the diff formulation matches his intent: "if it's a skill and the agent has not read the skill, then we don't need to change notice because the agent hasn't read the previous version... when it loads it the first time, it can be the new version. Perfect."

He also pushed the economy further than the doc had. Some elements may not deserve notices even when exposed: "there's no reason to notify about certain things like, for example, the skill description. Assumably, that's not changing too much... These things are maybe debatable, but I think we need to draw these lines. Otherwise, we'll get too much change notifications coming into the event stream."

So the classification table's job includes a *notify-at-all* threshold per element, not only a mode. Where that line sits is deliberately left debatable.

And notices stay minimal because reload is always available: "the events don't have to be large. They only have to say that something has changed. And the agent, as long as it's got a way to... read the new information at will, we don't actually have to include it."

### A notice is never a reason to call the model

Invariant 2 and `context-and-agent-loop.md` are unambiguous. The note lists "tool schema changes" and "process config changes" among the things that only piggyback.

So: change detection never triggers a request. Notices ride on a request that was going to happen anyway — a tool-loop continuation, a user turn ending, a proactive handover.

There is a consequence that is easy to miss and pleasant once seen. If the session never sends another request, the notices are **never paid for**. Under the diff formulation they are never even constructed.

So a file save in a project whose session has gone quiet costs nothing. That is exactly right. It is the difference between a design where the user can edit freely and one where saving a file costs money.

The failure mode to test for is the trivial one, and it deserves a black-box test rather than an argument:

1. Save a file. Observe zero requests on the fake provider's log.
2. End a turn. Observe one request, carrying the notice.

### Time

The rule is the note's: inject elapsed time past roughly an hour, "less no point".

The action it enables is re-verification. The agent's branch may have moved. Its running command may be long dead. Its "just now" may have been six weeks ago.

One mechanical point that the diff formulation makes obvious: elapsed time is a fact about *now*, so it must be computed at flush, never at queue time. A notice constructed when the gap was two hours, and delivered when the gap is three days, would be actively misleading. Under the diff formulation this is automatic — which is a small piece of evidence that the formulation is right.

Two things the notes do not settle, recorded rather than invented.

First, what form the injection takes: elapsed duration, absolute timestamp, or both. "Both" is probably right, since the agent may care that it is now Monday as well as that six days passed.

Second, whether the *stated date* in the system prompt is itself a notifiable element for a session that spans midnight. By the actionability rule it is — the agent can update its belief, and there is a real action, which is to stop asserting yesterday's date. But nobody has ruled on it.

### Rebuild is the opportunity, not the fallback

Why #6's forward drill already names this. It is worth holding onto, because it inverts the intuition.

Rebuild is not what you do when notices fail. It is what you do when you were **already going to pay for a cache miss**.

And at that moment there is no reason not to take everything: refresh the AGENTS.md and skill content, refresh the schemas, canonicalise the option sets, truncate old tool output harder. The note's phrasing: "If we expect a cache miss, then there's no reason to not optimize the context somewhat."

Which makes the interesting property of rebuild its *schedule*. Rebuilds arrive whenever the cache lapses, whenever the model changes, whenever a handover happens, whenever a fresh session starts — and occasionally, whenever something is worth paying a break for.

So the design job is to make sure those moments are used fully. Not to make rebuild rare.

Two rules constrain what a rebuild produces.

**Rule one: a rebuild produces the canonical current context, and canonical means it looks as though the session had started now.**

In particular it must not replay the append-only notices. Those described how the *old* context became stale. Carried forward, they are noise describing a superseded state, and they would accumulate across successive rebuilds.

Nor should a rebuild carry a notice's *effects* as a special case. If a skill changed and the agent re-read it, the rebuild contains the current skill once. Not the old version, plus a notice, plus the new version.

The user's framing of the same rule (2026-08-04) ties it to the portfolio-wide snapshot ruling: "the context fresh rebuild is basically the new snapshot, and it doesn't need to contain any of the history unless it's explicitly relevant somehow." Notices are "events that get rolled in".

So a rebuild is this design's snapshot. Dropping the notices is not a special rule. It is what snapshotting means.

**Rule two: a rebuild must have no hidden state.**

It is a function of its declared inputs — the config, the limb's layers, the current content of the files that feed it, and the clock — and of nothing else. No ordering dependence. No leftovers from the context it replaces.

What a rebuild must *not* be asked for is byte-identical output across two runs. That is unachievable, and it would be wrong to want. A rebuild states the current time and picks up files as they are when it runs. Two runs a minute apart legitimately differ.

That distinction matters because of what compaction-handover actually needs from a rebuild.

Its briefing describes the successor's context *before* the successor exists. It does that from the content-version record — which elements this context holds, at which versions, against what the world holds now. It does not build a hypothetical prefix and diff the text.

So what compaction needs from a rebuild is that the elements it produces are the elements the record says it will. Nothing more. And in particular, not determinism.

One thing the notes explicitly leave open: what rebuild does with the conversation *body*. "Truncate old tool calls harder" is named. How hard, and whether truncation is reversible, is not. Nor is whether a rebuild may re-order or drop body content at all.

My reading is that it may truncate but may not re-order or reinterpret, because the body is the record of what happened. That is a proposal. `P`.

### Progressive disclosure

Progressive disclosure arrives from a different root — why #6 — but it lands in the same machinery. That is the point of the closing section below.

The mechanism is the note's, in four parts. Not everything is available up front. Skills carry descriptions rather than content. Skills can be **gated behind other, strictly more broadly applicable skills** being loaded first. And descriptions "need to say when to load" rather than what the skill contains.

The limb model contributes from the other side. A subagent with a specific limb gets a context-specific tool set, so tools irrelevant to that limb need not exist in that session at all.

The tradeoff must not be flattened, and the note states it exactly: "a careful balance between always up-front input cost and conditional repeated cached-input cost from tool calling to get more info".

Both sides are real, and it is worth spelling out each.

Up-front cost is paid on every session, including the sessions that never use the thing.

On-demand cost is paid per fetch. Plus a round trip. Plus permanence — a fetched skill sits in the cached prefix for the rest of the session, so an unnecessary load is a permanent tax. That is the same arithmetic as why #3.

So progressive disclosure is not strictly cheaper. It moves cost from *certain and universal* to *conditional and permanent-once-incurred*. It wins when the conditional probability is low.

The design consequence is that the balance is a measurement, not a principle. The experiment should measure it rather than assert it.

The note also names a **prerequisite that is not a mechanism at all**. This only works if the descriptions are well written, which is why it wants "an info architecture skill and a skill writing workflow that helps motivate & get this correct".

That is worth taking seriously as a design fact rather than a nice-to-have. Think about what a badly described gate does: the agent neither loads what it needs, nor knows what it is missing. That is worse than no gating at all.

So the deliverable here has a documentation half. Pretending otherwise would be dishonest about why the mechanism works in the fork.

`source-notes/context-updates.md` records that this was "previously implemented in my opencode fork". So the mechanism has empirical backing, and the fork is where to look before inventing (`F`, per `source-notes/open-code-inspiration.md`).

What has *not* been validated is the same trick for tools, which the note says "can & should be done". That is the newer half.

### The two halves of this design are one axis

The source note has two sections, "context changes" and "progressive disclosure". They read as separate concerns.

They are not. They are the same axis seen from two ends.

Everything baked into the cached prefix is **paid on every session, and frozen once you have it**.

Everything fetched by tool call is **paid only when fetched — and then permanent for the rest of the session**.

Progressive disclosure is the decision about which side of that line each piece of content starts on.

Context updates is the machinery for the consequences. Things on the frozen side can only be *pointed at* by a notice. Things on the appended side can be *superseded* by another append — never removed, and still costing. And the only way to move something across the line, or to take anything out, is a rebuild.

Seeing it that way explains why the same three mechanisms keep appearing.

The **diff** is one mechanism. Between context and world for a notice. Between context and successor for a handover. Between old and new prefix for a rebuild. It is the only way to know what is stale without replaying history.

**Cache-state prediction** is one mechanism. It decides append versus rebuild here, exactly as it decides when to compact and whether to fork.

The **actionability rule** is one policy. It decides notice content, notice existence, and how tools and option sets must be declared in the first place.

Read that way, the design has a single sentence: *the context is a frozen prefix plus an append-only body, and every mechanism here is either a diff against the frozen part, a decision about when the freeze lifts, or a rule about what to put where.*

Everything above is detail on those three.

### The cache is genuinely unknown, and that is the experiment

This design rests on cache semantics nobody here knows. The honest move is to mark them as questions rather than decide them.

`context-and-agent-loop.md` already hedges the central one. The context must be treated as "append only (or at least, append only with respect to some prefix - this depends on the provider's caching implementation and needs experimenting)".

Here is what needs finding out. None of it is decided in this doc.

- **What append mode is append-only with respect to.** Does a provider's cache tolerate appends at all without an explicit breakpoint? If breakpoints are explicit and limited in number, where should they go, and what does that cost in flexibility?
- **Whether the tools array participates in the cached prefix.** This is the assumption the whole added-versus-removed asymmetry rests on. If it does not participate, a tool can simply be added mid-session, and the asymmetry disappears entirely — along with the declaration rule that grew out of it.
- **Whether providers validate tool call arguments against the advertised schema.** The changed-schema policy assumes execution-time validation against the limb's current schema is possible. If the provider rejects first, the policy needs rethinking.
- **How long cache entries actually live, and how that is observable.** This feeds the prediction machinery described in compaction-handover.
- **What append mode means for a fork.** The note's own guess: "possibly the parent *as of the message before it sent the subagent tool call* - but that needs experimenting too."
- **Whether late system parts are supported**, per provider. This decides the channel for both notices and compaction briefings.

Every one of these is cheap to test against real providers, and impossible to settle by reasoning. That makes them the natural first work of the experiment rather than a risk to it.

### What this makes falsifiable

The thesis: an agent in a warm session can be kept honest about a changing world using nothing but small appended notices, plus a rebuild taken opportunistically when the cache lapses — and doing so is cheap in the common case and does not derail the work.

It is falsified if any of these happen:

- An agent given a notice fails to act on a change that mattered.
- An agent acts on a change that did not matter, and derails.
- Notices accumulate to a size that matters, which would mean the diff formulation failed.
- A notice triggers a request, directly contradicting invariant 2.
- A rebuild replays obsolete notices, or otherwise fails to be canonical.
- The appended-schema policy produces calls that the provider or the limb rejects.
- Progressive disclosure measurably costs *more* than always-up-front, for a realistic skill and tool library. The note's own honest framing of the tradeoff admits this is possible.

Invariants touched: **2** (the whole no-trigger rule, and the piggyback path), **3** (a notice and a rebuild are two projections of the same change facts; the per-model parenthetical bears on the model question), **5** (content versions and hashes must be durable and queryable, since the diff is computed against them), and **10** (everything the model sees must be derivable from the session record — a notice whose meaning depended on limb-local state would violate this, which is why change facts travel in the message).

## Interactions

### What this design owns, and what it assumes

This design owns the policy layer. That is: the actionability rule; the classification of every context element by update policy; what a notice says and when it is flushed; the diff-at-flush formulation; the rule that a notice never triggers a request; what a rebuild produces; and progressive disclosure.

It also owns one rule that reaches outward into somebody else's tool surface: a set which can change during a session must not be baked into a schema.

And it owns one mechanism that a sibling consumes: the harness voice. That is developed here rather than in compaction-handover, because notices are the high-frequency case and the late-system-part provider probe is already on this design's list of unknowns. That placement is a call rather than a ruling; it is in Questions for review.

What this design assumes rather than tests is short and specific.

Cache-state prediction is shared machinery (`INTERACTIONS.md`). This design needs it only as a two-way decision — append or rebuild — with the honest caveat that the decision is an expected-value bet.

The durable record of what-version-of-what a context contains is persistence-analytics' schema. This design assumes content hashes and epoch anchoring exist, and tests the *behaviour* the diff produces from them.

Compaction-handover is a consumer of both the record and the rebuild, not the other way round. What this design owes it is canonicality and freedom from hidden state, both stated above.

Change detection is required here but hosted by the limb. The limb reports change and the brain decides what to do about it. So limb-model owns the message surface, and this design owns the requirement that lands on it.

### Self-modification proposes the same mechanism from the other side

This is the interaction that changed the most, and it changed by agreement rather than by conflict.

Self-modification classifies every plugin reload into one of three kinds: schema-identical, schema-additive, or schema-breaking. Each lands on a row in the classification above.

A **schema-identical** change needs no notice at all. Nothing the agent can observe changed. The wire is untouched, and the behaviour behind an existing call is simply current.

A **schema-additive** change — a new tool, or a new optional parameter on an existing one — has the shape of the tool-added row. The addition is not on the wire, so a warm session cannot use it until a rebuild, a deliberate break, or an escape hatch makes it callable.

A **schema-breaking** change is either the changed-schema row with full content injection, or the explicit cache break that self-modification reserves for exactly this case.

Worth noticing: self-modification's willingness to break the cache for a breaking change is the same lever the added-tool case needs, arrived at from the other side. Neither design should treat the break as unavailable to the other.

More striking is that its central call — pin the schema, run the newest implementation — is the same policy as this design's. The wire's tools array is a possibly-stale advertisement. The truth at execution time is the limb's current schema.

Two designs arriving at one mechanism from unrelated roots is the strongest evidence either has for it.

It also means the risk is shared and correlated. If a provider validates tool call arguments against the advertised schema, both designs lose their central move at the same moment. That is the single most valuable thing the provider probe can tell us, and it should be probed before either design commits.

Invariant 2 lands identically on both. A plugin reload is a context change, so it must never trigger a request — the same rule as a file save, for the same reason.

### User-turn: two facts about one edit, and why the diff formulation makes them compose

Why #1 says this design is load-bearing *because* user-turn makes mid-session change routine. Developing that turns out to narrow the interaction rather than widen it, which is the useful result.

User activity is notified by construction. It *is* an append, and user-turn owns projecting it. So the agent already learns that the user edited a file.

What this design adds is orthogonal: the edited file may also be a context element that the agent's understanding depends on. That is a different fact about the same edit.

So the user editing a loaded skill produces one activity projection (he edited it, here is the diff) and would otherwise also produce one notice (the skill you loaded has changed, re-read it).

The diff formulation makes those compose without any deduplication logic. That is not a coincidence so much as a consequence. Walk it through:

- A notice is computed at flush time, by comparing the world against what the context actually contains.
- If user-turn's activity projection has already carried the new content into the context, the comparison is empty. No notice fires.
- If it carried only a summary, or a partial diff, the comparison is non-empty. A notice fires, correctly.

Neither design has to know about the other. The shared left-hand side does the work.

A queue-based implementation would have needed an explicit rule here. That is a small piece of evidence for the diff formulation, on top of the arguments already given.

### Forked-subagents: a rebuild must drop notices, and a fork must not

Fork is append mode with respect to an ambiguous baseline, and resolving that ambiguity is forked-subagents' job.

But one consequence belongs here, and it is worth stating plainly, because the naive reading gets it backwards.

A rebuild must drop prior notices, because they describe how a superseded context went stale.

A fork must *not* drop them. The entire economic point of forking is prefix identity with the parent. A child whose prefix differs from the parent's by the removal of some notices has no shared prefix at all.

So the same content is noise in one operation and load-bearing in the other. The distinction is not about the content's usefulness. It is about whether the operation is allowed to change bytes. Rebuild is the only operation that may.

The user confirmed this (2026-08-04) and gave it a sharper frame: it is the event-streaming-versus-snapshotting distinction again. "Obviously, a fork, which has an immutable history... has to keep those events. It's just how it works... clearly, change notices don't survive when we do snapshotting... There are events that get rolled in... the context fresh rebuild is basically the new snapshot, and it doesn't need to contain any of the history unless it's explicitly relevant somehow."

So: a fork extends a stream. A rebuild delivers a snapshot. Notices are events that get rolled in.

A second consequence follows, for the diff's left-hand side. A forked child inherits the parent's conversation. So it must also inherit the parent's record of what-version-of-what that context contains. Otherwise the child's first flush will re-notice every skill and AGENTS.md the parent already knew about.

The baseline forks with the context.

The declaration rule also lands here rather than staying local. Agent types, limbs and other option sets appear as parameters on forked-subagents' Task tool. The rule above says those parameters cannot be schema enums if the sets can change during a session.

So this design owns the rule, and that design's tool declaration has to conform. That crossing is already in Questions for review; what stage 3 adds is which side owns which half.

### Limb-model: the watcher's home, and one row that cannot fire

Change detection lives in the limb. That suits the limb's existing role — it owns an environment, and it already contributes unsolicited facts about it.

Limb-model proposes that limb-contributed context arrives as labelled blocks naming the layer that produced it. That labelling is what makes a notice able to say *which* thing to re-read. So the actionability rule depends on it existing.

The other consequence is a tidy piece of subtraction, and it is worth following through because it shrinks the design.

A changed limb is the notes' canonical un-notifiable change. But a limb is identified by host plus directory, and a session is bound to exactly one limb. So the situation cannot arise inside a session. That row documents a boundary rather than a case.

The role row goes nearly the same way. A session's agent type is fixed at launch. The only route to a different one is resuming as a different agent type, which forked-subagents treats as creating a new session rather than mutating a running one.

So the two changes the notes single out as too load-bearing to append are both unreachable. And the reasoning that made them look hard is exactly what proves they cannot happen.

Which leaves tool additions as the only change that routinely lands mid-session and cannot be acted on immediately.

### Compaction-handover, and the schedule that makes waiting tolerable

The reciprocal requirements are developed in that doc and not repeated here.

What belongs on this side is a dependency that is easy to miss. "Wait for the rebuild" is only an acceptable answer because a rebuild is a *scheduled* event. Compaction is what schedules it.

So consider what happens if handover quality has to be demonstrated before the proactive cache-driven trigger is enabled — which is what compaction-handover argues. Then rebuild arrives less predictably than this design assumes. The wait-for-rebuild rows stay stale for longer. And that is exactly when paying for a deliberate break stops being exotic.

One thing this design owes compaction, and should state plainly: the content-version record that its notice diff is computed from is the *same* record compaction's briefing diff reads. One record, one comparison, two boundaries — the world here, the successor there. Nothing hypothetical needs building on either side.

### The cells that are empty

Multi-client-ui and this design barely meet. Draft buffers and pane state are not context elements, so there are no notices about them. The negative requirement runs the other way: shared-live state must never reach a projection, which is a constraint on where that state lives rather than on what this design says about it. Two faces means the user can edit from two places, but that is still just a file change.

Topology contributes nothing beyond an invariant already honoured. Change facts travel in the message rather than by reference to limb-local state. That is what invariant 10 requires, and what the diff formulation already does.

Oauth-credentials, cancellation-economics and layered-shutdown have no relationship with this design at all.

Two thin ones are worth a line each, because both are counterintuitive.

Operator-lifecycle: a relaunch is *not* a rebuild boundary. The provider-side cache handle is durable, so a session resumes with its context intact and its prefix still warm. Restarting the harness does not entitle it to rebuild.

Modular-components: the no-trigger rule's central assertion — save a file, observe zero requests; end a turn, observe one carrying the notice — depends on being able to inspect the provider wire *between* steps. That is a property modular-components has to preserve when the suite moves in-process.

### The unknown this design sits under

Everything above rests on cache semantics nobody here has measured. `INTERACTIONS.md` records that as the portfolio's largest shared unknown, rather than as a risk local to this doc.

The pieces this design specifically cannot proceed without are two. Whether the tools array participates in the cached prefix, which the added-versus-removed asymmetry rests on entirely. And whether providers validate arguments against the advertised schema, which the changed-schema policy rests on.

Both are cheap to test. Both are impossible to reason out.

## Questions for review

- Is the attention-before-cost framing of why #3 right, or is the honest root simply resource? The notes emphasise cost; the attention argument is mine.
- Why #1 claims this design is load-bearing *because* user-turn and self-modification create routine mid-session change. That reframes context-updates from a good-taste item to something closer to soul. Do you agree, and if so should it move bucket in `PLAN.md`?
- Should cache-state prediction be its own experiment? It is now required by context-updates, compaction-handover, and forked-subagents alike, and all three are blocked on the same unknown provider cache semantics.
- **Adding a tool costs a cache break, and the note treats additions and removals together.** A removed tool can keep its schema on the wire and fail informatively; a new tool is not on the wire at all. There are three ways to make it callable — wait for the next scheduled rebuild, deliberately break the cache now, or keep a generic dispatch escape hatch in the prefix from the start — and I have made waiting the default, the break available to the agent rather than reserved to the harness, and the escape hatch a measured option rather than the answer. Which of the three do you want as the default, and is the escape hatch worth building at all given that a model is probably worse at dispatching than at native tool calls?
- **Option sets must not be declared as schema enums** if they can change mid-session — otherwise a notice about a new agent type or limb is unactionable. That is a constraint on how the subagent tool is declared, arriving from this design into another one, and it wants your ruling before it becomes assumed.
- **Working directory and hostname: I have turned your "maybe" into a "yes, and it isn't even a change".** A limb is identified by `ssh_host` + `directory` and a session is bound to one limb, so a moved session is a different session. Is that the reading you intended, or is there a case where a session's directory legitimately changes?
- **Model change: I have resolved your "Unclear" using the walking-skeleton ruling** that `model` is a request fact, not a context fact — so it is not a context update at all, just an automatic free rebuild via cache invalidation. Worth confirming, and worth checking against invariant 3's "possibly per-model" projections, which would make the rebuild required rather than merely free.
- **Notices as a diff rather than a queue.** This is the mechanism I would most want you to look at. It makes coalescing, revert-to-original, and long idle periods all disappear as problems, but it requires the context to record content hashes for everything config-ish that it contains.
- **The appended-schema policy assumes execution-time validation against the limb's current schema**, treating the wire's tools array as a possibly-stale advertisement. Your own note hedges here ("appending is probably still correct here") and I have kept the hedge; this is the piece most likely to break against a strict provider.
- **The harness voice.** Same question as compaction-handover: notices and compaction briefings are both the harness speaking, and both need a channel that does not attribute them to you. One mechanism or two?
- **Rebuild and the conversation body.** I have proposed that a rebuild may truncate body content but may not re-order or reinterpret it, because the body is the record of what happened. The notes only say "truncate old tool calls harder".
- **Progressive disclosure has a documentation prerequisite**, per your own note. I have treated that as a design fact rather than an adjacent nice-to-have — the mechanism does not work with badly written descriptions. Does the experiment therefore need the info-architecture skill and skill-writing workflow in scope, or does it validate the mechanism on hand-tuned descriptions and defer the workflow?
- **Is the frozen-prefix/append-only-body synthesis the right way to unify the note's two halves?** I have argued context changes and progressive disclosure are one axis rather than two topics. If you agree, this doc's structure and the experiment's shape both follow from it; if not, they should be pulled apart again.
- **A rebuild drops prior notices; a fork must keep them.** The same content is noise in one operation and load-bearing in the other, because a fork's whole economic point is byte-identical prefix with the parent. I am confident in the reasoning but it is mine, and it puts a constraint on forked-subagents' fork encoding: whichever encoding wins, it cannot tidy the parent's context on the way in.
- **The harness voice is developed here rather than in compaction-handover.** Both designs need it; I have put the mechanism in this one because notices are the high-frequency case and the late-system-part probe is already on this design's list, with compaction consuming it. That is a placement call on top of the earlier one-mechanism-or-two question.
- **The core deliverable is one policy plus one orthogonal axis, not a three-way classification.** *Notify or don't, carrying the minimum the corrective action needs* — plus, separately, *does this change raise the value of rebuilding sooner*. A three-way split (notifiable / notifiable-with-full-injection / cannot-be-notified) does not survive scrutiny: full injection is the same actionability rule with a bigger minimum, and the third bucket empties out once limb and role turn out to be structurally unreachable. The two-axis version is simpler and covers every row, but it changes what the experiment has to demonstrate, so it wants your ruling.
- **Waiting for a rebuild is only tolerable because compaction schedules rebuilds.** Compaction-handover argues the proactive cache-driven trigger should wait until handover quality is proven. If that holds, rebuilds arrive less predictably than this design assumes and stale-for-longer becomes a real cost — which is exactly when paying for a deliberate break stops being exotic. Worth your ruling on whether that is acceptable as it stands.
- **A rebuild needs no hidden state; it does not need to be deterministic.** Byte-identical output across two runs is unachievable and would be wrong to want — a rebuild states the current time and reads files as they are. Compaction's briefing diff therefore reads the content-version record rather than building a hypothetical prefix to compare against. This has since been corrected in `INTERACTIONS.md`, which previously listed determinism and a dry-run rebuild as a portfolio-level requirement.

## Index

| Aspect | L1 | L2 | L3 |
|---|---|---|---|
| Model framing | P | §The rule that does the classifying: can the agent act on it?, §What a notice actually says | §Notices are a diff, not an event log |
| Wire & cache | E | §First, what a context is actually made of, §A notice is never a reason to call the model | §The cache is genuinely unknown, and that is the experiment |
| Tool surface | P | §The classification | §Adding a tool is a different shape of problem from removing one, §Option sets are frozen sets, and that is a declaration problem |
| UX & input | | | |
| Ownership & placement | P | §Forward: what these roots force | §Why role and limb cannot be appended, but cwd and model are different questions |
| Lifecycle | | | |
| Storage | P | | §Notices are a diff, not an event log |
| Economics | E | §Progressive disclosure | |
| Security | | | |
| Testing & verification | P | §What this makes falsifiable | |
| Code shape | P | §Rebuild is the opportunity, not the fallback | |
| Dev workflow & references | S | §Progressive disclosure | |
| Core migration | | | |
