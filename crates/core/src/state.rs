//! The brain's fold: every state anyone reads.

use std::collections::{BTreeMap, BTreeSet};

use imbl::{OrdMap, OrdSet};

use crate::attention::{Queued, Ranking};
use crate::call::{Call, CallEvent, Caller, Outcome as CallOutcome, Owner};
use crate::context::{Entry, EntryId};
use crate::delivery::{Content, Delivery, Prompt};
use harness_substrate::{Durability, Folding, Schema};

use crate::build::{Launch, Launched};
use crate::draft::{Draft, DraftEvent, Fate, Sent, Typed};
use crate::environment::{
    Audience, Description, Environment, Limb, LimbKey, LimbKind, LimbRow, Reach,
};
use crate::fact::{Author, Fact, Refusal, Rejected, Stated};
use crate::id::{
    CallId, DeliveryId, EnvironmentId, EventId, LimbId, ModelCallId, ReplicaId, Seq, TaskId, Time,
    Timestamp, ValueId,
};
use crate::model_call::{Ending, ModelCall, ModelCallEvent, Quota, Stop, Usage};
use crate::resource::{Change, Observation, ResourceKey, Version};
use crate::schema::Harness;
use crate::task::{
    Abandoner, Created, Drain, Draining, Life, Mode, Move, Origin, Pause, Paused, Phase, Scope,
    Start, Task, TaskEvent, ends,
};
use crate::value::Bytes;

/// Everything the brain's records say is true now.
///
/// **Concept.** The fold of every record the brain holds, and the only door
/// into the model. [`Harness`]'s fold derives who stated a record, checks
/// the values it names and folds each of its facts ([`crate::schema::Harness`]); the
/// brain's [`harness_substrate::Decider`] is the only caller, and folds into
/// a copy it keeps only once the record is stored. Every view a face, the scheduler or the brain's loop needs is a
/// method here, computed from the folded identities rather than kept beside
/// them, so none can disagree with another: who reads a call is read off the
/// tasks ([`Task::awaited`]), what a task is waiting on is read off its life,
/// and the brain's whole to-do list is [`State::todo`].
///
/// It is persistent: every map shares structure with the state it was
/// folded from, so a clone costs nothing and never changes. That one choice
/// makes a record atomic for every author without rebuilding anything, and
/// lets the brain derive each follower's view from its state as of any of
/// its times.
///
/// It holds the context tree itself ([`State::entry`], [`State::context`]),
/// learned from the facts that carry entries, so a request's head, a
/// breakpoint and a move back are checked against the tree, and a face renders
/// any context from the fold and the value store alone.
///
/// What it guarantees, by refusing every record that would break it: a task
/// is in exactly one [`Life`], and a live one has a context; no request
/// leaves a tool use unanswered, is sent while its task drains, or grows a
/// context other than its task's; a scope's call settles only when every
/// member is done, and nothing joins a closed scope; only Max starts tasks
/// in his scope, and an agent starts only what its environment lists,
/// through its own meta call; a place is filled only by a limb of its kind,
/// and a root routes only to its task's environment and places, and only
/// tools those limbs offer; a call by an agent goes only along its binding;
/// a standing task lives only in Max's scope; an aside joins only the scope
/// its source waits on; a fork starts where its source is; a settled call moves again only from unknown to its real outcome; a
/// meta call is never run twice; a limb never changes kind; a limb states
/// only its own description, its own calls' outcomes and its own resources'
/// changes, and the brain only what is its to say; a quote is Max's words,
/// heard by the task that quotes them; and Max's facts say what he had
/// seen, through the face he used.
///
/// What it leaves to the brain, as policy over these views: what to render
/// and when (notes, wording, breakpoints), handover timing, retry and
/// give-up thresholds, what to do after a stop short of a turn's end,
/// holding siblings until the first one's first byte, and the scheduler's
/// ranking.
///
/// **Vision.** "Strictly events + derived views"; the tree Max watches
/// ([`State::waiting_on`] is the throbber); his queue ([`State::queue`]);
/// restart (a fresh brain folds its records and carries on from
/// [`State::todo`] with nothing else to recover); structured concurrency
/// (stops cascade, scopes close).
///
/// **Existing harnesses.** Each keeps its live state in memory beside its
/// store and reconciles on load: Oh My Pi rebuilds a session's context from
/// its entry tree on resume, Codex reconstructs a thread from its rollout,
/// OpenCode's session status lives in memory only, so a restart forgets
/// which subagents were running. Here there is no live state but the fold.
///
/// **Bx.** Shared: a state that is a value, named by the brain time it
/// was folded to, which anyone can hold without it changing under them.
/// Not shared: it is not content-addressed or stored; it is always
/// re-derivable from the records the brain holds, which are the only thing
/// kept.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    last: Option<Seq>,
    launch: Option<(Mark, Launched)>,
    /// When the current launch must have exited, once it is shutting down.
    shutdown: Option<Timestamp>,
    entries: OrdMap<EntryId, Entry>,
    tasks: OrdMap<TaskId, Task>,
    calls: OrdMap<CallId, CallRecord>,
    model_calls: OrdMap<ModelCallId, ModelCallRecord>,
    deliveries: OrdMap<DeliveryId, Delivered>,
    environments: OrdMap<EnvironmentId, Environment>,
    limbs: OrdMap<LimbId, Limb>,
    described: OrdMap<LimbId, Described>,
    reach: OrdMap<LimbId, Reached>,
    resources: OrdMap<ResourceKey, Current>,
    /// Max's words, each with when he first said them.
    said: OrdMap<ValueId, Mark>,
    /// Which of Max's words each task has heard. A task continuing another's
    /// context (a fork, resume or aside) starts with its source's set, shared.
    heard: OrdMap<TaskId, OrdSet<ValueId>>,
    ranking: Ranking,
    focus: OrdMap<ReplicaId, TaskId>,
    /// The replicas the brain accepted as Max's faces.
    faces: OrdSet<ReplicaId>,
    drafts: OrdMap<DeliveryId, Draft>,
    /// Every value a kept record carried, so a later fact may name it
    /// without carrying it again. A value only a collected record carried
    /// ([`Durability::Latest`]) is not here: a fact naming it carries it.
    kept_values: OrdSet<ValueId>,
    limited_until: Option<Timestamp>,
    open: Open,
}

/// What is still in motion, kept as the fold goes, so the views over live
/// work ([`State::todo`], settlement, the members of a scope) cost what
/// live work costs rather than what all history costs. Derived from the
/// facts in [`State::apply`], never stated beside them.
#[derive(Clone, Debug, Default, PartialEq)]
struct Open {
    /// Tasks not done.
    tasks: OrdSet<TaskId>,
    /// Calls not settled.
    calls: OrdSet<CallId>,
    /// Model calls not ended.
    model_calls: OrdSet<ModelCallId>,
    /// Every task of each scope, in order of creation.
    scopes: OrdMap<Scope, Vec<TaskId>>,
    /// The model call each response came from.
    responses: OrdMap<EntryId, ModelCallId>,
    /// Each task's last model call to end.
    last_turn: OrdMap<TaskId, ModelCallId>,
    /// Tasks told they are lingering ([`Prompt::Lingering`]).
    lingered: OrdSet<TaskId>,
    /// Asides told the rest of their scope is done ([`Prompt::Unblocked`]).
    unblocked: OrdSet<TaskId>,
    /// The last model call to report the quota.
    quota: Option<ModelCallId>,
}

/// Where and when something was recorded. The position orders it against
/// every other fact (was this sent before the current connection?); the
/// time is for people.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark {
    pub seq: Seq,
    pub at: Timestamp,
}

impl Mark {
    pub(crate) fn of(record: &Stated) -> Mark {
        Mark {
            seq: record.seq,
            at: record.at,
        }
    }
}

/// A call and where it is in its life.
#[derive(Clone, Debug, PartialEq)]
pub struct CallRecord {
    pub call: Call,
    /// The limb that runs it: its caller's, or for an agent, its task's
    /// binding's route for the tool. `None` when the binding has no such
    /// tool, which the brain answers as a failed call
    /// ([`Todo::Unroutable`]).
    pub limb: Option<LimbId>,
    pub made: Mark,
    /// Who it is for now: its caller's, until it is passed on.
    pub owner: Owner,
    /// Each time it was passed up the tree, oldest first.
    pub passed: Vec<Handed>,
    /// Its owner no longer wants it ([`CallEvent::Abandon`]).
    pub abandoned: bool,
    pub dispatched: Option<Dispatch>,
    /// When its limb was last asked to stop it, and by when.
    pub stopping: Option<CallStop>,
    pub settled: Option<Settlement>,
    /// The real outcome of a call first settled as unknown.
    pub late: Option<Settlement>,
}

/// One hand-off of a background call ([`CallEvent::Passed`]): the task that
/// owned it, who owns it since, and the passer's reason, which the receiver
/// is shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handed {
    pub by: TaskId,
    pub to: Owner,
    pub why: String,
    pub at: Mark,
}

impl CallRecord {
    /// The outcome a context is shown: the real one, if it arrived late.
    pub fn outcome(&self) -> Option<&CallOutcome> {
        self.late
            .as_ref()
            .or(self.settled.as_ref())
            .map(|s| &s.outcome)
    }
}

/// A call's stop under way ([`CallEvent::Stopping`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallStop {
    /// When its limb was last asked.
    pub asked: Mark,
    /// When it must be gone: the earliest deadline it has been given.
    pub deadline: Timestamp,
}

/// A call's sends to its limb.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dispatch {
    /// The first send: since when it has been running.
    pub first: Mark,
    /// The latest dispatch.
    pub last: Mark,
    /// The replica (the limb's instance) it was first sent to; `None` for a
    /// limb the brain runs. A later instance with no record of it knows its
    /// state was lost since ([`LimbRow::Call`]).
    pub first_to: Option<ReplicaId>,
}

/// How a call ended, and when.
#[derive(Clone, Debug, PartialEq)]
pub struct Settlement {
    pub outcome: CallOutcome,
    pub at: Mark,
}

/// A request and what came of it.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelCallRecord {
    pub call: ModelCall,
    pub sent: Mark,
    pub first_byte: Option<Mark>,
    pub ended: Option<Ended>,
    /// The entry its response is, for a request that responded.
    pub response: Option<EntryId>,
    /// The calls of its response's tool uses, in order.
    pub calls: Vec<CallId>,
}

