# Compaction / handover — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why (user-involved) · what, interactions, summary (agent-drafted, unreviewed).** Derives from `source-notes/compaction.md`, the "Compaction note" in `source-notes/context-updates.md` (freshest thinking), and `source-notes/handoff-improvements.md`. Part of this design is already empirically validated in the user's OpenCode fork (`source-notes/open-code-inspiration.md` names the fork the behavioural source of truth where features overlap).

A naming caveat up front, because it colours everything: "handover" is an imperfect term. It is *othering* — it implies the agent is finished and a stranger picks up from scratch. That is roughly the mechanics, but not the feel the user wants. The intent is that **the agent considers itself continuing, yet still does everything it would need to do to hand over to a fresh agent.** The word is a stand-in for the *quality* being aimed at, not the feeling. The model-facing name stays open.

## Summary

Compaction here is not shortening a conversation. It is **the construction of a successor context**, carried out by the agent whose context is about to end, because that agent knows things the harness cannot: what the goal is, where the work has got to, and which of the last two hundred tool results still matter. Everything below is a consequence of taking that framing literally.

Two things force the mechanics. Compaction is lossy, so the one thing that must survive is the goal and the current place — losing those is the cardinal sin, and it is what the "handover" framing is reaching for. And the operation that exists to save the cache must not be the thing that breaks it, so the live context is *sealed*: never edited, never truncated, only appended to. The successor context is built *beside* the old one, and the old one stays in the session record intact, which is what makes retry and introspection possible.

So the flow is a two-stage append. The agent calls a first tool to declare intent; a **briefing** is appended, saying what the successor will and will not have; the agent may call ordinary tools; then a second call carries the payload — the written handover plus attachments — and the successor's context is built from it. Two stages rather than one for three independent reasons: the briefing is situation-specific and so cannot live in a system prompt, the model must have read the rules before it writes, and the middle step is the last moment at which the agent still knows for certain what it owns and created. This two-tool shape is the part of the design that already works — the user's OpenCode fork implements it and the note records that it does.

The briefing exists because of the design's least obvious requirement: to write forward well you must know what the fresh context will and will not contain. It sorts the situation into what is **carried for free**, what is **gone unless you keep it**, and what has **changed** — and that third bucket is what stops a handover manufacturing stale facts, which also makes the handover the point where all mid-session drift is reconciled. Attachments then let the payload stop paraphrasing: a file handed forward is re-read as it is *now* rather than summarised from memory, so it can never become the stale fact the diff exists to prevent.

There are two situations, not one, and the distinction is empirical rather than tidy: a compaction that *feels* mid-task produces a successor that carries on working on a task that is already finished. So **compact-and-continue** and **compact-and-report-back** are separate flows, distinguished at the briefing.

Timing is the other half of the value. The agent initiates, because the agent knows the seams, and for that judgement to be real the harness has to keep telling it the state of play — which is appended and therefore free, since appending is not asking. Proactive timing then lets compaction happen in dead time, while a long command runs or the user is away, so a short warm context is waiting when work resumes and a lapse is a cheap miss rather than an expensive one.

The largest risk is not mechanical. Frequency multiplies quality in whichever direction quality points, so a proactive trigger sitting on top of a handover procedure that loses things makes matters worse rather than better — which splits the secret sauce across two stages: demonstrate quality first, enable the proactive trigger second. Failure is otherwise mild, because nothing was mutated; the one new problem is misleading litter in a context whose handover was abandoned.

Two things stay deliberately open. The model-facing **name** is unsettled, and naming is treated as an experiment parameter rather than a gap, because framing measurably changes what the agent writes — the report-back finding is already evidence of that. And the **maintained handover document**, updated through the session rather than written at the end, stays a maybe.

## Why

Priority, from the user: **#1 is most important; #3 and #4 together are the secret sauce.**

### 1. Continuity of long work past the context limit — *correctness*

Work longer than the context window simply dies mid-task without compaction. The situation: you're hours into a task, the context fills, and the agent stops with the goal lost. The user wants long work to *continue*, intact.

### 2. Above all, don't lose the goal and current place — *correctness (the cardinal sin)*

Compaction is lossy, so this is the quality bar: the one thing that must survive is intent and where you are in the work. This is what the "handover" framing is reaching for — the *quality* of continuation — rather than a "summarise"/"compact" framing. Not a claim that one word is good and another bad; a claim about what the operation must achieve.

### 3. Keep active context cheap — *resource* — **secret sauce**

Cache *reads* are much cheaper than fresh input — roughly an order of magnitude. But a cache read re-accrues the *entire* context history on *every* subsequent tool response or user message, so a long warm context costs more per turn than a short warm one, in direct proportion to its length. Compacting does not save once; it saves on *every* future turn, and that recurrence is what makes it a resource lever rather than a tidying operation.

