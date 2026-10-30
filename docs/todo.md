# To do

Open work between `main` and [the vision](vision.md), grouped by the [structure](vision.md#architecture) or part of the system it belongs to. Each item says what is missing and where the evidence is; file paths are as of `main` at #123 (`8d8271f`). Max's own asks carry his hedges. What only he can decide is in [plan.md](plan.md#questions-only-max-can-answer), not here.

## Wrong steps to undo

Where `main` went a way the vision doesn't, as opposed to something not built yet:

- **`Task` means an agent.** In the vision a task is anything asynchronous and an agent is an agent; requests and calls have lifecycles of their own instead of being tasks in one tree ([glossary](glossary.md#agents-and-work)).
- **The brain is "the one decider".** The vision decides per object, at the replica that owns it (S1).

## Substrate (S1)

- **Snapshots and collection.** A decider keeps every record it has folded, since it has no snapshot of its state; values are never collected; a store is replayed from its first record. Max: "we should deliver snapshots, not just event streams [...] otherwise it gets really, like, really expensive." (`substrate/src/lib.rs:59-63`, `store/src/lib.rs:22-24`, `core/src/schema.rs:62-77`)
- **Durability declared per kind of fact.** Two kinds are built: kept for good, and only the latest per key (a draft's text, a tab's focus); streamed tokens and command output are never stored. "Kept until processed" and "kept for a context or a session" are not built (`substrate/src/record.rs`). Analytics is a retention question. Max: "what do we keep, for how long, and especially why. 'keep everything' is an option but not an answer."
- **Incremental views.** Views are recomputed in full on each change (`face-web/src/view.rs:59-64`, `program/src/render.rs:28`), and the brain's `work()` calls `todo()` once per item (`brain/src/brain.rs:394-401`).
- **Which invariants need a decider.** No analysis yet of which refusals need one replica to decide and which can merge (`substrate/src/decider.rs:22-25`); `core/src/lib.rs` still calls the brain "the one decider".
- **`Change` carries no previous version** (`core/src/state.rs:2030-2063`).
- **`cold` duplicates fold state** (`brain/src/brain.rs:146,934,969`).
- **Credentials in the domain's database,** replicated only within a brain, perhaps encrypted by the OS keychain (Max, 2026-08). On 28 Oct he chose the database over a token file for the harness's own login, "so that it can sync across machines, ideally", with encryption from "some kind of keychain or SSH agent" in the future. Today the login is a `login` table beside the home brain's records, unencrypted and not replicated: every brain on the machine reads it there (`provider/src/login.rs`).
- **Any 400 from a refresh signs the harness out** (`provider/src/login.rs`, `post`: `rejected` is 400 or 401), including `invalid_scope`, which is the harness's own mistake and leaves the refresh token good. Only `invalid_grant` (and 401) says the refresh token is dead; anything else could keep the tokens and fail, as a network error does. Kept for now because signed out is the state the page offers a way out of.
- **A relaunch while signed out stays on probation until Max signs in.** The check passes signed out (`brain/src/check.rs`), but the new build sends no request until then, so `Verdict::Proving` holds and the builds limb watches it with no deadline (`limb-builds/src/relaunch.rs`, `watch`); after the sign-in it proves itself as usual.

## Tasks (S2)

- **Threads with no owner.** Bare `thread::spawn` calls that no task owns: `brain/src/brain.rs:331,1086`, `brain/src/link.rs:47`, `brain/src/process.rs:58,66`, `limb/src/serve.rs:86,303,320`, `limb/src/calls.rs:142`, `limb/src/attach.rs:115`, `limb-browser/src/cdp.rs:53`.
- **A shell runs on a pipe, not a pty** (`limb-workspace/src/shell.rs:88`).
- **Spawn controls.** `spawn` can't set a child's model or effort (`program/src/meta.rs`); a parent can't see the progress of one member, cancel it alone or give it a time limit; there is no spawn under a background scope (`meta.rs:82`). Every spawn blocks its parent: Max wants a background spawn too, joined later by a tool call that waits on work the agent owns, with the foreground kept for fork-per-phase ([vision](vision.md#agents-that-work-well-and-cheaply)). Asides and the refusal to interrupt a parent waiting on its scope (`Rejected::Scoped`) exist because spawns block; whether they stay for foreground spawns is open.
- **A message queued before a spawn waits for the scope.** A message sent to a task mid-turn, before it blocks on `spawn`, stays unread until the scope ends, while messages sent once it waits open asides. Max: "A queued message and a new message should go through the same mechanism." The fix needs the brain to open the aside, which changes who may create one (`core/src/state.rs`, `check_aside`): a model decision.
- **Isolation as its own action.** Forks share their parent's directory on purpose; making an isolated directory should be a separate action, with a `dir` per spawn member (`meta.rs:144-146`). Max has a workflow for it that "potentially the harness can help" with.
- **Cancel keeps a finished response.** Max (2026-07): on cancel, wait for the assistant response to complete, "It cost us money, and it's probably good", don't run its tool calls, and run them on resume. `core/src/cancel.rs` cuts an abandoned task's request at once.
- **Re-wiring dependencies after launch** (A starts A' to fix credentials and makes B and C wait on it): "yes, maybe [...] Maybe A can't do it, but A can ask the orchestrator to do it. I don't know." Spawn order is fixed at launch. The 28 Oct shape: an order such as "B before A" between different owners' work is priority for the planner, and a dependency is a watch, refused if it would close a circle of waits.
- **A failing owner ends its workers.** While an owner is failing, the fold accepts its abandon of its workers (`core/src/state.rs:1318`, `Some(Draining::Failing(_)) => {}`), so a failure ends work as an abandon would. Max (28 Oct): the subtree is held, and a failing parent is "basically a pause"; whether held work needs pausing too is open.
- **Messages and watches.** No relationship between tasks except owning: an agent can't message another live agent, or watch work it doesn't own and be told when it settles or is abandoned ([vision](vision.md#what-agents-can-do)). A watch that would close a circle of waits is refused.
- **Standing tasks under an owner.** Only Max's scope may hold a standing task (`core/src/task.rs:1249`, `Rejected::WrongScope`), so an area's main agent, or a service under it, can't stand.
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

## Roles, agents and limbs (S4)

- **No sandbox.** `bash` can read `~/.claude/.credentials.json`. The JS sandbox limb (an engine with only the limb's functions) is the planned first sandbox. Later, a PID namespace through bubblewrap, so an agent can't signal processes that aren't its own, perhaps not for every role: "maybe I should have a special role for machine maintenance stuff, like an admin role."
- **Chats with no directory,** and roles that take parameters Max fills in when he starts one: "limb types need varying parameters, potentially". A rich picker for directories and remote environments.
- **The SSH limb** (`Host::Ssh`, refused in `brain/src/brain.rs`), which the browser limb needs to reach Max's own browser on his desktop. Which sites the browser agent may log into is his to say.
- **Face-web's dictation depends on the provider crate.**
- **Limbs from before limbs were per machine** (one per directory, the browser, the builds and Max's config) are reached only with `attach --existing` while one of their calls is unsettled. One the brain doesn't link exits 15 minutes later (`ABSENT`), but the brain never closes a link it no longer needs, so one it linked runs until the brain next restarts, and their state directories are never deleted. More generally, limbs from an old build are never replaced in place, their state is never retired, and nothing lists, kills, restarts or updates them (Max, 28 Oct: "We do need to shut them down, however. We also need to be able to start them back up. We also need to be able to update them, probably."). Without a systemd user manager, a restart ends them.
- **The builds limb doesn't reap a `harness-limb` it adopts.** After the 31 Oct move onto 9c5a5fa, PID 227530 was `[harness-limb] <defunct>`, its parent the old build's builds limb (`harness-limb-builds serve`, 2dace7a7c6a7). It is in the parent's session and leads its own group, so `reap_orphans` (`limb/src/serve.rs`) takes it for a child a thread waits on and skips it; probably the `harness-limb attach` of a rehearsal brain that was ended, orphaned to the limb as subreaper.
- **A limb that dies loses what agents read.** The one `harness-limb serve` runs in its own `harness-limb-<id>.scope`, outside the brain's unit, so a `systemctl --user restart` of the brain leaves it and its calls running (`demos/cancel`, E). A reboot or the limb's own crash still loses each agent's reads, and an `edit` after it is refused: they could be kept on disk, or a lost read could ask the agent to read again rather than refuse. Max wants a restart to "just naturally pause everything and then resume".
- **A long call pins a limb to its build.** A superseded limb exits only once nothing it runs would die (`limb/src/serve.rs:123`), so one long-running call, such as a server passed to Max, keeps it on its old build, and one limb serves every agent on its machine. The fix: the limb `execve`s the new build in place, keeping its pid, its children, its lock and its socket, and hands its running calls over in a file.
- **The role files are polled every 2 s** (`brain/src/roles.rs`); there is no file watcher.
- **The brain decides a rerun by caller, not by tool.** Every other limb's tool set says which of its tools may be rerun (`harness_limb::ToolSet::rerun`), but the fold reruns a call to the brain because Max made it (`State::reruns`), and the brain dispatches its own sets by tool name (`Brain::dispatch`). The same today, since Max's only call to the brain is `usage`; the brain's sets should answer `rerun` per tool as any limb's do.
- **The brain reads an agent's directory** to pick a new clone's source (`clone_of`, `brain/src/brain.rs`, running `git` on the brain's thread), though the directory is the limb's: on another machine it would be wrong. The limb's `workspace` set should decide it.
- **Files limb.** A refused edit re-shows up to 64 KiB (`limb-workspace/src/files.rs:532-540`); a non-zero exit marks all output as an error (`files.rs:164`); `rescan` re-hashes under a mutex (`limb-workspace/src/lib.rs`).
- **Scope descriptions name the chat by its mandate's first line,** so a call's scope can read `tool read …: sleep 15` in `systemctl --user list-units`.
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
- **A rollback forks history.** On 31 Oct the first move of the home brain onto 9c5a5fa rolled back because the old build's watcher couldn't read the record holding the new build's `Launched`. The old build then ran again for 32 minutes: it skipped the 27 records the new build had written, ran a read-only `bash` call a second time (its settlement was in a skipped record), and carried sessions on from an earlier point. Moving forward needed a backup and those 27 records deleted by hand; a mutating call would also have run twice. The design is open; one shape suggested: any build folds a rolled-back span only as far as the build that took back over could read it.
- **A missing limb at relaunch fails a task permanently,** where it should wait, as a signed-out login does (`Hold::SignedOut`).

## Max and his attention (S6)

- **The attention scheduler.** The brain's `attention` set is produced by nothing; `Ranking` and `Focus` are unchecked (`core/src/state.rs:884,890-901`). An `attention::Ranking` is one whole order and the newest wins, so Max reordering by hand while the scheduler ranks loses one of the two.
- **Domains** as data partitions with their own brains.
- **Areas and their main agents** (28 Oct): an owning coordinator per area, and an agent above the areas that may pause work when usage runs low and abandon its own subtrees ([vision](vision.md#managing-his-attention-and-his-life)). Nothing models an area.
- **His to-dos and waiting items in the graph:** a node for work with no agent behind it, executed by Max or waiting on someone outside, shown faintly until it is his action ([vision](vision.md#seeing-and-steering-the-work)).
- **One mutating face per domain** (the action lock).
- **Agents see what Max is doing:** whether he is away, what he edited, his branch and its uncommitted changes.
- **Non-blocking asks** (2026-09): an agent builds up a queue of actions for Max that expire, each with a strict schema and "self contained": it reiterates "the chain from what the user ultimately wants, to why the requests exists", with what he needs to decide.
- **The continuing user-facing child:** "if the parent is user-facing, it should be opt-out." Today Max opens an aside himself.
- **Agents read his weekly usage** (`program/src/web.rs:4,18`).

## Face

- **A text CRDT for drafts** edited on two devices; today a conflict opens a second draft.
- **The attach button** needs a real design.
- **The usage pill is lowercase:** "no reading yet · click to refresh" and "as of …" (`face-web/src/page/chrome.js:77`).
- **Directory and remote picker** for starting a chat.
- **Notifications on a closed app** (Web Push): "Nah, dw about push notifications on android - but a good feature for later!!"
- **`/view` has no running-tasks field.**
- **Records nobody read** show only on stderr.
- **An engineer that receives harness errors.** Max: "ideally, let's just go for it and direct it straight at an agent that should be fixing the errors. It should be an engineer that receives harness errors. It triages them. It determines the root cause. If it needs design, it flags it and raises it somehow. And it fixes the obvious ones at the root cause. Making the product smaller and better." The page's errors reach the brain as `PageError` (`face-web/src/lib.rs`, `Wiring::report`), which only logs them; handing them to an agent is left.
- **Chats that start alike look alike.** `act chat` takes no name, so a chat is named after the first words of its mandate (`name_from` in `brain/examples/device/mod.rs`, as the page's `nameFrom` does). In a home-life pilot on the home brain on 25 Oct, five of the six chats started began "Max is away until this evening" and looked identical in the chat list. A chat can be renamed (`TaskEvent::Renamed`), but not through `act`. Old chats on the home brain include five named "Hi". Max expected a title generator: "I guess we just didn't make that yet."
- **A chat Max starts has nothing above it.** `act chat` and the page's new chat start only roots (`brain/examples/device/mod.rs:303`, `face-web/src/page/newchat.js:85`), and nothing moves a chat under another, so the pilot's chats stood alone, though Max thought it "would make sense" to have one chat above them "to orchestrate and manage" (the vision's main agent for each area). A chat can start children; whether Max can talk to them directly is the continuing user-facing child, above.
- **Rough edges.** The TLS listener polls every 20 ms (`face-web/src/lib.rs:134`); the token sum is written out in four places; `Wiring` is full of `Option`s.

## Provider and cache

- **Empty `tool_use` id** becomes an empty string (`brain/src/brain.rs:1033-1037`); the text-block helper is duplicated.
- **Unmeasured provider questions:** whether a cancelled request is still charged (Max thinks so; each response's 5-hour utilisation can show it); whether the previous assistant turn is charged as a write; TTL behaviour and whether expiry is observable; which tool-array changes keep the prefix ("is it ever possible to change anything about tools _without_ involving the cached prefix"); late system-section changes; whether a smaller model reads a larger model's prefix.
- **Dictation recognition hints.** The speech endpoint takes `x-config-keyterms` (comma-separated, at most 1024 characters), which could carry project words like "limb" and "brain". Only `linear16` audio returned text.
- **A 400 is resent like any failure.** An `invalid_request_error` is the request's own fault, so its resends get the same answer, and after `GIVE_UP` (5) the task fails (`brain/src/brain.rs`, `send`; `program/src/policy.rs`). The resends cost little (a refused request is not billed, and they take about 30 s), but a 400 the harness itself caused, such as an image too large (now shrunk by `provider/src/image.rs`), would be better held for a build that fixes it than failed. Not changed: there is no principled line yet between a 400 a new build fixes and one only the context's owner can.
- **An image over 10 MB of base64 is still sent as is,** and the API refuses it: `provider/src/image.rs` shrinks by dimensions only.
- **Agent-run tuning.** Tunables as settings with recorded outcomes, so a background agent could tune them by A/B test, compaction instructions included (Max, 2026-08).

## Docs and dev

- **Cut the core crate's "Provisional" list** (`crates/core/src/lib.rs`) down to code-local caveats that link to [plan.md](plan.md#questions-only-max-can-answer), dropping what Max has answered, so open questions live in one list.
- **Small helpers duplicated:** `now()` in five places, the random replica id twice; a `Superseded` row (`brain/src/brain.rs:1657`); `LimbRow::Call.owner` is an `Option` (`core/src/limb.rs`); a pure step function for the brain is not done.
- **Demos.** The spine demo's step numbering (`brain/examples/spine.rs`); demos grep log wording; the relaunch and browser demos haven't been rerun since their last changes; `face-controls` needs a prior build; `dev/serve` has a scripted wait; `dev/drive.mjs` gaps; test brains that are durable need retiring.
- **As-built diagrams.** Max wants diagrams of the current state kept up to date, as mermaid in the crate docs where each subject lives; the dashboard's Built page shows them. Still undrawn: how a limb runs a call at most once (`harness_limb::Calls::call`: running, settled, stopping, shutting down, started and not rerun, run), the brain's shutdown (`Brain::signal`, `Brain::down`), and two from the old page on `archive/dashboard-v1` that the processes-and-seams diagram may cover only in part: "One fold, many readers" (the data flow; `gen/flow.json`) and "Every copy of the data" (replicas and consistency, who decides and where each record lives; `gen/replicas.json`). Max asked whether there should be more, "for example, potentially limb launch, or um, title generation": there is no title generation to draw, since a chat is named from its first words (`nameFrom`) and Max can rename it.
- **`Start.awaits` and `Caller::Meta`** are modelled and read, but nothing sets `awaits` (`Brain::start_of` leaves it empty, `brain/src/brain.rs:722`) or makes a `Caller::Meta`.
- **A merged fix can sit undeployed.** On 29-30 Oct login fixes for the home brain were merged but not deployed while the orchestrator waited on a scope, so Max signed in a third time. The reviewer that merges such a fix could deploy it, or a background spawn could free the orchestrator.
- **No agent works through this list on its own.** Max asked whether there is "an agent that's just pulling things off the to-do list and getting them done"; there isn't.
- **Design questions for Max.** Design questions for Max "need to jog my memory": quote the claim and both sides, and ask him to confirm, deny or refine.
