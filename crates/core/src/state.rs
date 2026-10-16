//! The fold of the journal: every state anyone reads.

use std::collections::{BTreeMap, BTreeSet};

use imbl::{OrdMap, OrdSet};

use crate::attention::{Queued, Ranking};
use crate::call::{Call, CallEvent, Caller, Outcome as CallOutcome};
use crate::context::{Entry, EntryId};
use crate::delivery::{Content, Delivery, Prompt};
use crate::environment::{Audience, Description, Environment, Limb, LimbKind, Reach};
use crate::id::{
    CallId, DeliveryId, EnvironmentId, FaceId, InstanceId, LimbId, ModelCallId, Seq, AgentId,
    Timestamp, ValueId,
};
use crate::agent::{
    Agent, AgentEvent, Cancel, Canceller, Created, Drain, Life, Mode, Move, Origin, Phase, Scope,
    Start, leaving,
};
use crate::journal::{Author, Fact, Launched, Record, Refusal, Rejected};
use crate::model_call::{Ending, ModelCall, ModelCallEvent, Quota, Stop, Usage};
use crate::resource::{Change, ResourceKey, Version};
use crate::task::{Exit, Owner, Status, Task};

/// Everything the journal says is true now.
///
/// **Concept.** The fold of every record, and the only door into the model.
/// [`State::apply`] folds one record; [`State::with`] folds a batch into a
/// copy, or refuses it and leaves this state as it was;
/// [`crate::journal::Journal`] is the only caller that keeps it in step with
/// a store. Every view a face, the scheduler or the brain's loop needs is a
/// method here, computed from the folded identities rather than kept beside
/// them, so none can disagree with another: who reads a call is read off the
/// agents ([`Agent::awaited`]), what an agent is waiting on is read off its life,
/// and the brain's whole to-do list is [`State::todo`].
///
/// It is persistent: every map shares structure with the state it was
/// folded from, so a clone costs nothing and never changes. That one choice
/// makes a batch atomic for every author without rebuilding anything, lets
/// a face in the brain's process read the brain's state as of any commit
/// instead of folding the journal a second time, and lets a face ask
/// whether Max's act would be accepted ([`State::with`]) before he makes it.
///
/// It holds the context tree itself ([`State::entry`], [`State::context`]),
/// learned from the facts that carry entries, so a request's head, a
/// breakpoint and a rewind are checked against the tree, and a face renders
/// any context from the fold and the value store alone.
///
/// What it guarantees, by refusing every record that would break it: an agent
/// is in exactly one [`Life`], and a live one has a context; no request
/// leaves a tool use unanswered, is sent while its agent drains, or grows a
/// context other than its agent's; a scope's call settles only when every
/// member is done, and nothing joins a closed scope; only Max starts agents
/// in his scope, and an agent starts only what its environment lists,
/// through its own meta call; a place is filled only by a limb of its kind,
/// and a root routes only to its agent's environment and places, and only
/// tools those limbs offer; a call by an agent goes only along its binding;
/// a standing agent lives only in Max's scope; an aside joins only the scope
/// its source waits on; a fork starts where its source is; a settled call moves again only from unknown to its real outcome; a
/// meta call is never run twice; a limb never changes kind; a limb states
/// only its own description, its own calls' outcomes and its own resources'
/// changes, and the brain only what is its to say; a quote is Max's words,
/// heard by the agent that quotes them; and Max's facts say what he had
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
/// restart (a fresh brain folds the journal and carries on from
/// [`State::todo`] with nothing else to recover); structured concurrency
/// (stops cascade, scopes close).
///
/// **Existing harnesses.** Each keeps its live state in memory beside its
/// store and reconciles on load: Oh My Pi rebuilds a session's context from
/// its entry tree on resume, Codex reconstructs a thread from its rollout,
/// OpenCode's session status lives in memory only, so a restart forgets
/// which subagents were running. Here there is no live state but the fold.
///
/// **Bx.** Shared: a state that is a value, named by the journal position it
/// was folded to, which anyone can hold without it changing under them.
/// Not shared: it is not content-addressed or stored; it is always
/// re-derivable from the journal, which is the only thing kept.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    last: Option<Seq>,
    launch: Option<(Mark, Launched)>,
    entries: OrdMap<EntryId, Entry>,
    agents: OrdMap<AgentId, Agent>,
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
    /// Which of Max's words each agent has heard. An agent continuing another's
    /// context (a fork, resume or aside) starts with its source's set, shared.
    heard: OrdMap<AgentId, OrdSet<ValueId>>,
    ranking: Ranking,
    focus: OrdMap<FaceId, AgentId>,
    limited_until: Option<Timestamp>,
    open: Open,
}

