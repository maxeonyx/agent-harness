//! The agent harness's core model: the identities the harness is made of,
//! the facts about them, and the folds that say what is true now.
//!
//! Everything else (the brain's loop, limbs, faces, the scheduler) is
//! written in terms of these types. Each concept exists once here; a second
//! mechanism for one of them is a sign the model is wrong.
//!
//! # Identities
//!
//! | Identity | Type | Its facts | What is true now |
//! |---|---|---|---|
//! | The journal | [`journal::Record`] | everything, in order, through [`journal::Journal::commit`] | [`state::State`] |
//! | A writer of it | [`journal::Launched`] | one record | [`state::State::launch`] |
//! | A value | [`id::ValueId`] | none: immutable [`value::Bytes`], arriving in the [`journal::Batch`] that first names them | itself |
//! | A context | [`context::Entry`], named by [`context::EntryId`] | none: carried inside the fact that first sends or receives it | [`state::State::context`] |
//! | A task | [`task::Task`] | [`task::TaskEvent`], and the facts below that concern it | [`task::Task`], folded by [`state::State::apply`] |
//! | A request | [`model_call::ModelCall`] | [`model_call::ModelCallEvent`] | [`state::ModelCallRecord`] |
//! | A call | [`call::Call`] | [`call::CallEvent`] | [`state::CallRecord`] |
//! | A delivery | [`delivery::Delivery`] | made once | the task's pending inputs |
//! | An environment: a role | [`environment::Environment`] | each new version | [`state::State::environment`] |
//! | A limb | [`environment::Limb`] | each new version; [`environment::Reach`] | [`state::State::limb`], [`state::State::connected`] |
//! | A resource | [`resource::ResourceKey`] | [`resource::Change`] | [`state::State::resource`], [`state::State::stale`] |
//! | Max's attention | [`attention::Ranking`], [`attention::Focus`] | each new one | [`state::State::queue`] |
//!
//! A scope is not an identity: it is the spawn call that owns it
//! ([`task::Scope`]). Max's queue, the throbber, who reads a call, what each
//! context is stale on, and the brain's to-do list are views, never stored.
//!
//! There is one store of bytes and one vocabulary. Every byte the journal
//! names (a system prompt, a tool result, a response, Max's pasted image) is
//! a value; entries, calls and settlements are small structure that names
//! values. A limb speaks the journal's own facts
//! ([`environment::FromLimb::Facts`]), so nothing translates between a
//! protocol and the model, and the fold checks a limb's facts as it checks
//! anyone's.
//!
//! # How the vision falls out
//!
//! - **Contexts.** A task's head moves through one immutable Merkle tree of
//!   wire messages, held by the fold. A fork starts on its parent's
//!   response; fresh siblings with the same brief and attachments append the
//!   same entries, so a shared seed is one cache prefix with no bookkeeping;
//!   a handover or a rebuild moves the head to a new root; a resume starts on a
//!   finished task's last entry; a rewind moves back and restores what the
//!   context embodied there; an aside is Max's fork of a busy task. A
//!   request's head is computed from its base and its messages, so it cannot
//!   claim a context its task does not have. Nothing is re-rendered, so
//!   nothing drifts when the harness changes its own code, and the entry id
//!   is the cache key.
//! - **Structured concurrency.** Spawn is a call. A parent waiting on
//!   children is acting on one outstanding call; the scope closes when the
//!   call settles, which it can only do once every member is done; a stop
//!   cascades through [`state::Todo::Stop`]; background work is a call the
//!   task watches and cannot finish while it runs. Members are created in
//!   the batch that dispatches their spawn, so membership is atomic with
//!   the scope. Only Max starts tasks in his scope; an agent starts, through
//!   its own meta call, only forks of itself, resumes of its finished
//!   children, and fresh tasks in the environments its own environment
//!   lists, so a member adding a sibling is held to its own list.
//! - **Environments are roles.** An orchestrator, a designer, an engineer
//!   and the attention scheduler are each an [`environment::Environment`]:
//!   a role prompt, a model and request parameters, the limbs every task of
//!   the kind shares, the [places](environment::Place) each fills with a
//!   limb of its own (an engineer's clone), and what it can start. A task is
//!   in one for life. A fresh task is created before its own limbs exist
//!   and is rooted once they have connected and described themselves, so
//!   the clone's `AGENTS.md`, directory and branch are in its first root. A
//!   definition is a versioned fact, read by the brain from a role file
//!   that is reviewed like code; an edit reaches running tasks as a notice
//!   and takes effect at their next root, and `params` (effort) at their
//!   next request, since they are not part of the cached prefix.
//! - **Max as a participant.** His words are deliveries, his edits in a
//!   chat are calls to the agent's own tools, his controls over the tree are
//!   task events he authors, and each of his facts records how far his face
//!   had got. Waiting on him is derived, so his queue is never out of step
//!   with the tasks.
//! - **Restart.** Not a mode: positions in the journal. A fresh brain folds
//!   the journal and works through [`state::State::todo`]. A request sent
//!   before the current [`journal::Launched`] with no end was lost and is
//!   sent again; every limb a live task can reach is connected again
//!   ([`state::Todo::Connect`]); every unsettled call is sent again to its
//!   limb once the limb has connected to the new brain, and the limb, which
//!   runs each id at most once, answers from its own record; a task whose
//!   clone was being made is rooted once its limb connects. A brain
//!   restart, a limb restart and a dropped connection are one case.
//! - **Context updates and the environment between turns.** Each task knows
//!   the version of everything its context embodies; a change becomes one
//!   notice at its next request, and wakes a standing task that listens.
//!   A watcher's result, a page event and a change to Max's queue are
//!   changes, so nothing an environment says needs an addressee. A limb
//!   reconnecting is told every version the journal holds, so every change
//!   it states is one.
//! - **No permission prompts.** An agent's call names only its tool and is
//!   routed by its task's binding, made with its root's tools from the same
//!   descriptions ([`environment::Binding::new`]), so what an agent cannot
//!   do is what its environment lacks. A tool use the binding does not route
//!   is answered as a failed call ([`state::Todo::Unroutable`]). A parent
//!   filling a child's place can hand it only a limb of the place's kind.
//! - **Integrity.** [`journal::Journal::commit`] is the one writer: it
//!   stamps positions, checks every value a fact names is stored or in the
//!   batch, folds, and only then appends. Each author may state only its own
//!   facts. A refused batch is never written.
//!
//! # Left to the brain
//!
//! Policies, not model: reading role files into environment versions and
//! names into ids; which directory a place's new clone goes in, and reusing
//! the limb of a directory that has one; when to let go of a limb process
//! no task needs; which pending inputs a request renders beyond the
//! results it must; the wording of notes; where breakpoints go; keep-alive
//! and handover timing, an expected-value decision over
//! [`model_call::ModelCall`] records; when to hold a fan-out's siblings
//! (until the first one's first byte); retry and give-up thresholds,
//! including when an account limit or a bad billing claim stops sending;
//! and everything the scheduler decides.
//!
//! # Bx
//!
//! Shared with Bx: an immutable value is named by its hash
//! ([`id::ValueId`] is Bx's artifact identity, plausibly the same store);
//! structure is named by the hash of its encoding over those names (an
//! [`context::EntryId`]); a request is a build record whose input is a
//! content identity and whose output is a value; several records for one
//! input expose nondeterminism instead of hiding it; provenance lives in
//! the record, not in the value.
//!
//! Not shared: a model's output cannot be re-derived, so the record is the
//! history, not a cache of it; inputs are not pinned, a changed input
//! becomes a notice instead of a rebuild; the provider's cache is a store
//! the harness can neither address nor list, so warmth is inferred; nothing
//! is signed; and tasks and calls are mutable identities with lifecycles,
//! which Bx's artifacts are not.
//!
//! # Provisional
//!
//! Each of these is marked where it lives and is Max's to decide:
//!
//! - Acting in a finished chat: Max's calls there are accepted and ride
//!   into a resume of it; his words to it are refused, and the face resumes
//!   with them as the new mandate.
//! - What an environment says between turns is a resource change, never a
//!   delivery ([`resource::Change`]).
//! - A limb may state only how its own dispatched calls ended and how its
//!   own resources changed.
//! - Talking to a parent waiting on its children: an aside
//!   ([`task::Origin::Aside`]), with background scopes as the other path.
//!   An interrupt meanwhile is refused ([`journal::Rejected::Scoped`]) so
//!   the face can offer the aside; under background scopes it would not
//!   arise.
//! - A provisional choice is an ask of Max made in the background, which
//!   queues him without blocking the asker ([`state::State::queue`]); the
//!   other path is a hold that waits for him.
//! - Where role files live (shared with Claude Code's `.claude/agents/`
//!   during the transition, or the harness's own), and whether the
//!   orchestrator's environment holds a shell or only meta tools and Max
//!   ([`environment::Environment`]).
//! - Which agents may ask Max mid-turn ([`environment::LimbKind::Max`]).
//! - An autonomous turn that ends with background work running: it lingers,
//!   told once ([`delivery::Prompt::Lingering`]).
//! - A limb cloned per fork waits for a limb with such state
//!   ([`environment::Fork`]).
//! - No tool is given to every agent implicitly; each comes from a limb.

pub mod attention;
pub mod call;
pub mod context;
pub mod delivery;
pub mod environment;
pub mod id;
pub mod journal;
pub mod live;
pub mod model_call;
pub mod resource;
pub mod state;
pub mod task;
pub mod value;
