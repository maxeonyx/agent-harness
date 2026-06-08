# context and agent loop

**Context append does not always mean API request**

User turn end triggers API request and starts agent loop.

Agent loop continues as tool calls trigger next API request(s).

However- user activity should *not* trigger API requests.

If there is one already happening, then that activity should *be sent with the next API request* - a user message alongside the tool result or user message. However, many types of "context additions" should not themselves trigger a request, for cost reasons.

Something that *would* trigger a request is our idea of "cache-expiry-driven handover".

> There are only a few things that should drive API requests:
> 
> agent tool-call loop continues
> user ends a turn
> cache-nearly-expired proactive handover/compaction
> maybe explicit “resume/continue” actions

Something that *would not* is "user opened file" - that would piggyback instead, or just remain queued until user hits ctrl+enter to send their full turn.

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

However, there's another layer of thing to think about:

**Append vs rebuild**

If we expect a cache miss, then there's no reason to not optimize the context somewhat. We might eagerly update the AGENTS.md and other system prompt info like agent and skills, maybe update tool call schemas (although I think that's confusing, because the chat likely contains tool calls - appending is probably still correct here), we might truncate old tool calls harder, etc. 

This is called "rebuilding" the context. There's no reason to not do so if we expect a cache miss.

However, if we expect a cache hit, we must instead treat the context as fully immutable, append only (or at least, append only with respect to some prefix - this depends on the provider's caching implementation and needs experimenting). If we're here, instead of changing the system prompt, we append a tiny notification that would allow the agent to know that it might need to reload some file or system prompt instructions etc. that have changed.

Of course, we always rebuild on fresh context eg. handover/compaction, or fresh agent. but, crucially, *forked* subagents are trying to be *append mode* subagents (possibly w.r.t the parent *as of the message before it sent the subagent tool call* - but that needs experimenting too.)