The cost of compacting, by contrast, is a one-off — writing the successor's context into the cache, the output tokens of the handover itself, and one request. So the decision is a payback calculation rather than a size threshold; it is worked through in §The break-even is a payback period, not a ratio. What matters for the *why* is only this asymmetry: the saving recurs and the cost does not.

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
2. That call gets a **minimal result** — an acknowledgement. It has to get *something*, because a tool call and its result must stay adjacent on the wire and the model must never see a call without a result (walking-skeleton current truth, `REQUIREMENTS.md`) — but adjacency is satisfied by any result and says nothing about what the result contains. The briefing is not carried here because of what it *is*: the harness instructing the model, which wants a channel with more standing than a tool result. `handoff-improvements.md` specifies a user message, the fork implements that, and a late system part is preferable where a provider supports one — see §Which channel the briefing arrives on.
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

**Gone unless you keep it.** The entire conversation body: loaded skill content, every tool call and result, the user's messages, the user's in-band activity, subagent results, earlier briefings and earlier handover text. This is the bucket that decides whether the compaction was good, and the quality bar from why #2 applies to it — goal and current place above all. Four kinds of content are worth calling out because they are systematically under-preserved and expensive to rediscover: **negative results** (what was tried and did not work, which a successor will otherwise cheerfully retry), **decisions with their reasons** (a decision without its reason gets relitigated), **the current place** in a multi-step plan, and **the user's own words** where they carry intent. That last one was a proposal and is now confirmed (2026-08-04): "This is my current handover instruction in my OpenCode harness, so yes" — the briefing asks for the user's own words carried through unparaphrased where they carry intent.

On size and structure, ruled the same day: "I do not like required structures in general. The handover should be as large as it needs to be while covering all necessary points. That said, giving vague instructions like 'cover everything' tends to produce low-quality output from agents. So there needs to be strong guidance on how to write a good handover. Ideally, the guidance should not imply or force a specific length or rigid structure, but it must ensure the agent reliably covers everything required in the right way." So the briefing's coverage list (goal, place, negatives, decisions-with-reasons, user's words, attachments) is guidance with teeth, not a template — and the fork's `handover.md` prompt is the starting text to adapt rather than design fresh.

**Changed.** The diff proper — the parts of the successor's context that will differ from the current one. This is the piece that stops a handover from silently manufacturing stale facts, and it deserves its own treatment.

The briefing also states the **situation** (continue versus report back, below) and tells the agent what it may **attach**, because attachments change what is worth writing.

What the notes do *not* say, and I am not going to invent: how long the payload should be, whether it has a required structure or fields, and whether the agent should be given a token budget for it. The fork's prompt presumably has opinions; this is a place to go and read it (`source-notes/open-code-inspiration.md` names the fork the behavioural source of truth) rather than design fresh. Recorded as a gap.

#### Computing the old→new diff

The diff has to say what the successor's context will contain before the successor exists. That sounds like it needs a hypothetical rebuild to compare against, and it does not — the harness already keeps the record that answers it. Context-updates computes its change notices by comparing what *this* context actually contains, element by element and version by version, against the world as it is now. That is the same comparison, taken at a different boundary: context against the world there, context against its successor here. One record, one computation, two consumers.

So the requirement this design puts on the harness is small: **the sealed context's content-version record is the diff's left side and the current state of the world is its right side.** Nothing hypothetical is built, and no special purity property is needed of a rebuild — which is just as well, because a rebuild run twice would legitimately differ anyway: it states the current time, and it picks up files as they are when it runs.

Two things follow, and they are the kind of thing that is cheap to design in now and painful to retrofit.

The diff must be expressed in the **elements the request builder assembles** — this AGENTS.md, this skill, this tool's schema — rather than as text differences between two rendered prefixes. Element-level is what the agent can act on ("do not assert what that skill said; the successor will read the new one"), and it survives the successor's prefix being rendered slightly differently.

The diff is **where mid-session drift surfaces**. The AGENTS.md the user edited two hours ago, the skill another agent rewrote, the tool that appeared — all of it shows up here, whether or not a notice was ever appended about it, because the record does not care which changes were notified. That makes the handover the reconciliation point for everything context-updates could only notify about, and it means the diff is not a nicety: without it, the successor's fresh AGENTS.md silently contradicts a handover written against the old one.

Note the asymmetry the diff has to express. Something that *left* the system prompt matters because it must now be carried explicitly. Something that *changed* matters more, because the successor will read the new version and the handover may be describing the old one — so the correct instruction is not "carry this forward" but "do not assert this; the successor will see a different version".

### Attachments: prepaying the successor's first turns

The compaction call takes attachments: files and resources that are loaded immediately into the fresh context, appearing there as ordinary tool calls, executed in a single init step. This is the same mechanism `handoff-improvements.md` describes for the task tool, and the user explicitly extends it to handover: "Rather than the parent agent needing to use output tokens to re-summarize everything, it can attach files. These files are read directly." AGENTS.md-style preloading is named as a use case.

