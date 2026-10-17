//! The agent harness's core model: the identities the harness is made of,
//! the facts about them, and the folds that say what is true now, as one
//! schema ([`schema::Harness`]) on the data flow substrate
//! (`harness-substrate`).
//!
//! Everything else (the brain's loop, limbs, faces, the scheduler) is
//! written in terms of these types. Each concept exists once here; a second
//! mechanism for one of them is a sign the model is wrong.
//!
//! The brain, every limb process and every browser tab is a replica
//! ([`id::ReplicaId`]) with records of its own. A limb or a face ships its
//! records to the brain; the brain, the one decider, includes each into its
//! own timeline, folds it into [`state::State`] or records in band why not,
//! and ships each follower a view derived from that state
//! ([`state::State::limb_rows`] for a limb). There is no protocol beside
//! this: no run or stop message, no reply, no refusal on a wire.
//!
//! # Identities
//!
//! | Identity | Type | Its facts | What is true now |
//! |---|---|---|---|
//! | A replica: the brain, a limb's instance, a face | [`id::ReplicaId`] | its own records ([`schema::Record`]); the brain includes the others' | the brain's [`state::State`], a persistent value any reader can hold; a follower's view of it |
//! | A build of the harness | [`build::BuildId`] per executable, made from a [`build::Source`]; a version of the brain is the span a [`build::Launched`] opens | [`fact::Fact::Launched`], [`fact::Fact::Shutdown`] | [`state::State::launch`], [`state::State::shutdown`], [`state::State::probation`], [`state::State::superseded`] |
//! | A value | [`id::ValueId`] | none: immutable [`value::Bytes`], arriving in the [`fact::Batch`] that first names them | itself |
//! | Max's words | a value he said | the delivery, mandate or settlement that carried them | [`state::State::said`], [`state::State::heard`] |
//! | A context | [`context::Entry`], named by [`context::EntryId`] | none: carried inside the fact that first sends or receives it, or the [`task::Compaction`] that appends it | [`state::State::context`], [`state::State::checkpoint`], [`state::State::compacted`] |
//! | A task | [`task::Task`] | [`task::TaskEvent`], and the facts below that concern it | [`task::Task`]: its [`task::Life`], folded by the brain's fold ([`schema::Harness`]) |
//! | A request | [`model_call::ModelCall`] | [`model_call::ModelCallEvent`] | [`state::ModelCallRecord`] |
//! | A call | [`call::Call`] | [`call::CallEvent`] | [`state::CallRecord`] |
//! | Max's message | [`id::DeliveryId`]: a [`draft::Draft`], then the [`delivery::Delivery`] it becomes | [`draft::DraftEvent`], from any of his faces | [`state::State::draft`], [`state::State::fate`], the task's pending inputs |
//! | An environment: a role | [`environment::Environment`] | each new version | [`state::State::environment`] |
//! | A limb | [`environment::Limb`], unique by name, a server on its [`environment::Host`] | each new version, never of another kind or host; [`fact::Fact::Described`]; [`environment::Reach`], which names the replica speaking for it and its build | [`state::State::limb`], [`state::State::limb_named`], [`state::State::described`], [`state::State::connected`], [`state::State::limb_rows`] |
//! | A resource | [`resource::ResourceKey`] | [`resource::Change`] | [`state::State::resource`], [`state::State::stale`] |
//! | Max's attention | [`attention::Ranking`], [`attention::Focus`] (the newest per face is kept) | each new one | [`state::State::queue`] |
//! | Max's faces | [`fact::Fact::Face`] | the brain accepting each replica | [`state::State::is_face`]; what each showed him is [`fact::Fact::Seen`] |
//!
//! A scope is not an identity: it is the spawn call that owns it
//! ([`task::Scope`]). Max's queue, the throbber, who reads a call, what each
//! context is stale on, and the brain's to-do list are views, never stored.
//!
//! There is one store of bytes and one vocabulary. Every byte a fact names
//! (a system prompt, a tool result, a response, Max's pasted image) is a
//! value; entries, calls and settlements are small structure that names
//! values. A limb states the same facts as anyone, in records of its own
//! replica, its description included, so nothing translates between a
//! protocol and the model, and the fold checks a limb's facts as it checks
//! anyone's. Who stated a fact is derived from the replica its record came
//! from ([`fact::Author`]), and what it creates is named by the fact
//! itself ([`id::EventId`]), so neither can be forged.
//!
//! # How the vision falls out
//!
//! - **Contexts.** A task's head moves through one immutable Merkle tree of
//!   wire messages, held by the fold. A fork starts on its parent's
//!   response; fresh siblings with the same brief and attachments append the
//!   same entries, so a shared seed is one cache prefix with no bookkeeping;
//!   a compaction moves the head back to the task's latest checkpoint, or to
//!   a new root, and appends what it carries forward, so the agent moves
//!   between phases of its work without losing itself and nothing is
//!   deleted; a resume starts on a finished task's last entry; Max moving it
//!   back puts it at an entry it has been at and restores what the context
//!   embodied there; an aside is Max's fork of a task waiting on
//!   its children, a member of the scope it waits on, so its result reaches
//!   the task with theirs. A request's head is computed from its base
//!   and its messages, so it cannot claim a context its task does not have.
//!   Nothing is re-rendered, so nothing drifts when the harness changes its
//!   own code, and the entry id is the cache key.
//! - **Structured concurrency.** Spawn is a call. A parent waiting on
//!   children is acting on one outstanding call; the scope closes when the
//!   call settles, which it can only do once every member is done;
//!   background work is a call its owner watches and cannot finish while it
//!   runs. Every call has one owner ([`call::Owner`]); an agent hands a
//!   running background call up a level only by saying so, and what reaches
//!   Max is his to stop. Stopping is one protocol at every level, from a task down to a
//!   command's processes and the brain itself ([`cancel`]): a stop carries a
//!   deadline that shrinks on the way down, everything owned confirms its
//!   exit before its owner's tidy-up turn, and the deadline is a hard kill
//!   that needs no cooperation. Max has three controls over a subtree,
//!   inherited down it: pause (nothing new starts, nothing running is cut,
//!   no turn is spent), park (each agent in the middle of its own loop
//!   takes one turn to reach a safe point, then waits; [`task::Pause`]),
//!   and abandon (that protocol: the work is done, but abandoned). Members are created in
//!   the batch that dispatches their spawn, so membership is atomic with
//!   the scope. Only Max starts tasks in his scope; an agent starts, through
//!   its own meta call, only forks of itself, resumes of its finished
//!   children, and fresh tasks in the environments its own environment
//!   lists, so a member adding a sibling is held to its own list.
//! - **Environments are roles.** An orchestrator, a designer, an engineer
//!   and the attention scheduler are each an [`environment::Environment`]:
//!   a role prompt, a model and request parameters, the limbs every task of
//!   the kind shares, the [places](environment::Environment::places) each
//!   fills with a plugin limb of its own of the place's kind (an engineer's
//!   clone), and what it can start. A task is in one for life. A fresh task
//!   is created before its own limbs exist and is rooted once they have
//!   connected and described themselves, so the clone's `AGENTS.md`,
//!   directory and branch are in its first root. A definition is a versioned
//!   fact, read by the brain from a role file that is reviewed like code; an
//!   edit reaches running tasks as a notice and takes effect at their next
//!   root, and `params` (effort) at their next request, since they are not
//!   part of the cached prefix.
//! - **Max acting on a chat's turns.** Retry, edit, going back and
//!   choosing between versions of a reply are one move: the task back at an
//!   entry it has been at ([`task::Move::Back`]), what follows read off the
//!   entry (a turn's end is his move; a request's head, a retry, is the
//!   model's). Every version stays in the tree, so nothing he moves away
//!   from is lost and a face shows the versions as the tree's branches. An
//!   edit is a draft at an entry ([`draft::DraftEvent::Opened`]), staged
//!   like any message and landing as move and words together. Only the
//!   context moves: files stay as the path it left made them, and the
//!   context is told ([`context::Note::Moved`]). The model and effort are a
//!   setting of the task he turns while it works ([`environment::Tuning`]),
//!   recorded on each request, so each response says what it ran at.
//! - **Max as a participant.** His words are deliveries, his edits in a
//!   chat are calls to the agent's own tools, his controls over the tree are
//!   task events he authors, and each of his facts records how far his face
//!   had shown him ([`fact::Fact::Seen`]). Waiting on him is derived, so his queue is never out of step
//!   with the tasks. Whatever he says (a message, a mandate) is a
//!   value the fold remembers as his and as heard by the task he said it to,
//!   so a task, the root one included, can hand it on verbatim as a quote
//!   ([`task::Mandate::quotes`]) and nobody can forge one. A message is a
//!   [`draft::Draft`] from his first keystroke, held by the brain beside its
//!   session, so the phone shows what the PC typed and either can send it;
//!   its sent status is read off the draft and his face's
//!   [`harness_substrate::Standing`], never told.
//! - **Restart.** Not a mode: positions in the brain's timeline. A fresh
//!   brain folds the records it holds, following the verdicts it recorded,
//!   and works through [`state::State::todo`]. A request sent before the
//!   current [`build::Launched`] with no end was lost and is sent again;
//!   every limb a live task can reach is connected again
//!   ([`state::Todo::Connect`]); every unsettled call is a row of its limb's
//!   view, which the limb gets whole on connecting, and the limb runs each id
//!   at most once; every follower ships what the brain's standing of it does
//!   not include, and the brain skips what it already has. A brain restart,
//!   a limb restart and a dropped connection are one case.
//! - **Limbs live on their own.** A plugin limb is a server on its
//!   [host](environment::Host): on the brain's machine, or on another one
//!   through SSH. The brain starts it if it is not running and dials it,
//!   because the brain always knows where its limbs are and a remote one
//!   often cannot reach the brain. After that it lives apart from any
//!   brain: a brain going away, cleanly or not, closes a link and stops
//!   none of its calls, which are its tasks' work; the limb leaves only once
//!   no brain has been connected for 15 minutes. Its state on its host is
//!   its address, its lock and its records at once, so a second start is a
//!   no-op, two builds of one brain reach one instance, and the newest
//!   connection replaces a stale one. A limb serves the brain whose records
//!   define it, so two domains' brains never share a limb process, even on
//!   one machine. Several tasks share a limb as they share a workspace or a
//!   browser; what one task holds inside it (a tab, a sandbox's state) is a
//!   [resource](resource::ResourceKey) of that limb. How much of this is
//!   built yet: [`environment::Host`].
//! - **Upgrades.** An agent whose environment holds the builds limb
//!   relaunches the harness onto a commit with one call: the limb proves
//!   the new build beside the running brain (on a copy of the database it
//!   folds every record, has the provider check every live session's next
//!   request as rendered, and serves a page that loads, and the old build
//!   reopens that copy; a throwaway brain of it completes a tool round trip
//!   on the scripted model), then the running brain drains, the new build takes over the
//!   same database, and the call settles. Every session moves at its next
//!   step, keeping its data and the tool definitions its root declared.
//!   The new build is on [`build::Probation`] until it shows a response and
//!   a limb's round trip; the limb, still the old build, judges that with
//!   its own fold and goes back to the old build's installed bytes on any
//!   failure, so a broken build bricks nothing. Once it is proven, limbs
//!   follow at their next idle moment ([`state::State::superseded`]).
//! - **Context updates and the environment between turns.** Each task knows
//!   the version of everything its context embodies; a change becomes one
//!   notice at its next request, and wakes a standing task that listens.
//!   A watcher's result, a page event and a change to Max's queue are
//!   changes, so nothing an environment says needs an addressee. A limb's
//!   view lists every version the brain holds, so every change it states is
//!   one.
//! - **No permission prompts.** An agent's call names only its tool and is
//!   routed by its task's binding, made with its root's tools from the same
//!   descriptions ([`environment::Binding::new`]), and a root may route only
//!   tools its limbs offer, so what an agent cannot do is what its
//!   environment lacks. A tool use the binding does not route is answered as
//!   a failed call ([`state::Todo::Unroutable`]). A parent filling a child's
//!   place can hand it only a limb of the place's kind.
//! - **Integrity.** The brain's [`schema::Brain`] is the one decider: it
//!   folds each record into a copy of the state (free because the state is
//!   persistent), checking who stated it and that every value its facts
//!   name is carried with it or by a kept record, stores it, and only then
//!   keeps the copy. Each author may state only its own facts. A refused
//!   record of Max's or a limb's is a race, kept in band: held, included,
//!   and refused in the brain's [`harness_substrate::Included`]. A refused
//!   record of the brain's own is a bug and is never written. A face offers
//!   Max only acts its view says are possible; the brain decides the races.
//!
//! # Left to the brain
//!
//! Policies, not model: turning role files (read by a co-hosted limb, since
//! the brain reads nothing but its own records) into environment versions; which directory a place's new clone goes in, and reusing
//! the limb of a directory that has one ([`state::State::limb_named`]); when
//! to let go of a limb process no task needs; which pending inputs a
//! request renders beyond the results it must; the wording of notes; where
//! breakpoints go; keep-alive and compaction timing, an expected-value
//! decision over [`model_call::ModelCall`] records; when to hold a fan-out's
//! siblings (until the first one's first byte); retry and give-up
//! thresholds, including when an account limit or a bad billing claim stops
//! sending; what to do after a stop short of a turn's end
//! ([`model_call::Stop`]); and everything the scheduler decides.
//!
//! # Bx
//!
//! Shared with Bx: an immutable value is named by its hash
//! ([`id::ValueId`] is Bx's artifact identity, plausibly the same store),
//! and Max's words handed on as a quote are that value, not a copy, so
//! provenance is by identity; structure is named by the hash of its
//! encoding over those names (an [`context::EntryId`]); a request is a
//! build record whose input is a content identity and whose output is a
//! value; several records for one input expose nondeterminism instead of
//! hiding it, which is what Max's retry asks for, and a model or effort he
//! changed between them is one input under two definitions; a task's head
//! is a mutable name for an immutable node, as a branch names a commit, and
//! its moves are that name's history; provenance lives in the record, not
//! in the value; and a
//! state, like a Bx snapshot, is an immutable value a reader holds while
//! the writer moves on.
//!
//! The substrate shares with Bx that a record is immutable and keeps its
//! identity wherever it is held (a follower's record is the same record at
//! the brain, as a Bx artifact is the same artifact in every store), and
//! that an id says where it came from: an [`id::EventId`] is provenance, as
//! a Bx hash is.
//!
//! Not shared: a model's output cannot be re-derived, so the record is the
//! history, not a cache of it; inputs are not pinned, a changed input
//! becomes a notice instead of a rebuild; the provider's cache is a store
//! the harness can neither address nor list, so warmth is inferred; nothing
//! is signed; a state is not content-addressed or stored, only re-derived
//! from the brain's records; tasks and calls are mutable identities with
//! lifecycles, which Bx's artifacts are not; going back to an entry does
//! not go back in the world, since the files a context's calls wrote are
//! not versioned with it (restoring them would be an act of its workspace's
//! limb); a limb on another machine runs a build installed there under its
//! hash, as a Bx artifact is, but who may reach it is the host's own access
//! control, outside any definition, as Bx keeps trust outside the build; a
//! replica is a mutable timeline
//! and a time is not a hash, so the same bytes said twice are two records;
//! and records are collected by the durability their schema declares, not
//! by what is still referenced.
//!
//! # Provisional
//!
//! Each of these is marked where it lives and is Max's to decide:
//!
//! - Acting in a finished chat: Max's calls there are accepted and ride
//!   into a resume of it; his words to it are refused, and the face resumes
//!   with them as the new mandate.
//! - Moving a task back leaves the files the path it left wrote as they
//!   are, and tells the context; restoring them to the entry, or giving
//!   each version a workspace of its own, waits on how often Max goes back
//!   in a chat that writes files ([`task::Move::Back`]).
//! - Moving a task back while a spawn on the path it leaves still runs
//!   abandons that spawn's members, after the face names them and Max
//!   confirms, rather than refusing until he abandons them himself.
//! - What an environment says between turns is a resource change, never a
//!   delivery ([`resource::Change`]).
//! - A limb may state only its own description, how its own dispatched
//!   calls ended and how its own resources changed.
//! - Talking to a parent waiting on its children: an aside
//!   ([`task::Origin::Aside`]), the user-facing member of the scope it
//!   waits on, with background scopes as the other path. An interrupt
//!   meanwhile is refused ([`fact::Rejected::Scoped`],
//!   [`state::State::scoped`]) so the face can offer the aside; under
//!   background scopes it would not arise.
//! - An agent needing Max faces him: a user-facing task ends its turn
//!   with what it needs in its message ([`state::Waiting::Max`]); an
//!   autonomous one tells its owner, which faces him or carries it up.
//!   There is no question separate from a message, and no agent
//!   interrupts him mid-turn.
//! - An autonomous turn that ends with background work running: it
//!   lingers, told once
//!   ([`delivery::Prompt::Lingering`]), and decides whether to wait for the
//!   work, stop it, or pass it up ([`call::CallEvent::Passed`]).
//! - Where role files live (shared with Claude Code's `.claude/agents/`
//!   during the transition, or the harness's own), and whether the
//!   orchestrator's environment holds a shell or only meta tools and Max
//!   ([`environment::Environment`]).
//! - No tool is given to every agent implicitly; each comes from a limb.
//! - Sending one draft from two faces at once: the first Send the brain
//!   includes delivers it; a later Send of other words opens a new draft
//!   holding them ([`draft::DraftEvent::Send`]), rather than delivering a
//!   second message.
//! - A Send to a finished session delivers nothing and the draft keeps the
//!   words and says why ([`draft::Draft::held`]), rather than resuming the
//!   session.
//! - A face holds the brain's derived rows, not its facts: it shows its
//!   own pending acts as pending rather than predicting the brain's verdict.

pub mod attention;
pub mod build;
pub mod call;
pub mod cancel;
pub mod context;
pub mod delivery;
pub mod draft;
pub mod environment;
pub mod fact;
pub mod id;
pub mod live;
pub mod model_call;
pub mod resource;
pub mod schema;
pub mod state;
pub mod task;
pub mod value;
