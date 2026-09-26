# Context updates and progressive disclosure

The Context updates aspect of the harness design: what a live session is told when something already in its context changes, and what it is given up front. The code doctrine it works inside is `code-shape.md`.

## Max's statements

### What goes stale, and why tell the agent

> If the agent loads a skill, but it gets changed by the user or another agent or by git, it'd be nice if the agent could somehow know.

— `source-notes/context-updates.md`

> But if the agent has loaded the skill, it would be nice for it to get notified that it's changed since last time it was loaded. This goes for other stuff too:
>
> - Tool availability (new tools, missing tools)
> - Skill content
> - New skills
> - The time (special rules around this one)

— `source-notes/context-updates.md`

> If it's in the system prompt, it's there with a desc only. We ignore desc changes until the context is rebuilt.

— `source-notes/context-updates.md`

> if it would not change the agent's actions in any way, then it doesn't need to know!

— decision 2026-08-12 (the notify-at-all test)

> how are we gonna know?

— decision 2026-08-12 (his own objection to the actionability test, before the utility-model idea)

> there's a practical constraint on actually knowing whether free-text content update X affects session Y or not

— decision 2026-08-12

### How much a notice carries

> We don't include the new content eagerly. We only provide the *bare minimum* for the agent to efficiently invalidate its current understanding - to know that viewing the new content is an option, or to explain a missing tool, etc.

— `source-notes/context-updates.md`

> Skills - the briefest possible mention of "these skills have changes"

— `source-notes/context-updates.md`

> It could even be something like 'one or more skills have gone stale.' (although that is too un-specific probably, it gets across the idea - we don't neccessarily need to list 10 skills that have updates - we might elide even that info as long as we give the agent a way to re-discover reality reliably & cheaply ie. it shouldn't have to reload everything just to be sure)

— decision 2026-08-12

> not necessarily - it still depends on the economics & the agent's reaction. but _all else equal_, the minimum.

— decision 2026-08-12, on whether a reference always beats carrying the content

> i would call this detail, and note that it trades off against distraction (task quality) & economics

— decision 2026-08-12, naming the aspect

> smaller notices are cheaper if the agent doesn't need them but mean that the agent may need to go get more detail if it does need them. there always needs to be a clear, reliable path to that information.

— decision 2026-08-12

> the agent actually has to be able to retrieve the new info somehow if it thinks it _is_ relevant.

— decision 2026-08-12

> for example, the skill description. Assumably, that's not changing too much... maybe debatable, but I think we need to draw these lines. Otherwise, we'll get too much change notifications coming into the event stream.

— decision, on elements that may warrant no notice even when the agent was exposed to them

> also related to the user-turn stuff

— decision 2026-08-12, on agents overreacting to notices

### Elapsed time

> This is important to have an agent understand how long between its response and the user message. It can be many weeks in some cases! Probably more than 1h is a good point to start injecting this, less no point.

— `source-notes/context-updates.md`

> discoverable, that's my naive guess

— decision 2026-08-12 (the ~1h threshold; hedge his)

### Piggybacking — nothing here drives a request

> However- user activity should *not* trigger API requests.

— `source-notes/context-and-agent-loop.md`

> If there is one already happening, then that activity should *be sent with the next API request* - a user message alongside the tool result or user message. However, many types of "context additions" should not themselves trigger a request, for cost reasons.

— `source-notes/context-and-agent-loop.md`

> There are only a few things that should drive API requests:
>
> agent tool-call loop continues
> user ends a turn
> cache-nearly-expired proactive handover/compaction
> maybe explicit “resume/continue” actions

— `source-notes/context-and-agent-loop.md`

> Most other events only piggyback:
>
> user opens file
> user edits file
> user searches
> user terminal output arrives
> tool schema changes
> process config changes
> client app reconnects
> sibling/child agent status changes

— `source-notes/context-and-agent-loop.md`

### Append versus the cache

> If we expect a cache miss, then there's no reason to not optimize the context somewhat. We might eagerly update the AGENTS.md and other system prompt info like agent and skills, maybe update tool call schemas (although I think that's confusing, because the chat likely contains tool calls - appending is probably still correct here), we might truncate old tool calls harder, etc.

