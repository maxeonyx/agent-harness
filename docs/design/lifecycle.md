# Lifecycle

The Lifecycle aspect of the harness design, elicited for the `forked-subagents` experiment.

## Max's statements

### Why subagents are synchronous at all

> A sub-agent handoff is interesting because we have two approaches taken by different harnesses. In some harnesses, sub-agents are asynchronous: they get started and then later joined. But in my harness idea, sub-agents are synchronous. Why? Because that leads to structured concurrency.

— `source-notes/handoff-improvements.md`

> You can start up multiple sub-agents, and notably, you can start up a sub-agent with the role of orchestrating the other sub-agents. This is a common pattern and may become the default. From the parent's perspective, all sub-agents are blocking.

— `source-notes/handoff-improvements.md`

> forked subagents is the one that drives me. I want to see "agents as structural concurrency"

— Max 2026-09-08

### Blocking and scopes

> The agent hierarchy uses structured concurrency. A parent agent blocks when it launches children, and resumes only when all children have finished.

— `source-notes/agent-hierarchy.md`

> ```
> parent (suspended)
>   ├── child A (autonomous)
>   ├── child B (autonomous)
>   └── child C (user-facing) ← user interacts here
>         ... all three run in parallel ...
> parent resumes with { A: result, B: result, C: result }
> ```

— `source-notes/agent-hierarchy.md`

> A scope is created when a parent blocks. The parent does not continue until all agents in the scope complete. This is textbook structured concurrency — every task has a local owner (the parent). There is no global spawn context.

— `source-notes/agent-hierarchy.md`

> - **13. Structured concurrency purity** — resolved: intentional design, remains textbook structured concurrency.

— `source-notes/open-questions.md`

### Sibling launch and visibility

> - There is no "launching into a child scope" — you either block (creating a scope with children) or launch a sibling into the current scope.

— `source-notes/agent-hierarchy.md`

> - Agents can launch siblings into their own scope. The parent gains a new child it didn't explicitly anticipate.

— `source-notes/agent-hierarchy.md`

> - Agents can view the *status* of their siblings (they share a scope), but not their *results* — results are only available to the parent when the scope completes.

— `source-notes/agent-hierarchy.md`

> - Agents cannot view the status of their children — they are suspended until all children complete.

— `source-notes/agent-hierarchy.md`

> When the sub-agents complete, the parent agent receives their result messages from their final turn and then continues. It does not get to see their intermediate contents. This is already mentioned and implicit.

— `source-notes/handoff-improvements.md`

### Fork vs fresh — how a child session comes into being

> - **Fork by default** — children start from a copy of the parent's conversation context. Good for KV cache reuse. Same limb.

— `source-notes/agent-hierarchy.md`

> - **Fresh** — required when crossing limb boundaries. Also valid within the same limb (e.g. spinning out a meta/global session in the home dir context).

— `source-notes/agent-hierarchy.md`

> - **Different limb → always fresh.** Same limb → fork by default, fresh allowed.

— `source-notes/agent-hierarchy.md`

> - Fresh wins over forked-but-stale when the task is narrow/focused — cheaper than a long-context cache miss if the new agent ends up needing less context.

— `source-notes/agent-hierarchy.md`

> - The stale-cache case (fresh subagent wanting forked siblings, but parent + all forks are stale): open question, tentatively lean toward fresh for narrow tasks.

— `source-notes/agent-hierarchy.md`

> However, there is also a second concept: forked sub-agents, which will probably be the default method of launching sub-agents.

— `source-notes/handoff-improvements.md`

> In this case, we do not take a `context`. Instead, the sub-agents share the parent's context. The parent agent runs the `task` tool and then splits itself into many sub-agents, using its own context as the shared seed.

— `source-notes/handoff-improvements.md`

> So three paths:
>
> - Task tool (default forked?): forks parent.
> - Task tool (fresh context): shared seed context established by initial API request, subsequent forked instructions sent as follow up.
> - Compaction / handover tool: model told to produce a regular tool call. Similar structure to fresh variant of task tool: context, attachments, and task.

— `source-notes/handoff-improvements.md`

