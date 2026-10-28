# To do

Open work between `main` and [the vision](vision.md), grouped by the [structure](vision.md#architecture) or part of the system it belongs to. Each item says what is missing and where the evidence is; file paths are as of `main` at #123 (`8d8271f`). Max's own asks carry his hedges. What only he can decide is in [plan.md](plan.md#questions-only-max-can-answer), not here.

## Wrong steps to undo

Where `main` went a way the vision doesn't, as opposed to something not built yet:

- **`Task` means an agent.** In the vision a task is anything asynchronous and an agent is an agent; requests and calls have lifecycles of their own instead of being tasks in one tree ([glossary](glossary.md#agents-and-work)).
- **Roles are not yet limbs.** `Environment` is a role with a prompt, rather than a set of limbs plus model and effort.
- **The brain is "the one decider".** The vision decides per object, at the replica that owns it (S1).
- **`Environment::places`** means workspace slots, though "place" was renamed "replica" everywhere else.

## Substrate (S1)

- **Snapshots and collection.** A decider keeps every record it has folded, since it has no snapshot of its state; values are never collected; a store is replayed from its first record. Max: "we should deliver snapshots, not just event streams [...] otherwise it gets really, like, really expensive." (`substrate/src/lib.rs:59-63`, `store/src/lib.rs:22-24`, `core/src/schema.rs:62-77`)
- **Durability declared per kind of fact.** Two kinds are built: kept for good, and only the latest per key (a draft's text, a tab's focus); streamed tokens and command output are never stored. "Kept until processed" and "kept for a context or a session" are not built (`substrate/src/record.rs`). Analytics is a retention question. Max: "what do we keep, for how long, and especially why. 'keep everything' is an option but not an answer."
- **Incremental views.** Views are recomputed in full on each change (`face-web/src/view.rs:59-64`, `program/src/render.rs:28`), and the brain's `work()` calls `todo()` once per item (`brain/src/brain.rs:394-401`).
- **Which invariants need a decider.** No analysis yet of which refusals need one replica to decide and which can merge (`substrate/src/decider.rs:22-25`); `core/src/lib.rs` still calls the brain "the one decider".
- **`Change` carries no previous version** (`core/src/state.rs:2030-2063`).
- **`cold` duplicates fold state** (`brain/src/brain.rs:146,934,969`).
- **Credentials in the domain's database,** replicated only within a brain, perhaps encrypted by the OS keychain (Max, 2026-08). Today the harness's own login is `~/.local/share/agent-harness/login.json`.

## Tasks (S2)

- **Threads with no owner.** Bare `thread::spawn` calls that no task owns: `brain/src/brain.rs:331,1086`, `brain/src/link.rs:47`, `brain/src/process.rs:58,66`, `limb/src/serve.rs:86,303,320`, `limb/src/calls.rs:142`, `limb/src/attach.rs:115`, `limb-browser/src/cdp.rs:53`.
- **The harness captures PIDs itself,** so an agent never needs `pgrep -f` or `pkill -f` to wait on or stop a process it started. Max: "the important step is *capturing the PID* - how about we make *our* harness do that automatically?" (not now).
- **Background shells.** `( … &)` dies with its bash call; `env -i setsid` escapes cancellation; a shell runs on a pipe, not a pty (`limb-workspace/src/shell.rs:58`).
- **Daemons an agent starts** (a dev server) need an owner that outlives the call: today a task handed to Max dies at its tool timeout. Leases are one shape.
- **Spawn controls.** `spawn` can't set a child's model or effort (`program/src/meta.rs`); a parent can't see the progress of one member, cancel it alone or give it a time limit; there is no spawn under a background scope (`meta.rs:82`).
- **Isolation as its own action.** Forks share their parent's directory on purpose; making an isolated directory should be a separate action, with a `dir` per spawn member (`meta.rs:144-146`). Max has a workflow for it that "potentially the harness can help" with.
- **Cancel keeps a finished response.** Max (2026-07): on cancel, wait for the assistant response to complete, "It cost us money, and it's probably good", don't run its tool calls, and run them on resume. `core/src/cancel.rs` cuts an abandoned task's request at once.
- **Re-wiring dependencies after launch** (A starts A' to fix credentials and makes B and C wait on it): "yes, maybe [...] Maybe A can't do it, but A can ask the orchestrator to do it. I don't know." Spawn order is fixed at launch. One shape to weigh: an order such as "B before A" is priority for the scheduler rather than an edge in the tree, and ownership shaped as a graph waits for a real case.
- **Root tasks held after a long outage** ([vision](vision.md#a-harness-that-keeps-going-and-improves-itself)). A restart carries every task on however long the brain was down; how long counts as long is open. He wrote "Perhaps if it's been more than an hour", then "Actually - I don't think that's so clear. Probably this should be optional too."
- **Rough edges.** `Caller::Meta` and `Start.awaits` are never created; `Rejected::CannotStart` carries no names (`core/src/fact.rs:274`); passed calls can clash by name; `end(mark)` is duplicated (`limb-builds/src/relaunch.rs:362`); an unexplained 180 s pause was seen once.

## Contexts (S3)

- **Compaction started by Max or the harness,** not only by the agent: forced at a fixed token threshold ("80-85% (or better, a fixed token threshold like 100k-200k tokens)"), on cache expiry while idle (re-rooting a cold context; a call still running shows in the new context as in progress, and carries on), and by Max. Also an "uncached compaction" with a traditional summariser prompt, for when the original model isn't available.
- **Undo a fresh compaction.**
- **Refurbishing a context** ([glossary](glossary.md#agents-and-work)) "needs to be designed still": code, and maybe a utility model where the economics justify it, that makes a context smaller without compacting it.
- **Two-stage compaction,** as in his OpenCode fork: the first call gives the agent what a good compaction needs, and lets it tidy up with other tools before a second call completes it. `compact_and_continue` and `compact_and_report` are one call each (`program/src/meta.rs`). A related idea of his, not decided: a fork of the parent writes each child's mandate before a spawn, at the cost of latency and spend on every spawn.
- **Attaching calls to a result.** `attach` (`program/src/meta.rs`) is the working version Max asked for first; the whole design space is still to look at.
- **Old cold contexts:** "ideally we just keep an 'old cold' context and make it an 'old warm context', append some [...] notices, and keep going [...] Perhaps an option for the user? I don't think this is settled."
- **Notices for tools and options.** Removing or changing a tool never gets a notice; old tools keep working until the cache expires, or the context is compacted. Whether adding a tool can work without breaking the cached prefix, Max is "unsure". Option sets (skill names, role names) "absolutely get notices"; `AGENTS.md` changes "almost certainly yes"; skill description changes, no. Provenance of a change (the harness, git, an outside edit) is "potentially _very_ useful for the model". A utility model could classify and summarise changes, "only if the economics justifies it": it is "potentially possible" for one to re-use the main model's prefix at 0.1x cost, which "should be confirmed empirically" (see Provider and cache); otherwise a one-shot task on little context "may often be cheaper than 100k+ context at 0.1x".
- **Mandate reminder after a tool round.** The fork-framing spike measured it cheap and effective: a depth-1 fork's uncached input fell from about 600-1,800 tokens to 2-4. Ready to build (listed in `core/src/context.rs`). Forking at the parent's request (`rewind`) measured the smallest prefix at depth 2-3, if deep forks get expensive.
- **A cache breakpoint for message undo:** "everything up to n-2 messages ago (or something like that)". `program/src/policy.rs::breakpoints` marks root, checkpoint, shared brief and tail only.
- **Tell the model which model it is:** "I think we do want to tell the model which model it is."
- **Cache keep-alive** for a context waiting on Max inside the 1-hour window.
- **Compaction warning wording** compared with his OpenCode fork's ([opencode-fork.md](opencode-fork.md)); check it doesn't warn a context that a compaction started already over the threshold.
- **`Note::Stale.shown` is always empty** (`program/src/render.rs:307`).
- **Prompt length** from web search result blocks (`program/src/web.rs:51`).

## Environments and limbs (S4)

- **No sandbox.** `bash` can read `~/.claude/.credentials.json`. The JS sandbox limb (an engine with only the limb's functions) is the planned first sandbox.
- **Chats with no directory,** and roles whose limbs take parameters Max fills in when he starts one: "limb types need varying parameters, potentially". A rich picker for directories and remote environments.
- **The SSH limb** (`Host::Ssh`, `brain/src/brain.rs:524-530`), which the browser limb needs to reach Max's own browser on his desktop. Which sites the browser agent may log into is his to say.
- **The brain depends on limb crates' `Settings`,** and face-web's dictation depends on the provider crate.
- **Limbs found by file name** (`brain/src/brain.rs:1921-1946`); limbs from an old build are never replaced in place, their state is never retired, and nothing lists or kills them. Without a systemd user manager, a restart ends them.
- **The roles limb is a fourth copy of limb plumbing** (`brain/src/roles.rs`), and polls every 2 s; there is no file watcher. It needs `Instance` over an in-memory store.
- **Meta and web limbs don't declare their tools** (`brain/src/brain.rs:1080,1112`).
- **Files limb.** A refused edit re-shows up to 64 KiB (`limb-workspace/src/files.rs:532-540`); a non-zero exit marks all output as an error (`files.rs:164`); `rescan` re-hashes under a mutex (`limb-workspace/src/lib.rs:476-497`); `clone_of` runs git synchronously on the brain loop (`brain/src/brain.rs:1329,1962-1969`).
- **Browser limb.** Just after `tab` starts, `browser` and `page` can say "no tab t1"; a tab doesn't survive a limb restart; no downloads or live output; a child can't use a tab passed to its parent.
- **MCP client and plugins.** No MCP or plugin runtime yet.
- **Skills per role,** and which `CLAUDE.md` lines the harness loads (Max: "later"). `AGENTS.md` discovery up to the git superproject root, as his OpenCode fork does.
- **A generic parallel tool.** Max: "listen to the model [...] tool calls in order, spawn together in parallel, or maybe better just a generic parallel tool that can be used for any tool." Today parallelism is `spawn` only.
- **A JS tool for tool plumbing.** Measure how many turns go to plumbing (filtering big output, chaining calls) before deciding on a tool that runs a short JS program over the other tools.

## Replicas and versions (S5)

- **One binary** running any mix of brain, limb and face.
- **Old sessions on old builds:** the old process finishing its work after a relaunch, deadlines, and collecting old builds.
- **No go-back button after a relaunch** (`face-web/src/page/builds.js:19`), and rehearsal doesn't catch a broken page.
- **TLS certificate path and bind address are hard-wired** (`brain/src/main.rs:213-219,245`).
- **The relaunch watcher folds every 2 s.**
- **A missing limb at relaunch fails a task permanently,** and a missing or dead login (never signed in, or its refresh token revoked) fails a task after about 30 s, where both should wait. A signed-out login is the same concept as `Ending::Limited` (the account waits; not the task's failure) but with no known end and a different reason, so it wants a decision on the core model before the brain can wait on it and say so on the page.

## Max and his attention (S6)

- **The attention scheduler.** `LimbKind::Attention` is produced by nothing; `Ranking` and `Focus` are unchecked (`core/src/state.rs:884,890-901`). An `attention::Ranking` is one whole order and the newest wins, so Max reordering by hand while the scheduler ranks loses one of the two.
- **Domains** as data partitions with their own brains.
- **One mutating face per domain** (the action lock).
- **Agents see what Max is doing:** whether he is away, what he edited, his branch and its uncommitted changes.
- **Non-blocking asks** (2026-09): an agent builds up a queue of actions for Max that expire, each with a strict schema and "self contained": it reiterates "the chain from what the user ultimately wants, to why the requests exists", with what he needs to decide.
- **The continuing user-facing child:** "if the parent is user-facing, it should be opt-out." Today Max opens an aside himself.
- **Agents read his weekly usage** (`program/src/web.rs:4,18`).

## Face

- **A text CRDT for drafts** edited on two devices; today a conflict opens a second draft.
- **The attach button** needs a real design.
- **Directory and remote picker** for starting a chat.
- **Notifications on a closed app** (Web Push): "Nah, dw about push notifications on android - but a good feature for later!!"
- **`/view` has no running-tasks field.**
- **Records nobody read** show only on stderr.
- **An engineer that receives harness errors.** Max: "ideally, let's just go for it and direct it straight at an agent that should be fixing the errors. It should be an engineer that receives harness errors. It triages them. It determines the root cause. If it needs design, it flags it and raises it somehow. And it fixes the obvious ones at the root cause. Making the product smaller and better." The page's errors reach the brain as `PageError` (`face-web/src/lib.rs`, `Wiring::report`), which only logs them; handing them to an agent is left.
- **Chats that start alike look alike.** `act chat` takes no name, so a chat is named after the first words of its mandate (`name_from` in `brain/examples/device/mod.rs`, as the page's `nameFrom` does). In a home-life pilot on the home brain on 25 Oct, five of the six chats started began "Max is away until this evening" and looked identical in the chat list. A chat can be renamed (`TaskEvent::Renamed`), but not through `act`. Old chats on the home brain include five named "Hi". Max expected a title generator: "I guess we just didn't make that yet."
- **A chat Max starts has nothing above it.** `act chat` and the page's new chat start only roots (`brain/examples/device/mod.rs:303`, `face-web/src/page/newchat.js:85`), and nothing moves a chat under another, so the pilot's chats stood alone, though Max thought it "would make sense" to have one chat above them "to orchestrate and manage" (the vision's home-life line). A chat can start children; whether Max can talk to them directly is the continuing user-facing child, above.
- **Rough edges.** The TLS listener polls every 20 ms (`face-web/src/lib.rs:134`); the token sum is written out in four places; `Wiring` is full of `Option`s.

## Provider and cache

- **Empty `tool_use` id** becomes an empty string (`brain/src/brain.rs:1033-1037`); the text-block helper is duplicated.
- **Unmeasured provider questions:** whether a cancelled request is still charged (Max thinks so; each response's 5-hour utilisation can show it); whether the previous assistant turn is charged as a write; TTL behaviour and whether expiry is observable; which tool-array changes keep the prefix ("is it ever possible to change anything about tools _without_ involving the cached prefix"); late system-section changes; whether a smaller model reads a larger model's prefix.
- **Dictation recognition hints.** The speech endpoint takes `x-config-keyterms` (comma-separated, at most 1024 characters), which could carry project words like "limb" and "brain". Only `linear16` audio returned text.
- **Agent-run tuning.** Tunables as settings with recorded outcomes, so a background agent could tune them by A/B test, compaction instructions included (Max, 2026-08).

## Docs and dev

- **Cut the core crate's "Provisional" list** (`crates/core/src/lib.rs`) down to code-local caveats that link to [plan.md](plan.md#questions-only-max-can-answer), dropping what Max has answered, so open questions live in one list.
- **Small helpers duplicated:** `now()` in five places, the random replica id twice; a `Superseded` row (`brain/src/brain.rs:1657`); `LimbRow::Call.owner` is an `Option` (`core/src/environment.rs:655`); a pure step function for the brain is not done.
- **Demos.** The spine demo's step numbering (`brain/examples/spine.rs`); demos grep log wording; the relaunch and browser demos haven't been rerun since their last changes; `face-controls` needs a prior build; `dev/serve` has a scripted wait; `dev/drive.mjs` gaps; test brains that are durable need retiring.
- **As-built diagrams.** Max wants diagrams of the current state kept up to date, as mermaid in the crate docs where each subject lives; the dashboard's Built page shows them. Still undrawn: how a limb runs a call at most once (`harness_limb::Calls::call`: running, settled, stopping, shutting down, started and not rerun, run), the brain's shutdown (`Brain::signal`, `Brain::down`), and two from the old page on `archive/dashboard-v1` that the processes-and-seams diagram may cover only in part: "One fold, many readers" (the data flow; `gen/flow.json`) and "Every copy of the data" (replicas and consistency, who decides and where each record lives; `gen/replicas.json`). Max asked whether there should be more, "for example, potentially limb launch, or um, title generation": there is no title generation to draw, since a chat is named from its first words (`nameFrom`) and Max can rename it.
- **`Start.awaits` and `Caller::Meta`** are modelled and read, but nothing sets `awaits` (`Brain::start_of` leaves it empty, `brain/src/brain.rs:722`) or makes a `Caller::Meta`.
- **Design questions for Max.** Design questions for Max "need to jog my memory": quote the claim and both sides, and ask him to confirm, deny or refine.
