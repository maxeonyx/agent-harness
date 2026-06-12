# Task handoff thoughts

Basically, I have an idea for the task handoff flow from the model's perspective in the harness.

There are two kinds of handoffs that a harness naturally supports:

1. A continuation handoff, or a compaction, which I call a "handover" because it needs to contain enough information to get a fresh agent started.
2. A sub-agent handoff.

A sub-agent handoff is interesting because we have two approaches taken by different harnesses. In some harnesses, sub-agents are asynchronous: they get started and then later joined. But in my harness idea, sub-agents are synchronous. Why? Because that leads to structured concurrency.

You can start up multiple sub-agents, and notably, you can start up a sub-agent with the role of orchestrating the other sub-agents. This is a common pattern and may become the default. From the parent's perspective, all sub-agents are blocking.

This is all background, which is already mentioned in the spec.

Now, there are two main ideas:

First idea: the `task` tool.

The `task` tool takes a `context`, which is generic across all started sub-agents. If there are multiple parallel sub-agents, they all receive the same context prompt.

Second: attachments, or context attachments.

The agent using the `task` tool can attach tool calls. eg.: file reads, skills, searches, maybe command output. These "attachments" (??) are immediately sent to the sub-agent upon startup, appearing as normal tool calls, but crucially are all executed in one "init" step and are shared across all parallel sub-agents.

When launching multiple sub-agents, we start one with only the shared context and tell it to wait for further instructions. This first API request establishes a shared cache. We then send different instructions to different sub-agent branches so that they all share a cache prefix. This is a cost-saving measure.

However, there is also a second concept: forked sub-agents, which will probably be the default method of launching sub-agents.

In this case, we do not take a `context`. Instead, the sub-agents share the parent's context. The parent agent runs the `task` tool and then splits itself into many sub-agents, using its own context as the shared seed.

When the sub-agents complete, the parent agent receives their result messages from their final turn and then continues. It does not get to see their intermediate contents. This is already mentioned and implicit.

Finally, for the `handover` tool, we also support attachment arguments.

Rather than the parent agent needing to use output tokens to re-summarize everything, it can attach files. These files are read directly. Attachments reduce the number of turns taken and are especially useful for parallel sub-agents. If a file is referenced by name in the task, all parallel sub-agents would otherwise need to read it individually. By attaching them directly, we can reduce the number of turns significantly.

We can also preload files such as `agents.md` using this mechanism.

So three paths:

- Task tool (default forked?): forks parent.
- Task tool (fresh context): shared seed context established by initial API request, subsequent forked instructions sent as follow up.
- Compaction / handover tool: model told to produce a regular tool call. Similar structure to fresh variant of task tool: context, attachments, and task.

We need to work out the exact framing. I think that "handover" is a bit of an "othering" term so compaction tool might be better, but needs to be essentially the same.

Consider "two part launch":

- Agent calls tool
- Agent recieves a user message (not tool call result) with instructions how to correctly use the tool
- Agent calls tool again to confirm.

For handover I think this is worth it because handover is important. For subagents, probably not. However we could still try it, as it might give us a good cache point for diverging the parent agent into forked subagents.

As an aside, we need to *very* correctly use OpenAI responses API & Anthropic messages API w.r.t. caching for this all to work.

Another note:

- The task tool prompt should make the parent act well as a "router" for deciding whether subagents should be "forked" or "fresh" - do they need the same context as the parent? If the task is a natural continuation, yes. Is the parent's context bloated (it shouldn't be, because we hope the parent does compaction before that happens, but), so fresh would be cheaper anyway? Do they need only *some* of the context, or do the subagents need to run in a different limb? If so, fresh.
- I have a feeling making our task calls be actual code is probably simpler:
- Create at most one shared "seed context" per limb, containing context and attachments.
- Create subagents in a limb by attaching a task to that context.
- Your own context is always available, to make "forked" agents.
- Await on all subagents, or have them await each other before starting, by attaching their outputs to each others' tasks.
- Finally return a response, typically just the array of all subagent results.

Well, that's complicated but we can certainly consider it.