— `source-notes/context-and-agent-loop.md`

> However, if we expect a cache hit, we must instead treat the context as fully immutable, append only (or at least, append only with respect to some prefix - this depends on the provider's caching implementation and needs experimenting). If we're here, instead of changing the system prompt, we append a tiny notification that would allow the agent to know that it might need to reload some file or system prompt instructions etc. that have changed.

— `source-notes/context-and-agent-loop.md`

### The three operations, and the ladder

> rebuild is not 'build a new context'!! rebuild is 'transform an existing context'... maybe we've been abusing terminology here. let's veto 'rebuild'.

— decision 2026-08-12

> this refers to the system prompt only - and happens on new sessions, on compactions, and yes, on refurbishments

— decision 2026-08-12 (initialise)

> transform existing to reduce token count, but NOT compact - the messy one

— decision 2026-08-12 (refurbish)

> keep using warm context; rebuild [refurbish] existing context (incorporate notices etc.); compact (make fresh context)

— decision 2026-08-12, which he called "the original vision". The bracket is the recorder's gloss, applying the veto above to his earlier word.

> we build the harness for correctness, then choose the cheapest option within that?

— decision 2026-08-12 (hedge his)

> re-projection of session history into a new context, but notably NOT a compaction. It's done with only regular code, and maybe utility model calls. Note that this needs to be designed still because it's a bit odd, because it mixes 'event stream' and 'rollup' in a messy way.

— decision 2026-08-12 (refurbishment)

> when rebuilding [refurbishing] a context we can coalesce notices into the system prompt & elide edits where we have a later read, etc? I think that's reasonable. TBH I haven't thought about context rebuild [refurbishment] much here.

— decision 2026-08-12 (hedges his)

> tbc if rebuild [refurbishment] is always 'compact immediately prior' or not. I think it is, if our compaction is good.

— decision 2026-08-12

### Cache prefixes

> system section (system prompt, tools, etc); system section + messages _up to the last fork boundary_; system section + messages _up to .._ + all subsequent messages.

— decision 2026-08-12

> For user-facing messages, we also need a prefix which is _everything up to n-2 messages ago_ (or something like that) to support message undo.

— decision 2026-08-12

> effectively a branching structure

— decision 2026-08-12, on the append-only cache

> That is why forked sub-agents work at all.

— decision 2026-08-12, on discarding a suffix where a cache point can be predicted

### Tool schemas and the tool set

> Tools - added / missing tools get similar notification. Tools with changed schema need full content injection.

— `source-notes/context-updates.md`. **Later:** the 2026-08-12 ruling below supersedes the full-content-injection half — a changed schema gets no notice at all.

> whenever I was referring to 'tool schemas' I _also_ meant 'tool set' as well. Tool removal is a breaking change to the tool schema

— decision 2026-08-12

> changed tool schema never gets a notice, as discussed

— decision 2026-08-12

> tool schemas are about correctness more than any other thing in the system context. either the tools change underneath, and so we either append a notice (but I don't think this is wise or possible) or force a rebuild [refurbish/compact], or the tools _don't_ change underneath (they do, but we keep the old ones around until the next rebuild [initialise]).

— decision 2026-08-12

> to deal with correctness, we _keep the old tools working_. or it might be stuff that doesn't really affect correctness.

— decision 2026-08-12, on the two ways waiting for the next initialise is safe

> until all sessions that used those old tools have reached cache expiry (and so would be rebuilt [re-initialised])

— decision 2026-08-12, on how long the limb accepts old tool calls

> I don't see any other constraint? The question is - is there ever going to be another use of this tool code version, or not?

— decision 2026-08-12

> I'm not sure if new tools work yet or not. Seems fine to me?

— decision 2026-08-12, on mid-session tool additions (hedge his)

> I'm unsure

— decision 2026-08-12, on tool addition, against "almost certainly" for tool removal

> the _reverse_ question is the real one: is it ever possible to change anything about tools _without_ involving the cached prefix. That's unanswered.

— decision 2026-08-12

> I'm unsure about whether tool addition works robustly at all via append (ie. without breaking prefix)

— decision 2026-08-12

> no, highly doubt it? but yes, unsure.

— decision 2026-08-12, on whether providers validate tool arguments against the advertised schema

### Option sets

> Avaialble agent types for subagent tool, available limbs for subagent tool, other tool option sets etc.

— `source-notes/context-updates.md`, listed among things a notice covers

> they're still in the context just not in that spot, and they obviously still change - that's the whole _point_ of not putting them in the schema proper - and so yes they absolutely get notices!

— decision 2026-08-12

> skill names are options for the skill tool! subagent names are options for the task tool!

— decision 2026-08-12

### Per-element decisions

> only if loaded

— decision 2026-08-12 (skill content)

> a safe general rule

— decision 2026-08-12, on not notifying when only a skill description changed

> skill descriptions changing is unusual without skill content changes too, and descriptions are not usually load bearing

— decision 2026-08-12

> only if it would be available

— decision 2026-08-12 (a new skill)

> almost certainly yes

— decision 2026-08-12 (AGENTS.md and other limb context)

> that feels more like a contract to me. It's technically the same though.

— decision 2026-08-12

> Other context eg. AGENTS.md files, global / machine / user context.

— `source-notes/context-updates.md`

### What cannot change without an initialise

> Changed limb (notably - this changes the limb-specific context hierarchy. this is load bearing and shan't change without a compaction / context re-build.)

— `source-notes/context-updates.md`

> limb is not one thing... it's tools, context, cwd, and more

— decision 2026-08-12

> Maybe likewise for changed working directory and/or hostname etc.

— `source-notes/context-updates.md`

> hostname change can be legit. maybe a limb can relocate too? not sure... also - not every limb has a cwd

— decision 2026-08-12 (low confidence, his)

> not sure if this is allowed or not - I tend to think not without a compaction, as model can't be expected to respect role changes that occur later in the context

— `source-notes/context-updates.md`, on changing the agent's role / mandate

> Changed model? Unclear.

— `source-notes/context-updates.md`

> I think we do want to tell the model which model it is. But yes, changing it invalidates cache so we do rebuild [initialise].

— decision 2026-08-12

### Economics

> the actual economics is the fundamental here (that the notices are actually a contingent choice - unconditional input smaller, conditional billing an extra turn (so more cache read in that branch), vs unconditional billing of larger number of tokens at 'input' cost but no extra turn. And yes, this is exactly the same as progressive disclosure.

— decision 2026-08-12

> a connection, not a fundamental

— decision 2026-08-12, on the progressive-disclosure link

> maybe a point that it typically requires empirical measurement or experiment?

— decision 2026-08-12

> an economic decision as well

— decision 2026-08-12, on progressive disclosure

> compaction economics, cancellation economics... and forking economics all feel like empirical domains. I am not very strong in this area, so I would prefer a mechanism for agents to run these experiments or perform observational tuning. For example, a background meta-agent could tune global harness settings via A/B testing. If we can run a scheduled meta-agent, it could also tune handover instructions and other parameters over time.

— decision (hedges his; a capability want, not a committed design)

### Progressive disclosure

> Not all innformation can or should be made available to the agent at the get go. this is a careful balance between always up-front input cost and conditional repeated cached-input cost from tool calling to get more info.

— `source-notes/context-updates.md`

> In real world cases, skill and tool descs can otherwise take up massive context paid on *every* session.

— `source-notes/context-updates.md`

> Some skills can be gated behind other, strictly more broadly applicable skills being loaded first. Skill desc need to say when to load.

— `source-notes/context-updates.md`

> We should have an info architecture skill and a skill writing workflow that helps motivate & get this correct.

— `source-notes/context-updates.md`

> previously implemented in my opencode fork.

— `source-notes/context-updates.md`

> something similar can & should be done for tools.

— `source-notes/context-updates.md`

> Limb model should also help to reduce this - a subagent can be given a specific limb that has a context-specific tool set. those tools do not need to be available in every session.

— `source-notes/context-updates.md`

### Change thresholds, and the utility model

> you don't always need to actually say anything.

— decision 2026-08-12. "Change thresholds for content notices" is his preferred name for this over "versioning".

> not just time, hash (or frankly just equality - hashes are for when you don't want to keep the content itself around) is better.

— decision 2026-08-12

> there's one more thing to consider - again only if the economics justifies it - passing these things through a 'small model' or 'utility model' for better quality classification & summary.

— decision 2026-08-12 (hedge his)

> it's potentially possible that a utility model (eg. claude haiku) can re-use the same prefix at 0.1x cost. This should be confirmed empirically - if true, it's useful. Otherwise we'd give it a one-shot task with a cached system prompt, just enough context, and have it produce a one-word answer. (which tbh may often be cheaper than 100k+ context at 0.1x.

— decision 2026-08-12

### Provenance

> this is great, something I'd never thought of, and is potentially _very_ useful for the model, if we can give it that info.

— decision 2026-08-12, on the derived claim that the harness reliably knows only three buckets: changes it caused itself, observable git state, and unattributable external edits

### Rendering at request build

> good point!! this is a great question! render notices at request build makes a lot of sense.

— decision 2026-08-12

> we compute + lock notices & other piggybacked contributions as we build the request

— decision 2026-08-12, generalising the rule past change notices to every piggybacked contribution

### Data sources, the graph, and the cut

> I think there are multiple data sources. A limb is a data source, yes, and is so for multiple sessions (eg. forked sessions especially) but I think there's also machine context, user context, maybe face-specific context. Probably the user-turn stuff gets implemented as a data source.

— decision 2026-08-12

> there's a computation graph. we ask it for the 6pm context. it gets built for us.

— decision 2026-08-12

> while the agent turn is going, there's constant demand - we're streaming live updates so that the latest notice set is immediately ready to piggy back on the next request.

— decision 2026-08-12

> Think of the tool call loop as another data source maybe, and only render + send once you've got derived data based on the vector clock value that is greater (or the same) over all data sources. We don't necessarily have to literally implement that (ie. what we build could be a 'manually rolled out / manually compiled' version of that), but that's the logic behind what we want to do.

— decision 2026-08-12

> re-ordered data should NOT be a problem, a data source should NOT do this.

— decision 2026-08-12

### Topology, and the I/O around the pure logic

> it shouldn't just be one node - first of all, it's one reader per limb, one notifier per session / context, etc.

— decision 2026-08-12

> We have to build some I/O stuff! We have to read the time, read the context files for a 'ordinary fs limb' & emit the new versions, etc.

— decision 2026-08-12

### Cold contexts

> I think we only ever need to load it up in order to compact it

— decision 2026-08-12, his first take, revised the same day

> I think ideally we just keep an 'old cold' context and make it an 'old warm context', append some (perhaps copious, but oh well) notices, and keep going. That is the ideal state btw. it's minimum cost - we get re-billed at input to compact, we might as well up it to cache write & _not_ compact? Perhaps an option for the user? I don't think this is settled. Tool schemas for tools that will no longer work is a great reason to force the compaction, though.

— decision 2026-08-12 (hedges his)

> just leave it purely as it was? I think the latter - much easier

— decision 2026-08-12, on whether a revived context is refurbished or re-initialised on load

> An old context will contain old info. where that's important for correctness, we need notices or rebuild [refurbish/compact]. because we're relatively sure that _tool schemas_ / _tool presence_ can't be fixed via notices..., and because we don't want to keep around old tool code versions forever, this might mean we have to force rebuild _if the context contains tool description content that is stale in a correctness-affecting way_. Importantly, the same logic would apply to any _other_ things, perhaps for example _subagent_ description content?

— decision 2026-08-12 (hedges his)

### Pruning, and what the fork model absorbs

> I think the forked agent design gets around this by subbing the tail with a (admittedly output so 5.0x) task summary / report (kinda a compaction).

— decision 2026-08-12

> pruning is for either a rebuild [refurbishment], or rewriting pre-cache, in a fixed-size suffix maybe (fixed number of messages or token size whichever is larger). That's what opencode has done in the past I believe, perhaps still.

— decision 2026-08-12 (hedges his)

> I'm not confident on 'prune within suffix', it implies re-sending uncached content over and over, but on the other hand it also implies much smaller context & cache write due to tool call pruning / coalescing. Again, an economic decision.

— decision 2026-08-12

> Perhaps we should think about a 'tool call summary' field for every tool call that can be elided, then we only present that one sentence summary when we prune. That's another contingent economics thing though.

— decision 2026-08-12 (hedge his)

> The summary is not input, but output. Which is even more expensive! So yeah it seems pretty unlikely to be net positive, actually.

— decision 2026-08-12, pricing the model-written summary he had just proposed

### Persistence and stored contexts

> we need to preserve enough information to produce exactly the same prefix in the next API request so that we can keep cache across restarts. So we store notices (naturally, as part of the context storage), but we don't actually need to store all data sources to produce a new context from scratch. That can & should be reloaded all the time, and so neither that nor the derived data need to be persisted (although the data sources themselves maybe be, but that's separate).

— decision 2026-08-12

> there's one for each warm cache point.

— decision 2026-08-12, on stored context state

> in practice we will probably just store the context directly. That is much simpler than trying to reconstruct it deterministically from raw events. While full determinism is a nice aspiration, it feels overly ambitious and not important enough to justify the complexity.

— decision 2026-08-12

> any other surrogate ids or whatever that refer to cache affinity or cache points

— decision 2026-08-12, noted as a dependency on another doc rather than designed here

### Analytics

> analytics should be about _what_ do we keep, _for how long_, and especially _why_. 'keep everything' is an option but not an answer.

— decision 2026-08-12

### Harness voice

> not sure about this, but yes... harness voice is for ground truth - the time, but also 'the AGENTS.md contains this content', but the _content_ while it is being shown by the harness, is not in the voice of the harness.... tbh I think this is all relatively obvious to the agent. but the channels or roles do matter, but also I just want the agent to do what I want. I don't have an authority model across multiple users or whatever.

— decision 2026-08-12 (hedges his)

### Tension: reload the sources, or store them

> That can & should be reloaded all the time, and so neither that nor the derived data need to be persisted

— decision 2026-08-12

> we may actually have to store some stuff in order to know what's _different_ when resuming a weeks-old session. so maybe storing both source info and rendered api request content for caching.

— decision 2026-08-12

> I meant we might have to store both input (for diffing in the right place) and output (rendered API request for caching purposes / close to it). Yes, this contradicts my earlier statement about not storing data, just reloading it.

— decision 2026-08-12

Both are his, same day. He named the contradiction himself and left it there; no ordering between them is recoverable from the record.

## Open questions

1. **Are there exactly three rungs?** The previous doc claimed the ladder has no fourth rung, because the three are the three cost regimes: append pays 0.1× on the prefix plus a write on the delta, refurbish forfeits the prefix and pays ~1.25× on the new whole, compaction pays model output. Those prices come from Anthropic's docs via `PLAN.md`, not from you. Your "original vision" names three. Is "no fourth" your claim, or just the three you happened to list?

2. **Elapsed time: the value, or the fact?** The per-element table said the elapsed-time notice carries the elapsed time itself, and argued that minimising it to "time has passed" would be wrong because the value is a few tokens and retrieval costs a whole turn. Your note gives the threshold — `"Probably more than 1h is a good point to start injecting this"` — but not the payload. The value, the bare fact, or a clock tool?

3. **Is there a notifier?** The doc concluded `"There is no notifier entity"`, because notices are rendered at request build so nothing needs owning between requests. Your topology note says `"it's one reader per limb, one notifier per session / context, etc."` Did render-at-build remove the notifier, or is the notifier the thing that does the rendering?

4. **A reader per data source, or per limb?** The doc said `"a reader per data source observes them and reports current content to the brain"`. You said one reader per limb. Machine context, user context and the user-turn stream are data sources that are not limbs — do they each get a reader, and where does it live?

5. **Is a contribution's identity `(data source, kind, name-or-path)`?** You named the aspect "identity of a context contribution". The doc's answer was that triple, over a contribution defined as `"anything that goes into a context: skill content, an AGENTS.md layer, a tool description, an option set, a notice, user activity"`. Its consequence, which it called genuinely confusing and left open: a rename becomes a delete plus an add, so the agent sees "skill gone" plus "new skill". Is the triple right, and is delete-plus-add acceptable?

6. **Is "content versions" the right name and the right granularity?** The doc used it for the per-context record of which contributions went in and what each said at the time — the thing that answers "has this session seen the old version?". You renamed the neighbouring aspect away from "versioning" to "change thresholds". Same objection here, or is this one fine?

7. **What does a notice actually say?** The doc proposed four parts — `"What changed, the kind of change, who changed it, and the available action"` — with the example `"Skill github changed (content edit, by the user). Reload it with the skill tool if relevant."` Your own worked example is at the other end: `"one or more skills have gone stale."` Is the four-part form the default and yours the batched fallback, or the reverse?

8. **Placement.** No source behind any of it: the doc asserted that everything the build-time comparison finds goes `"in one block, not one per element"`, that the block `"never separates a tool call from its result"`, and proposed notices before the user's message `"so the user's words are the last thing read"`. Confirm, deny, refine.

9. **Channel.** You said harness voice carries ground truth and that channels and roles matter. The doc went further: `"System-reminder-style is fine; a real provider channel would be better."` Is that your preference, where a provider offers one?

10. **What is debouncing for?** The doc said it is `"Not a cost mechanism"` — render-at-build already collapses repeated edits — and that its `"only job is behavioral"`: while you are actively editing a skill, a notice on every request may destabilise the agent. Is behaviour the only reason left, and is the window a tunable like the ~1h one?

11. **Does the harness ever choose a prefix?** The doc claimed `"The provider uses the longest previously cached sequence automatically, so the harness places breakpoints but never selects a prefix."` That comes from Anthropic's documentation quoted in `PLAN.md`. Is it a design commitment, or a `provider-cache-probe` question — given the OpenAI responses API may differ?

12. **Was pruning rejected?** The doc recorded it as `"Considered and rejected"`: inside the cached region pruning is a refurbishment, in the tail it only pays within a turn or two and that is the freshest content, and the fork model subsumes it. Your words are softer — pruning `"is for either a rebuild [refurbishment], or rewriting pre-cache, in a fixed-size suffix maybe"`, and `"I'm not confident on 'prune within suffix'"`. Rejected, or still an open economic question?

13. **What does a fork inherit?** The doc said `"A child inherits content versions at the fork point"`, and that what prefix a fork actually inherits `"is an experiment"`. The inheritance rule has no source. Does a child that never loaded a skill itself, but inherited it from the parent, get that skill's notices?

14. **Authority.** The doc said `"No permission model over who may change sources; personal limbs run YOLO and approval theatre is explicitly unwanted."` Your words are `"I don't have an authority model across multiple users or whatever"`. The YOLO and approval-theatre position appears only in agent-written text — `REQUIREMENTS.md` L40 and `PLAN.md` L144 — with no hit in `source-notes/`. Do you stand by it as written, or is it just "no authority model, not designed"?

15. **Tool addition, if append turns out to work.** You are unsure whether tool addition works robustly via append at all, and the doc made that the whole content of its per-element row: `"the uncertainty is not the notify decision but whether tool addition works robustly via append at all"`. Suppose the experiment says append works. Is the answer then simply "notify, like an option set"?

16. **What else forces the expensive path?** You hedged that the correctness logic applies beyond tools — `"perhaps for example _subagent_ description content?"` The doc hardened that into an aspect: `"tool schemas and the tool set for certain, perhaps subagent descriptions too"`. Which other content is correctness-affecting enough that a notice will not do?

17. **Where does revive-as-warm stop?** You called it the ideal state, said `"I don't think this is settled"`, and named stale tool schemas as `"a great reason to force the compaction"`. The doc left the boundary open. Is anything besides tool schemas and the tool set enough to stop it — a very long gap, a very large context, a model change?

18. **Source resolution.** Marked unreviewed in the doc: the "same" skill can come from different data sources, so resolution and precedence between them is a real question, overlapping context-layer composition (`source-notes/configuration-model.md`, flagged there as needing significant design work). Does precedence belong to this aspect or to the configuration model?

19. **What analytics keeps, for this feature.** You ruled that analytics is about what we keep, for how long, and why. The doc's answer for this feature was change facts and rendered notices, `"and the reason to keep them is that the overreaction question and the tunables cannot be answered without them"`. Is that the right pair, and for how long?