/// What is still in motion, kept as the fold goes, so the views over live
/// work ([`State::todo`], settlement, the members of a scope) cost what
/// live work costs rather than what all history costs. Derived from the
/// facts in [`State::apply`], never stated beside them.
#[derive(Clone, Debug, Default, PartialEq)]
struct Open {
    /// Agents not done.
    agents: OrdSet<AgentId>,
    /// Calls not settled.
    calls: OrdSet<CallId>,
    /// Model calls not ended.
    model_calls: OrdSet<ModelCallId>,
    /// Every agent of each scope, in order of creation.
    scopes: OrdMap<Scope, Vec<AgentId>>,
    /// The model call each response came from.
    responses: OrdMap<EntryId, ModelCallId>,
    /// Each agent's last model call to end.
    last_turn: OrdMap<AgentId, ModelCallId>,
    /// Agents told they are lingering ([`Prompt::Lingering`]).
    lingered: OrdSet<AgentId>,
    /// Asides told the rest of their scope is done ([`Prompt::Unblocked`]).
    unblocked: OrdSet<AgentId>,
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
    fn of(record: &Record) -> Mark {
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
    /// Who owns it now: its caller, or for the harness's call the owner of
    /// the call it runs for, until an agent hands it up
    /// ([`CallEvent::Handed`]).
    pub owner: Owner,
    /// The limb that runs it: its caller's, or for an agent, its agent's
    /// binding's route for the tool. `None` when the binding has no such
    /// tool, which the brain answers as a failed call
    /// ([`Todo::Unroutable`]).
    pub limb: Option<LimbId>,
    pub made: Mark,
    pub dispatched: Option<Dispatch>,
    /// When its owner asked for it to be stopped ([`CallEvent::Cancel`]).
    pub cancelled: Option<Mark>,
    /// When its limb was last asked to stop it.
    pub stopping: Option<Mark>,
    pub settled: Option<Settlement>,
    /// The real outcome of a call first settled as unknown.
    pub late: Option<Settlement>,
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

/// A call's sends to its limb.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dispatch {
    /// The first send: since when it has been running.
    pub first: Mark,
    /// The latest send, which decides whether it must be sent again.
    pub last: Mark,
    /// The instance it was last sent to; `None` for a limb the brain runs.
    pub instance: Option<InstanceId>,
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

/// Something an agent's context embodies that has moved since, and that it
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

/// What an agent is waiting on, and since when: the throbber, and the answer
/// to "why is nothing happening?".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Waiting {
    /// On its limbs, before its first root: the plugin limbs it can reach
    /// that have not connected and described themselves (a clone being
    /// made, a process starting). Empty when they all have, and the brain
    /// is rooting it.
    Starting {
        limbs: Vec<LimbId>,
        since: Timestamp,
    },
    /// On the brain to send its next request.
    Brain {
        since: Timestamp,
    },
    /// Held by Max, on itself or an ancestor.
    Held {
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
    /// On Max, since `since`: its turn ended, he interrupted, or it asked.
    Max {
        since: Timestamp,
        ask: Option<CallId>,
    },
    /// Leaving, on what it owns to exit: its calls, and through a spawn
    /// the members of its scope. Empty when everything has, and the brain is
    /// telling it to tidy up.
    Exiting {
        owned: Vec<Task>,
        since: Timestamp,
    },
    /// On a change to what it embodies, or the background work it owns.
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
/// an earlier one made moot is refused by the journal rather than checked
/// again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Todo {
    /// Connect to this plugin limb, starting its process if need be: a live
    /// agent can reach it, or a call to it is unsettled, and it is not
    /// connected. A limb nobody needs is never on the list, so its process
    /// can be let go.
    Connect(LimbId),
    /// The agent is starting and every limb it can reach has connected and
    /// described itself: render its first root from their descriptions and
    /// commit [`Move::Root`].
    Root(AgentId),
    /// Build and send the agent's next request, rendering every pending
    /// input that the policy includes (every result, always). If its last
    /// turn stopped short ([`Stop`]), the policy decides how to go on.
    Send(AgentId),
    /// A request an earlier brain sent and never saw end: end it as
    /// [`Ending::Lost`], after which its agent sends again.
    Lost(ModelCallId),
    /// Send the call to its limb (`ToLimb::Run`), or for a limb the brain
    /// runs, run it and commit its effects with the dispatch. A plugin's
    /// call is due again whenever its limb has connected since it was last
    /// sent.
    Dispatch(CallId),
    /// The call's tool is not in its agent's binding: settle it as failed,
    /// saying so.
    Unroutable(CallId),
    /// Cancel a task whose owner is draining: the cascade step of the
    /// [cancel protocol](crate::task::Task), one item for every kind.
    ///
    /// - A request: stop reading it, which ends it as abandoned.
    /// - A call: if it was dispatched and its limb is connected, ask it
    ///   (`ToLimb::Stop`) and commit [`CallEvent::Stopping`]; if its limb is
    ///   unreached, settle it as unknown, so a dead limb never holds an agent
    ///   (its real outcome is still taken if it comes back); if it was never
    ///   dispatched, settle it as interrupted.
    /// - An agent, a member of a scope whose owner is leaving: commit its
    ///   [`AgentEvent::Cancel`] by [`Canceller::Owner`], with its owner's
    ///   choice of a tidy-up turn ([`State::cancel_for`]).
    Cancel(Task),
    /// A leaving agent's tidy-up turn is due: everything it owned has
    /// exited. Deliver [`Prompt::TidyUp`].
    Tidy(AgentId),
    /// Every member of this spawn's scope is done: settle it with their
    /// outcomes.
    Settle(CallId),
    /// Every member of an aside's scope but the asides is done, and it has
    /// not been told: deliver [`Prompt::Unblocked`].
    Unblock(AgentId),
    /// An autonomous agent ended its turn with background work running (an
    /// unanswered ask of Max included) and has not been told what that
    /// means: deliver [`Prompt::Lingering`].
    Linger(AgentId),
}

impl State {
    /// Store an agent's new state, and whether it is still open.
    fn put(&mut self, agent: Agent) {
        if agent.is_done() {
            self.open.agents.remove(&agent.id);
        } else {
            self.open.agents.insert(agent.id);
        }
        self.agents.insert(agent.id, agent);
    }

    /// The agents not done.
    fn live(&self) -> impl Iterator<Item = &Agent> {
        self.open.agents.iter().filter_map(|id| self.agents.get(id))
    }

    /// Fold `records` into a copy of this state, or refuse the first that
    /// does not fold, with its index, and leave this state as it was. What
    /// [`crate::journal::Journal::commit`] does, and what a face asks before
    /// Max acts.
    pub fn with(&self, records: &[Record]) -> Result<State, (usize, Rejected)> {
        let mut next = self.clone();
        for (index, record) in records.iter().enumerate() {
            next.apply(record).map_err(|why| (index, why))?;
        }
        Ok(next)
    }

    /// Fold one record, or refuse it. On a refusal this state may be part
    /// way through the record: fold into a copy ([`State::with`]) to keep it.
    pub fn apply(&mut self, record: &Record) -> Result<(), Rejected> {
        let expected = self.next();
        if record.seq != expected {
            return Err(Rejected::OutOfOrder { expected });
        }
        if !self.authorised(&record.by, &record.fact) {
            return Err(Rejected::Author);
        }
        let mark = Mark::of(record);
        match &record.fact {
            Fact::Launched(launched) => {
                if let Some(call) = launched.by
                    && !self.calls.contains_key(&call)
                {
                    return Err(Rejected::Unknown);
                }
                self.launch = Some((mark, launched.clone()));
                // A connection belongs to the brain that made it.
                self.reach.clear();
            }
            Fact::Agent(id, AgentEvent::Created(created)) => self.create(*id, created, record)?,
            Fact::Agent(id, event) => self.agent_event(*id, event, record)?,
            Fact::ModelCall(id, event) => self.fold_model_call(*id, event, record)?,
            Fact::Call(id, event) => self.fold_call(*id, event, record)?,
            Fact::Delivery(delivery) => self.deliver(delivery, record)?,
            Fact::Change(change) => self.change(change, record)?,
            Fact::Environment(environment) => self.define(environment)?,
            Fact::Limb(limb) => {
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
                    return Err(Rejected::NotForAgent);
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
                    return Err(Rejected::NotForAgent);
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
            Fact::Focus(focus) => match focus.agent {
                Some(agent) => {
                    self.focus.insert(focus.face, agent);
                }
                None => {
                    self.focus.remove(&focus.face);
                }
            },
            Fact::Refused(refusal) => self.refused(refusal, record)?,
        }
        self.last = Some(record.seq);
        Ok(())
    }

    /// Whether `by` may state `fact`.
    ///
    /// The brain states what it decides and observes, but never Max's words,
    /// his marking an agent done, his stop, what his face shows, an agent no agent's call created, a
    /// limb's description, how a call a limb was sent ended, or a change to
    /// a resource it does not own (its own limbs' resources, such as the
    /// role files the meta limb reads, are its to state). A limb states only
    /// its own description, how its own dispatched calls ended and how its
    /// own resources changed. Max states his words, his calls and his
    /// answers, his controls over the tree, the agents of his own scope, what
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
                | Fact::Agent(
                    _,
                    AgentEvent::Cancel(Cancel {
                        by: Canceller::Max,
                        ..
                    }),
                )
                | Fact::Focus(_)
                | Fact::Described { .. } => false,
                Fact::Agent(_, AgentEvent::Created(created)) => created.by.is_some(),
                Fact::Call(_, CallEvent::Made(call)) => !matches!(call.caller, Caller::Max { .. }),
                Fact::Call(id, CallEvent::Settled(outcome)) => {
                    self.calls.get(id).is_some_and(|c| {
                        let Some(limb) = c.limb.and_then(|l| self.limbs.get(&l)) else {
                            return c.dispatched.is_none();
                        };
                        match &limb.kind {
                            // Max's answer is his; the brain only withdraws the question.
                            LimbKind::Max => matches!(outcome, CallOutcome::Interrupted { .. }),
                            LimbKind::Meta | LimbKind::Attention => true,
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
                Fact::Call(id, CallEvent::Cancel) => self
                    .calls
                    .get(id)
                    .is_some_and(|c| matches!(c.owner, Owner::Agent(_))),
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
                _ => false,
            },
            Author::Max { face, seen } => {
                *seen < self.next()
                    && match fact {
                        Fact::Agent(_, event) => match event {
                            AgentEvent::Created(created) => {
                                created.by.is_none()
                                    && (created.scope == Scope::Max
                                        || matches!(created.origin, Origin::Aside { .. }))
                            }
                            AgentEvent::Interrupt => true,
                            AgentEvent::Cancel(cancel) => cancel.by == Canceller::Max,
                            AgentEvent::Moved(movement) => matches!(movement, Move::Rewind { .. }),
                            AgentEvent::ModeSet(_) | AgentEvent::Renamed(_) | AgentEvent::Held(_) => {
                                true
                            }
                        },
                        Fact::Delivery(delivery) => matches!(
                            delivery.content,
                            Content::Words(_) | Content::Prompt(Prompt::Done)
                        ),
                        Fact::Call(_, CallEvent::Made(call)) => {
                            matches!(call.caller, Caller::Max { .. })
                        }
                        Fact::Call(id, CallEvent::Cancel) => {
                            self.calls.get(id).is_some_and(|c| c.owner == Owner::Max)
                        }
                        Fact::Call(id, CallEvent::Settled(_)) => self
                            .calls
                            .get(id)
                            .and_then(|c| c.limb)
                            .is_some_and(|l| self.is_max(l)),
                        Fact::Change(change) => self.is_max(change.resource.limb),
                        Fact::Focus(focus) => focus.face == *face,
                        Fact::Environment(_) | Fact::Limb(_) | Fact::Ranking(_) => true,
                        Fact::Launched(_)
                        | Fact::ModelCall(..)
                        | Fact::Call(..)
                        | Fact::Described { .. }
                        | Fact::Reach { .. }
                        | Fact::Refused(_) => false,
                    }
            }
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
    fn say(&mut self, words: ValueId, to: AgentId, at: Mark) {
        if !self.said.contains_key(&words) {
            self.said.insert(words, at);
        }
        self.heard.entry(to).or_default().insert(words);
    }

    fn create(&mut self, id: AgentId, created: &Created, record: &Record) -> Result<(), Rejected> {
        if self.agents.contains_key(&id) {
            return Err(Rejected::Exists);
        }
        let creator = match created.by {
            Some(by) => Some(self.creator(by, created)?),
            None => None,
        };
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
        let agent = Agent::new(id, created, record.at, self)?;
        let mark = Mark::of(record);
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
        self.open.scopes.entry(agent.scope).or_default().push(id);
        self.put(agent);
        Ok(())
    }

    /// The agent whose meta call `by` creates an agent, if that call may: it
    /// is still running, it is an agent's call to the meta limb, the new
    /// agent joins the scope the call opens or, as a sibling, its caller's
    /// own spawn scope, and it is a fork of the caller, a resume of an agent
    /// the caller owned, or fresh in an environment the caller's own
    /// environment starts.
    fn creator(&self, by: CallId, created: &Created) -> Result<AgentId, Rejected> {
        let call = self.calls.get(&by).ok_or(Rejected::Unknown)?;
        if call.settled.is_some() {
            return Err(Rejected::Finished);
        }
        let meta = call
            .limb
            .and_then(|l| self.limbs.get(&l))
            .is_some_and(|l| l.kind == LimbKind::Meta);
        let Caller::Agent { agent: caller, .. } = call.call.caller else {
            return Err(Rejected::CannotStart);
        };
        if !meta {
            return Err(Rejected::CannotStart);
        }
        let agent = self.agents.get(&caller).ok_or(Rejected::Unknown)?;
        if agent.drain().is_some() {
            return Err(Rejected::Draining);
        }
        let scope = created.scope == Scope::Spawn(by)
            || (created.scope == agent.scope && agent.scope != Scope::Max);
        if !scope {
            return Err(Rejected::WrongScope);
        }
        let origin = match &created.origin {
            Origin::Fresh { environment, .. } => self
                .environments
                .get(&agent.environment)
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
    fn check_aside(&self, from: AgentId, created: &Created) -> Result<(), Rejected> {
        let source = self.agents.get(&from).ok_or(Rejected::Unknown)?;
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

    /// An agent's new root must route only to its environment's limbs and its
    /// places, only tools those limbs offer agents, each tool once, and
    /// attach only the harness's calls along the binding.
    fn check_start(&self, agent: &Agent, start: &Start) -> Result<(), Rejected> {
        let environment = self
            .environments
            .get(&agent.environment)
            .ok_or(Rejected::Unknown)?;
        let mut tools = BTreeSet::new();
        for route in &start.binding.routes {
            let reachable = environment.limbs.contains(&route.limb)
                || agent.places.values().any(|l| *l == route.limb);
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
                return Err(Rejected::NotForAgent);
            };
            if start.binding.limb(&call.call.tool) != Some(limb) {
                return Err(Rejected::WrongEnvironment);
            }
        }
        Ok(())
    }

    fn agent_event(
        &mut self,
        id: AgentId,
        event: &AgentEvent,
        record: &Record,
    ) -> Result<(), Rejected> {
        let agent = self.agents.get(&id).ok_or(Rejected::Unknown)?;
        let mut root = None;
        match event {
            AgentEvent::Moved(Move::Handover { call, start }) => {
                let made = self.calls.get(call).ok_or(Rejected::Unknown)?;
                if !matches!(made.call.caller, Caller::Agent { agent, .. } if agent == id) {
                    return Err(Rejected::NotForAgent);
                }
                self.check_start(agent, start)?;
                root = Some(&start.root);
            }
            AgentEvent::Moved(Move::Root(start)) => {
                self.check_start(agent, start)?;
                root = Some(&start.root);
            }
            AgentEvent::Moved(Move::Rewind { to }) => {
                let on_chain = agent
                    .context()
                    .is_some_and(|c| self.descends(c.head, *to) && self.descends(*to, c.start));
                if !on_chain || !self.ended_turn(*to) {
                    return Err(Rejected::Detached);
                }
            }
            AgentEvent::Interrupt if self.scoped(agent) => {
                return Err(Rejected::Scoped);
            }
            AgentEvent::Cancel(Cancel {
                by: Canceller::Owner,
                ..
            }) => {
                let owner_leaving = self
                    .owner(id)
                    .and_then(|o| self.agents.get(&o))
                    .is_some_and(|o| leaving(o.drain()).is_some());
                if !owner_leaving {
                    return Err(Rejected::NotForAgent);
                }
            }
            _ => {}
        }
        let mut agent = agent.clone();
        agent.apply(record, self)?;
        if let Some(root) = root {
            self.entries.insert(root.id(), Entry::Root(root.clone()));
        }
        self.put(agent);
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
        record: &Record,
    ) -> Result<(), Rejected> {
        let mark = Mark::of(record);
        match event {
            ModelCallEvent::Sent(call) => {
                if self.model_calls.contains_key(&id) {
                    return Err(Rejected::Exists);
                }
                let agent = self.agents.get(&call.agent).ok_or(Rejected::Unknown)?;
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
                let mut agent = agent.clone();
                agent.apply(record, self)?;
                self.put(agent);
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
                let owner = sent.call.agent;
                let response = match ending {
                    Ending::Responded { content, .. } => Some(sent.call.response(*content)),
                    _ => None,
                };
                let mut agent = self.agents.get(&owner).ok_or(Rejected::Unknown)?.clone();
                agent.apply(record, self)?;
                self.put(agent);
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
        record: &Record,
    ) -> Result<(), Rejected> {
        let mark = Mark::of(record);
        match event {
            CallEvent::Made(call) => self.make(id, call, record),
            CallEvent::Cancel => {
                let made = self.calls.get_mut(&id).ok_or(Rejected::Unknown)?;
                if made.settled.is_some() {
                    return Err(Rejected::Finished);
                }
                made.cancelled.get_or_insert(mark);
                Ok(())
            }
            CallEvent::Handed { to } => self.hand(id, *to, record),
            CallEvent::Dispatched => {
                let made = self.calls.get(&id).ok_or(Rejected::Unknown)?;
                if made.settled.is_some() {
                    return Err(Rejected::Finished);
                }
                let limb = made.limb.ok_or(Rejected::Unroutable)?;
                let limb = self.limbs.get(&limb).ok_or(Rejected::Unknown)?;
                let instance = match &limb.kind {
                    LimbKind::Max => return Err(Rejected::ForMax),
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
                if self.cancels(id, made) {
                    return Err(Rejected::Draining);
                }
                let first = made.dispatched.map_or(mark, |d| d.first);
                if let Some(made) = self.calls.get_mut(&id) {
                    made.dispatched = Some(Dispatch {
                        first,
                        last: mark,
                        instance,
                    });
                }
                Ok(())
            }
            CallEvent::Stopping => {
                let made = self.calls.get_mut(&id).ok_or(Rejected::Unknown)?;
                if made.settled.is_some() {
                    return Err(Rejected::Finished);
                }
                if made.dispatched.is_none() {
                    return Err(Rejected::Busy);
                }
                made.stopping = Some(mark);
                Ok(())
            }
            CallEvent::Settled(outcome) => self.settle(id, outcome, record),
        }
    }

    fn make(&mut self, id: CallId, call: &Call, record: &Record) -> Result<(), Rejected> {
        if self.calls.contains_key(&id) {
            return Err(Rejected::Exists);
        }
        let mut thinking = None;
        let mut reader = None;
        let limb = match &call.caller {
            Caller::Agent { agent, tool_use } => {
                let agent = self.agents.get(agent).ok_or(Rejected::Unknown)?;
                if agent.is_done() {
                    return Err(Rejected::Finished);
                }
                let context = agent.context().ok_or(Rejected::Starting)?;
                let limb = context.binding.limb(&call.tool);
                match (tool_use, agent.phase()) {
                    (Some(_), Some(Phase::Thinking(request))) => thinking = Some(*request),
                    (Some(_), _) => return Err(Rejected::Busy),
                    (None, _) if limb.is_none() => return Err(Rejected::Unroutable),
                    (None, _) => {
                        let mut agent = agent.clone();
                        agent.apply(record, self)?;
                        reader = Some(agent);
                    }
                }
                limb
            }
            Caller::Max { limb, chat } => {
                let defined = self.limbs.get(limb).ok_or(Rejected::Unknown)?;
                if !defined.kind.is_brains() && !self.offered(*limb, &call.tool, true) {
                    return Err(Rejected::Unoffered);
                }
                if let Some(chat) = chat {
                    let agent = self.agents.get(chat).ok_or(Rejected::Unknown)?;
                    let routed = agent
                        .context()
                        .is_some_and(|c| c.binding.routes.iter().any(|r| r.limb == *limb));
                    if !routed {
                        return Err(Rejected::WrongEnvironment);
                    }
                    let mut agent = agent.clone();
                    agent.apply(record, self)?;
                    reader = Some(agent);
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
        let owner = match &call.caller {
            Caller::Agent { agent, .. } => Owner::Agent(*agent),
            Caller::Max { .. } => Owner::Max,
            Caller::Meta { call: by, .. } => self.calls.get(by).map_or(Owner::Max, |c| c.owner),
        };
        if let Some(request) = thinking
            && let Some(sent) = self.model_calls.get_mut(&request)
        {
            sent.calls.push(id);
        }
        if let Some(agent) = reader {
            self.put(agent);
        }
        self.open.calls.insert(id);
        self.calls.insert(
            id,
            CallRecord {
                call: call.clone(),
                owner,
                limb,
                made: Mark::of(record),
                dispatched: None,
                cancelled: None,
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
        record: &Record,
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
        let readers: Vec<&Agent> = if late {
            match made.call.caller {
                Caller::Agent { agent, .. }
                | Caller::Max {
                    chat: Some(agent), ..
                } => self
                    .agents
                    .get(&agent)
                    .filter(|t| !t.is_done())
                    .into_iter()
                    .collect(),
                Caller::Max { chat: None, .. } | Caller::Meta { .. } => Vec::new(),
            }
        } else {
            // A finished agent still reads Max's actions in its chat.
            let chat = match made.call.caller {
                Caller::Max { chat: Some(t), .. } => self.agents.get(&t).filter(|t| t.is_done()),
                _ => None,
            };
            self.live().chain(chat).filter(|t| t.awaits(id)).collect()
        };
        let mut touched = Vec::with_capacity(readers.len());
        for reader in readers {
            let mut agent = reader.clone();
            agent.apply(record, self)?;
            touched.push(agent);
        }
        for agent in touched {
            self.put(agent);
        }
        let mark = Mark::of(record);
        // Max's answer to a question is his words, heard by whoever asked.
        if let Author::Max { .. } = record.by
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

    /// An agent hands one of its background calls up to its own owner.
    fn hand(&mut self, id: CallId, to: Owner, record: &Record) -> Result<(), Rejected> {
        let made = self.calls.get(&id).ok_or(Rejected::Unknown)?;
        if made.settled.is_some() {
            return Err(Rejected::Finished);
        }
        let Owner::Agent(from) = made.owner else {
            return Err(Rejected::NotForAgent);
        };
        let up = self.owner(from).map_or(Owner::Max, Owner::Agent);
        if to != up {
            return Err(Rejected::NotForAgent);
        }
        let mut giver = self.agents.get(&from).ok_or(Rejected::Unknown)?.clone();
        giver.apply(record, self)?;
        let taker = match to {
            Owner::Agent(parent) => {
                let mut taker = self.agents.get(&parent).ok_or(Rejected::Unknown)?.clone();
                taker.apply(record, self)?;
                Some(taker)
            }
            Owner::Max => None,
        };
        self.put(giver);
        if let Some(taker) = taker {
            self.put(taker);
        }
        if let Some(made) = self.calls.get_mut(&id) {
            made.owner = to;
        }
        Ok(())
    }

    /// A refusal of the brain's own batch is a bug in the brain: the agent
    /// it concerns is cancelled by the harness, with no tidy-up turn, so the
    /// brain does not try the same thing forever and everything else carries
    /// on. A refusal of anyone else's changes nothing.
    fn refused(&mut self, refusal: &Refusal, record: &Record) -> Result<(), Rejected> {
        if refusal.by != Author::Brain {
            return Ok(());
        }
        let Some(agent) = refusal
            .facts
            .get(refusal.index)
            .and_then(|f| self.concerns(f))
            .and_then(|a| self.agents.get(&a))
            .filter(|a| !a.is_done())
        else {
            return Ok(());
        };
        let mut agent = agent.clone();
        let cancel = Cancel {
            by: Canceller::Harness {
                reason: format!("the harness refused its own fact: {}", refusal.why),
            },
            tidy: false,
        };
        agent.cancel(&cancel, record.at)?;
        self.put(agent);
        Ok(())
    }

    fn deliver(&mut self, delivery: &Delivery, record: &Record) -> Result<(), Rejected> {
        if self.deliveries.contains_key(&delivery.id) {
            return Err(Rejected::Exists);
        }
        let mut agent = self
            .agents
            .get(&delivery.to)
            .ok_or(Rejected::Unknown)?
            .clone();
        agent.apply(record, self)?;
        self.put(agent);
        let mark = Mark::of(record);
        match &delivery.content {
            Content::Words(words) => self.say(*words, delivery.to, mark),
            Content::Prompt(Prompt::Lingering) => {
                self.open.lingered.insert(delivery.to);
            }
            Content::Prompt(Prompt::Unblocked) => {
                self.open.unblocked.insert(delivery.to);
            }
            Content::Prompt(Prompt::Done) => {}
        }
        self.deliveries.insert(
            delivery.id,
            Delivered {
                delivery: delivery.clone(),
                at: mark,
                by: record.by.clone(),
            },
        );
        Ok(())
    }

    fn change(&mut self, change: &Change, record: &Record) -> Result<(), Rejected> {
        if !self.limbs.contains_key(&change.resource.limb) {
            return Err(Rejected::Unknown);
        }
        if let Some(by) = change.by
            && !self.calls.contains_key(&by)
        {
            return Err(Rejected::Unknown);
        }
        let mut woken = Vec::new();
        for agent in self.live() {
            if agent.mode == Mode::Standing && agent.phase() == Some(&Phase::Listening) {
                let mut agent = agent.clone();
                agent.apply(record, self)?;
                woken.push(agent);
            }
        }
        for agent in woken {
            self.put(agent);
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

    /// The position the next record must have.
    pub fn next(&self) -> Seq {
        Seq(self.last.map_or(1, |s| s.0 + 1))
    }

    /// The last position folded.
    pub fn last(&self) -> Option<Seq> {
        self.last
    }

    /// The brain writing the journal now, and where it began.
    pub fn launch(&self) -> Option<(Mark, &Launched)> {
        self.launch
            .as_ref()
            .map(|(mark, launched)| (*mark, launched))
    }

    pub fn entry(&self, id: EntryId) -> Option<&Entry> {
        self.entries.get(&id)
    }

    /// The context that ends at `head`, root first: exactly what a request
    /// with that head sent, given the values its entries name. Empty for an
    /// entry the journal has never carried.
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

    pub fn agent(&self, id: AgentId) -> Option<&Agent> {
        self.agents.get(&id)
    }

    /// Every agent, done ones included, in id order.
    pub fn agents(&self) -> impl Iterator<Item = &Agent> {
        self.agents.values()
    }

    /// The agents not done.
    pub fn live_agents(&self) -> impl Iterator<Item = &Agent> {
        self.live()
    }

    pub fn call(&self, id: CallId) -> Option<&CallRecord> {
        self.calls.get(&id)
    }

    /// Every call, in id order.
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

    /// Whether `agent` has heard Max say `words`: sent to it, given to it as
    /// its mandate, answered to its question, or quoted in its mandate. What
    /// it may quote on.
    pub fn heard(&self, agent: AgentId, words: ValueId) -> bool {
        self.heard.get(&agent).is_some_and(|w| w.contains(&words))
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

    /// The instance a limb is connected to under this brain, and since when.
    pub fn connected(&self, limb: LimbId) -> Option<(InstanceId, Mark)> {
        match self.reach.get(&limb)? {
            Reached {
                reach: Reach::Connected { instance },
                at,
            } => Some((*instance, *at)),
            Reached {
                reach: Reach::Lost, ..
            } => None,
        }
    }

    pub fn resource(&self, key: &ResourceKey) -> Option<&Current> {
        self.resources.get(key)
    }

    pub fn ranking(&self) -> &Ranking {
        &self.ranking
    }

    /// The agent a face shows Max.
    pub fn focus(&self, face: FaceId) -> Option<AgentId> {
        self.focus.get(&face).copied()
    }

    /// Until when the account is limited, if it is at `now`.
    pub fn limited(&self, now: Timestamp) -> Option<Timestamp> {
        self.limited_until.filter(|until| *until > now)
    }

    /// The agent that owns this one: the caller of its scope's spawn. `None`
    /// in Max's scope.
    pub fn owner(&self, agent: AgentId) -> Option<AgentId> {
        match self.agents.get(&agent)?.scope {
            Scope::Max => None,
            Scope::Spawn(call) => self.call_owner(call),
        }
    }

    /// The agent a call belongs to: its agent, or for the harness's call, the
    /// agent whose call it runs for. Max's calls are his own.
    pub fn call_owner(&self, call: CallId) -> Option<AgentId> {
        match &self.calls.get(&call)?.call.caller {
            Caller::Agent { agent, .. } => Some(*agent),
            Caller::Max { .. } => None,
            Caller::Meta { call, .. } => self.call_owner(*call),
        }
    }

    /// How the agent's last model call to end, ended.
    pub fn last_turn(&self, agent: AgentId) -> Option<&Ended> {
        let id = self.open.last_turn.get(&agent)?;
        self.model_calls.get(id)?.ended.as_ref()
    }

    /// The last report of the account's quota.
    pub fn quota(&self) -> Option<&Ended> {
        self.model_calls
            .get(self.open.quota.as_ref()?)?
            .ended
            .as_ref()
    }

    /// The agents in a scope, in order of creation.
    pub fn members(&self, scope: Scope) -> impl Iterator<Item = &Agent> {
        let ids = self.open.scopes.get(&scope).map_or(&[][..], Vec::as_slice);
        ids.iter().filter_map(|id| self.agents.get(id))
    }

    /// Whether the agent or any ancestor is held.
    pub fn held(&self, agent: AgentId) -> bool {
        let mut next = Some(agent);
        while let Some(id) = next {
            if self.agents.get(&id).is_some_and(|t| t.held) {
                return true;
            }
            next = self.owner(id);
        }
        false
    }

    /// The limbs an agent can reach now: its environment's, as currently
    /// defined, and its places. A new root is made from these.
    pub fn limbs_of(&self, agent: &Agent) -> Vec<LimbId> {
        let mut limbs: Vec<LimbId> = self
            .environments
            .get(&agent.environment)
            .map(|e| e.limbs.clone())
            .unwrap_or_default();
        limbs.extend(agent.places.values().copied());
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

    /// Whether the agent's round waits on a scope with members still running,
    /// so the move cannot become Max's until it closes: a face offers an
    /// aside instead of an interrupt.
    pub fn scoped(&self, agent: &Agent) -> bool {
        match agent.phase() {
            Some(Phase::Acting(step)) => step
                .outstanding
                .iter()
                .any(|c| self.members(Scope::Spawn(*c)).any(|m| !m.is_done())),
            _ => false,
        }
    }

    /// Whether a call asks Max something.
    pub fn is_ask(&self, call: CallId) -> bool {
        self.calls
            .get(&call)
            .and_then(|c| c.limb)
            .is_some_and(|l| self.is_max(l))
    }

    fn is_max(&self, limb: LimbId) -> bool {
        self.limbs
            .get(&limb)
            .is_some_and(|l| l.kind == LimbKind::Max)
    }

    /// The agent's outstanding question to Max: one its round waits on, or
    /// one it asked in the background and carried on (a provisional choice
    /// he may overturn).
    fn ask(&self, agent: &Agent) -> Option<CallId> {
        agent.awaited().find(|c| self.is_ask(*c))
    }

    /// A tool use is acted on only once its response is complete and its
    /// agent is acting on it.
    fn acting_on(&self, id: CallId, call: &Call) -> bool {
        match call.caller {
            Caller::Agent {
                agent,
                tool_use: Some(_),
            } => self.agents.get(&agent).is_some_and(|t| {
                matches!(t.phase(), Some(Phase::Acting(step)) if step.outstanding.contains(&id))
            }),
            _ => true,
        }
    }

    /// What an agent is waiting on now.
    pub fn waiting_on(&self, agent: AgentId, now: Timestamp) -> Option<Waiting> {
        let t = self.agents.get(&agent)?;
        let since = t.since;
        let live = match &t.life {
            Life::Starting => {
                return Some(Waiting::Starting {
                    limbs: self
                        .limbs_of(t)
                        .into_iter()
                        .filter(|l| !self.reached(*l))
                        .collect(),
                    since,
                });
            }
            Life::Done { .. } => return Some(Waiting::Done { since }),
            Life::Live(live) => live,
        };
        Some(match &live.phase {
            Phase::Ready if self.held(agent) => Waiting::Held { since },
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
            Phase::Acting(step) => match step.outstanding.iter().find(|c| self.is_ask(**c)) {
                Some(ask) => Waiting::Max {
                    since: live
                        .owed_since
                        .or(self.calls.get(ask).map(|c| c.made.at))
                        .unwrap_or(since),
                    ask: Some(*ask),
                },
                None => Waiting::Calls {
                    calls: step.outstanding.clone(),
                    since,
                },
            },
            Phase::AwaitingMax => Waiting::Max {
                since: live.owed_since.unwrap_or(since),
                ask: None,
            },
            Phase::Listening => Waiting::Listening {
                background: t.background.clone(),
                since,
            },
        })
    }

    /// Max's queue, in order: every live agent he owes a word or that has
    /// asked him something, mid-turn or in the background.
    pub fn queue(&self) -> Vec<Queued> {
        let queued = self
            .live()
            .filter_map(|t| {
                let ask = self.ask(t);
                let asked = ask.and_then(|a| self.calls.get(&a)).map(|c| c.made.at);
                Some(Queued {
                    agent: t.id,
                    since: t.owed_since().or(asked)?,
                    ask,
                    why: None,
                })
            })
            .collect();
        self.ranking.order(queued)
    }

    /// What an agent's context embodies that has moved since, and that it has
    /// not been told of: the next request's notice.
    pub fn stale(&self, agent: &Agent) -> Vec<Stale> {
        agent.context()
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
    /// Restart is not a mode: it is read off journal positions. A request
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
        let mut readers: BTreeMap<CallId, Vec<&Agent>> = BTreeMap::new();
        let mut needed: BTreeSet<LimbId> = BTreeSet::new();
        let mut starting = Vec::new();
        for agent in self.live() {
            for call in agent.awaited() {
                readers.entry(call).or_default().push(agent);
            }
            let limbs = self.limbs_of(agent);
            if agent.life == Life::Starting && limbs.iter().all(|l| self.reached(*l)) {
                starting.push(Todo::Root(agent.id));
            }
            needed.extend(limbs);
            let lingering = agent.mode == Mode::Autonomous
                && !agent.background.is_empty()
                && !self.open.lingered.contains(&agent.id);
            match (agent.phase(), agent.drain()) {
                (Some(Phase::Listening), None) if lingering => {
                    todo.push(Todo::Linger(agent.id));
                }
                (Some(Phase::Ready), None)
                    if !self.held(agent.id) && self.limited(now).is_none() =>
                {
                    todo.push(Todo::Send(agent.id));
                }
                (Some(Phase::Thinking(call)), Some(_))
                    if !self.model_calls.get(call).is_some_and(lost) =>
                {
                    todo.push(Todo::Abandon(*call));
                }
                _ => {}
            }
        }
        for id in &self.open.calls {
            let Some(record) = self.calls.get(id) else {
                continue;
            };
            if self.open.scopes.contains_key(&Scope::Spawn(*id)) {
                let stopping = self
                    .call_owner(*id)
                    .and_then(|o| self.agents.get(&o))
                    .is_some_and(|o| ends(o.drain()));
                let live: Vec<&Agent> = self
                    .members(Scope::Spawn(*id))
                    .filter(|m| !m.is_done())
                    .collect();
                let aside = |m: &&Agent| matches!(m.origin, Origin::Aside { .. });
                if live.is_empty() {
                    todo.push(Todo::Settle(*id));
                } else if stopping {
                    let stop = live.iter().filter(|m| !ends(m.drain()));
                    todo.extend(stop.map(|m| Todo::Stop(m.id)));
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
            if interrupts(*id, &record.call, readers) {
                let due = match (&record.dispatched, &limb.kind) {
                    (None, _) => true,
                    (Some(_), LimbKind::Plugin { .. }) => match connection {
                        Some(c) => record.stopping.is_none_or(|s| s.seq < c),
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
                LimbKind::Meta | LimbKind::Attention => record.dispatched.is_none(),
                LimbKind::Plugin { .. } => {
                    connection.is_some_and(|c| record.dispatched.is_none_or(|d| d.last.seq < c))
                }
            };
            if due && self.acting_on(*id, &record.call) {
                todo.push(Todo::Dispatch(*id));
            }
        }
        let connect = needed.into_iter().filter(|l| {
            self.limbs.get(l).is_some_and(|limb| !limb.kind.is_brains())
                && self.connected(*l).is_none()
        });
        todo.extend(connect.map(Todo::Connect));
        todo.extend(starting);
        todo
    }
}

/// Whether every live agent reading this call is draining in a way that
/// stops it. An interrupt stops the round, not background work; a stop
/// stops both. A shared attachment runs on while any sibling still wants
/// it. Max's own calls are never stopped for an agent.
fn interrupts(id: CallId, call: &Call, readers: &[&Agent]) -> bool {
    if matches!(call.caller, Caller::Max { .. }) || readers.is_empty() {
        return false;
    }
    readers.iter().all(|t| match t.drain() {
        None => false,
        Some(Drain::ToMax) => {
            matches!(t.phase(), Some(Phase::Acting(step)) if step.outstanding.contains(&id))
        }
        Some(Drain::Stop(_) | Drain::Fail(_)) => true,
    })
}