/// How a request ended, and when.
#[derive(Clone, Debug, PartialEq)]
pub struct Ended {
    pub at: Mark,
    pub ending: Ending,
    pub usage: Option<Usage>,
    pub quota: Option<Quota>,
}

/// A delivery, when, and by whom.
#[derive(Clone, Debug, PartialEq)]
pub struct Delivered {
    pub delivery: Delivery,
    pub at: Mark,
    pub by: Author,
    /// What Max had seen when he wrote it, for his words.
    pub seen: Option<Time>,
}

/// A plugin limb's description, and when it said it.
#[derive(Clone, Debug, PartialEq)]
pub struct Described {
    pub description: Description,
    pub at: Mark,
}

/// A resource's current version, who moved it there, and what its owner
/// said a context should be shown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Current {
    pub version: Version,
    pub by: Option<CallId>,
    pub content: Option<ValueId>,
    pub at: Mark,
}

/// The brain's connection to a limb, as last recorded.
#[derive(Clone, Debug, PartialEq)]
pub struct Reached {
    pub reach: Reach,
    pub at: Mark,
}

/// Something a task's context embodies that has moved since, and that it
/// has not been told of: one line of its next request's notice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stale {
    pub resource: ResourceKey,
    pub has: Version,
    pub now: Version,
    /// The call that moved it, whose caller the notice names.
    pub by: Option<CallId>,
    /// What its limb said to show rather than announce.
    pub content: Option<ValueId>,
}

/// What a task is waiting on, and since when: the throbber, and the answer
/// to "why is nothing happening?".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Waiting {
    /// On its limbs, before its first root: the plugin limbs it can reach
    /// that have not connected and described themselves (a clone being
    /// made, a process starting). Empty when they all have, and the brain
    /// is rooting it. Each says why its last process failed, if it did
    /// ([`Reach::Failed`]): Max sees why it is stuck, and can stop it.
    Starting {
        limbs: Vec<(LimbId, Option<String>)>,
        since: Timestamp,
    },
    /// On the brain to send its next request.
    Brain {
        since: Timestamp,
    },
    /// Paused by Max, on itself or an ancestor ([`State::pause`]), between
    /// steps of its loop: nothing new of its own starts until he resumes it.
    Paused {
        since: Timestamp,
    },
    /// Parked: it brought its work to a safe point and waits for Max to
    /// resume it.
    Parked {
        since: Timestamp,
    },
    /// On the subscription's limit.
    Limited {
        until: Timestamp,
        since: Timestamp,
    },
    /// On the model: sent at `since`, streaming once its first byte came.
    Model {
        call: ModelCallId,
        since: Timestamp,
        streaming: bool,
    },
    /// On its round's calls: tools, its children (a spawn), attachments. A
    /// face shows a call whose limb is unreached as waiting on reconnect,
    /// and a round that waits on running children ([`State::scoped`]) as one
    /// Max reaches through an aside.
    Calls {
        calls: Vec<CallId>,
        since: Timestamp,
    },
    /// Facing Max, since `since`: its user-facing turn ended or he
    /// interrupted, and the move is his. Whatever it wants from him (a
    /// question, a report, a choice to make) is in its last message; there
    /// is no separate question.
    Max {
        since: Timestamp,
    },
    /// On a change to what it embodies, or the background work it started.
    Listening {
        background: Vec<CallId>,
        since: Timestamp,
    },
    Done {
        since: Timestamp,
    },
}

/// One thing the brain should do now. The whole of its loop: act on each,
/// commit what happened, repeat. Each item stays in the list until the fact
/// that does it is committed, so a restart needs nothing else, and an item
/// an earlier one made moot is refused by the brain rather than checked
/// again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Todo {
    /// Connect to this plugin limb, starting its process if need be: a live
    /// task can reach it, or a call to it is unsettled, and it is not
    /// connected. A limb nobody needs is never on the list, so its process
    /// can be let go.
    Connect(LimbId),
    /// The task is starting and every limb it can reach has connected and
    /// described itself: render its first root from their descriptions and
    /// commit [`Move::Root`].
    Root(TaskId),
    /// Build and send the task's next request, rendering every pending
    /// input that the policy includes (every result, always). If its last
    /// turn stopped short ([`Stop`]), the policy decides how to go on.
    Send(TaskId),
    /// A request an earlier brain sent and never saw end: end it as
    /// [`Ending::Lost`], after which its task sends again.
    Lost(ModelCallId),
    /// Dispatch the call: a plugin's becomes a row of its limb's view
    /// ([`State::limb_rows`]), which the limb holds across reconnections,
    /// so it is dispatched once; for a limb the brain runs, run it and
    /// commit its effects with the dispatch, or for the provider, commit the
    /// dispatch and run it off the brain's thread.
    Dispatch(CallId),
    /// The call's tool is not in its task's binding: settle it as failed,
    /// saying so.
    Unroutable(CallId),
    /// Stop the call. If it was dispatched and its limb is connected, commit
    /// [`CallEvent::Stopping`], with time to stop gracefully before
    /// [`State::stop_by`] where its readers have a deadline; its row in the
    /// limb's view shows the deadline. If its limb is unreached, settle it as
    /// unknown, so a dead limb never holds a task (its real outcome is still
    /// taken if it comes back); if it was never dispatched, settle it as
    /// interrupted. Due again when a reader's deadline comes before the
    /// call's.
    Interrupt(CallId),
    /// Cut the request: its task is abandoned or interrupted, or the
    /// brain's shutdown deadline has come.
    Cut(ModelCallId),
    /// Abandon a member of a scope whose owner is being abandoned: with less
    /// time than the owner has, and again whenever the owner's deadline
    /// comes sooner than the member's.
    Abandon(TaskId),
    /// An abandoned task has nothing of its own left running: deliver
    /// [`Prompt::Tidy`], asking for its tidy-up turn.
    Tidy(TaskId),
    /// An abandoned task's deadline has come: commit [`TaskEvent::Expired`].
    Expire(TaskId),
    /// Max parked the task, it is in the middle of its own loop, and it has
    /// not been asked yet: deliver [`Prompt::Park`].
    Park(TaskId),
    /// The task parked, or was asked to, and Max has resumed it: deliver
    /// [`Prompt::Resume`].
    Resume(TaskId),
    /// Every member of this spawn's scope is done: settle it with their
    /// outcomes.
    Settle(CallId),
    /// Every member of an aside's scope but the asides is done, and it has
    /// not been told: deliver [`Prompt::Unblocked`].
    Unblock(TaskId),
    /// An autonomous task ended its turn with background work running and
    /// has not been told what that means: deliver [`Prompt::Lingering`].
    Linger(TaskId),
}

impl State {
    /// Store a task's new state, and whether it is still open.
    fn put(&mut self, task: Task) {
        if task.is_done() {
            self.open.tasks.remove(&task.id);
        } else {
            self.open.tasks.insert(task.id);
        }
        self.tasks.insert(task.id, task);
    }

    /// The tasks not done.
    fn live(&self) -> impl Iterator<Item = &Task> {
        self.open.tasks.iter().filter_map(|id| self.tasks.get(id))
    }

    /// Fold one record of the brain's, or refuse it: who stated it, from
    /// the replica it came from; the values its facts name, each carried with
    /// it or by a kept record; then each fact, in order. On a refusal this
    /// state may be part way through the record, and the decider drops it.
    pub(crate) fn fold(&mut self, folding: Folding<'_, Harness>) -> Result<(), Refusal> {
        let Folding {
            here,
            clock,
            at,
            said,
        } = folding;
        let refuse = |fact: usize, why| Refusal {
            fact: fact as u32,
            why,
        };
        let mut seen = None;
        for (i, fact) in said.facts.iter().enumerate() {
            if let Fact::Seen(time) = fact {
                // A view of this brain, as of a time it has written.
                if seen.is_some() || time.replica != here.replica || time.n >= here.n {
                    return Err(refuse(i, Rejected::Author));
                }
                seen = Some(*time);
            }
        }
        let by = self
            .author(here.replica, at.replica)
            .ok_or(refuse(0, Rejected::Author))?;
        let given: BTreeMap<ValueId, usize> =
            said.values.iter().map(|v| (v.id(), v.len())).collect();
        for (i, fact) in said.facts.iter().enumerate() {
            for value in fact.values() {
                match given.get(&value) {
                    Some(len) if *len > Bytes::LIMIT => {
                        return Err(refuse(i, Rejected::TooLarge(value)));
                    }
                    Some(_) => {}
                    None if self.kept_values.contains(&value) => {}
                    None => return Err(refuse(i, Rejected::Missing(value))),
                }
            }
        }
        for (i, fact) in said.facts.iter().enumerate() {
            let stated = Stated {
                seq: Seq {
                    n: here.n,
                    i: i as u32,
                },
                at: clock,
                by,
                fact,
                event: at.event(i as u32),
                seen,
            };
            self.apply(&stated).map_err(|why| refuse(i, why))?;
        }
        if Harness::durability(at, said) == Durability::Kept {
            self.kept_values.extend(given.into_keys());
        }
        Ok(())
    }

    /// Fold a record the brain refused, as refused: a draft edit that raced
    /// another face's becomes a draft of its own, named by its event, with
    /// its words, beside the one it raced.
    pub(crate) fn refused(&mut self, folding: Folding<'_, Harness>, why: &Refusal) {
        let Folding { at, said, .. } = folding;
        let Some(Fact::Draft(from, DraftEvent::Text(text))) = said.facts.get(why.fact as usize)
        else {
            return;
        };
        let (Rejected::Concurrent, Some(raced)) = (&why.why, self.drafts.get(from)) else {
            return;
        };
        let id = DeliveryId(at.event(why.fact));
        let draft = Draft {
            id,
            session: raced.session,
            at: raced.at,
            text: Some(*text),
            typed: None,
            sent: None,
            held: None,
            discarded: false,
            from: Some(*from),
        };
        self.drafts.insert(id, draft);
        // The refused record is kept for good, and its words with it.
        self.kept_values
            .extend(said.values.iter().map(Bytes::id).filter(|v| v == text));
    }

    /// Who speaks from `from` to the brain at `here`: the brain itself, the
    /// limb the brain last connected as that replica, or Max at a face.
    fn author(&self, here: ReplicaId, from: ReplicaId) -> Option<Author> {
        if from == here {
            return Some(Author::Brain);
        }
        let limb = self.reach.iter().find_map(|(limb, reached)| {
            matches!(reached.reach, Reach::Connected { instance, .. } if instance == from)
                .then_some(*limb)
        });
        match limb {
            Some(limb) => Some(Author::Limb(limb)),
            None => self.faces.contains(&from).then_some(Author::Max),
        }
    }

