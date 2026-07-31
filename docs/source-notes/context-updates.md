# Context changes & progressive disclosure

## context changes

If the agent loads a skill, but it gets changed by the user or another agent or by git, it'd be nice if the agent could somehow know.

If it's in the system prompt, it's there with a desc only. We ignore desc changes until the context is rebuilt.

But if the agent has loaded the skill, it would be nice for it to get notified that it's changed since last time it was loaded. This goes for other stuff too:

- Tool availability (new tools, missing tools)
- Skill content
- New skills
- The time (special rules around this one)

We don't include the new content eagerly. We only provide the *bare minimum* for the agent to efficiently invalidate its current understanding - to know that viewing the new content is an option, or to explain a missing tool, etc.

re. the time:

- This is important to have an agent understand how long between its response and the user message. It can be many weeks in some cases! Probably more than 1h is a good point to start injecting this, less no point.

Other changes:

- Skills - the briefest possible mention of "these skills have changes"
- Tools - added / missing tools get similar notification. Tools with changed schema need full content injection.
- Other context eg. AGENTS.md files, global / machine / user context.
- Avaialble agent types for subagent tool, available limbs for subagent tool, other tool option sets etc.

Some changes we *don't* allow, I think:

- Changed limb (notably - this changes the limb-specific context hierarchy. this is load bearing and shan't change without a compaction / context re-build.)
- Maybe likewise for changed working directory and/or hostname etc.
- Changed model? Unclear.

Changing "agent" (ie. role / mandate part of system prompt)

- not sure if this is allowed or not - I tend to think not without a compaction, as model can't be expected to respect role changes that occur later in the context.

## progressive disclosure

Not all innformation can or should be made available to the agent at the get go. this is a careful balance between always up-front input cost and conditional repeated cached-input cost from tool calling to get more info.

Some skills can be gated behind other, strictly more broadly applicable skills being loaded first. Skill desc need to say when to load.

We should have an info architecture skill and a skill writing workflow that helps motivate & get this correct.

previously implemented in my opencode fork.

something similar can & should be done for tools.

In real world cases, skill and tool descs can otherwise take up massive context paid on *every* session.

Limb model should also help to reduce this - a subagent can be given a specific limb that has a context-specific tool set. those tools do not need to be available in 
