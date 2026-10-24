# Glossary

The words the vision, the plan and Max use. Where the code names a thing differently, the last column says so; the difference is the vision's naming waiting to reach the code, unless the row says otherwise. A cell starting `Differs:`, `Partly:` or `Not built` is a gap, and the dashboard lists it.

## The running system

| Term | Meaning | In code |
|---|---|---|
| Brain | The one process that owns model calls, storage and decisions for a domain. Faces and limbs connect to it. | `crates/brain` |
| Limb | A process that gives agents their tools and the context for using them: a shell and files in a directory, the web, a browser, Max. It outlives a brain restart. | `environment::Limb`; `crates/limb*` |
| Face | What Max sees and acts through: the page, on the PC or the phone. Each tab is a replica. | `crates/face-web`, `crates/face-tab` |
| Domain | A sphere of Max's obligations (home, work) with one attention: its own brain, queue, priority document and data. | Not built. |
| Environment | What an agent can do and know: its limbs, plus its model and effort. A role is one way of naming an environment, and roles are to be "subsumed by limbs". | Differs: `environment::Environment` is a role (`orchestrator`, `designer`), with a role prompt. Its `places` are the kinds of workspace slot it needs, not replicas. |
| Build | One compiled version of the harness, named by the hash of its executable. A supervisor starts a new build beside the old and rolls back if it fails. | `build::BuildId`, `crates/limb-builds` |
| Program | Everything a session's behaviour depends on that can change under it: how its records fold, its policy, how its requests are written. No I/O, so it can later load as WASM. | `crates/program` |

## Data

| Term | Meaning | In code |
|---|---|---|
| Substrate | The data layer under the harness: every replica logs its own records, and views are derived from them. Structure S1. | `crates/substrate` |
| Replica | Anything that keeps its own log of records and syncs it with the others: a brain, a limb, a browser tab. One process can host several. | `id::ReplicaId` |
| Record, fact | One thing that happened, stored in a replica's log: a message sent, a tool call ended, a draft edited. A record carries a fact. | `schema::Record`, `fact::Fact` |
| Value | Immutable bytes named by their hash: a system prompt, a tool result, a pasted image. Facts name values. | `id::ValueId`, `value::Bytes` |
| View | Rows derived from records, such as the task tree a tab shows. Everything a person or an agent sees is a view. | `state::State` and its methods; the face's `/view` |
| Fold | Reading records in order to build the current state. | `schema::Harness` |
| Durability | How long a kind of fact is kept: never, until processed, current state only, for a context or session, or forever. | Partly: `substrate::Durability` has only `Latest` (current state only) and `Kept` (forever); never-stored data is `Live`. |

## Agents and work

| Term | Meaning | In code |
|---|---|---|
| Task | Anything asynchronous with one owner: a process, a request, a tool call, an agent, a chat. Everything that runs is a task in one tree. | Differs: `task::Task` is one agent working toward a result. Requests and calls have their own types (below). |
| Agent | A model working in a loop with its environment toward a result. | `task::Task` |
| Chat | An agent Max is in the environment of: user-facing. It waits on him whenever its turn ends, and only he says it is done. | A `task::Task` with Max as a limb; `state::Waiting::Max` |
| Request | One call to the model. | `model_call::ModelCall` |
| Call | One tool call. | `call::Call` |
| Scope | The set of tasks a spawn starts. The parent makes no further decision until every result is in, and a task may add siblings to its own scope. | `task::Scope`, owned by its spawn call |
| Fork | A child agent that starts from its parent's context, sharing its cache, and returns a summary. | `task::Origin::Fork` |
| Aside | A user-facing child Max opens while its parent waits on other children. | `task::Origin::Aside` |
| Compaction | An agent moving its head to a new root that carries its own summary, and carrying on as the same agent. Not "handover": Max, "It is too othering". | `task::Compaction` |
| Context | The immutable, content-addressed tree of exactly what was sent to the model. | `context::Entry`, `context::EntryId` |
| Notice | A dated message telling an agent that something in its context changed after a turn. | `resource::Change`, `state::State::stale` |
| Pause, stop, abandon | The three acts over a subtree: freeze at once and spend nothing; ask each agent to wind down; say the work is no longer needed, then tidy up and end. | Stop: `core/src/cancel.rs` |
| Tidy-up turn | One last request to a stopped agent, asking it to report and clean up. | |
| Drain | On shutdown, waiting for requests already streaming before exiting, so they aren't paid for twice. Ctrl-C once drains; twice kills. | |

## Max

| Term | Meaning | In code |
|---|---|---|
| Staging box | The box Max types and dictates into. Enter or a click stages; the second sends. Send now delivers at the agent's next request; Queue holds it until the turn ends. | `draft::Draft`, then `delivery::Delivery` |
| The user turn | Max contributing to a chat the way an agent does: what he opens, runs or edits is attached to the conversation. | Partly: `fact::Fact::Seen` |
| Map, card | The page's top level is a map of every agent and chat; a card is a chat opened in place on it. Finished projects fold to an archive card. | `crates/face-web/src/page` |
| Attention scheduler | A standing agent per domain that orders Max's queue, says why, and learns from his disagreement. | Not built: `attention::Ranking` and `attention::Focus` exist, set by nothing. |
| Action lock | Max acts on one tree at a time in each domain; the scheduler moves it. | `attention::Focus` |

## Running it

| Term | Meaning | In code |
|---|---|---|
| Home brain | The long-lived brain Max uses every day, on an installed build. | A `dev/serve` unit |
| Dogfood brain | A long-lived brain this project's own agents work on, so the harness is used to build itself. | `dev/serve dogfood 4327` |
| Test brain | A throwaway brain, on the scripted model unless told, for trying a change. | `dev/drive.mjs` |
| Round | One batch of feature work run on the dogfood brain by an orchestrator there, ending in merged PRs and a report. | |
| Mandate | The first message a task starts with: what it is for. | |
| `act` | A command that does what Max does at a tab (start a chat, Done, stop, abandon), for scripts and demos. | `crates/brain/examples/act.rs` |
| Relaunch | Replacing a running brain with a new build: drain, start the new build on the same database, roll back if it fails. | `crates/limb-builds/src/relaunch.rs` |
| Rehearsal | Starting the new build on a copy of the database before a relaunch, so a build that won't start changes nothing. | `rehearse` in `relaunch.rs` |

## The plan

| Term | Meaning |
|---|---|
| Structure | One of the six building blocks (S1 to S6): the substrate, the task tree, contexts as values, environments of limbs, replicas and versions, Max's attention. Every requirement should be an instance of one. |
| Milestone, rung | A step in the plan, A to F, named by what Max can do when it lands. B to F are rungs: one structure each, built with full discipline. A is the feature track, built fast on what exists. |
| Spine | The work that moved `main` onto the substrate: milestone B. Not a part of the running system. |
| Spike | A branch that tests one thesis against a few aspects and is never merged; its result carries what it learned. |
| Advocate | The `max-advocate` agent: answers as Max would when he is away, from its model of him. |
| Orchestrator, designer, engineer | This project's agent roles; [`AGENTS.md`](../AGENTS.md#how-the-work-runs) says what each does. |