The important semantic call is that attachments are **re-executed at successor start, not copied forward**. The successor sees the file as it is *now*, not as the predecessor saw it. This is the right default for three reasons: it is what the successor would have got by reading the file itself, it keeps the attachment mechanism identical to a normal tool call (which is how the model already understands it), and it means an attachment cannot become the stale-fact vector the diff exists to prevent. The cost is that an attachment can *fail* — the file moved, the repo changed, the command no longer works — so a failed attachment needs a defined representation in the fresh context: the call appears, with an error result, visible to the successor. Silently dropping it would leave a handover referring to content that is not there. **Ruled 2026-08-04: it appears in the fresh context as a failure**, and the reason the compaction proceeds anyway is the user's: "We do want the compaction to happen, and compaction messages are very large. We do not want them repeated. That is the real reason: they are expensive in terms of output, and repeating them is undesirable." (His hedge on the premise kept: "A failed attachment is pretty unlikely. In fact, I am not sure I fully agree with the concept.")

Attachments change what the agent should *write*. If a file can be attached, summarising it is strictly worse: the summary costs output tokens now, loses fidelity, and cannot be re-read. So the briefing should say so — attach, don't paraphrase — and the payload's prose should be about intent, decisions, state and place, not about file contents.

The economics are worth stating precisely because they are not the obvious "attachments are cheaper". The tokens are the same either way; the file's bytes enter the fresh context regardless. What attachments remove is **turns**: without them the successor spends its first several exchanges re-reading, and each of those exchanges is a full cache-read pass over the context so far plus a cache write. That is the "associated cache-read round-trips" the note names, and it is why attachments matter more the longer the fresh context already is.

There is a risk on the other side, and it is the same permanence arithmetic as context-updates' why #3: an attachment the successor never needed sits in the cached prefix for the rest of the session and is re-read on every subsequent request. Attachments are not free guesses. Few and load-bearing is the rule; proposed here.

### Two situations: continue, or report back

This is the empirical finding recorded in why #4, and it is the one place where getting the design wrong produces a specific, observed failure: a compaction that *feels* mid-task makes the successor carry on working on a task that is already done, instead of reporting back to its parent. So there are two flows, distinguished at the briefing.

**Compact and continue** is the ordinary case. The successor's job is to keep working. The briefing frames the situation as continuation, and the payload's job is goal, place, and everything needed to proceed. Milestone compaction — the deliberate tidy-and-compact at a natural seam — is this flow with different timing, not a third flow.

**Compact and report back** is the case where the work is finished and the remaining job is to deliver the result. Ruled 2026-08-04, replacing two open questions this section previously carried. The user's model, wording preserved: "Compact and report... lets the first agent build the report as well as their compaction summary. And then the compaction summary deals with the initial context of the new agent, and the report is given as if it was its first message. I guess. something like that." (His hedge kept.) So the **predecessor writes the report**, while it still has the full context the report is about — the same principle as the tidy-up step, applied to the result. The compaction summary builds the successor's initial context as in the continue case, and the report is delivered as the successor's first message rather than being work the successor performs.

That dissolves both questions the earlier draft flagged. Whether the successor needs tools stops mattering, because nothing is asked of it — no reporting work exists on its side of the boundary, so there is no reporting agent to disarm. And whether a successor exists is answered structurally: one does, because the session continues past the compaction, but its existence is not *for* producing the report. He also confirmed the two situations are exhaustive and that report-back is one thing, not two: "Report back to the user... report back as a subagent... reports back to the parent. I think that's actually exactly the same," and "it's just only those two" — compact-and-continue, compact-and-report-back.

Whether the distinction is two tools or one tool with a mode is a model-framing question, deliberately left to the experiment; it belongs with naming, below.

### When it fires

Only four things may drive a provider request (invariant 2), and one of them is "cache-nearly-expired proactive handover". So compaction is not a background operation — every compaction is a request, and the design question is who initiates it.

**The agent initiates**, normally. This is why #4: the agent knows the seams, so it calls the first tool when it judges the moment is right. For that judgement to be real rather than decorative the agent needs numbers, which means the harness must *tell* it the state of play: roughly how large the context is, roughly how long the cache has left, roughly what a compacted context would cost. That information is appended, and appending is not asking (invariant 2), so it piggybacks on requests that were happening anyway and costs nothing extra to deliver.

**Three more triggers are forcible, and they are the harness's and the user's — ruled 2026-08-04**, replacing an earlier draft in which the harness could only ever *invite* and never act. The user's taxonomy, wording preserved: "Agent-triggered compaction is just one kind: the agent reaches a milestone, recognizes a natural boundary, calls the compact tool, receives the brief, performs the compaction, and continues in a new context. But there are other triggers:

- **Context window limit**: "When reaching something like 80-85% (or better, a fixed token threshold like 100k-200k tokens), the harness should forcibly trigger compaction."
- **Cache expiry**: "This is also harness-triggered. If the agent is idle (for example, waiting on a long tool call), and the provider cache is about to go cold, the harness performs compaction. In the new context, the tool call is rewritten as still in progress, and execution continues seamlessly when it completes."
- **User-triggered**: "The user can also forcibly trigger compaction."

