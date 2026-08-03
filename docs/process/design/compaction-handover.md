# Compaction / handover — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why (user-involved) · what (agent-drafted, unreviewed) · interactions, summary — not yet done.** Derives from `source-notes/compaction.md`, the "Compaction note" in `source-notes/context-updates.md` (freshest thinking), and `source-notes/handoff-improvements.md`. Part of this design is already empirically validated in the user's OpenCode fork (`source-notes/open-code-inspiration.md` names the fork the behavioural source of truth where features overlap).

A naming caveat up front, because it colours everything: "handover" is an imperfect term. It is *othering* — it implies the agent is finished and a stranger picks up from scratch. That is roughly the mechanics, but not the feel the user wants. The intent is that **the agent considers itself continuing, yet still does everything it would need to do to hand over to a fresh agent.** The word is a stand-in for the *quality* being aimed at, not the feeling. The model-facing name stays open.

## Why

Priority, from the user: **#1 is most important; #3 and #4 together are the secret sauce.**

### 1. Continuity of long work past the context limit — *correctness*

Work longer than the context window simply dies mid-task without compaction. The situation: you're hours into a task, the context fills, and the agent stops with the goal lost. The user wants long work to *continue*, intact.

### 2. Above all, don't lose the goal and current place — *correctness (the cardinal sin)*

Compaction is lossy, so this is the quality bar: the one thing that must survive is intent and where you are in the work. This is what the "handover" framing is reaching for — the *quality* of continuation — rather than a "summarise"/"compact" framing. Not a claim that one word is good and another bad; a claim about what the operation must achieve.

### 3. Keep active context cheap — *resource* — **secret sauce**

Cache reads and writes are much cheaper than fresh input — roughly 10×. But cache-read cost re-accrues the *entire* context history on *every* subsequent tool response or user message. So once the context is long enough, its sheer length outweighs the per-token cache discount: even at 10× cheaper reads, 10× more context than a compacted version would carry means you are losing money versus having compacted earlier. Compacting earlier saves on *every* future turn.

### 4. The agent picks the moment, not the clock — *quality + resource* — **secret sauce**

A harness-forced cut fires mid-thought and loses more; the agent knows the good seams. Proactive, agent-chosen cutting is precisely what *enables* #3 — compact earlier rather than later — and the agent is who knows when it can afford to. Proactive timing also lets compaction happen in otherwise-dead time (waiting on a long command, or an absent user), so a short warm context is ready when work resumes, and a lapsed cache is a cheap miss.

Empirical knowledge from prior experiments: a compaction fired *just before an agent reports back to its parent* must be a **separate flow** from mid-task compaction. A compaction that *feels* mid-task makes the successor carry on with an already-*done* task instead of simply reporting back. So "compact-then-report-back" needs its own tool/flow distinct from "compact-and-continue". This is really an instance of #6 — the successor must be told which situation it is in.

### 5. Don't break the cache doing the thing meant to save the cache — *resource/correctness*

The two-stage append-only flow exists for this. It is the obvious piece that OpenCode currently gets wrong. Agents handle a compaction flow appended to the end of the context fine — it is not complicated, and the general shape is very likely well-represented in training data.

### 6. To write forward correctly, you must know what the fresh context will and won't contain — *correctness*