### Session ↔ limb binding

> - A session is bound to exactly one "main" limb. No multi-limb sessions. Switching context = launching a fresh subagent in the other limb. This is because a limb / context might have load bearing instructions in AGENTS.md files etc. - so to do something in that context, you MUST do it via an agent in that context.

— `source-notes/agent-harness-design.md`

> Limb model should also help to reduce this - a subagent can be given a specific limb that has a context-specific tool set. those tools do not need to be available in every session.

— `source-notes/context-updates.md`

> - Zero-tool limb (pure model session) is a valid type.

— `source-notes/agent-harness-design.md`

> - Local limb: lives as long as connected to the brain, or exits with the brain.

— `source-notes/agent-harness-design.md`

### Results, failure, and what completion means

> A result is the last message part in a turn. Prompting determines what's in it — ideally "what the parent needs to know." Failure counts as completion with an error result. Whether one child failing aborts the scope or just propagates an error to the parent needs experimenting.

— `source-notes/agent-hierarchy.md`

> - **4. What is a result** — resolved: last message part in a turn.

— `source-notes/open-questions.md`

### User-facing vs autonomous completion

> | | User-facing | Autonomous |
> |---|---|---|
> | Completion | User signals `/done`, then agent writes response | Completes automatically at end of turn |
> | Launch | Requires user permission; only user-facing agents can request this | Can be launched freely by any agent |

— `source-notes/agent-hierarchy.md`

> - **`user_facing`** — whether the session is user-facing or autonomous. Currently conflated with agent_type informally (some types are subagent-only, some user-facing-only) but kept as a separate orthogonal parameter in the model.

— `source-notes/agent-hierarchy.md`

### Launch permission

> Tentative rule: only user-facing agents can request to launch new user-facing sessions. Autonomous agents can only launch autonomous siblings. This avoids autonomous agents creating unexpected blocking dependencies on the user.

— `source-notes/agent-hierarchy.md`

> User-facing agents are already in a mode where blocking on user input is expected — a permission prompt is just another form of that.

— `source-notes/agent-hierarchy.md`

> Possible escape hatch: permission requests could expire after a timeout, so even a user-facing agent's request doesn't block indefinitely if the user is absent.

— `source-notes/agent-hierarchy.md`

> Not yet designed. Principle: strict and principled, or not at all. A half-measures permissions model is worse than none — it creates false confidence without real protection.

— `source-notes/agent-harness-design.md` (§ Permissions / safety model). The opposing "YOLO mode" position is agent-written — `REQUIREMENTS.md` L40 and `PLAN.md` L144 — and `YOLO`, `approval theatre` and `permission prompts ... unwanted` have no hit in `source-notes/`.

### The main-thread pattern

> 1. Parent forks a **user-facing child** — from the user's perspective, the conversation just continues
> 2. Parent also launches one or more **autonomous siblings** in the same scope
> 3. User-facing child runs the interactive part; autonomous siblings run in parallel
> 4. User calls `/done` when satisfied — user-facing child completes
> 5. Autonomous siblings may still be running; scope completes when **all** are done
> 6. Parent resumes with all results — it does not see intermediate content, only final responses

— `source-notes/agent-hierarchy.md`

### Multiple user-facing sessions, and entering a scope

> Supported. They appear in a session switcher view. The user can have multiple active user-facing sessions and switch between them.

— `source-notes/agent-hierarchy.md`

> The user has a button to launch a user-facing session into a scope that doesn't currently have one.

— `source-notes/agent-hierarchy.md`

> Oh - one obvious user tool is a subagent tool!! e.g "find me that nix issue where XYZ" and then the user's prompt + the subagent's response is included. Yeah, that's really great. Support both forked & fresh agents. Forked should warn if cache likely expired (note that forked can still be cheaper even if cache expired, if the model would have to do many sequential tool calls to get back up to speed. User can judge.). In this case we don't have to attach what the subagent saw - only what the user saw.

— `source-notes/user-turn.md`

### Stuck and abandoned children