Two design consequences. The cache-expiry trigger requires the successor context to represent an **in-flight tool call as still in progress** — a call whose result arrives after the handover lands in the fresh context, which constrains the diff and the briefing (the call is neither completed nor litter; it is open). And a *forced* compaction still runs through the same two-stage flow — the agent still writes the payload — so "forcible" means the harness decides *when*, never that the harness writes the handover itself.

Why #4's other half is that proactive timing lets compaction happen in **dead time** — while a long command runs, while the user is away. Both are situations where a request costs latency nobody is waiting on, and the result is a short warm context ready when work resumes. And if the cache does lapse before the work resumes, it lapsed on a short context, which is a cheap miss rather than an expensive one. This is the clearest case where compaction earns its keep without any context-limit pressure at all.

The note's caveat is a real design constraint and not a hedge: cache-aware compaction "requires the compaction/handover procedure to be well-tuned. If it loses important info, compacting frequently makes things worse, not better." Frequency multiplies whatever the quality of the operation is, in whichever direction. So the proactive trigger should not be enabled until the quality bar of why #2 is demonstrated, which makes it a *second* experiment question rather than something to validate simultaneously.

#### The break-even is a payback period, not a ratio

It is tempting to write why #3's economics as a context-size ratio — *compact once the context is N times what a compacted one would be* — and that is wrong in a way worth being explicit about, because it is exactly the kind of rule that ends up hard-coded in a trigger.

Per turn, a warm context costs its length times the cache-read price. So a longer warm context loses money against a shorter one on *every* turn, at *any* ratio above one. There is no ratio below which carrying extra context is free, and therefore no ratio that marks a break-even. What the ratio actually governs is how quickly compaction pays back its one-off cost.

That one-off cost is writing the successor's context into the cache, plus the output tokens the agent spends on the handover, plus one request. The recurring saving is the cache-read price times the tokens removed. So the rule the agent and the harness can both apply is:

> Compact when the expected number of remaining turns, times the per-turn saving, exceeds the one-off cost of compacting.

Which makes **expected remaining turns** the variable a size ratio hides — and it is what separates two cases that look alike. Mid-task with a long road ahead, almost any worthwhile reduction pays back within a turn or two, so compacting early is close to free. Near the end of a piece of work, per-turn savings barely accrue at all — which is precisely why the dead-time case in why #4 is justified by **lapse avoidance** rather than by per-turn savings: what it buys is that the inevitable miss lands on a short context.

Two figures, labelled, because unlabelled numbers are how this gets designed wrong.

**Cache reads run roughly 10× cheaper than fresh input; cache writes cost slightly more than fresh input, on the order of 1.25×.** Both are general provider knowledge rather than anything measured here or stated in the notes, and both are inputs the provider-cache probe has to verify before the rule above is trusted. The write figure is the one that is easy to forget and the one that matters most here: a compaction *pays* a cache write, so a compaction is not free even though the request carrying it is a cache hit.

**"I think it is typically worth it at a conservative assumption of 5x cheaper cache reads"** (`source-notes/compaction.md`) is about a *different quantity*. It is the user's deliberately conservative assumption under which the append-based **handover tool** pays for itself — the tool's benefit being that "no cache break is required" — not a break-even ratio for whether to compact. So: design the handover tool so that it still pays at the conservative 5×, and decide *when* to compact with the payback calculation above. Two numbers, two jobs. The user confirmed the payback framing and does not want the figures locked down: "These are assumptions, which is fine. I would not want to lock them down, since timelines are uncertain and model pricing will likely change."

He also placed this whole area, deliberately, beyond design (2026-08-04, hedges his): "compaction economics, cancellation economics (which you already have as an experiment), and forking economics all feel like empirical domains. I am not very strong in this area, so I would prefer a mechanism for agents to run these experiments or perform observational tuning. For example, a background meta-agent could tune global harness settings via A/B testing. If we can run a scheduled meta-agent, it could also tune handover instructions and other parameters over time." That is a new capability idea — agent-run observational tuning of harness parameters, including the handover prompt text itself — recorded here because compaction is where he raised it, and flagged to `PLAN.md` as a candidate rather than committed anywhere.

#### What the harness must know to predict cache state

Cache-state prediction is required by this design, and it is required in the same shape by context-updates (append versus rebuild) and by forked-subagents (fork versus fresh). Here is what it can honestly be.

What is knowable locally: when the last request on this session was sent, how large the context is, whether the last request wrote a cache entry, the provider and model, and whatever TTL the provider documents. What is *not* knowable is whether the entry actually survived — caches are best-effort, undocumented in detail, and vary by provider. So prediction is not a boolean; it is a **cost model with a confidence**, and every decision made from it is an expected-value bet rather than a fact.

That has a pleasant consequence. Every provider response reports how many input tokens were cached versus fresh, so every request is a labelled observation of whether the previous prediction was right. If those observations are stored — and invariant 5 says durable session data is analytics-grade and queryable — the predictor is calibratable from the session record, per provider and per model, without any special instrumentation. The design requirement that falls out is small and specific: record the prediction alongside the outcome, not just the outcome.

