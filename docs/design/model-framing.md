# Model framing

The Model framing aspect of the harness design, elicited for the `forked-subagents` experiment.

## Max's statements

### The turn-ending problem — the stated existential risk

> Also - highly forked agent threads are not empirically tested and will likely need careful model-facing framing. As to the model, it appears it is given task A, then told to only focus on A.1, then told to only focus on A.1.3. But importantly, it must not continue on to complete A.1.4 or the full A.1. It must end its turn after A.1.3 - this must be reliable for forked agents to work well.

— `source-notes/agent-hierarchy.md` (in the `> Status: **incomplete WIP**` banner at the top of the file)

### Agent types

> Loosely maps to OpenCode-style agent personas. Currently informal — some types are subagent-only, some user-facing-only, but this is not a hard constraint in the model. Kept orthogonal from `user_facing`.

— `source-notes/agent-hierarchy.md`

> Not yet fully specified.

— `source-notes/agent-hierarchy.md` (§ Agent types, the whole body)

> - **`agent_type`** — which agent persona to use (see Agent Types below)

— `source-notes/agent-hierarchy.md`

### What the result is, and how prompting shapes it

> A result is the last message part in a turn. Prompting determines what's in it — ideally "what the parent needs to know."

— `source-notes/agent-hierarchy.md`

> - Finally return a response, typically just the array of all subagent results.

— `source-notes/handoff-improvements.md`

### Framing the parent as a fork/fresh router

> - The task tool prompt should make the parent act well as a "router" for deciding whether subagents should be "forked" or "fresh" - do they need the same context as the parent? If the task is a natural continuation, yes. Is the parent's context bloated (it shouldn't be, because we hope the parent does compaction before that happens, but), so fresh would be cheaper anyway? Do they need only *some* of the context, or do the subagents need to run in a different limb? If so, fresh.

— `source-notes/handoff-improvements.md`

### The shared-context prompt and the seed agent

> The `task` tool takes a `context`, which is generic across all started sub-agents. If there are multiple parallel sub-agents, they all receive the same context prompt.

— `source-notes/handoff-improvements.md`

> When launching multiple sub-agents, we start one with only the shared context and tell it to wait for further instructions. This first API request establishes a shared cache. We then send different instructions to different sub-agent branches so that they all share a cache prefix. This is a cost-saving measure.

— `source-notes/handoff-improvements.md`

### Attachments, and how they appear to the child

> The agent using the `task` tool can attach tool calls. eg.: file reads, skills, searches, maybe command output. These "attachments" (??) are immediately sent to the sub-agent upon startup, appearing as normal tool calls, but crucially are all executed in one "init" step and are shared across all parallel sub-agents.

— `source-notes/handoff-improvements.md`

> Rather than the parent agent needing to use output tokens to re-summarize everything, it can attach files. These files are read directly. Attachments reduce the number of turns taken and are especially useful for parallel sub-agents. If a file is referenced by name in the task, all parallel sub-agents would otherwise need to read it individually. By attaching them directly, we can reduce the number of turns significantly.

— `source-notes/handoff-improvements.md`

> We can also preload files such as `agents.md` using this mechanism.

— `source-notes/handoff-improvements.md`

### Two-part launch

> Consider "two part launch":
>
> - Agent calls tool
> - Agent recieves a user message (not tool call result) with instructions how to correctly use the tool
> - Agent calls tool again to confirm.

— `source-notes/handoff-improvements.md`

> For handover I think this is worth it because handover is important. For subagents, probably not. However we could still try it, as it might give us a good cache point for diverging the parent agent into forked subagents.

— `source-notes/handoff-improvements.md`

> **NOTE**: done in opencode fork - handover tool works well. just recently done: "two stage" handover - two tools (handover and handover_complete) - the agent calls the first one, which then injects a user message with all the required info for the agent to perform a good hand over. it also allows & encourages the agent to call other tools to tidy things up prior to completion of the handover.

— `source-notes/compaction.md`

### Naming as framing

> Calling compaction a **handover** (rather than "summarise" or "compact") produces better results. The word naturally implies that all relevant information must be passed forward — goals, progress, decisions, blockers, next steps.

— `source-notes/compaction.md`

> Current approach in OpenCode: a `/handover` command backed by a prompt (`handover.md`) that explicitly instructs the model to:
> - Pay extra attention to any handover notes at the start of the conversation
> - Carry all relevant context forward
> - Maintain the goal and current place in the overall work
>
> This works well in practice.

