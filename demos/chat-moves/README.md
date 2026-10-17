# Moving a chat: retry, edit, back, versions

`demos/chat-moves/run.sh` starts a test brain on port 4362 (`PORT=…` for another) on the scripted model, which costs nothing, starts a chat, and prints its transcript and link. Open it and try the moves below; on the scripted model a retry says the same words again, so for a second version run it with `MODEL=sonnet`, or edit your message. `node dev/drive.mjs down 4362` ends it.

Hover a message in the open card:

- **↻ Retry** under a reply asks for it again. Under your message it runs the whole turn again.
- **‹ n/m ›** appears above an entry with more than one version; the arrows move between them.
- **✎ Edit** on your message puts its words in the box with an "Editing your message" bar; sending replaces that message and what followed (kept as another version). **Cancel edit** drops it.
- **⤺ Back to here** under a reply that ended a turn makes the next word yours from there.

While the agent works, these say "stop it first" and do nothing. Files are not put back: a chat whose left path wrote files is told so in its next request.