    /// Max's draft `id`. Never refused for a race: a Send folds as the
    /// delivery it makes, and when the session cannot take his words the
    /// draft keeps them and says why.
    fn fold_draft(
        &mut self,
        id: DeliveryId,
        event: &DraftEvent,
        record: &Stated,
    ) -> Result<(), Rejected> {
        match event {
            DraftEvent::Opened { session, at } => {
                minted(id.0, record)?;
                if self.drafts.contains_key(&id) || self.deliveries.contains_key(&id) {
                    return Err(Rejected::Exists);
                }
                if !self.tasks.contains_key(session) {
                    return Err(Rejected::Unknown);
                }
                let draft = Draft {
                    id,
                    session: *session,
                    at: *at,
                    text: None,
                    typed: None,
                    sent: None,
                    held: None,
                    discarded: false,
                    from: None,
                };
                self.drafts.insert(id, draft);
            }
            DraftEvent::Text(text) => {
                let draft = self.drafts.get_mut(&id).ok_or(Rejected::Unknown)?;
                let face = record.event.at.replica;
                if let Some(typed) = draft.typed
                    && typed.by != face
                    && record.seen.is_none_or(|seen| seen.n < typed.at.seq.n)
                {
                    return Err(Rejected::Concurrent);
                }
                draft.text = Some(*text);
                draft.typed = Some(Typed {
                    by: face,
                    at: Mark::of(record),
                });
                draft.discarded = false;
            }
            DraftEvent::Send { text, read } => {
                let mut draft = self.drafts.get(&id).ok_or(Rejected::Unknown)?.clone();
                match &draft.sent {
                    Some(sent) if sent.text == *text => {}
                    Some(_) => {
                        let successor = DeliveryId(record.event);
                        let new = Draft {
                            id: successor,
                            session: draft.session,
                            at: draft.at,
                            text: Some(*text),
                            typed: None,
                            sent: None,
                            held: None,
                            discarded: false,
                            from: Some(id),
                        };
                        self.drafts.insert(successor, new);
                    }
                    None => {
                        let delivery = Delivery {
                            id,
                            to: draft.session,
                            content: Content::Words(*text),
                            read: *read,
                        };
                        let fact = Fact::Delivery(delivery.clone());
                        let as_delivery = Stated {
                            fact: &fact,
                            ..record.clone()
                        };
                        draft.text = Some(*text);
                        draft.typed = Some(Typed {
                            by: record.event.at.replica,
                            at: Mark::of(record),
                        });
                        match self.deliver(&delivery, &as_delivery) {
                            Ok(()) => {
                                draft.held = None;
                                draft.sent = Some(Sent {
                                    by: record.event,
                                    text: *text,
                                    at: Mark::of(record),
                                });
                            }
                            Err(why) => draft.held = Some(why),
                        }
                        self.drafts.insert(id, draft);
                    }
                }
            }
            DraftEvent::Discarded => {
                self.drafts.get_mut(&id).ok_or(Rejected::Unknown)?.discarded = true;
            }
        }
        Ok(())
    }

    /// Fold one fact, or refuse it. On a refusal this state may be part
    /// way through it.
    pub(crate) fn apply(&mut self, record: &Stated) -> Result<(), Rejected> {
        if !self.authorised(&record.by, record.fact) {
            return Err(Rejected::Author);
        }
        let mark = Mark::of(record);
        match record.fact {
            Fact::Launched(launched) => {
                if let Launch::Relaunched { by: Some(call), .. } = launched.how
                    && !self.calls.contains_key(&call)
                {
                    return Err(Rejected::Unknown);
                }
                self.launch = Some((mark, launched.clone()));
                self.shutdown = None;
                // A connection belongs to the brain that made it.
                self.reach.clear();
            }
            Fact::Shutdown { within } => {
                if self.launch.is_none() {
                    return Err(Rejected::Unknown);
                }
                let deadline = crate::cancel::deadline(record.at, *within);
                self.shutdown = Some(self.shutdown.map_or(deadline, |d| d.min(deadline)));
            }
            Fact::Task(id, TaskEvent::Created(created)) => {
                minted(id.0, record)?;
                self.create(*id, created, record)?
            }
            Fact::Task(id, event) => self.task_event(*id, event, record)?,
            Fact::ModelCall(id, event) => self.fold_model_call(*id, event, record)?,
            Fact::Call(id, event) => self.fold_call(*id, event, record)?,
            Fact::Delivery(delivery) => {
                minted(delivery.id.0, record)?;
                self.deliver(delivery, record)?
            }
            Fact::Change(change) => self.change(change, record)?,
            Fact::Environment(environment) => {
                if !self.environments.contains_key(&environment.id) {
                    minted(environment.id.0, record)?;
                }
                self.define(environment)?
            }
            Fact::Limb(limb) => {
                if !self.limbs.contains_key(&limb.id) {
                    minted(limb.id.0, record)?;
                }
                if self
                    .limbs
                    .values()
                    .any(|l| l.id != limb.id && l.name == limb.name)
                {
                    return Err(Rejected::Exists);
                }
                if let Some(old) = self.limbs.get(&limb.id)
                    && !old.kind.same(&limb.kind)
                {
                    return Err(Rejected::Redefined);
                }
                self.limbs.insert(limb.id, limb.clone());
            }
            Fact::Described { limb, description } => {
                let defined = self.limbs.get(limb).ok_or(Rejected::Unknown)?;
                if defined.kind.is_brains() {
                    return Err(Rejected::NotForTask);
                }
                if self.connected(*limb).is_none() {
                    return Err(Rejected::Unreached);
                }
                self.described.insert(
                    *limb,
                    Described {
                        description: description.clone(),
                        at: mark,
                    },
                );
            }
            Fact::Reach { limb, reach } => {
                let defined = self.limbs.get(limb).ok_or(Rejected::Unknown)?;
                if defined.kind.is_brains() {
                    return Err(Rejected::NotForTask);
                }
                self.reach.insert(
                    *limb,
                    Reached {
                        reach: reach.clone(),
                        at: mark,
                    },
                );
            }
            Fact::Ranking(ranking) => self.ranking = ranking.clone(),
            Fact::Face(replica) => {
                self.faces.insert(*replica);
            }
            Fact::Seen(_) => {}
            Fact::Draft(id, event) => self.fold_draft(*id, event, record)?,
            Fact::Focus(focus) => {
                let Author::Max = record.by else {
                    return Err(Rejected::Author);
                };
                let face = record.event.at.replica;
                match focus.task {
                    Some(task) => {
                        self.focus.insert(face, task);
                    }
                    None => {
                        self.focus.remove(&face);
                    }
                }
            }
        }
        self.last = Some(record.seq);
        Ok(())
    }

    /// Whether `by` may state `fact`.
    ///
    /// The brain states what it decides and observes, but never Max's words,
    /// his marking a task done, his stop, what his face shows, a task no agent's call created, a
    /// limb's description, how a call a limb was sent ended, or a change to
    /// a resource it does not own (its own limbs' resources, such as the
    /// role files the meta limb reads, are its to state). A limb states only
    /// its own description, how its own dispatched calls ended and how its
    /// own resources changed. Max states his words, his calls and his
    /// answers, his controls over the tree, the tasks of his own scope, what
    /// his own face shows, his presence, and the definitions of
    /// environments and limbs; never what a model or a limb did.
    fn authorised(&self, by: &Author, fact: &Fact) -> bool {
        let brains = |limb: LimbId| {
            self.limbs
                .get(&limb)
                .is_some_and(|l| l.kind.is_brains() && l.kind != LimbKind::Max)
        };
        match by {
            Author::Brain => match fact {
                Fact::Delivery(Delivery {
                    content: Content::Words(_) | Content::Prompt(Prompt::Done),
                    ..
                })
                | Fact::Task(
                    _,
                    TaskEvent::Drain(Drain::Abandon {
                        by: Abandoner::Max, ..
                    }),
                )
                | Fact::Focus(_)
                | Fact::Seen(_)
                | Fact::Draft(..)
                | Fact::Described { .. } => false,
                Fact::Task(_, TaskEvent::Created(created)) => created.by.is_some(),
                Fact::Call(_, CallEvent::Made(call)) => !matches!(call.caller, Caller::Max { .. }),
                Fact::Call(id, CallEvent::Settled(outcome)) => {
                    self.calls.get(id).is_some_and(|c| {
                        let Some(limb) = c.limb.and_then(|l| self.limbs.get(&l)) else {
                            return c.dispatched.is_none();
                        };
                        match &limb.kind {
                            // Max's answer is his; the brain only withdraws the question.
                            LimbKind::Max => matches!(outcome, CallOutcome::Interrupted { .. }),
                            LimbKind::Meta | LimbKind::Attention | LimbKind::Provider => true,
                            // A plugin's outcome is its own, unless it was never
                            // sent, or the brain gives up on it while the limb is
                            // unreached; the limb's real outcome is still taken
                            // when it reconnects.
                            LimbKind::Plugin { .. } => {
                                c.dispatched.is_none()
                                    || (c.settled.is_none()
                                        && *outcome == CallOutcome::Unknown
                                        && self.connected(limb.id).is_none())
                            }
                        }
                    })
                }
                Fact::Change(change) => brains(change.resource.limb),
                _ => true,
            },
            Author::Limb(limb) => match fact {
                Fact::Call(id, CallEvent::Settled(_)) => self
                    .calls
                    .get(id)
                    .is_some_and(|c| c.limb == Some(*limb) && c.dispatched.is_some()),
                Fact::Change(change) => {
                    change.resource.limb == *limb
                        && change.by.is_none_or(|call| {
                            self.calls.get(&call).is_some_and(|c| c.limb == Some(*limb))
                        })
                }
                Fact::Described { limb: of, .. } => of == limb,
                // A limb that holds role definitions states the
                // environments read from them.
                Fact::Environment(env) => env
                    .source
                    .as_ref()
                    .is_some_and(|s| s.resource.limb == *limb),
                _ => false,
            },
            Author::Max => match fact {
                Fact::Task(_, event) => match event {
                    TaskEvent::Created(created) => {
                        created.by.is_none()
                            && (created.scope == Scope::Max
                                || matches!(created.origin, Origin::Aside { .. }))
                    }
                    TaskEvent::Drain(drain) => matches!(
                        drain,
                        Drain::ToMax { .. }
                            | Drain::Abandon {
                                by: Abandoner::Max,
                                ..
                            }
                    ),
                    TaskEvent::Expired => false,
                    TaskEvent::Moved(movement) => matches!(movement, Move::Back { .. }),
                    TaskEvent::ModeSet(_)
                    | TaskEvent::Renamed(_)
                    | TaskEvent::Tuned(_)
                    | TaskEvent::Pause(_) => true,
                },
                Fact::Delivery(delivery) => matches!(
                    delivery.content,
                    Content::Words(_) | Content::Prompt(Prompt::Done)
                ),
                Fact::Call(_, CallEvent::Made(call)) => {
                    matches!(call.caller, Caller::Max { .. })
                }
                Fact::Call(_, CallEvent::Abandon) => true,
                Fact::Call(id, CallEvent::Settled(_)) => self
                    .calls
                    .get(id)
                    .and_then(|c| c.limb)
                    .is_some_and(|l| self.is_max(l)),
                Fact::Change(change) => self.is_max(change.resource.limb),
                Fact::Focus(_) | Fact::Seen(_) | Fact::Draft(..) => true,
                Fact::Environment(_) | Fact::Limb(_) | Fact::Ranking(_) => true,
                Fact::Launched(_)
                | Fact::Shutdown { .. }
                | Fact::ModelCall(..)
                | Fact::Call(..)
                | Fact::Described { .. }
                | Fact::Reach { .. }
                | Fact::Face(_) => false,
            },
        }
    }