— `source-notes/compaction.md`

### Option sets — subagent names are model-visible and change

> - Avaialble agent types for subagent tool, available limbs for subagent tool, other tool option sets etc.

— `source-notes/context-updates.md` (listed under things the agent should be notified changed)

> they're still in the context just not in that spot, and they obviously still change - that's the whole _point_ of not putting them in the schema proper - and so yes they absolutely get notices!

— decision 2026-08-12

> skill names are options for the skill tool! subagent names are options for the task tool!

— decision 2026-08-12

### Stale subagent descriptions may force a rebuild rather than a notice

> An old context will contain old info. where that's important for correctness, we need notices or rebuild [refurbish/compact]. because we're relatively sure that _tool schemas_ / _tool presence_ can't be fixed via notices..., and because we don't want to keep around old tool code versions forever, this might mean we have to force rebuild _if the context contains tool description content that is stale in a correctness-affecting way_. Importantly, the same logic would apply to any _other_ things, perhaps for example _subagent_ description content?

— decision 2026-08-12 (hedges his). The bracketed glosses are the recorder's, inserted under the `"rebuild"` veto of the same date.

### Voice, and what the harness may claim

> not sure about this, but yes... harness voice is for ground truth - the time, but also 'the AGENTS.md contains this content', but the _content_ while it is being shown by the harness, is not in the voice of the harness.... tbh I think this is all relatively obvious to the agent. but the channels or roles do matter, but also I just want the agent to do what I want. I don't have an authority model across multiple users or whatever.

— decision 2026-08-12 (hedges his)

> I think we do want to tell the model which model it is.

— decision 2026-08-12

### Tool-set framing that a fork inherits wholesale

> The user tools are NOT the same as the agent tools, and we should make sure that the context is clear to the agent that it has a different tool set to the user (we don't want the agent trying to use the user's tools).

— `source-notes/user-turn.md`

> Just as the user has a view of the agent's tool calls which is clear to the user "this is what the agent is/was doing" so does the agent now have the reverse.

— `source-notes/user-turn.md`

> The point is that the user and the agent should be on the same page about the history, not that they should see exactly the same stuff.

— `source-notes/user-turn.md`

### Lead by example in injected context

> I prefer markdown not to be wrapped - because almost all viewers deal with this gracefully, and it reduces burden on editors.
>
> Thus, the context and system prompts, skills etc injected by the harness should follow this and lead by example

— `source-notes/markdown-nowrap-lead-by-example.md`

### Brain-native tools the child also sees

> The brain exposes tools to agents that are not limb-provided, e.g.:
>
> - View session history
> - View status of sibling agents
> - Task (launch a subagent)
> - Resume (continue a previous session as a subagent)
>
> These are brain-native, always available regardless of limb.

— `source-notes/agent-hierarchy.md`

### Progressive disclosure as the framing budget

> Not all innformation can or should be made available to the agent at the get go. this is a careful balance between always up-front input cost and conditional repeated cached-input cost from tool calling to get more info.

— `source-notes/context-updates.md`

> In real world cases, skill and tool descs can otherwise take up massive context paid on *every* session.

— `source-notes/context-updates.md`

### The pre-step idea for writing the child's prompt

> A temp fork whose only job is to write a good task prompt, because what belongs in the prompt depends on the target agent type/context. Tradeoff: adds latency and cost to every Task call.

— `source-notes/agent-hierarchy.md` (headed `### Optional pre-step (idea, not decided)`)

### Tension: "handover" is the good word / "handover" is an othering word

> Calling compaction a **handover** (rather than "summarise" or "compact") produces better results.

— `source-notes/compaction.md`

> We need to work out the exact framing. I think that "handover" is a bit of an "othering" term so compaction tool might be better, but needs to be essentially the same.

— `source-notes/handoff-improvements.md`

Both Max, both imported 2026-07-30, no ordering recoverable from the files. He explicitly flags the second as unfinished: `"We need to work out the exact framing."`

### Tension: attachments appear as the agent's own tool calls / harness content keeps its own voice

> These "attachments" (??) are immediately sent to the sub-agent upon startup, appearing as normal tool calls

— `source-notes/handoff-improvements.md`

> harness voice is for ground truth - the time, but also 'the AGENTS.md contains this content', but the _content_ while it is being shown by the harness, is not in the voice of the harness

— decision 2026-08-12 (hedges his). Later than the source note. An attachment is content the harness injects, framed as a tool call the child never made.

