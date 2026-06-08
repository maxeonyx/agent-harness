# Tech notes / ideas for agent harness

We implement in Rust - it gives us performant async, safe multithreading & structured concurrency, easy binary wire formats, native compilation.

However I think we also embed Deno, and use it for almost all of the "business logic" of our harness, via plugins.
- tools
- providers
- user tools
- limbs?

One plugin can probably provide any number of these things. I think we should keep plugins in a hard sandbox somehow - no installing node modules. Ideally a provider plugin can operate without actual access to the auth, too. We implement the oauth / device / token flow *outside* of the plugin. it contributes a script or config, then we hand it back a pre-authenticated fetch wrapper or something. 

We should set up the harness for self modification as fast as possible - we implement the basic loop, then the agent can edit the code, and probably use a tool call to re-launch onto the newer code. This is for the rust binary itself, as well as plugins. The rust binary should therefore support some kind of state scheme so that we can launch back into the same session. For plugins, we can edit them and reload them in-place. However, I think we store plugins in the DB, perhaps? Why? because we want to be able to keep the context the same so long as the KV cache is still valid. If we relaunch or reload plugins, we still want the system prompt to remain the same, but the system prompt contains the tool call defs, so we still want the old tool calls to be usable by the agent until the next handover/compaction or the next cache break. Cache break should be an option for plugin reloads though. but not normal path, it should rather be explicit. If the new version of the plugin crashes we can also revert to the old version. While we want live editing, we don't want live bricking. I guess we probably want to exercise plugin code on plugin launch as much as we can. Point is though, a model should be able to edit a plugin or the harness implementation itself, build and reload autonomously - including relaunching the harness itself in entirety and smoothly continuing.

Backend server cache ids etc should not be ephemeral - they should be tracked in the DB by session, so that we can seamlessly continue on relaunch.

The brain can be running the agent loop for many sessions at once - I think if the brain relaunches it should simply continue these. I wonder if we'd wait for all API requests to complete first before closing. Yes - that makes good sense for graceful shutdown - wait for all API requests to complete, remember for later that we're about to run tool calls or whatever, then we re-launching we run those tool calls etc. and continue where we left off. Perhaps if it's been more than an hour, then (if interactive) we ask the user whether we should continue other agents. If it's the brain in server mode, it should definitely just continue. Actually - I don't think that's so clear. Probably this should be optional too. The brain relaunching within an hour can continue but beyond an hour, first client would have to decide whether or not to resume other agents.

We can also use it for providing a JS sandbox to the agent - a *persistent* repl that can keep state across turns, BUT that can be forked and undo-ed (although that's definitely a stretch goal - but it needs to be said here because it *can't* be implemented as an MCP.)

Deno can also give us a GUI - they have an (experimental?) Tauri-like option, so we can keep our gui in-process in our "decoupled monolith" app.

That's how our harness application should be - it should be able to run all components "in process" with communication happening over a channel, or it should be able to run the components separately on the same machine, and communicate via IPC, or should be able to run remotely and communicate via TCP/HTTP/Websockets/wg/ssh

The components being "faces, brain(s?) and limbs".

We should have communication happen by an evented model, with causal consistency. each actor should behave as sequential - eg. each session, including the UI in that somehow, probably. It's expected that the limb server might see things in a different order to the brain's agent loop and to the user's UI view. That doesn't matter so long as we can actually model it correctly and handle it correctly - the user sends a message but hasn't seen the latest tool call response yet? We can at least represent that state of affairs, then handle it correctly. Two agent sessions are usually independent, but when one reads another then we can naturally represent the dependence as at that point so that all actors can make sure they have all state they need before continuing.

We should use SQLite from the start. The brain sees it all, and has the DB. We should make sure the appropriate indices are there because it could get very large very fast. Message contents / large text / images etc should probably be separate to the event tables themselves. We should keep the DB as normalized as we can without degrading performance.

The app should be a single binary which can run in any mode - client (TUI or GUI), brain server (which also has GUI optionally - idea is that it can run in background and eg. have a tray icon and a management GUI) and limb server (probably no GUI needed on this one tho).

The app can run as all three, and split out additional limbs, and additional clients can connect. Our communication should be good enough to support multiple clients on the same session.

A common configuration will be to launch as both client and limb, but to connect to the brain for the agent loop. If possible, we should support transitioning the process to being only a limb - "detaching" the GUI from the process, maybe re-parenting it to systemd or task scheduler. The brain keeps using it.

Q on "multiple brains" - federated brains. I like the idea - if I can establish persistent communication between my laptop and my desktop, or my windows and my linux vm, or my cloud / saas one, then I could ideally route communication via that pathway, and connect any client to any brain via transparent proxying. So I'd be presented with the merged list of sessions across brains. A limb would have a primary brain affinity (it will be common for a brain server to be running on every machine which hosts limbs) but I could override it.

Our GUI should also be usable as a web app not just as a desktop app. Multi-client state is also a goal - I should be able to seamlessly transition between my phone (web app?) and a TUI on my desktop. Possibly even CRDT for the draft buffer. By being event based, we are essentially implementing CRDTs, and that's OK. Mostly for the live state though - as we do have a session-authoritative brain server.

For TUI, see https://github.com/Dicklesworthstone/opentui_rust

I think what is clear is that we need extreme discipline around an event-driven architecture, but that also we should be clear that that is NOT the product, only enabling it.

eg. there are multiple event *sources*: user actions, agent loop, external events eg. filesystem watcher
there are multiple views - context view, user UI, analysis
there are multiple *storage levels* - some events are durable, some are not. for some we store only a projection (for analysis) for others we store the events. Some durable events are only needed for the lifetime of the session, some only for the lifetime of the current context, others forever. Some not durable at all.

We want the architecture to *strictly* follow an "events + derived views" model, *even though* this is likely to be quite complicated in practice at the low level, *because* it will make the *high level* guarantees simpler to achieve.

It will need explicit planning. And despite all this work, it should not leak into the actual product.