    fn define(&mut self, environment: &Environment) -> Result<(), Rejected> {
        if !environment.limbs.iter().all(|l| self.limbs.contains_key(l)) {
            return Err(Rejected::Unknown);
        }
        if self
            .environments
            .values()
            .any(|e| e.id != environment.id && e.name == environment.name)
        {
            return Err(Rejected::Exists);
        }
        let mut places = BTreeSet::new();
        if !environment.places.iter().all(|p| places.insert(p)) {
            return Err(Rejected::Ambiguous);
        }
        self.environments
            .insert(environment.id, environment.clone());
        Ok(())
    }

    /// Max said `words` to `to`.
    fn say(&mut self, words: ValueId, to: TaskId, at: Mark) {
        if !self.said.contains_key(&words) {
            self.said.insert(words, at);
        }
        self.heard.entry(to).or_default().insert(words);
    }

    fn create(&mut self, id: TaskId, created: &Created, record: &Stated) -> Result<(), Rejected> {
        if self.tasks.contains_key(&id) {
            return Err(Rejected::Exists);
        }
        let creator = match created.by {
            Some(by) => Some(self.creator(by, created)?),
            None => None,
        };
        // A stopping task starts nothing.
        if creator.is_some_and(|c| ends(self.tasks.get(&c).and_then(Task::drain))) {
            return Err(Rejected::Draining);
        }
        match &created.origin {
            Origin::Fresh {
                environment,
                places,
            } => self.check_places(*environment, places)?,
            Origin::Aside { from } => self.check_aside(*from, created)?,
            Origin::Fork { .. } | Origin::Resume { .. } => {}
        }
        for quote in &created.mandate.quotes {
            let heard = match creator {
                Some(creator) => self.heard(creator, *quote),
                None => self.said.contains_key(quote),
            };
            if !heard {
                return Err(Rejected::Unheard);
            }
        }
        let mark = Mark::of(record);
        let task = Task::new(id, created, mark, self)?;
        // What its context holds, it has heard.
        if let Origin::Fork { from } | Origin::Resume { from } | Origin::Aside { from } =
            created.origin
            && let Some(heard) = self.heard.get(&from).cloned()
        {
            self.heard.insert(id, heard);
        }
        if creator.is_none() {
            // A mandate Max gives is his words.
            self.say(created.mandate.text, id, mark);
        }
        for quote in &created.mandate.quotes {
            self.heard.entry(id).or_default().insert(*quote);
        }
        self.open.scopes.entry(task.scope).or_default().push(id);
        self.put(task);
        Ok(())
    }

    /// The task whose meta call `by` creates a task, if that call may: it
    /// is still running, it is an agent's call to the meta limb, the new
    /// task joins the scope the call opens or, as a sibling, its caller's
    /// own spawn scope, and it is a fork of the caller, a resume of a task
    /// the caller owned, or fresh in an environment the caller's own
    /// environment starts.
    fn creator(&self, by: CallId, created: &Created) -> Result<TaskId, Rejected> {
        let call = self.calls.get(&by).ok_or(Rejected::Unknown)?;
        if call.settled.is_some() {
            return Err(Rejected::Finished);
        }
        let meta = call
            .limb
            .and_then(|l| self.limbs.get(&l))
            .is_some_and(|l| l.kind == LimbKind::Meta);
        let (Caller::Agent { task: caller, .. } | Caller::Background { task: caller, .. }) =
            call.call.caller
        else {
            return Err(Rejected::CannotStart);
        };
        if !meta {
            return Err(Rejected::CannotStart);
        }
        let task = self.tasks.get(&caller).ok_or(Rejected::Unknown)?;
        let scope = created.scope == Scope::Spawn(by)
            || (created.scope == task.scope && task.scope != Scope::Max);
        if !scope {
            return Err(Rejected::WrongScope);
        }
        let origin = match &created.origin {
            Origin::Fresh { environment, .. } => self
                .environments
                .get(&task.environment)
                .is_some_and(|e| e.starts.contains(environment)),
            Origin::Fork { from } => *from == caller,
            Origin::Resume { from } => self.owner(*from) == Some(caller),
            Origin::Aside { .. } => false,
        };
        if !origin || created.mode == Mode::Standing {
            return Err(Rejected::CannotStart);
        }
        Ok(caller)
    }

    /// An aside joins the scope its source's round waits on while members
    /// of it still run, and is a chat with Max.
    fn check_aside(&self, from: TaskId, created: &Created) -> Result<(), Rejected> {
        let source = self.tasks.get(&from).ok_or(Rejected::Unknown)?;
        let waits = match (created.scope, source.phase()) {
            (Scope::Spawn(call), Some(Phase::Acting(step))) => {
                step.outstanding.contains(&call)
                    && self.members(created.scope).any(|m| !m.is_done())
            }
            _ => false,
        };
        if !waits {
            return Err(Rejected::WrongScope);
        }
        if created.mode != Mode::UserFacing {
            return Err(Rejected::CannotStart);
        }
        Ok(())
    }

    /// Every place of the environment is filled, by a limb of its kind.
    fn check_places(
        &self,
        environment: EnvironmentId,
        places: &BTreeMap<String, LimbId>,
    ) -> Result<(), Rejected> {
        let environment = self
            .environments
            .get(&environment)
            .ok_or(Rejected::Unknown)?;
        if places.len() != environment.places.len() {
            return Err(Rejected::WrongEnvironment);
        }
        for kind in &environment.places {
            let fits = places
                .get(kind)
                .and_then(|l| self.limbs.get(l))
                .is_some_and(|l| matches!(&l.kind, LimbKind::Plugin { kind: k, .. } if k == kind));
            if !fits {
                return Err(Rejected::WrongEnvironment);
            }
        }
        Ok(())
    }

    /// Whether a plugin limb's current description offers `tool` to Max
    /// (`to_max`) or to agents.
    fn offered(&self, limb: LimbId, tool: &str, to_max: bool) -> bool {
        self.described.get(&limb).is_some_and(|d| {
            d.description.tools.iter().any(|t| {
                t.name == tool
                    && match t.audience {
                        Audience::Agent => !to_max,
                        Audience::Max => to_max,
                        Audience::Both => true,
                    }
            })
        })
    }

    /// A task's new root must route only to its environment's limbs and its
    /// places, only tools those limbs offer agents, each tool once, and
    /// attach only the harness's calls along the binding.
    fn check_start(&self, task: &Task, start: &Start) -> Result<(), Rejected> {
        let environment = self
            .environments
            .get(&task.environment)
            .ok_or(Rejected::Unknown)?;
        let mut tools = BTreeSet::new();
        for route in &start.binding.routes {
            let reachable = environment.limbs.contains(&route.limb)
                || task.places.values().any(|l| *l == route.limb);
            if !reachable {
                return Err(Rejected::WrongEnvironment);
            }
            if !tools.insert(route.tool.as_str()) {
                return Err(Rejected::Ambiguous);
            }
            let limb = self.limbs.get(&route.limb).ok_or(Rejected::Unknown)?;
            if !limb.kind.is_brains() && !self.offered(route.limb, &route.tool, false) {
                return Err(Rejected::Unoffered);
            }
        }
        for id in &start.awaits {
            let call = self.calls.get(id).ok_or(Rejected::Unknown)?;
            let Caller::Meta { limb, .. } = call.call.caller else {
                return Err(Rejected::NotForTask);
            };
            if start.binding.limb(&call.call.tool) != Some(limb) {
                return Err(Rejected::WrongEnvironment);
            }
        }
        Ok(())
    }

    fn task_event(
        &mut self,
        id: TaskId,
        event: &TaskEvent,
        record: &Stated,
    ) -> Result<(), Rejected> {
        let task = self.tasks.get(&id).ok_or(Rejected::Unknown)?;
        let mut root = None;
        match event {
            TaskEvent::Moved(Move::Handover { call, start }) => {
                let made = self.calls.get(call).ok_or(Rejected::Unknown)?;
                if !matches!(made.call.caller, Caller::Agent { task, .. } if task == id) {
                    return Err(Rejected::NotForTask);
                }
                self.check_start(task, start)?;
                root = Some(&start.root);
            }
            TaskEvent::Moved(Move::Root(start)) => {
                self.check_start(task, start)?;
                root = Some(&start.root);
            }
            TaskEvent::Moved(Move::Back { to }) => {
                let on_chain = task
                    .context()
                    .is_some_and(|c| self.descends(c.head, *to) && self.descends(*to, c.start));
                if !on_chain || !self.ended_turn(*to) {
                    return Err(Rejected::Detached);
                }
            }
            TaskEvent::Drain(Drain::ToMax { turn }) => {
                let named = self.model_calls.get(turn).ok_or(Rejected::Unknown)?;
                if named.call.task != id {
                    return Err(Rejected::NotForTask);
                }
                let ended = self.model_calls.values().any(|m| {
                    m.call.task == id
                        && m.sent.seq >= named.sent.seq
                        && matches!(
                            m.ended.as_ref().map(|e| &e.ending),
                            Some(Ending::Responded {
                                stop: Stop::EndTurn,
                                ..
                            })
                        )
                });
                if ended {
                    return Err(Rejected::Finished);
                }
                if self.scoped(task) {
                    return Err(Rejected::Scoped);
                }
            }
            TaskEvent::Drain(Drain::Abandon {
                by: Abandoner::Owner,
                within,
            }) => {
                let owner = self.owner(id).and_then(|o| self.tasks.get(&o));
                match owner.and_then(Task::drain) {
                    // Nothing a task owns outlives it.
                    Some(Draining::Abandoning(s)) => {
                        let deadline = crate::cancel::deadline(record.at, *within);
                        if deadline > s.deadline.max(record.at) {
                            return Err(Rejected::Late);
                        }
                    }
                    Some(Draining::Failing(_)) => {}
                    Some(Draining::ToMax) | None => return Err(Rejected::NotForTask),
                }
            }
            _ => {}
        }
        let mut task = task.clone();
        task.apply(record, self)?;
        if let Some(root) = root {
            self.entries.insert(root.id(), Entry::Root(root.clone()));
        }
        self.put(task);
        Ok(())
    }

