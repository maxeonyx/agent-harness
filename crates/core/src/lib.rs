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
//! | The journal | [`journal::Record`] | everything, in order | [`state::State`] |
//! | A writer of it | [`journal::Launched`] | one record | [`state::State::build`] |
//! | A context | [`context::Entry`], named by [`context::EntryId`] | none: a value, stored by content | itself |
//! | A task | [`task::Task`] | [`task::TaskEvent`], and the facts below that concern it | [`task::Task::apply`] |
//! | A request | [`model_call::ModelCall`] | [`model_call::ModelCallEvent`] | [`state::ModelCallRecord`] |
//! | A call | [`call::Call`] | [`call::CallEvent`] | [`call::Status`] |
//! | A delivery | [`delivery::Delivery`] | made once | the task's pending inputs |
//! | An environment | [`environment::Environment`] | each new version | [`state::State::environment`] |
//! | A limb | [`environment::Limb`] | each new version; [`environment::Reach`] | [`state::State::limb`] |
//! | A resource | [`resource::ResourceKey`] | [`resource::Change`] | [`state::State::stale`] |
//! | Max's attention | [`attention::Ranking`], [`attention::Focus`] | each new one | [`state::State::queue`] |
//!
//! A scope is not an identity: it is the spawn call that owns it
//! ([`task::Scope`]). Max's queue, the throbber, what each context is stale
//! on, and the brain's to-do list are views, never stored.
//!
//! # How the vision falls out
//!
//! - **Contexts.** A task's head moves through one immutable tree of wire
//!   messages. A fork starts on its parent's response; fresh siblings with
//!   the same brief and attachments render the same entries, so a shared
//!   seed is one cache prefix with no bookkeeping; a handover or reseed
//!   moves the head to a new root; a resume starts on a finished task's last
//!   entry; a rewind moves back; an aside is Max's fork of a busy task.
//!   Nothing is re-rendered, so nothing drifts when the harness changes its
//!   own code, and the entry id is the cache key.
//! - **Structured concurrency.** Spawn is a call. A parent waiting on
//!   children is acting on one outstanding call; the scope closes when the
//!   call settles, which it can only do once every member is done; a stop
//!   cascades through [`state::Todo::Stop`]; background work is a call the
//!   task watches and cannot finish while it runs.
//! - **Max as a participant.** His words are deliveries, his edits in a
//!   chat are calls to the agent's own tools, his controls over the tree are
//!   task events he authors, and each of his facts records how far his face
//!   had got. Waiting on him is derived, so his queue is never out of step
//!   with the tasks.
//! - **Restart.** A fresh brain folds the journal and works through
//!   [`state::State::todo`]: a request with no end was lost and is sent
//!   again, a dispatched call is asked about, an unknown outcome is said to
//!   be unknown and may arrive late.
//! - **Context updates.** Each task knows the version of everything its
//!   context embodies; a change becomes one notice at its next request.
//! - **No permission prompts.** An agent's call is accepted only along its
//!   binding's routes, so what it cannot do is what its environment lacks.
//!
//! # Left to the brain
//!
//! Policies, not model: which pending inputs a request renders beyond the
//! results it must; the wording of notes; where breakpoints go; keep-alive
//! and handover timing, an expected-value decision over
//! [`model_call::ModelCall`] records; when to hold a fan-out's siblings
//! (until the first one's first byte); retry and give-up thresholds; and
//! everything the scheduler decides.
//!
//! # Bx
//!
//! Shared with Bx: an immutable value is named by its hash
//! ([`id::Digest`], plausibly the same store); a request is a build record
//! whose input is a content identity and whose output is a value; several
//! records for one input expose nondeterminism instead of hiding it;
//! provenance lives in the record, not in the value.
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
//! - Talking to a parent waiting on its children: an aside
//!   ([`task::Origin::Aside`]), with background scopes as the other path.
//! - Which agents may ask Max mid-turn ([`environment::LimbKind::Max`]).
//! - An autonomous turn that ends with background work running: it lingers,
//!   told once ([`delivery::Prompt::Lingering`]).
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