## Open questions

1. **How is the subtask boundary expressed to the child so that it stops at A.1.3 and does not run on to A.1.4?** Max states the requirement — `"It must end its turn after A.1.3 - this must be reliable for forked agents to work well"` — and immediately says it is untested: `"highly forked agent threads are not empirically tested and will likely need careful model-facing framing."` Mechanisms visible in the notes: the `task` parameter (`"the prompt / instructions for the subagent"`); the shared `context` parameter (`"generic across all started sub-agents"`); the agent persona (`agent_type`); the two-part launch; and the pre-step temp fork (`"whose only job is to write a good task prompt"`). The notes name all five and choose none.

2. **Is two-part launch used for `task`, or only for handover?** Max: `"For handover I think this is worth it because handover is important. For subagents, probably not."` — then, in the same breath: `"However we could still try it, as it might give us a good cache point for diverging the parent agent into forked subagents."` So the cost argument says no and the cache argument says maybe.

3. **Are agent types a closed set, and is the subagent-only / user-facing-only split enforced?** Max: `"Currently informal — some types are subagent-only, some user-facing-only, but this is not a hard constraint in the model. Kept orthogonal from `user_facing`."` and `"Not yet fully specified."` Toward enforcement: the Lifecycle rule that autonomous agents may only launch autonomous siblings needs *something* to be checkable. Toward informality: `"not a hard constraint in the model"`.

4. **Does the forked child know it was forked?** The notes do not say. Toward transparent: `"the sub-agents share the parent's context"`, and append-mode w.r.t. the parent means the child's context is literally the parent's bytes, so any "you are a child" statement is an appended message, not a system-prompt fact. Toward explicit: `"there's a practical constraint on actually knowing whether free-text content update X affects session Y or not"` — nothing here — and the fact that the A.1.3 stop rule has to be conveyed somehow (Q1).

5. **What does the child see at the tail of its inherited context — the parent's `task` tool call, or the message before it?** Max: `"possibly w.r.t the parent *as of the message before it sent the subagent tool call* - but that needs experimenting too."` (`source-notes/context-and-agent-loop.md`). The two options are exactly the two he names, and he marks it experimental.

6. **Is fork-vs-fresh chosen by the model or by the harness?** Toward model: `"The task tool prompt should make the parent act well as a 'router' for deciding whether subagents should be 'forked' or 'fresh'"`, and the `context` parameter is a Task argument the model fills in. Toward harness: `"**Different limb → always fresh.**"` is a rule the harness can enforce without asking. The notes do not say whether the harness may override the model's choice.

7. **Does a child see sibling status, and in whose voice does it arrive?** Sibling status is a brain-native tool (`"View status of sibling agents"`), a piggyback-able context event (`"sibling/child agent status changes"`), and subject to the voice rule (`"harness voice is for ground truth"`). Whether it is pulled by tool call or pushed as an appended notice is not stated, and those have opposite economics under his own reference-vs-content framing.

8. **What does the parent literally see when a child fails?** Max: `"Failure counts as completion with an error result"` and `"**`id`** — ID returned by a previous Task call (on success or error)"`. Whether the error result is the child's own last message part, a harness-authored error, or both is not stated — and the voice rule bears on it.

9. **Is `task` the name the model sees, given the handover naming finding?** He established that word choice changes model behaviour (`"Calling compaction a **handover** ... produces better results"`) and flagged the naming as unfinished (`"We need to work out the exact framing."`), but never applied that lens to `task` / `Task` / subagent / child.

10. **Do subagent descriptions get a notice when they change, or force a refurbishment?** Max, hedged: `"the same logic would apply to any _other_ things, perhaps for example _subagent_ description content?"` Toward notice: `"subagent names are options for the task tool!"` and `"they absolutely get notices!"` — that is the option *set*. Toward forced refurbishment: the description text sits in the tool schema region, and `"changed tool schema never gets a notice, as discussed"`. So names and descriptions may fall on opposite sides.

11. **Does the child inherit the user-tools-are-not-your-tools framing, and is it still true for it?** Max: `"we should make sure that the context is clear to the agent that it has a different tool set to the user (we don't want the agent trying to use the user's tools)"`. An autonomous forked child inherits that text plus a transcript of the user's activity, but has no user attached to it.

12. **Does the pre-step temp fork happen?** Max marks the whole section `### Optional pre-step (idea, not decided)` and prices it himself: `"Tradeoff: adds latency and cost to every Task call."`