    /// Whether some request's response is `entry` and ended its model turn.
    fn ended_turn(&self, entry: EntryId) -> bool {
        self.response(entry).is_some_and(|m| {
            matches!(
                &m.ended,
                Some(Ended {
                    ending: Ending::Responded {
                        stop: Stop::EndTurn,
                        ..
                    },
                    ..
                })
            )
        })
    }

    /// The model call a response came from.
    pub fn response(&self, entry: EntryId) -> Option<&ModelCallRecord> {
        self.model_calls.get(self.open.responses.get(&entry)?)
    }

    fn fold_model_call(
        &mut self,
        id: ModelCallId,
        event: &ModelCallEvent,
        record: &Stated,
    ) -> Result<(), Rejected> {
        let mark = Mark::of(record);
        match event {
            ModelCallEvent::Sent(call) => {
                minted(id.0, record)?;
                if self.model_calls.contains_key(&id) {
                    return Err(Rejected::Exists);
                }
                let task = self.tasks.get(&call.task).ok_or(Rejected::Unknown)?;
                if !self.entries.contains_key(&call.base) {
                    return Err(Rejected::Detached);
                }
                let entries = call.entries();
                let on_chain = |at: EntryId| {
                    entries.iter().any(|(e, _)| *e == at) || self.descends(call.base, at)
                };
                if !call.breakpoints.iter().all(|b| on_chain(*b)) {
                    return Err(Rejected::Detached);
                }
                let mut task = task.clone();
                task.apply(record, self)?;
                self.put(task);
                for (entry_id, entry) in entries {
                    self.entries.insert(entry_id, entry);
                }
                self.open.model_calls.insert(id);
                self.model_calls.insert(
                    id,
                    ModelCallRecord {
                        call: call.clone(),
                        sent: mark,
                        first_byte: None,
                        ended: None,
                        response: None,
                        calls: Vec::new(),
                    },
                );
            }
            ModelCallEvent::FirstByte => {
                let sent = self.model_calls.get_mut(&id).ok_or(Rejected::Unknown)?;
                if sent.ended.is_some() {
                    return Err(Rejected::Finished);
                }
                sent.first_byte.get_or_insert(mark);
            }
            ModelCallEvent::Ended {
                ending,
                usage,
                quota,
            } => {
                let sent = self.model_calls.get(&id).ok_or(Rejected::Unknown)?;
                if sent.ended.is_some() {
                    return Err(Rejected::Finished);
                }
                // Calls are made only for a response that completed.
                if !sent.calls.is_empty() && !matches!(ending, Ending::Responded { .. }) {
                    return Err(Rejected::Busy);
                }
                let owner = sent.call.task;
                let response = match ending {
                    Ending::Responded { content, .. } => Some(sent.call.response(*content)),
                    _ => None,
                };
                let mut task = self.tasks.get(&owner).ok_or(Rejected::Unknown)?.clone();
                task.apply(record, self)?;
                self.put(task);
                if let Ending::Limited { until } = ending {
                    self.limited_until = self.limited_until.max(Some(*until));
                }
                let response = response.map(|entry| {
                    let entry_id = entry.id();
                    self.entries.insert(entry_id, entry);
                    self.open.responses.insert(entry_id, id);
                    entry_id
                });
                self.open.model_calls.remove(&id);
                self.open.last_turn.insert(owner, id);
                if quota.is_some() {
                    self.open.quota = Some(id);
                }
                if let Some(sent) = self.model_calls.get_mut(&id) {
                    sent.ended = Some(Ended {
                        at: mark,
                        ending: ending.clone(),
                        usage: *usage,
                        quota: quota.clone(),
                    });
                    sent.response = response;
                }
            }
        }
        Ok(())
    }

    fn fold_call(
        &mut self,
        id: CallId,
        event: &CallEvent,
        record: &Stated,
    ) -> Result<(), Rejected> {
        let mark = Mark::of(record);
        match event {
            CallEvent::Made(call) => {
                minted(id.0, record)?;
                self.make(id, call, record)
            }
            CallEvent::Dispatched => {
                let made = self.calls.get(&id).ok_or(Rejected::Unknown)?;
                if made.settled.is_some() {
                    return Err(Rejected::Finished);
                }
                let limb = made.limb.ok_or(Rejected::Unroutable)?;
                let limb = self.limbs.get(&limb).ok_or(Rejected::Unknown)?;
                let instance = match &limb.kind {
                    LimbKind::Max => return Err(Rejected::ForMax),
                    // Run off the brain's thread; again by a later brain.
                    LimbKind::Provider => None,
                    // Run once, with its effects, so never twice.
                    LimbKind::Meta | LimbKind::Attention if made.dispatched.is_some() => {
                        return Err(Rejected::Exists);
                    }
                    LimbKind::Meta | LimbKind::Attention => None,
                    LimbKind::Plugin { .. } => {
                        Some(self.connected(limb.id).ok_or(Rejected::Unreached)?.0)
                    }
                };
                if !self.acting_on(id, &made.call) {
                    return Err(Rejected::Busy);
                }
                let readers: Vec<&Task> = self.live().filter(|t| t.awaits(id)).collect();
                if made.abandoned || interrupts(id, &made.call, &readers) {
                    return Err(Rejected::Draining);
                }
                let (first, first_to) = made
                    .dispatched
                    .map_or((mark, instance), |d| (d.first, d.first_to));
                if let Some(made) = self.calls.get_mut(&id) {
                    made.dispatched = Some(Dispatch {
                        first,
                        last: mark,
                        first_to,
                    });
                }
                Ok(())
            }
            CallEvent::Stopping { within } => {
                let made = self.calls.get_mut(&id).ok_or(Rejected::Unknown)?;
                if made.settled.is_some() {
                    return Err(Rejected::Finished);
                }
                if made.dispatched.is_none() {
                    return Err(Rejected::Busy);
                }
                let deadline = crate::cancel::deadline(record.at, *within);
                made.stopping = Some(CallStop {
                    asked: mark,
                    deadline: made.stopping.map_or(deadline, |s| s.deadline.min(deadline)),
                });
                Ok(())
            }
            CallEvent::Passed { to, why } => self.pass(id, *to, why, record),
            CallEvent::Abandon => {
                let made = self.calls.get_mut(&id).ok_or(Rejected::Unknown)?;
                if made.settled.is_some() {
                    return Err(Rejected::Finished);
                }
                made.abandoned = true;
                Ok(())
            }
            CallEvent::Settled(outcome) => self.settle(id, outcome, record),
        }
    }

    /// A background call moves one level up the tree, from the task that
    /// owns it to that task's owner, or to Max from a task he started.
    fn pass(&mut self, id: CallId, to: Owner, why: &str, record: &Stated) -> Result<(), Rejected> {
        let made = self.calls.get(&id).ok_or(Rejected::Unknown)?;
        if made.settled.is_some() {
            return Err(Rejected::Finished);
        }
        if made.abandoned || made.stopping.is_some() {
            return Err(Rejected::Draining);
        }
        let Owner::Task(from) = made.owner else {
            return Err(Rejected::NotForTask);
        };
        let passer = self.tasks.get(&from).ok_or(Rejected::Unknown)?;
        if !passer.background.contains(&id) {
            return Err(Rejected::NotForTask);
        }
        let up = match self.owner(from) {
            Some(owner) => Owner::Task(owner),
            None if passer.scope == Scope::Max => Owner::Max,
            None => return Err(Rejected::WrongScope),
        };
        if to != up {
            return Err(Rejected::WrongScope);
        }
        let mut readers = vec![passer.clone()];
        if let Owner::Task(owner) = to {
            readers.push(self.tasks.get(&owner).ok_or(Rejected::Unknown)?.clone());
        }
        for task in &mut readers {
            task.apply(record, self)?;
        }
        for task in readers {
            self.put(task);
        }
        if let Some(made) = self.calls.get_mut(&id) {
            made.owner = to;
            made.passed.push(Handed {
                by: from,
                to,
                why: why.to_owned(),
                at: Mark::of(record),
            });
        }
        Ok(())
    }