Obvious once followed through: a good compaction is one that empowers the next agent going forward. To know what to *spend effort preserving*, you must know what is *already covered* by the fresh context (and so needn't be repeated), which lets you leave out what won't be needed and concentrate on what will. This means the flow must inject facts about the new situation — notably the **diff between the current system prompt/context and the successor's**, so differences don't silently become stale or incorrect.

### 7. (maybe) Record what matters as it happens — *unsettled*

A stateful handover document the agent maintains throughout the session, rather than a one-shot end summary. Kept as a *maybe* — the user is not sure about it. Not treated as a settled why.

## What

### Compaction is the construction of a successor context

The single most useful reframing for this design is that compaction is not "shorten the conversation". It is **the construction of a successor context**, performed by the agent whose context is about to end, using knowledge only that agent has. Say it that way and the mechanics fall out as three movements: the live context is *sealed*, the agent is *briefed* on exactly what its successor will and will not have, and the agent writes the *payload* the successor's context is built from. Everything in this section is detail on those three movements, plus the separate question of when they fire.

"Sealed" is load-bearing, and it comes straight from why #5. The live context is never edited, truncated, or rewritten in place. The whole compaction flow is *appended* to it, which is what makes the operation cheap: the request that asks for the compaction is a cache **hit**, and the only new input tokens are the briefing plus whatever the agent does while it is being compacted. The successor context is a *new* context built beside the old one, not a mutation of it. The old one stays in the session record intact, which is also what makes retry and introspection possible later.

### The two-stage flow, concretely

This part is fork-proven — the user's OpenCode fork implements it as two tools, `handover` and `handover_complete`, and the note records that it "works well". In wire order, appended to the end of the live context:

1. The agent calls the **first** tool. Its arguments are minimal — the agent is not writing the handover yet, it is declaring intent. Whether it should carry anything at all (a reason, a self-assessed situation) is an open call; the fork's version carries essentially nothing, and starting there is the honest default.
2. That call gets a **minimal result**, because a tool call and its result must stay adjacent on the wire and the model must never see a call without a result (walking-skeleton current truth, `REQUIREMENTS.md`). So the result is an acknowledgement, not the briefing.
3. The **briefing** is appended as its own message: what the successor situation is, what carries over for free, what will be lost, what will be *different*, and which of the two situations (continue, or report back) this is. This is the substantial injected content and it is detailed below.
4. The agent may now call **ordinary tools** — any number of them. This is not incidental; it is one of the reasons the flow has two stages (see below).
5. The agent calls the **second** tool, carrying the payload: the written handover, plus attachments.

The successor context is then built from the payload: a fresh rebuild of the system prompt (canonical, current — see the context-updates design for what "canonical" means), the handover text, and the attachments' results materialised as though they had just been fetched.

### Why two stages and not one

Three independent reasons, and it is worth separating them because they would each individually justify the extra round trip.

First, **the briefing is situation-specific and cannot live in the system prompt**. It has to name *this* successor's diff, *this* situation, *this* set of what-will-be-lost. Anything permanent enough to sit in the system prompt would be generic, and generic advice is exactly what does not tell the agent what to spend effort preserving (why #6). Put the other way: the briefing is expensive content that is worth paying for exactly once, at the moment it is true, and appending it at that moment is how you pay once.

Second, **the model must have read the rules before it writes**. In a one-stage flow the agent produces the handover in the same turn in which it learns what a good handover is for this situation — which is to say it does not learn it at all, it pattern-matches. Two stages force the briefing into the context *before* the output tokens that depend on it.

Third — and this is the parked-material point, absorbed here — **the middle step is a cleanup slot**, and it exists because the agent still has full context at that moment. It knows for sure what it owns, what it created, what it no longer needs: which scratch files are its, which branch it made, which process it started. Wait until after the handover and those facts are gone; the successor inherits a description of the work and no knowledge of the litter. So the briefing does not merely permit tool calls at step 4, it *encourages* them: tidy up now, while you can still be certain. (The same "clean up at the boundary" shape appears in forked-subagents' why #4; that pairing is flagged for stage 3, not resolved here.)

### Which channel the briefing arrives on

The notes point two ways and both are the user's own. `context-updates.md` says compaction should happen "by instructions (ideally system parts - depends on provider / model support) being appended to the end of the conversation". `handoff-improvements.md` says the agent "recieves a user message (not tool call result) with instructions how to correctly use the tool". The fork does the user-message version, and it works.

The resolution is that these are answers to different questions. The *preferred* channel is a system part appended at the end, because that is what the content actually is — the harness instructing the model — and because it should not be attributable to the user. The *proven* channel is a user message, and it is the fallback wherever a provider does not support late system parts. So channel selection is a provider-capability question and belongs in the experiment, not in this doc.

There is a consequence worth naming rather than leaving implicit: if the briefing lands as a user message, the session record now contains something the user appears to have said and did not. That is a small lie in exactly the place this project cares about honesty — invariant 3 says an event is about its emitter. Whatever channel carries it on the wire, the *recorded fact* must be attributed to the harness, and the wire form must be a projection of that fact rather than the fact itself. The same problem appears in context-updates for change notices, which suggests there is one "harness voice" mechanism serving both. Proposed here; see Questions for review.

### What the briefing has to say

Why #6 is the whole reason this content exists: to write forward correctly the agent must know what the fresh context will and will not contain. Concretely the briefing sorts the current situation into three buckets and then states the situation.

**Carried for free.** Everything the successor's system prompt will regenerate identically: the limb's instructions, the AGENTS.md set, the agent's role and mandate, the tool schemas, the machine and user context layers. The instruction that follows is "do not repeat any of this" — and it is worth being explicit, because the failure mode without it is an agent that spends a thousand output tokens faithfully restating its own AGENTS.md.

**Gone unless you keep it.** The entire conversation body: loaded skill content, every tool call and result, the user's messages, the user's in-band activity, subagent results, earlier briefings and earlier handover text. This is the bucket that decides whether the compaction was good, and the quality bar from why #2 applies to it — goal and current place above all. Four kinds of content are worth calling out because they are systematically under-preserved and expensive to rediscover: **negative results** (what was tried and did not work, which a successor will otherwise cheerfully retry), **decisions with their reasons** (a decision without its reason gets relitigated), **the current place** in a multi-step plan, and **the user's own words** where they carry intent. That last one is a proposal, not something the notes say: paraphrasing the user's instructions is precisely how intent quietly drifts across a handover, so the briefing should ask for verbatim preservation of the user's wording where it is load-bearing rather than a summary of it. See Questions for review.

**Changed.** The diff proper — the parts of the successor's context that will differ from the current one. This is the piece that stops a handover from silently manufacturing stale facts, and it deserves its own treatment.

The briefing also states the **situation** (continue versus report back, below) and tells the agent what it may **attach**, because attachments change what is worth writing.

What the notes do *not* say, and I am not going to invent: how long the payload should be, whether it has a required structure or fields, and whether the agent should be given a token budget for it. The fork's prompt presumably has opinions; this is a place to go and read it (`source-notes/open-code-inspiration.md` names the fork the behavioural source of truth) rather than design fresh. Recorded as a gap.

#### Computing the old→new diff

The diff is the most mechanically demanding part of the briefing, because it requires knowing the successor's context *before the successor exists*. That is possible — the successor's system prompt is a deterministic function of the limb, the config, and the current state of the files that feed it — but it forces a concrete requirement: **the harness performs a dry-run rebuild at briefing time**, diffs it against the live context's system prompt, and renders the difference into the briefing.

Three things follow, and they are the kind of thing that is cheap to design in now and painful to retrofit.

The rebuild has to be **pure enough to run twice** and produce the same answer, once for the dry run and once for real. If a rebuild has side effects, or reads a clock, or depends on ordering, the briefing describes a successor that never quite happens.

The diff is between **projections**, not between internal structures — which is another argument for the single shared projection the walking skeleton already established, where `/dump` renders exactly the request builder's view. Two projections would let the briefing describe a context that differs from the one actually sent.

The dry-run rebuild is **where mid-session drift surfaces**. The AGENTS.md the user edited two hours ago, the skill another agent rewrote, the tool that appeared — all of it shows up here as a difference between the sealed context and the successor's, whether or not a notice was ever appended about it. That makes the handover the reconciliation point for everything context-updates could only notify about, and it means the diff is not a nicety: without it, the successor's fresh AGENTS.md silently contradicts a handover written against the old one.

Note the asymmetry the diff has to express. Something that *left* the system prompt matters because it must now be carried explicitly. Something that *changed* matters more, because the successor will read the new version and the handover may be describing the old one — so the correct instruction is not "carry this forward" but "do not assert this; the successor will see a different version".

### Attachments: prepaying the successor's first turns

The compaction call takes attachments: files and resources that are loaded immediately into the fresh context, appearing there as ordinary tool calls, executed in a single init step. This is the same mechanism `handoff-improvements.md` describes for the task tool, and the user explicitly extends it to handover: "Rather than the parent agent needing to use output tokens to re-summarize everything, it can attach files. These files are read directly." AGENTS.md-style preloading is named as a use case.

The important semantic call is that attachments are **re-executed at successor start, not copied forward**. The successor sees the file as it is *now*, not as the predecessor saw it. This is the right default for three reasons: it is what the successor would have got by reading the file itself, it keeps the attachment mechanism identical to a normal tool call (which is how the model already understands it), and it means an attachment cannot become the stale-fact vector the diff exists to prevent. The cost is that an attachment can *fail* — the file moved, the repo changed, the command no longer works — so a failed attachment needs a defined representation in the fresh context: the call appears, with an error result, visible to the successor. Silently dropping it would leave a handover referring to content that is not there. This is proposed here; the notes do not address attachment failure.

Attachments change what the agent should *write*. If a file can be attached, summarising it is strictly worse: the summary costs output tokens now, loses fidelity, and cannot be re-read. So the briefing should say so — attach, don't paraphrase — and the payload's prose should be about intent, decisions, state and place, not about file contents.

The economics are worth stating precisely because they are not the obvious "attachments are cheaper". The tokens are the same either way; the file's bytes enter the fresh context regardless. What attachments remove is **turns**: without them the successor spends its first several exchanges re-reading, and each of those exchanges is a full cache-read pass over the context so far plus a cache write. That is the "associated cache-read round-trips" the note names, and it is why attachments matter more the longer the fresh context already is.

There is a risk on the other side, and it is the same permanence arithmetic as context-updates' why #3: an attachment the successor never needed sits in the cached prefix for the rest of the session and is re-read on every subsequent request. Attachments are not free guesses. Few and load-bearing is the rule; proposed here.

### Two situations: continue, or report back

This is the empirical finding recorded in why #4, and it is the one place where getting the design wrong produces a specific, observed failure: a compaction that *feels* mid-task makes the successor carry on working on a task that is already done, instead of reporting back to its parent. So there are two flows, distinguished at the briefing.

**Compact and continue** is the ordinary case. The successor's job is to keep working. The briefing frames the situation as continuation, and the payload's job is goal, place, and everything needed to proceed. Milestone compaction — the deliberate tidy-and-compact at a natural seam — is this flow with different timing, not a third flow.

**Compact and report back** is the case where the work is finished and the remaining job is to produce the result for the parent. The briefing must say this unambiguously: the task is complete, nothing further is to be attempted, and the only output is the report. The payload is correspondingly different — it is oriented at what the parent needs to know rather than at what a worker needs to proceed, and results in this harness are the last message part of a turn (`source-notes/open-questions.md`), so the successor's whole existence is to produce that part well.

Two things about report-back are genuinely undecided and I am flagging rather than guessing. First, whether the successor needs tools at all: the strict reading is that a reporting agent should not be able to act, which argues for a near-zero-tool situation (the limb model already admits a zero-tool limb as valid), but a reporting agent may legitimately want to verify a claim before making it. Second, whether there needs to be a successor at all — the degenerate version of this flow is that the compaction payload *is* the result returned to the parent, with no fresh context ever built. The user's why says the successor should "simply report back", which reads as though a successor exists, so that is the reading taken here. Both are in Questions for review.

Whether the distinction is two tools or one tool with a mode is a model-framing question, deliberately left to the experiment; it belongs with naming, below.

### When it fires

Only four things may drive a provider request (invariant 2), and one of them is "cache-nearly-expired proactive handover". So compaction is not a background operation — every compaction is a request, and the design question is who initiates it.

**The agent initiates**, normally. This is why #4: the agent knows the seams, so it calls the first tool when it judges the moment is right. For that judgement to be real rather than decorative the agent needs numbers, which means the harness must *tell* it the state of play: roughly how large the context is, roughly how long the cache has left, roughly what a compacted context would cost. That information is appended, and appending is not asking (invariant 2), so it piggybacks on requests that were happening anyway and costs nothing extra to deliver.

**The harness invites**, when the cache is about to lapse and the agent has not acted. The invitation is itself a request — legitimately, it is the named trigger — and its economics are favourable in exactly the moment it fires: the invite is priced at a cache *hit* on a context that is about to become a cache *miss*. If the agent declines, we spent one cheap request to ask. The harness never compacts on the agent's behalf; nothing in this design lets the harness rewrite a context it does not understand.

Why #4's other half is that proactive timing lets compaction happen in **dead time** — while a long command runs, while the user is away. Both are situations where a request costs latency nobody is waiting on, and the result is a short warm context ready when work resumes. And if the cache does lapse before the work resumes, it lapsed on a short context, which is a cheap miss rather than an expensive one. This is the clearest case where compaction earns its keep without any context-limit pressure at all.

The break-even is the arithmetic in why #3, and it is worth writing as a rule the agent and harness can both apply: cache reads run roughly 10× cheaper than fresh input, so carrying roughly 10× more context than a compacted version would carry costs about the same per turn as paying fresh price for the compacted one — and beyond that ratio you are losing money on every subsequent turn. The user's other figure, from `compaction.md`, is a deliberately conservative one: "I think it is typically worth it at a conservative assumption of 5x cheaper cache reads." These are not in conflict; 10× is the discount he observes and 5× is the assumption under which the handover tool still pays. Design to the conservative number.

The note's caveat is a real design constraint and not a hedge: cache-aware compaction "requires the compaction/handover procedure to be well-tuned. If it loses important info, compacting frequently makes things worse, not better." Frequency multiplies whatever the quality of the operation is, in whichever direction. So the proactive trigger should not be enabled until the quality bar of why #2 is demonstrated, which makes it a *second* experiment question rather than something to validate simultaneously.

#### What the harness must know to predict cache state

Cache-state prediction is required by this design, and it is required in the same shape by context-updates (append versus rebuild) and by forked-subagents (fork versus fresh). Here is what it can honestly be.

What is knowable locally: when the last request on this session was sent, how large the context is, whether the last request wrote a cache entry, the provider and model, and whatever TTL the provider documents. What is *not* knowable is whether the entry actually survived — caches are best-effort, undocumented in detail, and vary by provider. So prediction is not a boolean; it is a **cost model with a confidence**, and every decision made from it is an expected-value bet rather than a fact.

That has a pleasant consequence. Every provider response reports how many input tokens were cached versus fresh, so every request is a labelled observation of whether the previous prediction was right. If those observations are stored — and invariant 5 says durable session data is analytics-grade and queryable — the predictor is calibratable from the session record, per provider and per model, without any special instrumentation. The design requirement that falls out is small and specific: record the prediction alongside the outcome, not just the outcome.

What this doc does not decide, because the notes do not and the answer is empirical: the actual TTLs and invalidation rules of the providers in use. `handoff-improvements.md` is blunt that "we need to *very* correctly use OpenAI responses API & Anthropic messages API w.r.t. caching for this all to work", and that is an experiment, not a design call.

### Cancellation, failure, and litter in the sealed context

Append-only makes failure handling mostly pleasant and creates exactly one new problem.

The pleasant part: because nothing is mutated, a compaction that fails, errors, or is cancelled leaves the live context usable. The session simply continues from the sealed context, which is still warm. Retry is cheap for the same reason the first attempt was cheap — the briefing is now in the cached prefix, so a second attempt re-reads it rather than paying for it again. Cancellation semantics are the harness's existing ones (invariant 9: request → drain → finalize, four-valued outcomes), and a cancelled compaction is a cancelled turn like any other.

The new problem is **litter**. An abandoned compaction leaves the first tool call, its result, and the whole briefing permanently in the context, where they will be re-read on every subsequent request for the rest of the session — the same "appended material is permanent" arithmetic as context-updates' why #3. Worse, they are *misleading* litter: a context containing an unfinished handover briefing is a context in which the model has been told it is about to be replaced and then wasn't. Three options, none of them in the notes: leave it (cheap, confusing), append a short retraction (cheaper than it sounds, and honest), or treat an abandoned handover as a reason to prefer a real rebuild at the next opportunity. The retraction is the proposal here, on the grounds that an unexplained contradiction in the context is the failure mode this whole design is trying to avoid. See Questions for review.

There is a related question with no answer in the notes: whether the *successor* should be told that a previous handover attempt was abandoned. Recorded as a gap.

### The maintained document, if we build it

Why #7 is explicitly a *maybe* and stays one. The idea is a stateful handover document the agent maintains throughout the session, updating it as goals shift and work completes, rather than producing a one-shot summary at the end. The current pragmatic version is a gitignored file the agent writes to and reads from, which the note says "works reasonably well".

What is worth doing at this stage is naming the tradeoff so an experiment could settle it. In favour: it directly attacks the cardinal sin of why #2, because the goal and the current place are recorded continuously rather than reconstructed under pressure at the moment of compaction; it spreads the output-token cost across the session instead of spiking it; and it survives events that lose the context entirely, like a crash. Against: it is a per-turn overhead paid whether or not a compaction ever happens; a document maintained by many turns drifts and duplicates; and it competes with the handover payload for authority — if both exist, which one does the successor believe?

That last question is the one I would want answered before building it, and it suggests the two are not really alternatives: the maintained document, if it exists, is best understood as *input* to the handover payload rather than a replacement for it. Proposed here, hedged deliberately, because the user is not sure about this and nothing here should firm it up on his behalf.

### Naming stays open, deliberately

The header already records that "handover" is an imperfect, *othering* term, and that the model-facing name is unsettled. `handoff-improvements.md` says the same thing from the other side: "I think that 'handover' is a bit of an 'othering' term so compaction tool might be better, but needs to be essentially the same." The intent — the agent considers itself continuing, yet still does everything it would need to do to hand over to a fresh agent — is the requirement; the word is a stand-in for it.

So this design deliberately does not pick a name, and that is not evasion: naming is a *measurable* variable here. The same two-stage flow, briefed identically, under different tool names and different briefing framings, will produce measurably different payloads — and why #4's empirical finding (a mid-task-feeling compaction makes the successor carry on with a done task) is already evidence that framing changes behaviour and not just tone. The design's job is to make the flow independent of the name so that the name can be varied and compared. Concretely: the tool names, the briefing's framing of the situation, and whether continue and report-back are two tools or one tool with a mode are all experiment parameters, not design commitments.

### What this makes falsifiable

The thesis, in one sentence: an agent, given a two-stage append-only flow and an explicit account of what its successor will and will not have, can construct a successor context good enough that the work continues without a visible seam, at a cost that beats letting the cache expire.

It is falsified if any of these show up. A successor that loses the goal or the current place — the cardinal sin, and the bar why #2 sets. A successor that acts on a fact that changed across the boundary, which means the diff did not work. A report-back successor that resumes working instead of reporting, which means the two-situation distinction did not take. A flow that breaks the cache, measurable directly on the provider's reported cached-input tokens. Attachments that arrive stale, missing, or unexplained. A proactive trigger that fires often enough for the quality loss to outrun the cost saving — the note's own caveat, and the reason frequency should be validated after quality, not with it. And, at the economic level: a measured cost per unit of work that is *worse* with compaction than without, which is the one outcome that would falsify the secret-sauce claim in whys #3 and #4 outright.

Invariants touched: **2** (compaction is one of the four legal request triggers; the briefing appends without asking), **3** (the briefing and the diff are projections; append versus rebuild are different views of the same facts), **5** (cache predictions and their outcomes are durable, queryable session facts), **9** (an abandoned compaction is a cancelled turn with a recorded outcome), and **10** (the successor's context must be derivable from the session record by any consumer, which is what makes the dry-run rebuild and the shared projection non-negotiable).

## Interactions

### What this design owns, and what it stands on

Compaction owns the construction of a successor context and nothing wider than that: the two-stage append-only flow, the briefing's content and channel, the rendering of the old→new diff, the distinction between continue and report back, and the judgement about when to fire. Those are the pieces with no other home, and they are what the experiment has to demonstrate.

Almost everything the flow *stands on* belongs to a sibling. Naming those things precisely is the useful output of this stage, because the experiment can then assume them rather than re-prove them.

Rebuild belongs to context-updates. This design runs a rebuild twice — once as a dry run to describe the successor, once for real to produce it — but what a rebuild *is*, what "canonical" means, and which elements a rebuild refreshes are that design's deliverables. Only two properties are needed from it, and both are already stated there: a rebuild is pure enough to run twice and give the same answer, and it does not replay the notices that described how the old context went stale. If either fails, the briefing describes a successor that never quite happens — so these are assumptions this experiment depends on, not claims it tests.

The attachment mechanism belongs to forked-subagents. Attachments exist there for a sharper reason — three parallel children reading the same bytes — and the shared init step that fixes it is the Task tool's. What compaction adds is the semantics a *context boundary* forces, and that is a real contribution rather than a restatement: attachments are re-executed at successor start rather than copied forward, and a failed attachment appears as a call with an error result. Both rules exist because the successor is a new context and the whole purpose of the diff is to stop stale facts crossing that seam. Assigning the mechanism outward and the boundary semantics inward is a scoping call rather than something the notes settle; see Questions for review.

Storage belongs to persistence-analytics. Context epochs, the durable cache handle, and the request-attempt table are its schema. Compaction contributes two requirements to it and designs neither. The first is small and easy to miss: the cache *prediction* must be recorded next to the outcome, per provider and model, so the predictor becomes calibratable from the session record rather than needing special instrumentation. The second is a constraint rather than a column — the sealed predecessor context must stay reconstructible after the epoch boundary, because that is what makes retry, introspection and invariant 10 work. Persistence currently says both that every epoch is kept forever and that epoch-keyed rows become collectable once a later epoch supersedes them, and those two need reconciling in that design's terms rather than this one's. Recorded as a question.

Cleanup belongs to the single finish procedure, whose portfolio statement is in `INTERACTIONS.md`. The middle step of the two-stage flow is that procedure with the reason set to milestone compaction, and it is the case where the answer to "does the agent get a turn at all" is unambiguously yes — which makes it the friendliest of the three reasons to build first, and an argument for compaction rather than cancellation being where the procedure is shaped.

Cache-state prediction is shared machinery, also in `INTERACTIONS.md`. What this design needs from it is narrow: an expected-value answer to "will this prefix still be warm in a few minutes", per provider and model, good enough to price an invitation. It does not need a boolean and cannot have one.

### Context-updates: the handover is where deferred change is reconciled

This is the closest pairing in the portfolio and it runs in both directions, which is why it is worth developing rather than merely noting.

Forward, the dry-run rebuild surfaces every piece of mid-session drift whether or not a notice was ever appended about it. So context-updates' rebuild-only category is not a dead end where staleness accumulates; it is a queue that drains at the next handover. The changed bucket of the briefing and the notice set are the same information computed at two different boundaries — context against the world, context against its successor — which is also why the content-version record context-updates needs for its diff is the record this design's diff reads.

Backward, that gives context-updates a licence it could not justify alone. Its most aggressive proposal is that a newly added tool is unusable until rebuild, which is only tolerable because a rebuild is a *scheduled* event rather than a hope. Compaction is what schedules it. If the proactive trigger turns out to be unsafe until handover quality is demonstrated — which this doc argues — then context-updates' rebuild-only rows are stale for longer than that design assumes. That is a dependency between the two designs rather than merely a shared mechanism, and what follows from it for the experiment pool belongs to `PLAN.md`.

### Forked-subagents: report-back is a child's result written under pressure

The compact-and-report-back successor exists to produce the last message part of a turn, which is exactly what forked-subagents defines a result to be. So report-back compaction is "write the child's result, under compaction pressure, having just lost the context that produced it" — the same failure class as a bad handover, at the same cost, which is why that design says its result quality is load-bearing in the same way handover quality is.

Two things follow. Invariant 9's outcome class travels *alongside* the result rather than inside it, so a report-back compaction has to carry the outcome class through mechanically rather than leave the model to narrate it in prose.

And the open question of whether a report-back successor gets tools turns out to be a limb-model question with an inconvenient answer. A successor is in the same limb as its predecessor (below), so a near-zero-tool reporting agent cannot be obtained by putting it in a tool-less limb; it would need the brain to filter the limb's declared tool set, which limb-model deliberately keeps out of the default and permits only when the user configures it. So the strict reading of report-back — a reporting agent should not be able to act — needs a capability the limb model does not currently have, and the honest options are to let the reporting successor keep its tools or to make brain-side filtering a real feature. That is limb-model's call rather than this design's; recorded as a question.

The dependency also runs the other way, in the direction that is easy to forget. Forked-subagents' routing prompt asks whether the parent's context is bloated, with the aside that it should not be, because we hope the parent compacts before that happens. Fork's economics assume compaction happens on time.

### Self-modification: a handover is where a pinned schema is released

Self-modification pins a session's tool schemas when its context is built and keeps them until "the next handover/compaction or the next cache break". Read from this side, that makes the handover the cheap adoption path for plugin changes: a schema-additive change — a new tool, a new optional parameter — never needs to force a cache break, because it can wait for a handover, and the dry-run rebuild renders it into the changed bucket automatically.

That costs this design nothing and is worth stating for one reason: it means the briefing's changed bucket includes the *tool set*, not only prose context. It is easy to think of the diff as being about AGENTS.md and skills, and then discover that the successor has three tools the handover text never mentions.

### Limb-model: why the dry run is possible at all

The dry-run rebuild only works because limb-contributed context is a serialisable snapshot taken at a small number of known boundaries rather than a live feed into the model's view. A handover is one of those boundaries. If layer composition turned out to need live evaluation — limb-model's own leading falsifier — the briefing could not describe the successor, so this design has a direct stake in that result.

There is also a class of diff this design does not have to express, and it is worth knowing why. A session is bound to exactly one limb and crossing a limb is always a fresh subagent, so a successor is always in the same limb as its predecessor. The limb's *identity* therefore never appears in the diff — though the content it contributes certainly can, and that is most of what the diff is made of.

### User-turn: what the successor keeps of the user's own work

Accumulated user activity is body content, so it falls squarely in the gone-unless-you-keep-it bucket, and the proposal that the user's own wording is preserved verbatim is specifically about that material — paraphrasing an agent's tool result loses detail, while paraphrasing the user's instruction loses intent. Nothing about how user activity is *projected* is this design's business.

One connection that looks live and is not: a compaction firing in dead time while the user is away, with text staged but unsent, cannot lose that text, because staged text is shared-live state that never entered the context in the first place. Multi-client-ui's classification is what makes that safe, and it is the whole of the relationship between this design and that one.

### The cells that are empty

Topology, oauth-credentials and cancellation-economics have nothing to say to this design. Topology because compaction is entirely a brain-side operation over data the brain already owns, with no boundary crossed. Oauth because credentials never appear in any context. Cancellation-economics because compaction's economics are cache-read economics rather than cancellation billing — the portfolio file records the same judgement from the other side.

Modular-components touches this design only through testing, which is still worth one line because the falsification evidence depends on it: the cache claims are measured from the provider's reported cached-input token counts, read at the fake provider's request record, so the wire-level assertion surface has to be observable between steps rather than only at the end.

The two lifecycle designs touch it thinly and in one direction. Layered-shutdown's rule is that an agent-level cleanup turn is not started during shutdown but recorded as outstanding; an unfinished compaction is the same shape of thing, which makes the litter problem and the retraction proposal a *resume* concern rather than a shutdown concern. Operator-lifecycle inherits that and adds nothing else.

### The conflict recorded at portfolio level

The two cache figures — roughly 10× in why #3 against `source-notes/compaction.md`'s "conservative assumption of 5x" — are recorded as a portfolio-level conflict in `INTERACTIONS.md`. This doc already designs to the conservative number; the portfolio entry's point is that both figures should be labelled as observed-versus-conservative rather than left floating unattributed.

## Questions for review

- **Does the report-back flow build a successor at all?** Why #4 says the successor should "simply report back", which I have read as: a fresh context exists, and its only job is to write the result. The cheaper reading is that the compaction payload *is* the result and no successor is ever built. I went with the first. Which did you mean?
- **Should the report-back successor have tools?** A reporting agent arguably should not be able to act; equally it may want to verify a claim before making it. Not addressed in the notes.
- **Verbatim preservation of your wording across a handover.** I have proposed that the briefing explicitly asks for your own words to be carried through unparaphrased where they carry intent, on the grounds that paraphrase is how intent drifts. That is my addition, not something the notes say.
- **The harness voice.** Notices (context-updates) and compaction briefings have the same problem: they are the harness speaking, and if they land as user messages the session record implies you said things you did not. I have proposed one mechanism serving both, with the wire channel (late system part where supported, user message as the proven fallback) chosen per provider. Is one mechanism right, or should compaction own its own?
- **Abandoned handovers.** I propose appending a short retraction when a compaction flow is cancelled mid-way, rather than leaving an unfinished briefing sitting permanently in the context. Also unanswered: should a later successor be told a previous attempt was abandoned?
- **Attachment failure.** I have proposed that a failed attachment appears in the fresh context as a call with an error result rather than being silently dropped. Not in the notes.
- **Should the harness ever invite a compaction, or only ever wait for the agent?** The invite is a legal trigger and cheap in the moment it fires, but it is the harness making a judgement the design otherwise reserves for the agent.
- **Sequencing of the two proactive claims.** I have argued that the proactive cache-driven trigger should not be enabled until handover *quality* is demonstrated, because frequency multiplies quality in whichever direction it points. That splits the secret sauce (whys #3 and #4) across two experiment stages rather than validating it in one.
- **How big should a handover payload be, and does it have required structure?** The notes are silent, and the fork's `handover.md` prompt is the obvious place to go and read rather than design fresh.
- **Should cache-state prediction be its own experiment?** Raised in context-updates too: it is now required by compaction-handover, context-updates, and forked-subagents alike, and all three are blocked on the same unknown provider semantics.
- **I have scoped attachments outward.** The mechanism — attachments executed once in a shared init step, appearing as ordinary tool calls — is treated above as forked-subagents', with this design contributing only the boundary semantics (re-executed rather than copied forward, failure visible as an error result). That means this experiment *assumes* attachments work rather than proving them, and it means the two designs must agree on one mechanism rather than each growing their own.
- **The sealed predecessor context versus context-lifetime collection.** Retry, introspection and invariant 10 all need the old context to stay reconstructible after a new epoch opens. Persistence-analytics says every epoch is kept forever *and* that epoch-keyed rows become collectable once superseded. I have taken the reconstructibility requirement as binding and recorded the tension rather than resolving it in that design's file.
- **A handover as the adoption point for plugin schema changes.** Self-modification releases a session's pinned schemas at "the next handover/compaction or the next cache break". I have read that as making a handover the cheap path for schema-additive plugin changes, and as putting the tool set into the briefing's changed bucket. That is my reading of the interaction, not something either note states.
- **The harness voice is scoped to context-updates rather than to this design.** Both need it; I have put the mechanism there because notices are the higher-frequency case and the late-system-part provider probe is already on that design's list. This design then consumes it. That is a placement call on top of the one-mechanism-or-two question above.
- **A tool-less report-back successor would need something the limb model does not have.** The successor is in the same limb as its predecessor, so the strict "a reporting agent should not be able to act" reading cannot be delivered by choosing a tool-less limb — it needs the brain to filter the limb's declared tool set, which limb-model keeps out of the default on purpose. Either the reporting successor keeps its tools, or brain-side filtering becomes a real feature in that design. This sharpens the earlier report-back-tools question rather than replacing it.