What this doc does not decide, because the notes do not and the answer is empirical: the actual TTLs and invalidation rules of the providers in use. `handoff-improvements.md` is blunt that "we need to *very* correctly use OpenAI responses API & Anthropic messages API w.r.t. caching for this all to work", and that is an experiment, not a design call.

### Cancellation, failure, and litter in the sealed context

Append-only makes failure handling mostly pleasant and creates exactly one new problem.

The pleasant part: because nothing is mutated, a compaction that fails, errors, or is cancelled leaves the live context usable. The session simply continues from the sealed context, which is still warm. Retry is cheap for the same reason the first attempt was cheap — the briefing is now in the cached prefix, so a second attempt re-reads it rather than paying for it again. Cancellation semantics are the harness's existing ones (invariant 9: request → drain → finalize, four-valued outcomes), and a cancelled compaction is a cancelled turn like any other.

The new problem is **litter**. An abandoned compaction leaves the first tool call, its result, and the whole briefing permanently in the context, where they will be re-read on every subsequent request for the rest of the session — the same "appended material is permanent" arithmetic as context-updates' why #3. Worse, they are *misleading* litter: a context containing an unfinished handover briefing is a context in which the model has been told it is about to be replaced and then wasn't.

**Ruled 2026-08-04: append the retraction.** The user's reasoning adds a fact this section had missed and rules on it. First the fact: "While the cache is append-only, we are certainly allowed to throw away suffixes as long as we can predict where to put a cache point. So the cache is effectively a **branching structure**. That is why forked sub-agents work at all." So a cheaper option existed than any listed — resume from *before* the briefing, discarding the suffix. Then the ruling: "since two-part handovers have the agent taking mutable actions, you are probably right that treating it like an abandoned system message is better, because of the cleanup actions the agent would have taken. That is really the key reason." The tidy-up step has real side effects in the world; rewinding the context does not rewind them, so the record must keep what prompted them. Discard-the-suffix remains legal for flows with no mutable middle step.

### The maintained document, if we build it

Why #7 is explicitly a *maybe* and stays one. The idea is a stateful handover document the agent maintains throughout the session, updating it as goals shift and work completes, rather than producing a one-shot summary at the end. The current pragmatic version is a gitignored file the agent writes to and reads from, which the note says "works reasonably well".

What is worth doing at this stage is naming the tradeoff so an experiment could settle it. In favour: it directly attacks the cardinal sin of why #2, because the goal and the current place are recorded continuously rather than reconstructed under pressure at the moment of compaction; it spreads the output-token cost across the session instead of spiking it; and it survives events that lose the context entirely, like a crash. Against: it is a per-turn overhead paid whether or not a compaction ever happens; a document maintained by many turns drifts and duplicates; and it competes with the handover payload for authority — if both exist, which one does the successor believe?

That last question is the one I would want answered before building it, and it suggests the two are not really alternatives: the maintained document, if it exists, is best understood as *input* to the handover payload rather than a replacement for it. Proposed here, hedged deliberately, because the user is not sure about this and nothing here should firm it up on his behalf.

### Naming stays open, deliberately

The header already records that "handover" is an imperfect, *othering* term, and that the model-facing name is unsettled. `handoff-improvements.md` says the same thing from the other side: "I think that 'handover' is a bit of an 'othering' term so compaction tool might be better, but needs to be essentially the same." The intent — the agent considers itself continuing, yet still does everything it would need to do to hand over to a fresh agent — is the requirement; the word is a stand-in for it.

So this design deliberately does not pick a name, and that is not evasion: naming is a *measurable* variable here. The same two-stage flow, briefed identically, under different tool names and different briefing framings, will produce measurably different payloads — and why #4's empirical finding (a mid-task-feeling compaction makes the successor carry on with a done task) is already evidence that framing changes behaviour and not just tone. The design's job is to make the flow independent of the name so that the name can be varied and compared. Concretely: the tool names, the briefing's framing of the situation, and whether continue and report-back are two tools or one tool with a mode are all experiment parameters, not design commitments.

### What this makes falsifiable

The thesis, in one sentence: an agent, given a two-stage append-only flow and an explicit account of what its successor will and will not have, can construct a successor context good enough that the work continues without a visible seam, at a cost that beats letting the cache expire.

It is falsified if any of these show up. A successor that loses the goal or the current place — the cardinal sin, and the bar why #2 sets. A successor that acts on a fact that changed across the boundary, which means the diff did not work. A report-back flow that produces further work after the report is delivered, which means the two-situation distinction did not take. A flow that breaks the cache, measurable directly on the provider's reported cached-input tokens. Attachments that arrive stale, missing, or unexplained. A proactive trigger that fires often enough for the quality loss to outrun the cost saving — the note's own caveat, and the reason frequency should be validated after quality, not with it. And, at the economic level: a measured cost per unit of work that is *worse* with compaction than without, which is the one outcome that would falsify the secret-sauce claim in whys #3 and #4 outright.