    fn make(&mut self, id: CallId, call: &Call, record: &Stated) -> Result<(), Rejected> {
        if self.calls.contains_key(&id) {
            return Err(Rejected::Exists);
        }
        let mut thinking = None;
        let mut reader = None;
        let limb = match &call.caller {
            Caller::Agent { task, .. } => {
                let task = self.tasks.get(task).ok_or(Rejected::Unknown)?;
                if task.is_done() {
                    return Err(Rejected::Finished);
                }
                let context = task.context().ok_or(Rejected::Starting)?;
                match task.phase() {
                    Some(Phase::Thinking(request)) => thinking = Some(*request),
                    _ => return Err(Rejected::Busy),
                }
                context.binding.limb(&call.tool)
            }
            Caller::Background { task, from, name } => {
                let started = self.calls.get(from).ok_or(Rejected::Unknown)?;
                if !matches!(started.call.caller, Caller::Agent { task: by, .. } if by == *task) {
                    return Err(Rejected::NotForTask);
                }
                let task = self.tasks.get(task).ok_or(Rejected::Unknown)?;
                if task.is_done() {
                    return Err(Rejected::Finished);
                }
                let named = |c: &CallId| {
                    self.calls.get(c).is_some_and(|c| {
                        matches!(&c.call.caller, Caller::Background { name: n, .. } if n == name)
                    })
                };
                if !name.is_empty() && task.background.iter().any(named) {
                    return Err(Rejected::Exists);
                }
                let context = task.context().ok_or(Rejected::Starting)?;
                let limb = context.binding.limb(&call.tool);
                if limb.is_none() {
                    return Err(Rejected::Unroutable);
                }
                let mut task = task.clone();
                task.apply(record, self)?;
                reader = Some(task);
                limb
            }
            Caller::Max { limb, chat } => {
                let defined = self.limbs.get(limb).ok_or(Rejected::Unknown)?;
                if !defined.kind.is_brains() && !self.offered(*limb, &call.tool, true) {
                    return Err(Rejected::Unoffered);
                }
                if let Some(chat) = chat {
                    let task = self.tasks.get(chat).ok_or(Rejected::Unknown)?;
                    let routed = task
                        .context()
                        .is_some_and(|c| c.binding.routes.iter().any(|r| r.limb == *limb));
                    if !routed {
                        return Err(Rejected::WrongEnvironment);
                    }
                    let mut task = task.clone();
                    task.apply(record, self)?;
                    reader = Some(task);
                }
                Some(*limb)
            }
            Caller::Meta { call: by, limb } => {
                let by = self.calls.get(by).ok_or(Rejected::Unknown)?;
                if by.settled.is_some() {
                    return Err(Rejected::Finished);
                }
                if !self.limbs.contains_key(limb) {
                    return Err(Rejected::Unknown);
                }
                Some(*limb)
            }
        };
        if let Some(request) = thinking
            && let Some(sent) = self.model_calls.get_mut(&request)
        {
            sent.calls.push(id);
        }
        if let Some(task) = reader {
            self.put(task);
        }
        let owner = match &call.caller {
            Caller::Agent { task, .. } | Caller::Background { task, .. } => Owner::Task(*task),
            Caller::Max { .. } => Owner::Max,
            Caller::Meta { call: by, .. } => self.calls.get(by).map_or(Owner::Max, |c| c.owner),
        };
        self.open.calls.insert(id);
        self.calls.insert(
            id,
            CallRecord {
                call: call.clone(),
                limb,
                made: Mark::of(record),
                owner,
                passed: Vec::new(),
                abandoned: false,
                dispatched: None,
                stopping: None,
                settled: None,
                late: None,
            },
        );
        Ok(())
    }

    fn settle(
        &mut self,
        id: CallId,
        outcome: &CallOutcome,
        record: &Stated,
    ) -> Result<(), Rejected> {
        let made = self.calls.get(&id).ok_or(Rejected::Unknown)?;
        let late = match &made.settled {
            None => false,
            Some(first) if first.outcome == CallOutcome::Unknown && made.late.is_none() => true,
            Some(_) => return Err(Rejected::Finished),
        };
        if !late && self.members(Scope::Spawn(id)).any(|t| !t.is_done()) {
            return Err(Rejected::Busy);
        }
        // Who reads it: whoever awaits it now, or for a late outcome, the
        // context its first outcome reached, if it is still live.
        let readers: Vec<&Task> = if late {
            let reader = match (made.owner, &made.call.caller) {
                (_, Caller::Max { chat, .. }) => *chat,
                (_, Caller::Meta { .. }) | (Owner::Max, _) => None,
                (Owner::Task(task), _) => Some(task),
            };
            (reader.and_then(|t| self.tasks.get(&t)))
                .filter(|t| !t.is_done())
                .into_iter()
                .collect()
        } else {
            // A finished task still reads Max's actions in its chat.
            let chat = match made.call.caller {
                Caller::Max { chat: Some(t), .. } => self.tasks.get(&t).filter(|t| t.is_done()),
                _ => None,
            };
            self.live().chain(chat).filter(|t| t.awaits(id)).collect()
        };
        let mut touched = Vec::with_capacity(readers.len());
        for reader in readers {
            let mut task = reader.clone();
            task.apply(record, self)?;
            touched.push(task);
        }
        for task in touched {
            self.put(task);
        }
        let mark = Mark::of(record);
        // A settlement Max authors is his words, heard by the caller's task.
        if let Author::Max = record.by
            && let Some(output) = outcome.output()
            && let Some(asker) = self.call_owner(id)
        {
            self.say(output.model, asker, mark);
        }
        let settlement = Settlement {
            outcome: outcome.clone(),
            at: mark,
        };
        if let Some(made) = self.calls.get_mut(&id) {
            if late {
                made.late = Some(settlement);
            } else {
                made.settled = Some(settlement);
                self.open.calls.remove(&id);
            }
        }
        Ok(())
    }

    fn deliver(&mut self, delivery: &Delivery, record: &Stated) -> Result<(), Rejected> {
        if self.deliveries.contains_key(&delivery.id) {
            return Err(Rejected::Exists);
        }
        let mut task = self
            .tasks
            .get(&delivery.to)
            .ok_or(Rejected::Unknown)?
            .clone();
        task.apply(record, self)?;
        self.put(task);
        let mark = Mark::of(record);
        match &delivery.content {
            Content::Words(words) => self.say(*words, delivery.to, mark),
            Content::Prompt(Prompt::Lingering) => {
                self.open.lingered.insert(delivery.to);
            }
            Content::Prompt(Prompt::Unblocked) => {
                self.open.unblocked.insert(delivery.to);
            }
            Content::Prompt(Prompt::Done | Prompt::Tidy | Prompt::Park | Prompt::Resume) => {}
        }
        self.deliveries.insert(
            delivery.id,
            Delivered {
                delivery: delivery.clone(),
                at: mark,
                by: record.by,
                seen: record.seen,
            },
        );
        Ok(())
    }

    fn change(&mut self, change: &Change, record: &Stated) -> Result<(), Rejected> {
        if !self.limbs.contains_key(&change.resource.limb) {
            return Err(Rejected::Unknown);
        }
        if let Some(by) = change.by
            && !self.calls.contains_key(&by)
        {
            return Err(Rejected::Unknown);
        }
        let mut woken = Vec::new();
        for task in self.live() {
            if task.mode == Mode::Standing && task.phase() == Some(&Phase::Listening) {
                let mut task = task.clone();
                task.apply(record, self)?;
                woken.push(task);
            }
        }
        for task in woken {
            self.put(task);
        }
        self.resources.insert(
            change.resource.clone(),
            Current {
                version: change.version,
                by: change.by,
                content: change.content,
                at: Mark::of(record),
            },
        );
        Ok(())
    }

    // Reading.

    /// The last position folded.
    pub fn last(&self) -> Option<Seq> {
        self.last
    }

    /// The brain process writing now, and where it began.
    pub fn launch(&self) -> Option<(Mark, &Launched)> {
        self.launch
            .as_ref()
            .map(|(mark, launched)| (*mark, launched))
    }

    /// When the brain writing the journal must have exited, once it is
    /// shutting down ([`Fact::Shutdown`]).
    pub fn shutdown(&self) -> Option<Timestamp> {
        self.shutdown
    }

    pub fn entry(&self, id: EntryId) -> Option<&Entry> {
        self.entries.get(&id)
    }

    /// The context that ends at `head`, root first: exactly what a request
    /// with that head sent, given the values its entries name. Empty for an
    /// entry the brain has never carried.
    pub fn context(&self, head: EntryId) -> Vec<(EntryId, &Entry)> {
        let mut chain = Vec::new();
        let mut next = Some(head);
        while let Some(id) = next {
            let Some(entry) = self.entries.get(&id) else {
                break;
            };
            chain.push((id, entry));
            next = entry.parent();
        }
        chain.reverse();
        chain
    }

    /// Whether `ancestor` is on the chain ending at `entry`, itself included.
    pub fn descends(&self, entry: EntryId, ancestor: EntryId) -> bool {
        let mut next = Some(entry);
        while let Some(id) = next {
            if id == ancestor {
                return true;
            }
            next = self.entries.get(&id).and_then(Entry::parent);
        }
        false
    }

    pub fn task(&self, id: TaskId) -> Option<&Task> {
        self.tasks.get(&id)
    }

    /// Every task, done ones included, in id order.
    pub fn tasks(&self) -> impl Iterator<Item = &Task> {
        self.tasks.values()
    }

    /// The tasks not done.
    pub fn live_tasks(&self) -> impl Iterator<Item = &Task> {
        self.live()
    }

    pub fn call(&self, id: CallId) -> Option<&CallRecord> {
        self.calls.get(&id)
    }

    /// Every call, in id order.
    /// Running background work agents have passed to Max ([`Owner::Max`]):
    /// his to use and stop.
    pub fn handed(&self) -> impl Iterator<Item = (CallId, &CallRecord)> {
        (self.open.calls.iter())
            .filter_map(|id| Some((*id, self.calls.get(id)?)))
            .filter(|(_, r)| r.owner == Owner::Max && !r.passed.is_empty())
    }

    pub fn calls(&self) -> impl Iterator<Item = (CallId, &CallRecord)> {
        self.calls.iter().map(|(id, record)| (*id, record))
    }

    pub fn model_call(&self, id: ModelCallId) -> Option<&ModelCallRecord> {
        self.model_calls.get(&id)
    }

    /// Every request, in id order.
    pub fn model_calls(&self) -> impl Iterator<Item = (ModelCallId, &ModelCallRecord)> {
        self.model_calls.iter().map(|(id, record)| (*id, record))
    }

    pub fn delivery(&self, id: DeliveryId) -> Option<&Delivered> {
        self.deliveries.get(&id)
    }

    /// Every delivery, in id order.
    pub fn deliveries(&self) -> impl Iterator<Item = &Delivered> {
        self.deliveries.values()
    }

    /// When Max first said `words`, if they are his.
    pub fn said(&self, words: ValueId) -> Option<Mark> {
        self.said.get(&words).copied()
    }

    /// Whether `task` has heard Max say `words`: sent to it, given to it as
    /// its mandate, answered to its question, or quoted in its mandate. What
    /// it may quote on.
    pub fn heard(&self, task: TaskId, words: ValueId) -> bool {
        self.heard.get(&task).is_some_and(|w| w.contains(&words))
    }

    pub fn environment(&self, id: EnvironmentId) -> Option<&Environment> {
        self.environments.get(&id)
    }

    pub fn environments(&self) -> impl Iterator<Item = &Environment> {
        self.environments.values()
    }

    pub fn limb(&self, id: LimbId) -> Option<&Limb> {
        self.limbs.get(&id)
    }

    /// Every limb, in id order.
    pub fn limbs(&self) -> impl Iterator<Item = &Limb> {
        self.limbs.values()
    }

    /// The limb of this name: a workspace by its directory.
    pub fn limb_named(&self, name: &str) -> Option<&Limb> {
        self.limbs.values().find(|l| l.name == name)
    }

