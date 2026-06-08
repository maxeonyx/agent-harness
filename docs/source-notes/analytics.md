# analytics

we want to keep track of some things super early:

- actual token usage across all API requests, by session. I'm tempted to also say - don't cancel API requests (perhaps only if we already recieved first byte) as I think providers still charge for cancelled requests, so we want to know. Rather, we can keep around the request future and finish it but discard the results. probably worth experiment - does long thinking process actually get interrupted on the providers server or do they charge for the whole thing?
- session messages & data, in easily searchable format.
- provide a read-only tool surface or limb for meta-work.
- I want to be able to use my session data for timesheets, for example.
- Ideally we can query across all connected brains and the results should reflect this. I'm ok with it just being sql queries.
