# Moving a chat: retry, edit, back, versions

`demos/chat-moves/run.sh` starts a test brain on port 4362 (`PORT=…` for another), starts a chat ("Write a new four-word poem."), and retries its reply, so it has two versions. Open https://mx-hv-mj.local:4362/ and try the rest; `node dev/drive.mjs down 4362` ends it.

Hover a message in the open card:

- **↻ Retry** under a reply asks for it again. Under your message it runs the whole turn again.
- **‹ n/m ›** appears above an entry with more than one version; the arrows move between them.
- **✎ Edit** on your message puts its words in the box with an "Editing your message" bar; sending replaces that message and what followed (kept as another version). **Cancel edit** drops it.
- **⤺ Back to here** under a reply that ended a turn makes the next word yours from there.

While the agent works, these say "stop it first" and do nothing. Files are not put back: a chat whose left path wrote files is told so in its next request.