    /// What a plugin limb last said about itself, and when.
    pub fn described(&self, limb: LimbId) -> Option<&Described> {
        self.described.get(&limb)
    }

    pub fn reach(&self, id: LimbId) -> Option<&Reached> {
        self.reach.get(&id)
    }

    /// What `limb` holds of the brain: each call dispatched to it and not
    /// settled, with its stop's deadline; the resources of its that live
    /// contexts embody, with every version the brain holds of each (its
    /// current one where one is recorded, otherwise each a live context
    /// embodies); the environments read from its resources; and the
    /// brain's shutdown. The bytes of the values the
    /// call rows name ([`LimbRow::Value`]) are the store's, so the brain
    /// adds them.
    pub fn limb_rows(&self, limb: LimbId) -> BTreeMap<LimbKey, LimbRow> {
        let mut rows: BTreeMap<LimbKey, LimbRow> = self
            .open
            .calls
            .iter()
            .filter_map(|id| Some((*id, self.calls.get(id)?)))
            .filter(|(_, c)| c.limb == Some(limb))
            .filter_map(|(id, c)| {
                let row = LimbRow::Call {
                    call: c.call.clone(),
                    stopping: c.stopping.map(|s| s.deadline),
                    first_sent_to: c.dispatched?.first_to?,
                };
                Some((LimbKey::Call(id), row))
            })
            .collect();
        if let Some(definition) = self.limbs.get(&limb) {
            rows.insert(LimbKey::Definition, LimbRow::Definition(definition.clone()));
        }
        let mut known: Vec<Observation> = Vec::new();
        for context in self.live().filter_map(Task::context) {
            for (resource, embodied) in context.knows.iter().filter(|(r, _)| r.limb == limb) {
                let version = self.resource(resource).map_or(embodied.has, |c| c.version);
                let observation = Observation {
                    resource: resource.clone(),
                    version,
                };
                if !known.contains(&observation) {
                    known.push(observation);
                }
            }
        }
        if !known.is_empty() {
            rows.insert(LimbKey::Watch, LimbRow::Watch { known });
        }
        for env in self.environments.values() {
            if env.source.as_ref().is_some_and(|s| s.resource.limb == limb) {
                rows.insert(
                    LimbKey::Environment(env.id),
                    LimbRow::Environment(env.clone()),
                );
            }
        }
        rows
    }

    /// The replica a limb is connected as under this brain, and since when.
    pub fn connected(&self, limb: LimbId) -> Option<(ReplicaId, Mark)> {
        match self.reach.get(&limb)? {
            Reached {
                reach: Reach::Connected { instance, .. },
                at,
            } => Some((*instance, *at)),
            Reached {
                reach: Reach::Lost | Reach::Failed { .. },
                ..
            } => None,
        }
    }

    /// Why a limb's process ended before it connected, if its last one did.
    pub fn failed(&self, limb: LimbId) -> Option<&str> {
        match &self.reach.get(&limb)?.reach {
            Reach::Failed { why } => Some(why),
            _ => None,
        }
    }

    pub fn resource(&self, key: &ResourceKey) -> Option<&Current> {
        self.resources.get(key)
    }

    pub fn ranking(&self) -> &Ranking {
        &self.ranking
    }

    /// The task a face shows Max.
    pub fn focus(&self, face: ReplicaId) -> Option<TaskId> {
        self.focus.get(&face).copied()
    }

    /// Whether the brain accepted `replica` as one of Max's faces.
    pub fn is_face(&self, replica: ReplicaId) -> bool {
        self.faces.contains(&replica)
    }

    pub fn draft(&self, id: DeliveryId) -> Option<&Draft> {
        self.drafts.get(&id)
    }

    /// Every draft, sent or not, in id order.
    pub fn drafts(&self) -> impl Iterator<Item = &Draft> {
        self.drafts.values()
    }

    /// What became of the Send at `send` of `draft`'s `text`, once the
    /// brain has folded it.
    pub fn fate(&self, draft: DeliveryId, send: EventId, text: ValueId) -> Option<Fate> {
        let held = self.drafts.get(&draft)?;
        if self.drafts.contains_key(&DeliveryId(send)) {
            return Some(Fate::Successor);
        }
        match &held.sent {
            Some(sent) if sent.by == send => Some(Fate::Delivered),
            Some(sent) if sent.text == text => Some(Fate::Duplicate),
            None if held.held.is_some() => Some(Fate::Held),
            _ => None,
        }
    }

    /// Until when the account is limited, if it is at `now`.
    pub fn limited(&self, now: Timestamp) -> Option<Timestamp> {
        self.limited_until.filter(|until| *until > now)
    }

    /// The task that owns this one: the caller of its scope's spawn. `None`
    /// in Max's scope.
    pub fn owner(&self, task: TaskId) -> Option<TaskId> {
        match self.tasks.get(&task)?.scope {
            Scope::Max => None,
            Scope::Spawn(call) => self.call_owner(call),
        }
    }

    /// The task a call belongs to now ([`Owner`]): its agent, the task it
    /// was passed to, or for the harness's call, the task whose call it runs
    /// for. `None` for Max's: his own, and what was passed to him.
    pub fn call_owner(&self, call: CallId) -> Option<TaskId> {
        match self.calls.get(&call)?.owner {
            Owner::Task(task) => Some(task),
            Owner::Max => None,
        }
    }

    /// How the task's last model call to end, ended.
    pub fn last_turn(&self, task: TaskId) -> Option<&Ended> {
        let id = self.open.last_turn.get(&task)?;
        self.model_calls.get(id)?.ended.as_ref()
    }

    /// The last report of the account's quota.
    pub fn quota(&self) -> Option<&Ended> {
        self.model_calls
            .get(self.open.quota.as_ref()?)?
            .ended
            .as_ref()
    }

    /// The tasks in a scope, in order of creation.
    pub fn members(&self, scope: Scope) -> impl Iterator<Item = &Task> {
        let ids = self.open.scopes.get(&scope).map_or(&[][..], Vec::as_slice);
        ids.iter().filter_map(|id| self.tasks.get(id))
    }

    /// Whether Max has paused the task: the newest of what he said about
    /// pausing it or any of its ancestors, with the task he said it on.
    /// `None` when nothing pauses it.
    pub fn pause(&self, task: TaskId) -> Option<(Pause, TaskId)> {
        let mut newest: Option<(Paused, TaskId)> = None;
        let mut next = Some(task);
        while let Some(id) = next {
            if let Some(said) = self.tasks.get(&id).and_then(|t| t.pause)
                && newest.is_none_or(|(n, _)| said.at > n.at)
            {
                newest = Some((said, id));
            }
            next = self.owner(id);
        }
        newest
            .filter(|(said, _)| said.pause != Pause::Off)
            .map(|(said, id)| (said.pause, id))
    }

    /// Whether nothing new of the task's own may start now, because Max
    /// paused it: no request, and no dispatch of its calls. An abandoned
    /// task's tidy-up turn runs paused or not, and a task asked to park runs
    /// until it has.
    pub fn gated(&self, task: &Task) -> bool {
        let abandoning = matches!(task.drain(), Some(Draining::Abandoning(_)));
        let parking = task.live().is_some_and(|l| l.parking.is_some());
        match self.pause(task.id) {
            None => false,
            Some((Pause::Park, _)) if parking => false,
            Some(_) => !abandoning,
        }
    }

    /// The limbs a task can reach now: its environment's, as currently
    /// defined, and its places. A new root is made from these.
    pub fn limbs_of(&self, task: &Task) -> Vec<LimbId> {
        let mut limbs: Vec<LimbId> = self
            .environments
            .get(&task.environment)
            .map(|e| e.limbs.clone())
            .unwrap_or_default();
        limbs.extend(task.places.values().copied());
        limbs
    }

    /// Whether the brain can use a limb now: one it runs itself, or a plugin
    /// connected to it that has described itself on that connection.
    fn reached(&self, limb: LimbId) -> bool {
        match self.limbs.get(&limb) {
            Some(l) if l.kind.is_brains() => true,
            Some(_) => match (self.connected(limb), self.described.get(&limb)) {
                (Some((_, since)), Some(described)) => described.at.seq >= since.seq,
                _ => false,
            },
            None => false,
        }
    }

    /// Whether the task's round waits on a scope with members still running,
    /// so the move cannot become Max's until it closes: a face offers an
    /// aside instead of an interrupt.
    pub fn scoped(&self, task: &Task) -> bool {
        match task.phase() {
            Some(Phase::Acting(step)) => step
                .outstanding
                .iter()
                .any(|c| self.members(Scope::Spawn(*c)).any(|m| !m.is_done())),
            _ => false,
        }
    }

    /// Whether a limb is Max: his to settle calls on and change resources
    /// of ([`LimbKind::Max`]).
    fn is_max(&self, limb: LimbId) -> bool {
        self.limbs
            .get(&limb)
            .is_some_and(|l| l.kind == LimbKind::Max)
    }

    /// Whether a call waits undispatched because Max paused the task whose
    /// work it is. His own calls are never withheld.
    fn withheld(&self, call: CallId) -> bool {
        (self.call_owner(call))
            .and_then(|t| self.tasks.get(&t))
            .is_some_and(|t| self.gated(t))
    }

    /// A tool use is acted on only once its response is complete and its
    /// task is acting on it.
    fn acting_on(&self, id: CallId, call: &Call) -> bool {
        match call.caller {
            Caller::Agent { task, .. } => self.tasks.get(&task).is_some_and(|t| {
                matches!(t.phase(), Some(Phase::Acting(step)) if step.outstanding.contains(&id))
            }),
            _ => true,
        }
    }

    /// What a task is waiting on now.
    pub fn waiting_on(&self, task: TaskId, now: Timestamp) -> Option<Waiting> {
        let t = self.tasks.get(&task)?;
        let since = t.since;
        let live = match &t.life {
            Life::Starting => {
                return Some(Waiting::Starting {
                    limbs: self
                        .limbs_of(t)
                        .into_iter()
                        .filter(|l| !self.reached(*l))
                        .map(|l| (l, self.failed(l).map(str::to_owned)))
                        .collect(),
                    since,
                });
            }
            Life::Done { .. } => return Some(Waiting::Done { since }),
            Life::Live(live) => live,
        };
        Some(match &live.phase {
            Phase::Ready if self.gated(t) => Waiting::Paused { since },
            Phase::Ready => match self.limited(now) {
                Some(until) => Waiting::Limited { until, since },
                None => Waiting::Brain { since },
            },
            Phase::Thinking(call) => {
                let sent = self.model_calls.get(call)?;
                Waiting::Model {
                    call: *call,
                    since: sent.sent.at,
                    streaming: sent.first_byte.is_some(),
                }
            }
            Phase::Acting(step) => Waiting::Calls {
                calls: step.outstanding.clone(),
                since,
            },
            Phase::AwaitingMax => Waiting::Max {
                since: live.owed_since.unwrap_or(since),
            },
            Phase::Listening => Waiting::Listening {
                background: t.background.clone(),
                since,
            },
            Phase::Parked => Waiting::Parked { since },
        })
    }