Invariants touched: **2** (compaction is one of the four legal request triggers; the briefing appends without asking), **3** (the briefing and the diff are projections; append versus rebuild are different views of the same facts), **5** (cache predictions and their outcomes are durable, queryable session facts), **9** (an abandoned compaction is a cancelled turn with a recorded outcome), and **10** (the successor's context must be derivable from the session record by any consumer, which is what makes the content-version record and the shared projection non-negotiable).

## Interactions

### What this design owns, and what it stands on

Compaction owns the construction of a successor context and nothing wider than that: the two-stage append-only flow, the briefing's content and channel, the rendering of the old→new diff, the distinction between continue and report back, and the judgement about when to fire. Those are the pieces with no other home, and they are what the experiment has to demonstrate.

Almost everything the flow *stands on* belongs to a sibling. Naming those things precisely is the useful output of this stage, because the experiment can then assume them rather than re-prove them.

Rebuild belongs to context-updates. This design triggers one — the successor's context *is* a rebuild — but what a rebuild *is*, what "canonical" means, and which elements it refreshes are that design's deliverables. Two properties are needed from it, and both are already stated there: it is canonical, so it does not replay the notices that described how the superseded context went stale, and it has no hidden state, so the elements it produces are the elements the content-version record says it will. If either fails, the briefing describes a successor that never quite happens — so these are assumptions this experiment depends on, not claims it tests.

The record the briefing's diff reads is the same one that design's notice diff reads. That is the substantive half of the dependency and it is developed below.

The attachment mechanism belongs to forked-subagents. Attachments exist there for a sharper reason — three parallel children reading the same bytes — and the shared init step that fixes it is the Task tool's. What compaction adds is the semantics a *context boundary* forces, and that is a real contribution rather than a restatement: attachments are re-executed at successor start rather than copied forward, and a failed attachment appears as a call with an error result. Both rules exist because the successor is a new context and the whole purpose of the diff is to stop stale facts crossing that seam. Assigning the mechanism outward and the boundary semantics inward is a scoping call rather than something the notes settle; see Questions for review.

Storage belongs to persistence-analytics. Context epochs, the durable cache handle, and the request-attempt table are its schema. Compaction contributes two requirements to it and designs neither. The first is small and easy to miss: the cache *prediction* must be recorded next to the outcome, per provider and model, so the predictor becomes calibratable from the session record rather than needing special instrumentation. The second is a constraint rather than a column — the sealed predecessor context must stay reconstructible after the epoch boundary, because that is what makes retry, introspection and invariant 10 work. Persistence currently says both that every epoch is kept forever and that epoch-keyed rows become collectable once a later epoch supersedes them, and those two need reconciling in that design's terms rather than this one's. Recorded as a question.

Cleanup belongs to the single finish procedure, whose portfolio statement is in `INTERACTIONS.md`. The middle step of the two-stage flow is that procedure with the reason set to **compaction** — every compaction, not only the deliberate milestone kind, because the reason a compacting agent should tidy is that its knowledge of what it owns is about to be lost, and that is true whatever prompted the compaction. It is also the case where the answer to "does the agent get a turn at all" is unambiguously yes, which makes it the friendliest of the reasons to build first and an argument for compaction rather than cancellation being where the procedure is shaped.

Cache-state prediction is shared machinery, also in `INTERACTIONS.md`. What this design needs from it is narrow: an expected-value answer to "will this prefix still be warm in a few minutes", per provider and model, good enough to price an invitation. It does not need a boolean and cannot have one.

### Context-updates: the handover is where deferred change is reconciled

This is the closest pairing in the portfolio and it runs in both directions, which is why it is worth developing rather than merely noting.

Forward, the briefing's diff surfaces every piece of mid-session drift whether or not a notice was ever appended about it, because it reads the content-version record rather than the notice history. So the changes context-updates deliberately does *not* notify about are not a dead end where staleness accumulates; they are a queue that drains at the next handover. The changed bucket and the notice set are one computation at two boundaries — context against the world there, context against its successor here — over one record.

Backward, that gives context-updates a licence it could not justify alone. Its sharpest position is that a newly added tool is not callable in a warm session without paying a cache break, so the ordinary answer is to wait for the next rebuild. That is only tolerable because a rebuild is a *scheduled* event rather than a hope, and compaction is what schedules it. If the proactive trigger turns out to be unsafe until handover quality is demonstrated — which this doc argues — then those changes wait longer than that design assumes. That is a dependency between the two designs rather than merely a shared mechanism, and what follows from it for the experiment pool belongs to `PLAN.md`.

### Forked-subagents: report-back delivers a child's result

The report a compact-and-report-back flow delivers is the last message part of a turn, which is exactly what forked-subagents defines a result to be. Under the 2026-08-04 ruling the *predecessor* writes that report while it still holds the context the report is about, alongside its compaction summary. So the failure class an earlier draft worried about here — "write the child's result, under compaction pressure, having just lost the context that produced it" — is designed out rather than mitigated: the report is never written on the wrong side of the boundary.

One thing still follows. Invariant 9's outcome class travels *alongside* the result rather than inside it, so a report-back compaction has to carry the outcome class through mechanically rather than leave the model to narrate it in prose.

An earlier version of this section also derived a problem that does not exist — that a tool-less reporting successor would need brain-side tool filtering, which limb-model excludes by default. That was an invented constraint built on the wrong model of report-back: no reporting work is asked of the successor, so nothing about its tool surface matters to this flow and no filtering capability is needed anywhere. Kept as a note because the derivation looked sound and was not; the premise ("a successor performs the report") was the error.

The dependency also runs the other way, in the direction that is easy to forget. Forked-subagents' routing prompt asks whether the parent's context is bloated, with the aside that it should not be, because we hope the parent compacts before that happens. Fork's economics assume compaction happens on time.

### Self-modification: a handover is where a pinned schema is released

Self-modification pins a session's tool schemas when its context is built and keeps them until "the next handover/compaction or the next cache break". Read from this side, that makes the handover the cheap adoption path for plugin changes: a schema-additive change — a new tool, a new optional parameter — need never force a cache break of its own, because it can wait for a handover, and the diff renders it into the changed bucket automatically.

That costs this design nothing and is worth stating for one reason: it means the briefing's changed bucket includes the *tool set*, not only prose context. It is easy to think of the diff as being about AGENTS.md and skills, and then discover that the successor has three tools the handover text never mentions.

### Limb-model: why the diff can be described at all

The diff only works because limb-contributed context is a serialisable snapshot taken at a small number of known boundaries rather than a live feed into the model's view. That is what makes it *versionable*, and something with no version cannot appear on either side of the diff. A handover is one of those boundaries. If layer composition turned out to need live evaluation — limb-model's own leading falsifier — the briefing could not describe the successor, so this design has a direct stake in that result.

There is also a class of diff this design does not have to express, and it is worth knowing why. A session is bound to exactly one limb and crossing a limb is always a fresh subagent, so a successor is always in the same limb as its predecessor. The limb's *identity* therefore never appears in the diff — though the content it contributes certainly can, and that is most of what the diff is made of.

### User-turn: what the successor keeps of the user's own work

Accumulated user activity is body content, so it falls squarely in the gone-unless-you-keep-it bucket, and the proposal that the user's own wording is preserved verbatim is specifically about that material — paraphrasing an agent's tool result loses detail, while paraphrasing the user's instruction loses intent. Nothing about how user activity is *projected* is this design's business.

One connection that looks live and is not: a compaction firing in dead time while the user is away, with text staged but unsent, cannot lose that text, because staged text is shared-live state that never entered the context in the first place. Multi-client-ui's classification is what makes that safe, and it is the whole of the relationship between this design and that one.

### The cells that are empty

Topology, oauth-credentials and cancellation-economics have nothing to say to this design. Topology because compaction is entirely a brain-side operation over data the brain already owns, with no boundary crossed. Oauth because credentials never appear in any context. Cancellation-economics because compaction's economics are cache-read economics rather than cancellation billing — the portfolio file records the same judgement from the other side.

Modular-components touches this design only through testing, which is still worth one line because the falsification evidence depends on it: the cache claims are measured from the provider's reported cached-input token counts, read at the fake provider's request record, so the wire-level assertion surface has to be observable between steps rather than only at the end.

The two lifecycle designs touch it thinly and in one direction. Layered-shutdown's rule is that an agent-level cleanup turn is not started during shutdown but recorded as outstanding; an unfinished compaction is the same shape of thing, which makes the litter problem and the retraction proposal a *resume* concern rather than a shutdown concern. Operator-lifecycle inherits that and adds nothing else.

### The two cache figures are not a conflict

`INTERACTIONS.md` records the roughly-10× read discount used here against `source-notes/compaction.md`'s "conservative assumption of 5x" as a portfolio-level conflict. They are not in conflict, because they are not about the same quantity: 10× (and the ~1.25× write multiplier alongside it) is a general provider-pricing assumption that the cache probe must verify, while the user's 5× is his conservative floor for whether the append-based *handover tool* pays for itself. §The break-even is a payback period, not a ratio labels both. The portfolio entry needs correcting rather than carrying forward — recorded under Questions for review, since that file is not this doc's to edit.

## Questions for review

- ~~Does the report-back flow build a successor at all? / Should the report-back successor have tools?~~ **Ruled 2026-08-04.** The predecessor builds the report as well as the compaction summary; the summary builds the successor's initial context and the report is delivered as the successor's first message ("I guess. something like that" — his hedge). Both questions dissolve: a successor exists but performs no reporting work, so its tool surface is not this flow's concern. Report-back-to-user and report-back-to-parent are the same flow, and continue/report-back are the only two situations. See §Two situations.
All questions below were answered by the user on 2026-08-04; his wording is preserved in the body sections each points at. Summary of the rulings:

- ~~Verbatim preservation across a handover~~ **Confirmed**: "This is my current handover instruction in my OpenCode harness, so yes." See §What the briefing has to say.
- ~~The harness voice~~ **Confirmed**: "Correct. The harness needs some kind of voice. I think OpenCode does a system reminder XML tag or something like that. That is fine. If the provider supports a channel for that, then it is perfect." One mechanism, shared with notices.
- ~~Abandoned handovers~~ **Ruled: append the retraction**, with a new recorded fact — the cache is effectively a branching structure (suffix discard is legal), and the retraction wins *because the agent's stage-2 cleanup actions are mutable*. See §Cancellation, failure, and litter.
- ~~Attachment failure~~ **Ruled: appears in the fresh context as a failure**, because compaction messages are expensive output and must not be repeated. His skepticism about the concept's likelihood recorded. See §Attachments.
- ~~Should the harness invite compaction?~~ **Superseded by a trigger taxonomy**: agent-at-milestone is one kind; context-window limit (~80-85% or a fixed 100k-200k token threshold), cache-expiry-while-idle (with the in-flight tool call rewritten as still in progress), and the user are all *forcible* triggers. See §When it fires.
- ~~Sequencing of the two proactive claims~~ **Agreed**, with two refinements: quality should be demonstrated within the handover flow experiment itself, and quality/trigger are largely orthogonal in principle — the cache-driven trigger is singled out because the user is not present when it fires, so low quality is most disruptive there.
- ~~Payload size and structure~~ **Ruled: no required structure; strong guidance.** "The handover should be as large as it needs to be while covering all necessary points" — but vague instructions produce low-quality output, so the guidance must ensure reliable coverage without forcing length or rigid structure. See §What the briefing has to say.
- ~~Cache-state prediction as its own experiment~~ **Confirmed**: "Yes, definitely. That is a great, well-scoped experiment."
- ~~The payback-period correction to why #3~~ **Accepted**: "The break-even in number three is a payback period, not a size ratio." He added that compaction, cancellation and forking economics all feel like *empirical domains*, and wants a mechanism for agent-run tuning — a scheduled/background meta-agent A/B-testing harness settings, including the handover instructions. See §The break-even is a payback period.
- ~~Pricing figures~~ **Accepted as assumptions**: "I would not want to lock them down, since timelines are uncertain and model pricing will likely change."
- ~~The INTERACTIONS cache-figures entry~~ **Confirmed** not a conflict; the file has since been corrected.
- ~~Attachments scoped outward~~ **Agreed**, with emphasis that attachments are an empirical cost optimisation: what to include and when belongs in the handover brief instructions, tuned on cost economics — the same empirical-iteration point as above.
- ~~Sealed predecessor context versus context-lifetime collection~~ **Ruled: store the context directly.** "In practice we will probably just store the context directly. That is much simpler than trying to reconstruct it deterministically from raw events. While full determinism is a nice aspiration, it feels overly ambitious and not important enough to justify the complexity." This lands mostly on persistence-analytics and is recorded there. (He also flagged the question's own jargon as unclear — "context lifetime collection", "epoch-keyed rows" are not his terms — which is a fact about how these docs are written, recorded in `README.md`'s style rule.)
- ~~A handover as the adoption point for plugin schema changes~~ **Agreed, scoped**: "Handover (or compaction) is the adoption point for certain kinds of changes, such as tool or plugin schema updates. That said, not everything requires a handover. Some things, like skills, can be updated via notifications and reloaded without compaction. So handover is the change point for some categories (notably tool schemas), but not all."
- ~~Harness voice scoped to context-updates~~ **Agreed**: "a reasonable choice."
- ~~A tool-less report-back successor would need something the limb model does not have.~~ Withdrawn 2026-08-04: an invented constraint built on the wrong model of report-back. The predecessor writes the report, so no reporting agent needs disarming and no brain-side tool filtering is needed. See §Forked-subagents in the Interactions.

## Index

| Aspect | L1 | L2 | L3 |
|---|---|---|---|
| Model framing | P | §What the briefing has to say, §Two situations: continue, or report back | §Computing the old→new diff |
| Wire & cache | F | §The two-stage flow, concretely, §Which channel the briefing arrives on | §What the harness must know to predict cache state |
| Tool surface | F | §The two-stage flow, concretely, §Attachments: prepaying the successor's first turns | §Naming stays open, deliberately |
| UX & input | | | |
| Ownership & placement | | | |
| Lifecycle | P | §Cancellation, failure, and litter in the sealed context | |
| Storage | P | §Compaction is the construction of a successor context | §The maintained document, if we build it |
| Economics | E | §When it fires | §The break-even is a payback period, not a ratio |
| Security | | | |
| Testing & verification | P | §What this makes falsifiable | |
| Code shape | P | | §Computing the old→new diff |
| Dev workflow & references | S | §What the briefing has to say | |
| Core migration | | | |