> A forgotten `/done` or hung child blocks the parent scope indefinitely. Intentional that the user is responsible, but the permission prompt that precedes it is a known UX pain point. Expiry as escape hatch is noted but not decided.

— `source-notes/open-questions.md`

> - **8. Stuck scope** — intentional, user is in charge.

— `source-notes/open-questions.md`

### Resume

> Continues a previous agent session as a new subagent.

— `source-notes/agent-hierarchy.md`

> - **`id`** — ID returned by a previous Task call (on success or error)

— `source-notes/agent-hierarchy.md`

> - **`agent_type`** — open question: can a session be resumed as a different agent type? Does it break cache badly enough to matter, or should behaviour depend on whether cache is already expired?

— `source-notes/agent-hierarchy.md`

> - Resume targets — enough state to restart a session as a subagent

— `source-notes/agent-harness-design.md`

### Kill authority, cancellation, and shutdown of a scope

> it is its children for all intents and purposes, whilst it's blocked on its children

— decision 2026-08-12

> every layer should think about how it's shutting down gracefully in response to a cancellation,

— walking-skeleton ruling 2026-07-31

> I don't know what asupersync does here. We should look.

— walking-skeleton ruling 2026-07-31

> limb should own and clean up processes on graceful shutdown

— walking-skeleton ruling 2026-07-31

> we should wait for the completion of the assistant response because I don't think there's any point throwing that away. It cost us money, and it's probably good. But I don't think we should execute on the tool calls...

— walking-skeleton ruling 2026-07-31

> we should be able to resume later and then execute the tool call that we had pending.

— walking-skeleton ruling 2026-07-31

### Restart and crash behaviour of a suspended hierarchy

> Brain works out of the SQLite DB. On restart, the brain should pick up where it left off without requiring user interaction to resume threads.

— `source-notes/agent-harness-design.md`

> The brain can be running the agent loop for many sessions at once - I think if the brain relaunches it should simply continue these. I wonder if we'd wait for all API requests to complete first before closing. Yes - that makes good sense for graceful shutdown - wait for all API requests to complete, remember for later that we're about to run tool calls or whatever, then we re-launching we run those tool calls etc. and continue where we left off. Perhaps if it's been more than an hour, then (if interactive) we ask the user whether we should continue other agents. If it's the brain in server mode, it should definitely just continue. Actually - I don't think that's so clear. Probably this should be optional too. The brain relaunching within an hour can continue but beyond an hour, first client would have to decide whether or not to resume other agents.

— `source-notes/tech.md`

### Compaction inside a scope

> lets the first agent build the report as well as their compaction summary. And then the compaction summary deals with the initial context of the new agent, and the report is given as if it was its first message. I guess. something like that.

— decision 2026-08-12 (hedge his)

> I think the forked agent design gets around this by subbing the tail with a (admittedly output so 5.0x) task summary / report (kinda a compaction).

— decision 2026-08-12

> something like 80-85% (or better, a fixed token threshold like 100k-200k tokens)