    /// Max's queue, in order: every live task facing him, whose move is his.
    pub fn queue(&self) -> Vec<Queued> {
        let queued = self
            .live()
            .filter_map(|t| {
                Some(Queued {
                    task: t.id,
                    since: t.owed_since()?,
                    why: None,
                })
            })
            .collect();
        self.ranking.order(queued)
    }

    /// What a task's context embodies that has moved since, and that it has
    /// not been told of: the next request's notice.
    pub fn stale(&self, task: &Task) -> Vec<Stale> {
        task.context()
            .into_iter()
            .flat_map(|c| c.knows.iter())
            .filter_map(|(resource, known)| {
                let current = self.resources.get(resource)?;
                let moved = current.version != known.has && Some(current.version) != known.told;
                moved.then(|| Stale {
                    resource: resource.clone(),
                    has: known.has,
                    now: current.version,
                    by: current.by,
                    content: current.content,
                })
            })
            .collect()
    }

    /// Everything the brain should do now.
    ///
    /// Restart is not a mode: it is read off the brain's positions. A request
    /// sent before the current [`Launched`] with no end was lost. A plugin
    /// call is due whenever its limb's current connection (which is always
    /// this brain's) began after the call was last sent, so a restart of
    /// the brain, of the limb, or of the network between them all resend
    /// what is unsettled, and the limb answers each from its own record.
    pub fn todo(&self, now: Timestamp) -> Vec<Todo> {
        let launch = self.launch.as_ref().map(|(mark, _)| mark.seq);
        let lost = |sent: &ModelCallRecord| {
            sent.ended.is_none() && launch.is_some_and(|l| sent.sent.seq < l)
        };
        let mut todo: Vec<Todo> = self
            .open
            .model_calls
            .iter()
            .filter(|id| self.model_calls.get(id).is_some_and(lost))
            .map(|id| Todo::Lost(*id))
            .collect();
        let mut readers: BTreeMap<CallId, Vec<&Task>> = BTreeMap::new();
        let mut needed: BTreeSet<LimbId> = BTreeSet::new();
        let mut starting = Vec::new();
        for task in self.live() {
            for call in task.awaited() {
                readers.entry(call).or_default().push(task);
            }
            let limbs = self.limbs_of(task);
            if task.life == Life::Starting && limbs.iter().all(|l| self.reached(*l)) {
                starting.push(Todo::Root(task.id));
            }
            needed.extend(limbs);
            let lingering = task.mode == Mode::Autonomous
                && !task.background.is_empty()
                && !self.open.lingered.contains(&task.id);
            match (task.phase(), task.drain()) {
                (Some(Phase::Listening), None) if lingering => {
                    todo.push(Todo::Linger(task.id));
                }
                (Some(Phase::Ready), None) if !self.gated(task) && self.limited(now).is_none() => {
                    todo.push(Todo::Send(task.id));
                }
                // The tidy-up turn: asked for once, then sent, paused or not.
                (Some(Phase::Ready), Some(Draining::Abandoning(s))) if !s.hard => {
                    if !s.told {
                        todo.push(Todo::Tidy(task.id));
                    } else if self.limited(now).is_none() {
                        todo.push(Todo::Send(task.id));
                    }
                }
                // Every request but a tidy-up turn's.
                (Some(Phase::Thinking(call)), Some(drain))
                    if !self.model_calls.get(call).is_some_and(lost)
                        && !matches!(drain, Draining::Abandoning(s) if s.tidying && !s.hard) =>
                {
                    todo.push(Todo::Cut(*call));
                }
                _ => {}
            }
            if let Some(Draining::Abandoning(s)) = task.drain()
                && !s.hard
                && now >= s.deadline
            {
                todo.push(Todo::Expire(task.id));
            }
            if let Some(live) = task.live()
                && live.drain.is_none()
            {
                let asked = live.parking.is_some() || live.phase == Phase::Parked;
                match self.pause(task.id) {
                    Some((Pause::Park, _)) if !asked && task.mid_loop() && !self.scoped(task) => {
                        todo.push(Todo::Park(task.id));
                    }
                    None if asked => todo.push(Todo::Resume(task.id)),
                    _ => {}
                }
            }
        }
        // In the order they were made, so a limb is sent one response's
        // calls in the order the model wrote them.
        let mut open: Vec<(&CallId, &CallRecord)> = (self.open.calls.iter())
            .filter_map(|id| self.calls.get(id).map(|record| (id, record)))
            .collect();
        open.sort_by_key(|(_, record)| record.made.seq);
        for (id, record) in open {
            if self.open.scopes.contains_key(&Scope::Spawn(*id)) {
                let owner = self
                    .call_owner(*id)
                    .and_then(|o| self.tasks.get(&o))
                    .and_then(Task::drain);
                let stopping = ends(owner);
                let live: Vec<&Task> = self
                    .members(Scope::Spawn(*id))
                    .filter(|m| !m.is_done())
                    .collect();
                let aside = |m: &&Task| matches!(m.origin, Origin::Aside { .. });
                if live.is_empty() {
                    todo.push(Todo::Settle(*id));
                } else if stopping {
                    let due = |m: &&&Task| match (m.drain(), owner) {
                        (Some(Draining::Abandoning(m)), Some(Draining::Abandoning(o))) => {
                            !m.hard && m.deadline > o.deadline
                        }
                        (drain, _) => !ends(drain),
                    };
                    todo.extend(live.iter().filter(due).map(|m| Todo::Abandon(m.id)));
                } else if live.iter().all(aside) {
                    let unblock = live.iter().filter(|m| !self.open.unblocked.contains(&m.id));
                    todo.extend(unblock.map(|m| Todo::Unblock(m.id)));
                }
                continue;
            }
            let Some(limb) = record.limb.and_then(|l| self.limbs.get(&l)) else {
                todo.push(Todo::Unroutable(*id));
                continue;
            };
            needed.insert(limb.id);
            let connection = self.connected(limb.id).map(|(_, mark)| mark.seq);
            let readers = readers.get(id).map_or(&[][..], Vec::as_slice);
            if record.abandoned || interrupts(*id, &record.call, readers) {
                let due = match (&record.dispatched, &limb.kind) {
                    (None, _) => true,
                    (Some(_), LimbKind::Plugin { .. }) => match connection {
                        Some(_) => record.stopping.is_none_or(|s| {
                            needed_by(readers).is_some_and(|by| s.deadline > by.max(s.asked.at))
                        }),
                        None => true,
                    },
                    (Some(_), _) => false,
                };
                if due {
                    todo.push(Todo::Interrupt(*id));
                }
                continue;
            }
            let due = match &limb.kind {
                LimbKind::Max => false,
                // It only reads, so one an earlier brain was running is run again.
                LimbKind::Provider => record
                    .dispatched
                    .is_none_or(|d| launch.is_some_and(|l| d.last.seq < l)),
                LimbKind::Meta | LimbKind::Attention => record.dispatched.is_none(),
                LimbKind::Plugin { .. } => connection.is_some() && record.dispatched.is_none(),
            };
            if due && self.acting_on(*id, &record.call) && !self.withheld(*id) {
                todo.push(Todo::Dispatch(*id));
            }
        }
        let connect = needed.into_iter().filter(|l| {
            self.limbs.get(l).is_some_and(|limb| !limb.kind.is_brains())
                && self.connected(*l).is_none()
        });
        todo.extend(connect.map(Todo::Connect));
        todo.extend(starting);
        // A brain shutting down starts nothing, and cuts what it has not
        // been able to finish by its deadline.
        if let Some(deadline) = self.shutdown {
            todo.retain(|t| {
                !matches!(
                    t,
                    Todo::Send(_) | Todo::Dispatch(_) | Todo::Root(_) | Todo::Connect(_)
                )
            });
            if now >= deadline {
                for task in self.live() {
                    if let Some(Phase::Thinking(call)) = task.phase()
                        && !self.model_calls.get(call).is_some_and(lost)
                        && !todo.contains(&Todo::Cut(*call))
                    {
                        todo.push(Todo::Cut(*call));
                    }
                }
            }
        }
        todo
    }

    /// By when every task reading the call needs it gone: the latest of
    /// their stops' deadlines, or `None` where one has none (an interrupt, a
    /// failure), when the brain's grace applies.
    pub fn stop_by(&self, call: CallId) -> Option<Timestamp> {
        let readers: Vec<&Task> = self.live().filter(|t| t.awaits(call)).collect();
        needed_by(&readers)
    }
}

fn needed_by(readers: &[&Task]) -> Option<Timestamp> {
    let deadlines = readers.iter().map(|t| match t.drain() {
        Some(Draining::Abandoning(s)) => Some(s.deadline),
        _ => None,
    });
    deadlines.collect::<Option<Vec<_>>>()?.into_iter().max()
}

/// Whether every live task reading this call is draining in a way that
/// stops it. An interrupt stops the round, not background work; a stop
/// stops both, but for the calls of its tidy-up turn, until its deadline. A
/// shared attachment runs on while any sibling still wants it. Max's own
/// calls are never stopped for an agent.
fn interrupts(id: CallId, call: &Call, readers: &[&Task]) -> bool {
    if matches!(call.caller, Caller::Max { .. }) || readers.is_empty() {
        return false;
    }
    readers.iter().all(|t| match t.drain() {
        None => false,
        Some(Draining::ToMax) => {
            matches!(t.phase(), Some(Phase::Acting(step)) if step.outstanding.contains(&id))
        }
        Some(Draining::Abandoning(s)) => !s.tidying || s.hard,
        Some(Draining::Failing(_)) => true,
    })
}

/// Whether `id`, which `record` creates, is `record`'s own event: an id is
/// made only by the fact that creates its thing.
fn minted(id: EventId, record: &Stated) -> Result<(), Rejected> {
    if id == record.event {
        Ok(())
    } else {
        Err(Rejected::Minted)
    }
}
