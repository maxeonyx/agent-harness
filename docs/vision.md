# Vision

What the harness is when it is finished: the goals, what Max does with it, the six structures it is built from, the episodes that show a design is right, and the prior art. The source is Max's [vision gist](https://gist.github.com/maxeonyx/014463e0964bebd0add4b914971c492f); this restates it with his later words, which win where they differ. What is built today is in the crate docs (start with [`crates/core`](../crates/core/src/lib.rs)), the way there is in [plan.md](plan.md), and the words are in [glossary.md](glossary.md).

## Two goals

Max named two halves, "equally important": "First is the harness needs to work like I want it to. But second is I want it to be built how I want it to."

- **Works like he wants.** A harness he lives in every day: talking to agents, watching the work they do for him, and managing what needs him. The user stories below are this half.
- **Built how he wants.** "It's not just a harness that I want to use. It's a foundation for things I want to build." A small number of principled models, each concept built once, that he understands and can build other software on. The architecture below is this half.

Neither has to be finished first, though "obviously I want [...] as much of it done as soon as possible".

What is unique to it, in his weighting (2026-07-31): the user turn; the "hardcore 'forking' subagent model", with cache efficiency and structured concurrency; the limb model ([S4](#s4-environments-limbs-and-nothing-else)); and agents owning their own compaction. What he calls non-unique, non-exhaustively, "good taste" choices from other harnesses: a decoupled monolith, composable configuration, self-modification, and the context model ([S3](#s3-contexts-what-an-agent-was-told-as-values)).

## User stories

What Max does with the harness when it is finished, grouped by the part of his day it serves. His feature asks are evidence of what he wants, not specifications: each story states the want, and quotes him where his words carry a hedge. None is a requirement until it has been taken back to its root why, as a user story, and agreed: "it's not a requirement until it's been deconstructed and understood properly. And agreed upon, I guess" (28 Oct). The stories dated 28 Oct have not been taken back yet. They are his guesses at his best workflow: hypotheses to test.

### Talking to agents

- **One text box, dictated into, everywhere.** Max dictates most of what he says. Every place he answers is a free-text box, and dictation is built in, so it works where his operating system gives him none.
- **A staging box with two sends.** Enter or a click only stages: "I very, very frequently change my mind." A second Enter or click sends. **Send now** reaches the agent at its next request, saying what Max had seen when he wrote it; **Queue** holds the message until the agent's turn ends.
- **Nothing he types is lost.** Every text box is durable: a reload, a crash or the scheduler moving him elsewhere keeps his words, and the draft follows him from the PC to the phone. Typing in one draft on two devices never silently loses text (a text CRDT is the eventual answer; today a conflict opens a second draft).
- **Answering is just talking.** An agent facing Max ends its turn with whatever it needs from him; he replies in the chat, and he can reject its framing. There is no separate question tool or answer box.
- **Done is up to him.** A user-facing chat is waiting on Max whenever its turn has ended, and it is finished only when he says so.
- **The user turn.** Max contributes to a chat the way an agent does: he opens a file, runs a command or searches, and what he did is attached to the conversation, so the agent sees his work without being told. Opening a file to read is different from opening it to edit; an edit enters the chat. ("This is a cool rethinking of what it means to collaborate with an agent.")
- **Retry, edit and go back.** He can retry a reply, edit what he said, go back to an earlier point, and choose between versions of a reply. Nothing he moves away from is lost.

### Seeing and steering the work

- **The whole page is a graph of everything he is doing.** At the top level the page is a map of nodes: every agent and chat, with background work hanging off them, and disconnected components for separate projects. Each node shows the state of its task, from metadata its agent publishes. Clicking a node opens it in place; panning away closes it. Finished projects fold to their root as a small archive card.
- **Underview's way of working, hosted** (28 Oct). Underview, his dashboard shared across agents, "worked quite well for managing my home life", and he wants that workflow in the harness: regions, links between the things on it (not only agents), and tasks an agent presents to him, each of which he clicks to go to that place. "But we can do better than Underview because we're building an actual harness." He doesn't want it copied, "because it's not quite how I imagine it yet. And also because how I imagine it can be wrong." A writeup of the requirements behind Underview, from his work machine, is still to come.
- **An agent answers with a page** (28 Oct). He wants to talk to an agent "primarily through a dictation box like this, a rich view, not a text or chat view". The agent comes back to him "with its new HTML page, not with an update [...] response message", together with updated metadata about the state of its task, which is rendered into "a big graph of all the things I'm doing, the status of them all". Underview can't host the agents' pages; the harness can, and gives agents guidance on building them, perhaps through a sub-agent whose limb is for building pages for its parent task. That sub-agent would need to search context, to get everything the page needs.
- **Behind the page, the chat is a log of what happened.** Markdown, code, images, SVG and HTML render in place, and files an agent mentions open in context. A spawn links to its child's node and back. The exact context sent to the model is one toggle away.
- **Several tabs and devices at once.** He can open tabs side by side on different things, and use the same session from his phone and his PC.
- **Usage and context are visible.** How much of his usage window is left and when it resets, and how full each context is. A chat stopped by a usage limit carries on by itself once the limit resets.
- **Stop that stops.** Three acts over any subtree: **pause** freezes the loop at once and spends nothing; **stop** asks each agent to wind down gracefully; **abandon** tells it the work is no longer needed, lets it tidy up, and ends it. Each reaches every agent and process below.
- **Choosing the model and effort** for a chat, and changing them while it works.
- **Search across past chats.**
- **Notifications** when a chat needs him while the tab is in the background (on a closed app too, later: "a good feature for later").

### Agents that work well and cheaply

- **A fork-heavy workflow.** An agent forks for each phase of its work: the fork does the phase and returns a summary, so the main context keeps only the summaries and "naturally elide[s] all tool call info". A fork's result can attach the tool calls its parent needs to see. Fresh agents are for bigger tasks or other roles.
- **Agents compact themselves.** An agent compacts when it really switches tasks or its context is mostly summaries, warned as it nears the limit, and carries on as the same agent.
- **Structured concurrency: a parent owns its children.** Nothing runs that nobody owns. A task never ends with background work running unless it explicitly hands that work to its owner, or to Max, which makes it his obligation. A parent doesn't block on its children (28 Oct: "I think I've decided, though, that parent-agent blocking on sub-agents doesn't really work in my model"): it spawns them and carries on, but "potentially parent agent can't be considered done until its child agents are completed", and cancelling it abandons them too, cleaning up all their resources. In this hierarchy an agent and a task are "kind of similar, the same thing".
- **Waiting on the outside world is code.** Anything that depends on something external (emails, GitHub) is ideally handled by code the harness runs rather than by an agent, or else by "a really cheap agent in a really minimal limb".
- **The cache is never wasted.** Forks share their parent's cache, and the cache still hits after the harness restarts.
- **Agents know what they were told, and when.** A change to something in an agent's context (an `AGENTS.md`, a skill, a tool) reaches it as a dated notice, never a silent rewrite.
- **Agents see what Max is doing**: what he was looking at when he wrote, whether he is present, what he edited.
- **The model-facing harness is shaped by the model.** "We want to make the model-facing harness like *it* wants, all else equal": tool-call ordering, parallel calls and framing follow what agents say works.
- **A curated skill set**, separate from Max's personal skills, which he reviews.

### What agents can do

- **Prevention by construction.** There are no permission prompts ([settled](../AGENTS.md#what-is-settled)): an agent can do what its environment gives it and nothing else. "A full JavaScript sandbox counts. A JavaScript sandbox where you can write programs, but a classifier checks if they're read-only doesn't count."
- **Environments, not tool lists.** An environment is its tools plus the context for using them. An agent with a directory gets a shell; a chat needs no directory; roles may expose parameters Max fills in when he starts one, with a rich picker for directories and remote environments.
- **A browser agent for life admin.** A dedicated agent drives a Chromium browser through scripts, with screenshots used sparingly, its own tabs as resources it can hand to Max, and Max's own browser on his desktop as the eventual target. The usual shape is "get Max to do most of the browser usage, then do the tedious part, then let him continue."
- **Limbs anywhere.** A limb runs on the brain's machine or on another one over SSH, and survives the brain restarting.
- **Web search and fetch**, and an MCP client.
- **Workspaces and clones** follow Max's existing pattern (shared directories, lazy clones); isolation is a separate action when it is wanted.
- **Agents that serve many and own nothing** (28 Oct). Ownership is not the only relationship between agents: "is there just one kind of relationship between agents? Not really." Some agents serve many in the background without owning their work. His example is an agent-instructions agent, which receives improvements from many agents, user-facing or autonomous, and has the process, the tools and a limb fit for improving the instructions. It "doesn't have to be one agent": it could be one limb with several agents in it, depending on whether the data model lets several work on it at once. Code does, through git, which agents understand as "a distributed partial order model [...] with manual merges"; other data not necessarily.

### Managing his attention and his life

- **Inversion of control.** "I personally am not necessarily very good at prioritizing my time across all of the threads that need my input." An attention scheduler, an agent whose whole environment is reading the domain's sessions, reordering his queue and editing its own priority document, puts first the thing that blocks the most and says why. He sets its rules "in a more reflective headspace", argues with its order, and it learns: "a goal of mine to be managed, not just to manage." In Underview he has a planner agent that sees all the open work streams, his goals, and the tasks' due dates and importance. It plans the order and relationships between things beyond what the streams themselves publish, takes his updates on what he has done, and reprioritises rapidly. When he comes to the page he sees "1, 2, 3 in gold, what are my first actions today".
- **One tree to act on per domain.** He can look anywhere, but acts on one tree at a time in a domain, whichever tabs and devices he uses. It schedules his writing, not his reading.
- **Agents never create obligations for him.** Only his own action makes a session user-facing: "I'm supposed to approve the creation of user-facing sub-agents. And I think that that still holds" (28 Oct). An agent may ask, in the background, and carries on. Contacting a person on his behalf needs his say-so, unless he explicitly asked for it.
- **Work done while he is away needs nothing from him.** Left to work autonomously, agents should do work that improves his life, not work that leaves him "a bunch of approval actions when I came back". They also can't know what he did out of band: on 25 Oct, a day's prep for his evening was built from his notes, which were out of date, and most of it was for things he had already done.
- **A main agent for each area of his life** (25 and 28 Oct). He could "in theory" manage his home life (its admin tasks and responsibilities) in the harness's graph view, alongside agents' work. For any mode he is in he comes back to one main agent; hobby programming, life admin and support for Kibra's study are "reasonable divisions" with a main agent each. The main agent tells him how its sub-agents are doing: which finished on their own, and which are waiting on him, sending him to each for its part. There can be several main agents, and "even an agent over that would honestly make sense", to prioritise usage limits or coordinate shared pattern changes. "Should it be a tree? I don't think so. There's some stuff that hasn't been thought through yet." Whether an agent over the main agents is the planner (above) is open. An area is narrower than a domain (below): home holds several.
- **Domains.** Each sphere of his obligations (home, work) has its own brain, queue and data. Data stays on its own machines; a face can show more than one domain, but home devices never see work data.

### A harness that keeps going and improves itself

- **Restart loses nothing.** Ctrl-C drains and exits; a second Ctrl-C kills. On restart every agent carries on, with no input from Max.
- **A long outage holds the work.** After one, the root tasks are held instead, and his first task is probably deciding what continues. He thinks holding "makes sense", but that it is "not core" and "can be left for later".
- **Backups by default.** It "might be nice" for every brain he runs to hold all the data, synced in the background, so each backs up the others without confusing their records with its own. How that squares with a domain's data staying on its own machines is open.
- **It upgrades itself.** An agent edits the harness, builds it and relaunches it while its own session carries on: "While we want live editing, we don't want live bricking." A broken build bricks nothing.
- **Easy to run as a service**, reachable from his phone anywhere, and installable on Android.
- **Its own login, refreshed by itself** (28 Oct). The harness needs its own auth token, refreshed automatically: "every time I've gone to use the harness, it's been useless for me because the auth token is not working." He decided (28 Oct) that it gets its own login through its own OAuth flow, the way Claude Code does, so that the usage meter and dictation accept its token ("we'd be better off just pretending to be Claude Code"), and that it refreshes the login itself, "maybe with some kind of randomness on it": "We should just have two logins, I think." Today only Claude Code refreshes the login ([todo](todo.md#replicas-and-versions-s5)).
- **Session data is his.** Token usage per session, and his sessions searchable and queryable, across every brain he runs.

### Building on it

- **A foundation, not just a harness.** The substrate under the harness (facts, replicas, views, sync) is one he can build other applications on, in the line of his earlier designs for a relational, local-first application foundation.
- **His life-management system on the same foundation** (25 Oct). He wants his personal records hosted "somewhere where it can be given some actual compute, some functions that automatically run when the agent needs it to, or, the agent can write scripts against it": "My ultimate vision is to have it hosted on similar self-built architecture as the harness itself, but that doesn't need to happen now."
- **Lowest cost, best performance.** A far stretch goal: the leanest agent harness there is, down to a pass over its machine code.

## Architecture

The full system is six structures. Everything Max named is an instance of one of them: durability classes, sent status, drafts on two devices, refusals in band, Ctrl-C twice, asides, domains. A constraint that fits none of them means a structure is missing or wrong, and that reopens design instead of adding a mechanism. This is the answer to his worry that constraints interact quadratically: mechanisms do, but a constraint that is an instance of a structure already built costs about the same however many came before it.

These are hypotheses, not rulings. Max: "I'm not ruling I'm suggesting, hypotheses for what is the best way to implement this [...] I want to see a beautiful software system built. But exactly what that is, is not up to me. It's up to what we discover."

Three sentences tie the structures together. Everything that happens is a fact at a time and a replica. Everything that runs is a task in one tree. Every boundary is a parse.

In code, `crates/core` holds every structure's types as one schema and `crates/brain` runs them; each structure below names its own crates.

```mermaid
flowchart LR
  subgraph home["Home domain"]
    hb["Brain<br/>(SQLite replica)"]
    hl1["Limb: workspace"]
    hl2["Limb: browser"]
    hl3["Limb over SSH"]
    hb <--> hl1
    hb <--> hl2
    hb <--> hl3
  end
  subgraph work["Work domain"]
    wb["Brain<br/>(SQLite replica)"]
    wl["Limbs"]
    wb <--> wl
  end
  hpc["Tab on the home PC"]
  phone["Tab on the phone"]
  wpc["Tab on the work machine"]
  hpc <--> hb
  phone <--> hb
  wpc <--> wb
  wpc <--> hb
```

Each arrow is a pair of one-way syncs of filtered replicas, and each filter is a query. Nothing else crosses a boundary. Every tab is a replica too, so a draft lives in the tab first. Work can see home; home devices never see work.

### S1. The substrate: facts at a time and a replica

- **Records.** A record is a fact at a replica (a brain, a limb, a browser tab, the phone), at a time in that replica's own order. What one replica has seen of another ("this replica, at t, includes that one up to u") is a record of its own.
- **Views.** Everything a person or an agent sees is a view derived from the records as of a time, incrementally where that pays.
- **Validity is a view.** A fact is accepted where it happens, and a derived view decides whether it is valid; only the valid subset crosses to the next replica. That crossing is the parse in "parse, don't validate". A refusal is a fact the source replica sees in its own copy.
- **Coordination is per object.** The few invariants that need one decider (a task's life, a scope closing, a name) are decided by the replica that owns the object. Everything else merges. Which invariants need a decider is the invariant-confluence test (Bailis): one does when two replicas could each keep it and still break it by merging.
- **Durability is declared per kind of fact:** never stored (streamed tokens), kept until processed, current state only (a draft, a focus), kept for a context or a session, or kept forever. Collection is the default, and replicas register, so the harness knows what is still needed.
- **Storage is an effect at one replica:** SQLite under a brain, the browser's own storage under a tab.
- **Settled.** The substrate is its own crate, general in concept but carrying only what the harness uses, with a core that builds for `wasm32` so the page runs the same code. Views are hand-written for now ("handwritten now seems like a reasonable choice"), so an engine can replace them later. "Place" is called "replica". When two devices send one draft with different words, either outcome will do; Max leans to a Send that always sends.
- **The want.** Two screens never disagree, nothing he types is lost, and the data discipline is one he can build other software on: "I want to see a beautiful software system built."
- **You can tell by:** no type between processes is a message or a request, only facts, filters and views; a message's "sent" is derived; a refused click shows in the tab that made it; the database stops growing with clicks.
- **In code.** `crates/substrate` and `crates/store`; the harness's facts are `fact::Fact`, in `crates/core`.

### S2. Tasks: one tree, every level

```mermaid
flowchart TD
  n2["Chat (durable)"]
  n2 --> n3["Agent (durable)"]
  n3 --> n4["Fork (durable)"]
  n3 --> n5["Model request (process)"]
  n3 --> n6["Tool call (process)"]
  n6 --> n7["Shell process group"]
  n4 --> n8["Model request (process)"]
```

- **A task is anything asynchronous:** a process, a thread, an HTTP request, a tool call, a shell's process group, an agent, a chat, a watcher. Each has one owner. A task cannot finish while it owns live tasks unless it explicitly hands them to its own owner. Starting something is opting into its result.
- **Cancellation has two phases at every level.** The request arrives at once, even to a thread blocked in a read. Cleanup then makes a hard kill safe, cancels the children and waits for them, each level's deadline shorter than its parent's. For an agent, cleanup is a tidy-up turn.
- **Two kinds of task.** Durable tasks (agents, chats) live in the substrate and outlive any process. Process tasks (requests, threads, child processes) die with their process. Shutting a brain down drains its process tasks and suspends its durable ones; cancelling a durable task gives it the tidy-up turn. Ctrl-C once drains; twice kills.
- **Settled.** Every stop gets a tidy-up turn; a drain waits up to 5 minutes. Containment is by marks on processes, with cgroups later. Limbs outlive a brain restart.
- **The want.** Nothing runs that nobody owns, and he can walk away trusting nothing is left burning. The relationships between agents should "stick back to computer science fundamentals": "preemption and graceful shutdown, cancellation, ownership, scheduling, and more" (28 Oct).
- **You can tell by:** a request can be cancelled before its first byte; no process outlives its owner in the process table; a stop reaches the leaves and comes back within the deadline.
- **In code.** `crates/runtime`, the process half of cancellation; the tree is `task::Task`, in `crates/core`.

### S3. Contexts: what an agent was told, as values

- **A context is an immutable, content-addressed tree** of exactly what was sent, and an agent's head moves through it. Forks share their parent's prefix and its cache, and fresh agents started together share their common brief's. Compaction moves the head to a new root. Every block records where it came from.
- **A change to something a context holds becomes a dated notice** ("changed after your turn N"), never a silent rewrite.
- **Settled.** Agents use the 1-hour cache. An agent forks for steps and compacts on real task switches, with a warning as it nears the limit.
- **The want.** Cost (forks and compaction that hit the cache) and trust: an agent knows what it was told and when. The gist's testimony: "I could not distinguish 'this was always here' from 'this appeared three turns ago'".
- **You can tell by:** every block on the page names its source; a fork's first request reads its parent's cache; an edited `AGENTS.md` reaches a running agent as a notice with a turn index.
- **In code.** `crates/program`, what each request renders; a context is `context::Entry`, in `crates/core`.

### S4. Environments: limbs, and nothing else

- **Only limbs.** An agent can do and know only what its limbs give it: tools, plus the context for using them. There are no implicit tools. A sandbox counts only if it looks like the whole world to the agent: a JS engine with no file system counts, a shell behind a classifier doesn't.
- **Max is a limb.** An agent is user-facing exactly when he is in its environment, and only his own action puts him there.
- **Roles dissolve.** Roles are "subsumed by limbs": an engineer is a set of limbs plus its model and effort.
- **Definitions are versioned facts** in the domain's database. A file in a repo is one source of a definition: a limb reads the file, never the brain. Which source each definition has is chosen for it: "think about where the state should go rather than just blindly putting it in the repo".
- **Settled.** Extension is a JS sandbox limb when something needs scripting, and WASM modules later; programs as data is a long-term direction, not now, and Max says "Maybe, I'm not so convinced about that. In a language project, maybe. I don't know." Asks are not a separate mechanism: an agent facing Max ends its turn with what it needs. The browser limb drives Max's own browser on his Windows desktop ("ideally"), which needs the SSH limb.
- **The want.** Prevention by construction at every level, rather than by checking: "checking is the red flag that you're doing this wrong". And context that comes with the environment.
- **You can tell by:** the brain opens no file but its database; a new kind of limb needs no change to the brain; an agent can't reach `~/.claude/.credentials.json` unless its environment holds it.
- **In code.** `crates/limb`, `crates/limb-workspace`, `crates/limb-browser` and `crates/script`; an environment is `environment::Environment`, in `crates/core`.

### S5. Replicas and versions

- **Roles, not processes.** Brain, limb and face are roles; one binary runs any mix of them, and each seam is in-process, a pipe or the network, chosen at start: a "decoupled monolith".
- **A version is a replica.** Each process runs one build. Upgrading starts a replica at the new build and routes new sessions to it; rolling back routes back. Facts are the one compatibility contract: each replica parses facts from other versions at its boundary, as it parses anything foreign.
- **Settled.** Sessions move to the new build at once. Rebooting is the operator's job (a systemd user service).
- **The want.** Memory that scales with active work, running anywhere, and upgrades that lose no session.
- **You can tell by:** an agent changes the harness and relaunches it, its own session carries on, and its children start on the new build; a broken build bricks nothing.
- **In code.** `crates/limb-builds`, which relaunches the brain and rolls back; a build is `build::BuildId`, in `crates/core`.

### S6. Max: a participant, his attention and his domains

- **His acts are facts** at his replica (a tab, the phone), parsed into a session like anyone else's. Every text box is durable state. He can look anywhere, but acts on one tree at a time in each domain.
- **A domain** is a sphere of his obligations with one attention: its own queue, priority document and scheduler, and its own data, which stays on its own machines. Usually one brain per domain, and a face can show several.
- **The attention scheduler** is an agent in each domain whose whole environment is reading the domain's sessions, reordering his queue and editing its own priority document. Max thinks it probably stands, though he isn't sure it shouldn't be a one-off each time: a standing agent whose every wake is a short fresh request is both, so its context never grows long. It says why each item is where it is.
- **Settled.** One brain per domain. Work can see home; home and the phone never see work.
- **The want.** Inversion of control over his attention, with the rules set "in a more reflective headspace", and "an agent can't give me an obligation without my say-so."
- **You can tell by:** the scheduler puts first the thing that blocks the most and says why; he disagrees, and the order changes while he types, losing nothing.
- **In code.** `crates/face-web` and `crates/face-tab`; his words are `draft::Draft` and his attention `attention::Ranking`, in `crates/core`.

## How to tell a design is it

The full system passes these episodes, as demos Max can watch.

**Drafts, sends and refusals (S1, S6).**

1. He dictates into a chat on his phone, stops, and picks up the same draft on his PC. He sends it from the PC, and both devices show "sent" once the brain has included the PC's edit.
2. He clicks Done on a chat the brain had already finished. The refusal shows in that tab, from the tab's own replica, with no error path.
3. He types into a chat while the agent is mid-turn. The agent's next request carries his message, saying what he had seen when he wrote it.

**Stopping and restarting (S2, S1).**

4. He presses Stop on an orchestrator with three agents under it, one running `cargo build`. Each agent gets a tidy-up turn within its deadline, the build's process group is gone, and the orchestrator reports.
5. He presses Ctrl-C on a brain in a terminal. In-flight requests end and the brain exits; on restart every agent continues. A second Ctrl-C kills it at once, and restart still continues.
6. A request is cancelled while it waits for its first byte.

**Upgrades and extension (S5, S4).**

7. An agent edits the harness and relaunches it. Its own session carries on, and its next child runs on the new build. A build that won't start changes nothing.
8. A new kind of limb (a JS sandbox) is added without changing the brain.

**His attention (S6).**

9. He opens two tabs side by side on two chats. He can act in the one his scheduler has him on, and read the other.
10. He comes back after a week away, and the first thing in his queue is deciding what continues.
11. The scheduler reorders his queue while he is writing. His text survives, and the reason for the new order is on screen.
12. An agent without him in its environment can't open a chat with him. One with him in its environment opens one in the background and carries on, and can't finish until he is done with it.
13. Work data never reaches a home machine or his phone, and a tab on the work machine shows both domains.

**Hygiene (S1, S2).**

14. The brain's database stays roughly the same size over a week of clicks and focus changes.
15. No process, thread or temporary file outlives the task that owns it.

## Prior art
Max's own earlier designs are the closest prior art, and the substrate follows them: a relational CRDT database built from tuples with logical time, queries and side effects, where a transaction is a temporary replica and replication scope is a query; notes on databases where "every client is a replica, but scoped"; a tool that filters a history to a subset and composes it back keeping event identity; and LaterLang, where "a parent cannot exit until all children have exited." Bx, his content-addressed build system, shares the idea that an immutable value is named by its hash, so a version is an identity, not a label (the core crate's docs say what is and isn't shared).

The gist says to "steal *designs*, NOT code" from other harnesses. [Max's OpenCode fork](opencode-fork.md) is the one the harness should match where it has a similar feature. Repositories designers have read, re-cloned when needed:

| Area | Repositories |
|---|---|
| Agent harnesses | [codex](https://github.com/openai/codex), [opencode](https://github.com/sst/opencode), [pi](https://github.com/badlogic/pi-mono), [oh-my-pi](https://github.com/can1357/oh-my-pi), [goose](https://github.com/block/goose) |
| Sync and CRDTs | [automerge](https://github.com/automerge/automerge), [loro](https://github.com/loro-dev/loro), [jj](https://github.com/jj-vcs/jj), [ggrs](https://github.com/gschup/ggrs) |
| Databases and incremental views | [differential-dataflow](https://github.com/TimelyDataflow/differential-dataflow), [feldera](https://github.com/feldera/feldera), [salsa](https://github.com/salsa-rs/salsa), [spacetimedb](https://github.com/clockworklabs/SpacetimeDB), [xtdb](https://github.com/xtdb/xtdb), [turso](https://github.com/tursodatabase/turso), [restate](https://github.com/restatedev/restate) |
| Structured concurrency | [trio](https://github.com/python-trio/trio), [asupersync](https://github.com/Dicklesworthstone/asupersync) |
| Other | [bevy](https://github.com/bevyengine/bevy), [trunc](https://github.com/maxeonyx/trunc) |