— decision 2026-08-12 (the harness's forcible context-window trigger)

> rewritten as still in progress, and execution continues seamlessly when it completes

— decision 2026-08-12 (what happens to an in-flight tool call when the harness forcibly compacts on cache expiry while the agent is idle). The surrounding "four trigger kinds, three of them forcible" framing in `DECISIONS.md` is the recorder's prose, not his wording; only these two fragments are quoted from him.

### Topology of the lifecycle

> it shouldn't just be one node - first of all, it's one reader per limb, one notifier per session / context, etc.

— decision 2026-08-12

> Two agent sessions are usually independent, but when one reads another then we can naturally represent the dependence as at that point so that all actors can make sure they have all state they need before continuing.

— `source-notes/tech.md`

### Tension: can a parent see its children's status?

> - Agents cannot view the status of their children — they are suspended until all children complete.

— `source-notes/agent-hierarchy.md`

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

Both are Max. The second lists `sibling/child agent status changes` as a piggyback-able context event; the first says a parent cannot see child status. Not resolved in the notes; neither file is dated later than the other (both imported 2026-07-30).

### Tension: strict block-on-all vs siblings awaiting each other

> From the parent's perspective, all sub-agents are blocking.

— `source-notes/handoff-improvements.md`

> - There is no "launching into a child scope" — you either block (creating a scope with children) or launch a sibling into the current scope.

— `source-notes/agent-hierarchy.md`

> - Await on all subagents, or have them await each other before starting, by attaching their outputs to each others' tasks.

— `source-notes/handoff-improvements.md`

> Well, that's complicated but we can certainly consider it.

— `source-notes/handoff-improvements.md` (his hedge on the whole "task calls be actual code" block that the await-each-other line belongs to)

### Tension: stuck scope is intentional vs expiry escape hatches

> - **8. Stuck scope** — intentional, user is in charge.

— `source-notes/open-questions.md`

> Possible escape hatch: permission requests could expire after a timeout, so even a user-facing agent's request doesn't block indefinitely if the user is absent.

— `source-notes/agent-hierarchy.md`

> Expiry as escape hatch is noted but not decided.

— `source-notes/open-questions.md`

### Superseded wording that appears in Lifecycle-relevant statements

> Of course, we always rebuild on fresh context eg. handover/compaction, or fresh agent. but, crucially, *forked* subagents are trying to be *append mode* subagents (possibly w.r.t the parent *as of the message before it sent the subagent tool call* - but that needs experimenting too.)

— `source-notes/context-and-agent-loop.md`. **Later:** decision 2026-08-12 vetoes the word — `"rebuild is not 'build a new context'!! rebuild is 'transform an existing context'"` — and replaces it with three operations: `"this refers to the system prompt only - and happens on new sessions, on compactions, and yes, on refurbishments"` (initialise), `"transform existing to reduce token count, but NOT compact - the messy one"` (refurbish), and compact. The fork claim itself is not superseded, only the verb.

## Answers

### L1 — child failure: abort scope vs error result

Question: **Does one child failing abort the whole scope, or does it just propagate an error result to the parent?** Max: `"Failure counts as completion with an error result. Whether one child failing aborts the scope or just propagates an error to the parent needs experimenting."` (`source-notes/agent-hierarchy.md`). Toward error-return: `"Failure counts as completion"`, and the parent resumes with `{ A: result, B: result, C: result }`. Toward abort: nothing in the notes. `PLAN.md` records it as `(open experiment: abort-scope vs error-return semantics)`.

Max, 2026-09-08:

> Sorry, to be clear, this is about the structured concurrency model in the agent harness. I honestly don't have strong opinions about the failure behavior there, but I think we should distinguish in-band and out-of-band failures. If it's like a failure to read a file, that would somehow be fatal, that the agent couldn't complete its task. But we've still got API requests proceeding, and you could conceivably have the loop recover with some workaround. Then I don't think that's really a failure per se. The child would say, Oh, I'm blocked. It would be completed, and the parent could resume it, right? But if it's an out-of-band failure, for example, the credentials for the API are no longer valid, and the model, the harness can't access the model anymore. The harness can't progress. That can't count as task completion, realistically, because the parent is also blocked, and its parent is also blocked in exactly the same way. The harness is not working. So I don't think that makes sense to call that completion. So what I'm saying is this just needs thinking. A rule like you've written is not a universal thing.

### L2 — stale forks: siblings go fresh?

Question: **When a fresh subagent wants forked siblings but the parent and all forks are stale, do the siblings go fresh?** Max: `"open question, tentatively lean toward fresh for narrow tasks"` (`source-notes/agent-hierarchy.md`). Toward fresh: `"Fresh wins over forked-but-stale when the task is narrow/focused — cheaper than a long-context cache miss if the new agent ends up needing less context."` Toward fork: `"forked can still be cheaper even if cache expired, if the model would have to do many sequential tool calls to get back up to speed. User can judge."` (`source-notes/user-turn.md`) — that second one is written about the *user*-launched case; whether an agent gets the same judgement call is not stated.

Max, 2026-09-08:

> This question is confused. If a fresh sub-agent has started up, its prefix is now cached. If it has successfully... if there's an API request that has completed for it to then have written out a tool call that would start up some sub-agents, then there is a cache, and there is something to fork. This doesn't really make sense.

Max, 2026-09-08:

> This is about something different - forked subagents have lots of context, but use more cache read on every turn (message, tool call). Fresh subagents have less context but every turn is cheaper.

### L3 — resume as a different agent type

Question: **Can a session be resumed as a different agent type?** Max, verbatim as a question: `"open question: can a session be resumed as a different agent type? Does it break cache badly enough to matter, or should behaviour depend on whether cache is already expired?"` (`source-notes/agent-hierarchy.md`). The notes contain both sub-options and pick neither.

Max, 2026-09-08:

> Different agent = different system prompt = different context. Requires compaction. However, some APIs may allow appended system contributions.

### L4 — launch rule (only user-facing agents launch user-facing sessions)

Question: **Is the tentative launch rule adopted — only user-facing agents may request user-facing sessions?** Max: `"Tentative rule: only user-facing agents can request to launch new user-facing sessions. Autonomous agents can only launch autonomous siblings."` Reason given: `"This avoids autonomous agents creating unexpected blocking dependencies on the user."` Against adopting: nothing in the notes. An agent-written claim that permission prompts are "explicitly unwanted" has no Max source, and this is the one permission the model actually needs.

Max, 2026-09-08:

> Full rule is: Agent cannot create an obligation for a human. Only humans can create obligations for other humans. A session that needs a reply, a ticket on an issue tracker. The agent has one pathway for this, which is to ask the user. It can ask the user if the user would like to fork a new user-facing session. The user must confirm.

### L5 — permission request expiry

Question: **Do permission requests expire on a timeout?** Max: `"Possible escape hatch: permission requests could expire after a timeout"`. In tension with `"- **8. Stuck scope** — intentional, user is in charge."` and `"Expiry as escape hatch is noted but not decided."` (both `source-notes/open-questions.md`).

Max, 2026-09-08:

> Undecided on the necessity of permission prompts but I guess I just described one (user must confirm). I'm all for *non-blocking* permission prompts, though! Agent can build up a user action queue, essentially - non-blocking questions and permission requests. They expire and the agent can either dismiss or leave them queued.
>
> However I want all such actions to have a strict schema that the agent must follow - all requests of the user & reports back should be "self contained" - they must reiterate the chain from what the user ultimately wants, to why the requests exists, and also include relevant inrformation that the user might need to make the decision.

### L6 — sibling dependencies vs parent-blocks-on-all

Question: **Can siblings depend on each other — await each other before starting — or is the only ordering primitive "parent blocks on all"?** Toward all-block: `"From the parent's perspective, all sub-agents are blocking."` and `"There is no 'launching into a child scope'"`. Toward a dependency graph: `"Await on all subagents, or have them await each other before starting, by attaching their outputs to each others' tasks."` His own hedge on that block: `"Well, that's complicated but we can certainly consider it."`

Max, 2026-09-08:

> Sorry, I think you misunderstand slightly. So first of all, the parent calls sub-agents as a task, like as a tool call, and sees the result of the entire thing as a tool result. That's the first thing. And the tool result is in the form of the same sort of object as we use for compactions and task handoffs. It's a handoff back from the child agents to the parent. It sees all of the, like, back handoffs. Now, the parent can and will create. A common case would be like to say, fire off three worker sub-agents, an orchestrator sub-agent, which is also the user-facing sub-agent. Right? That continues the user's interaction while these worker sub-agents are going. The point being that when we finally are done with the sub-agents, the result will be returned to the parent, and the parent will have the high-level context still going. All of the, you know, sending messages to and fro, tracking stuff, blah blah blah, while the sub-agents were working. That will be wrapped up into the result from the three workers and the user-facing orchestrator sub-agent. So there's four total results to the parent, and the parent will see that as the result of a tool call. Right? So, but however, the parent can set up dependencies on the sub-agents between each other. So the parent could say, launch A, B, and C, but B only starts once A finishes. C only starts once B finishes. And the user-facing orchestrator sub-agent, which may or may not exist. It's like an orchestrator sub-agent is useful headless as well sometimes, but doesn't have to exist headless. The parent can set up that sort of structure, but the question is, can we change those structures after they have been set up? I think the answer is yes. So, for example, let's say we have a, you know, we have three agents, separate, independent: A, B, and C. They don't depend on each other. But then A encounters some kind of sub-task, right? Sub-task is like fix credential management. It's like credentials aren't working, and it's likely to be encountered by all of A, B, and C, right? A can spin out a sub-agent of its own to deal with that, but it should probably say it's probably blocking everyone. So it creates a sub-agent and then makes all the others dependent on it. They now block, waiting for that new sub-agent, you know, A prime. So now B and C are dependent on A prime by A's choice, right? A prime will complete. The credentials will be unblocked, and they'll all continue on independently now. That's the question: can we attach dependency, a blocking dependency, to another agent? I think the answer, obviously we have to have reference to that agent, but I think the answer is like yes, maybe. It kind of makes sense to do that sometimes. Maybe A can't do it, but A can ask the orchestrator to do it. I don't know. That's what I'm talking about, is the uncertainty here. I'm not making a decision on this now. I haven't thought about it enough.

### L7 — can a parent observe child status

Question: **Can a parent observe child status at all, or only sibling status?** `"Agents cannot view the status of their children"` versus the piggyback list's `"sibling/child agent status changes"`. If the answer is "only the *user* sees child status, not the parent agent", the notes do not say so.

Max, 2026-09-08:

> As before, parent sees one tool call only

## Open questions

1. **Does one child failing abort the whole scope, or does it just propagate an error result to the parent?** Max: `"Failure counts as completion with an error result. Whether one child failing aborts the scope or just propagates an error to the parent needs experimenting."` (`source-notes/agent-hierarchy.md`). Toward error-return: `"Failure counts as completion"`, and the parent resumes with `{ A: result, B: result, C: result }`. Toward abort: nothing in the notes. `PLAN.md` records it as `(open experiment: abort-scope vs error-return semantics)`.

Max, 2026-09-08:

> Sorry, to be clear, this is about the structured concurrency model in the agent harness. I honestly don't have strong opinions about the failure behavior there, but I think we should distinguish in-band and out-of-band failures. If it's like a failure to read a file, that would somehow be fatal, that the agent couldn't complete its task. But we've still got API requests proceeding, and you could conceivably have the loop recover with some workaround. Then I don't think that's really a failure per se. The child would say, Oh, I'm blocked. It would be completed, and the parent could resume it, right? But if it's an out-of-band failure, for example, the credentials for the API are no longer valid, and the model, the harness can't access the model anymore. The harness can't progress. That can't count as task completion, realistically, because the parent is also blocked, and its parent is also blocked in exactly the same way. The harness is not working. So I don't think that makes sense to call that completion. So what I'm saying is this just needs thinking. A rule like you've written is not a universal thing.

6. **Can siblings depend on each other — await each other before starting — or is the only ordering primitive "parent blocks on all"?** Toward all-block: `"From the parent's perspective, all sub-agents are blocking."` and `"There is no 'launching into a child scope'"`. Toward a dependency graph: `"Await on all subagents, or have them await each other before starting, by attaching their outputs to each others' tasks."` His own hedge on that block: `"Well, that's complicated but we can certainly consider it."`

Max, 2026-09-08:

> Sorry, I think you misunderstand slightly. So first of all, the parent calls sub-agents as a task, like as a tool call, and sees the result of the entire thing as a tool result. That's the first thing. And the tool result is in the form of the same sort of object as we use for compactions and task handoffs. It's a handoff back from the child agents to the parent. It sees all of the, like, back handoffs. Now, the parent can and will create. A common case would be like to say, fire off three worker sub-agents, an orchestrator sub-agent, which is also the user-facing sub-agent. Right? That continues the user's interaction while these worker sub-agents are going. The point being that when we finally are done with the sub-agents, the result will be returned to the parent, and the parent will have the high-level context still going. All of the, you know, sending messages to and fro, tracking stuff, blah blah blah, while the sub-agents were working. That will be wrapped up into the result from the three workers and the user-facing orchestrator sub-agent. So there's four total results to the parent, and the parent will see that as the result of a tool call. Right? So, but however, the parent can set up dependencies on the sub-agents between each other. So the parent could say, launch A, B, and C, but B only starts once A finishes. C only starts once B finishes. And the user-facing orchestrator sub-agent, which may or may not exist. It's like an orchestrator sub-agent is useful headless as well sometimes, but doesn't have to exist headless. The parent can set up that sort of structure, but the question is, can we change those structures after they have been set up? I think the answer is yes. So, for example, let's say we have a, you know, we have three agents, separate, independent: A, B, and C. They don't depend on each other. But then A encounters some kind of sub-task, right? Sub-task is like fix credential management. It's like credentials aren't working, and it's likely to be encountered by all of A, B, and C, right? A can spin out a sub-agent of its own to deal with that, but it should probably say it's probably blocking everyone. So it creates a sub-agent and then makes all the others dependent on it. They now block, waiting for that new sub-agent, you know, A prime. So now B and C are dependent on A prime by A's choice, right? A prime will complete. The credentials will be unblocked, and they'll all continue on independently now. That's the question: can we attach dependency, a blocking dependency, to another agent? I think the answer, obviously we have to have reference to that agent, but I think the answer is like yes, maybe. It kind of makes sense to do that sometimes. Maybe A can't do it, but A can ask the orchestrator to do it. I don't know. That's what I'm talking about, is the uncertainty here. I'm not making a decision on this now. I haven't thought about it enough.

8. **When the parent is cancelled or killed while blocked, what outcome does each child record?** Available material: kill authority is `"it is its children for all intents and purposes, whilst it's blocked on its children"`; the shutdown pattern is `"every layer should think about how it's shutting down gracefully in response to a cancellation"`; and the walking skeleton established four-valued outcomes with `"we should wait for the completion of the assistant response... It cost us money, and it's probably good"`. Whether a killed child's partial work is kept by the same rule is not stated.

9. **Does a child compact mid-scope, and if so does the parent ever see it?** Max: `"lets the first agent build the report as well as their compaction summary. And then the compaction summary deals with the initial context of the new agent, and the report is given as if it was its first message. I guess. something like that."` (hedge his). The unstated part is whether the *child's* handover is invisible to the parent, given `"It does not get to see their intermediate contents."`

10. **Does the harness's forcible cache-expiry compaction apply to a suspended parent?** A parent blocked on children is idle by construction, so its warm context decays for the whole scope. Max's forcible triggers include cache expiry while the agent is idle, with the in-flight tool call `"rewritten as still in progress, and execution continues seamlessly when it completes"` — the `task` call *is* an in-flight tool call, so this may already cover it, or may not have been written with a suspended parent in mind.

11. **Do N forked siblings sharing a limb get one limb or N?** Toward one: a limb is identified by `ssh_host` and `directory`, and `"Local limb: lives as long as connected to the brain"`. Toward per-session: `"it's one reader per limb, one notifier per session / context, etc."` — which distinguishes per-limb from per-session components without saying which side a limb process falls on.

12. **When the user launches a subagent from the user-turn subagent tool, who is the parent and what scope is it in?** Max: `"one obvious user tool is a subagent tool!! ... Support both forked & fresh agents."` and separately `"The user has a button to launch a user-facing session into a scope that doesn't currently have one."` The notes give the user two distinct ways to create agents and do not say whether they are the same mechanism.

13. **Is a resumed session a child in a fresh scope, or a continuation that rejoins its old one?** `"Continues a previous agent session as a new subagent"` says new-subagent; `"Resume targets — enough state to restart a session as a subagent"` says the same; neither says what happens to the original scope the session belonged to.

14. **On brain restart with a scope suspended mid-flight, is the whole hierarchy resumed or only the top?** Max: `"if the brain relaunches it should simply continue these"`, then immediately `"Perhaps if it's been more than an hour, then (if interactive) we ask the user whether we should continue other agents."` then `"Actually - I don't think that's so clear. Probably this should be optional too."` Three positions in one paragraph, all his.
