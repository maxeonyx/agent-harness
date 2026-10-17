//! Tasks: one agent's mandate over its whole life, and the agent loop as a
//! fold.

use std::collections::BTreeMap;

use imbl::{OrdMap, Vector};
use serde::{Deserialize, Serialize};

use crate::call::{CallEvent, Caller, Outcome as CallOutcome, Owner};
use crate::context::{Entry, EntryId, Message, Note, Rendered, Role, Root, Span};
use crate::delivery::{Content, Delivery, Prompt};
use crate::environment::Binding;
use crate::fact::{Author, Fact, Rejected, Stated};
use crate::id::{
    CallId, DeliveryId, EnvironmentId, LimbId, ModelCallId, TaskId, Timestamp, ValueId,
};
use crate::model_call::{Ending, ModelCallEvent, Stop};
use crate::resource::{Change, Observation, ResourceKey, Version};
use crate::state::{Mark, State};

/// The fold of the facts about one task: everything the brain, a face or
/// the scheduler needs to know about it now.
///
/// **Concept.** A task is the unit of structured concurrency and the thing
/// Max watches, talks to, and is asked to attend to. It has a [`Mandate`],
/// a [`Scope`] that owns it, a [`Mode`] that says what the end of a turn
/// means, an [environment](crate::environment::Environment) that is its role
/// for life, the limbs that fill that environment's places (its own clone),
/// and a [`Life`]: starting, live in a [`Context`], or done. A task is an
/// identity; its context is a value. The head moves through the context tree
/// (a fork starts a task on its parent's response, a compaction replaces
/// what the task did since a checkpoint with what it carries, a resume
/// starts a task on another's last entry, Max moves it back to an entry it
/// has been at: [`Move::Back`]), and none of that copies or edits a
/// context.
///
/// Creation and the first root are separate facts. A spawn creates its
/// members in the batch that dispatches it, so a scope's membership is
/// atomic with the call that owns it; a fresh member is then
/// [`Life::Starting`] until every limb it can reach has connected and
/// described itself (a clone made, a process started), when the brain gives
/// it its first root ([`Move::Root`]).
///
/// Its life is folded from the facts that concern it (`Task::apply`, run
/// only by the brain's fold): its own events, its model calls, the
/// settlements of the calls it waits on, deliveries to it, and changes to
/// what it embodies. The life is never written down as a fact, so it cannot
/// disagree with its history; "what is it waiting on" and "is it waiting on
/// Max" are read off it. What only a live task has (a context, a phase, a
/// pending drain, Max owing it a word, its run of failed requests) lives in
/// [`Live`], so no reader re-proves that a running task has a context.
///
/// **Vision.** Structured concurrency (a parent waiting on children is
/// [`Phase::Acting`] on its spawn call, like waiting on any tool, and a task
/// is done only when nothing it started is still running and nothing sent
/// to it is unread); the A.1.3 problem (the mandate is data, so the harness
/// can state it at the fork, after each tool round and in a compaction's
/// carry);
/// user-facing, autonomous and standing tasks, which differ only in
/// [`Mode`]; the throbber; restart (the life says what to redo); and Max's
/// words never lost (a pending delivery stops a turn from completing the
/// task, and survives a compaction).
///
/// **Existing harnesses.** OpenCode, Codex and Oh My Pi each keep a session
/// or thread with a status the agent loop keeps current as it runs, and
/// start a subagent as a child session from a fresh prompt by default (Oh
/// My Pi: `ref-oh-my-pi/packages/coding-agent/src/prompts/system/subagent-user-prompt.md`;
/// Codex can instead fork a filtered copy). Pi's `durable` package comes
/// closest, with owned tasks and joins, but each task is a replaceable
/// record with a per-kind checkpoint that must be migrated. None treats the
/// mandate as data the harness can restate, and none folds status from
/// history.
#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub id: TaskId,
    /// Short and descriptive, growing with depth (`tk-prodsync-findfiles`).
    pub name: String,
    /// Its role, and everything it can reach. Fixed for life, and inherited
    /// by forks, resumes and asides: models do not reliably honour a role
    /// that changes mid-context. A new version of the environment reaches
    /// it at its next root.
    pub environment: EnvironmentId,
    /// The limb filling each of its environment's
    /// [places](crate::environment::Environment::places), by plugin kind.
    /// Fixed for life, and shared by its forks.
    pub places: BTreeMap<String, LimbId>,
    pub scope: Scope,
    pub origin: Origin,
    /// The meta call that created it; `None` when Max did. Whoever made
    /// that call gave the mandate ([`State::call_owner`]).
    pub by: Option<CallId>,
    pub mode: Mode,
    /// What it is told its job is, restated after each new root. Its
    /// creation's, unless Max has since edited the first message of its
    /// context ([`crate::draft::DraftEvent::Opened`] at its start).
    pub mandate: Mandate,
    /// The model and effort Max chose for it, over its environment's.
    pub tuning: crate::environment::Tuning,
    /// What Max last said here about pausing it and everything under it;
    /// `None` if he never has. Whether it is paused now is
    /// [`State::pause`], which also reads its ancestors.
    pub pause: Option<Paused>,
    /// When it was created: the order of siblings and of Max's projects.
    pub created: Mark,
    /// When its life or phase last changed: how long the throbber has shown
    /// what it shows.
    pub since: Timestamp,
    /// Inputs no request has rendered yet. A turn cannot complete over one
    /// that [wakes](Input::wakes). They are for the task, not its context, so
    /// they survive a new root and ride into a resume.
    pub pending: Vec<Input>,
    /// Calls it started in the background that have not settled. It cannot
    /// finish while one runs, and each one's settlement wakes it.
    pub background: Vec<CallId>,
    /// Max's actions in its chat that have not settled. Each settlement
    /// rides into its next request without waking it.
    pub max_actions: Vec<CallId>,
    pub life: Life,
}

/// Where a task is in its life. Exactly one holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Life {
    /// Created, with no context yet: some limb it can reach has not
    /// connected or described itself (its clone is being made). The brain
    /// gives it its first root once every one has
    /// ([`crate::state::Todo::Root`]). Max can stop it; it cannot be
    /// interrupted, forked, resumed or moved back, and makes no calls.
    Starting,
    Live(Live),
    /// Finished. Its context is kept for a resume and for Max to read; a task
    /// stopped while starting never had one.
    Done {
        outcome: Outcome,
        context: Option<Context>,
    },
}

/// What a live task has, and only a live task.
#[derive(Clone, Debug, PartialEq)]
pub struct Live {
    pub context: Context,
    pub phase: Phase,
    /// Set when the task must stop doing what it is doing. No request is
    /// sent while it is set, but a stopping task's one tidy-up turn. It is
    /// pending only while something is outstanding (a request, a round,
    /// background work, members, a tidy-up turn); otherwise it takes effect
    /// as it arrives.
    pub drain: Option<Draining>,
    /// Since when Max has owed this task a word: set when a user-facing turn
    /// ends or Max interrupts, cleared by his words or by a change of mode.
    /// It survives the harness's own turns (a compaction before the cache
    /// expires), so the task keeps its place in his queue through them.
    pub owed_since: Option<Timestamp>,
    /// Requests that failed in a row; the brain's policy gives up at some
    /// count ([`Drain::Fail`]). The account's limit is not a failure.
    pub failures: u32,
    /// It has been asked to park, by this delivery of
    /// [`Prompt::Park`], and has not parked yet: it runs as if not paused
    /// until the turn that reads it ends. Cleared when it is resumed or
    /// interrupted.
    pub parking: Option<DeliveryId>,
}

impl Live {
    fn new(context: Context) -> Live {
        Live {
            context,
            phase: Phase::Ready,
            drain: None,
            owed_since: None,
            failures: 0,
            parking: None,
        }
    }
}

/// Whose move it is in a live task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The task's move: the brain builds and sends its next request, unless
    /// it is paused or the account is limited (derived, not stored here). An
    /// abandoned task is ready for its tidy-up turn.
    Ready,
    /// A request is in flight. The calls of its tool uses are made with its
    /// response and recorded on the request.
    Thinking(ModelCallId),
    /// The calls of one round are outstanding: a response's tool uses, or a
    /// fresh context's attachments. A spawn is one of them.
    Acting(Step),
    /// Max's move: a user-facing turn ended, or he interrupted.
    AwaitingMax,
    /// The turn ended and the task waits for something that wakes it: a
    /// change to what a standing task embodies, or background work.
    Listening,
    /// Max parked it ([`Pause::Park`]) and its turn to reach a safe point
    /// has ended: it waits for him to resume it, whatever its mode, so what
    /// it said is never taken for its result. Inputs arriving meanwhile
    /// wait with the resume ([`Prompt::Resume`]); only an abandon or an
    /// interrupt moves it otherwise.
    Parked,
}

/// The context a task is working in, and everything that belongs to it
/// rather than to the task: where it began, where it ends now, which limb
/// runs each of its tools, and what it embodies. A compaction onto a fresh
/// root replaces all but what it embodies; Max's words and actions pending
/// for the task survive any compaction, since they are for the task.
#[derive(Clone, Debug, PartialEq)]
pub struct Context {
    /// Where it began: its root, or the entry it was forked, resumed or
    /// opened at. A move back cannot go before it.
    pub start: EntryId,
    pub head: EntryId,
    /// Which limb runs each of its tools. Made with its root and kept until
    /// the next one, so a redefined environment takes effect when the task
    /// next compacts onto a fresh root, never under it.
    pub binding: Binding,
    /// The version of each resource it embodies, and what it has been told
    /// since. Persistent, so a fork's copy shares it until either changes.
    pub knows: OrdMap<ResourceKey, Known>,
    /// What each request since `start` changed in `knows`, so a move back
    /// restores what the earlier context embodied. Persistent, so folding a
    /// fact into a long-lived task does not copy its history.
    learned: Vector<Learned>,
    /// Max moved it here from this entry ([`Move::Back`]), and the path it
    /// left changed the world, which its next request must say
    /// ([`Note::Moved`]). Kept across further moves until a request is sent,
    /// so the note names the furthest point it left.
    pub moved: Option<EntryId>,
}

impl Context {
    /// A new root's context.
    fn rooted(start: &Start) -> Context {
        Context {
            start: start.root.id(),
            head: start.root.id(),
            binding: start.binding.clone(),
            knows: start
                .observes
                .iter()
                .map(|o| {
                    let known = Known {
                        has: o.version,
                        told: None,
                    };
                    (o.resource.clone(), known)
                })
                .collect(),
            learned: Vector::new(),
            moved: None,
        }
    }

    /// A context that continues this one from its head: a fork's, a
    /// resume's, an aside's.
    fn continued(&self) -> Context {
        Context {
            start: self.head,
            head: self.head,
            binding: self.binding.clone(),
            knows: self.knows.clone(),
            learned: Vector::new(),
            moved: None,
        }
    }

    /// The context a compaction leaves, before it learns from its carry:
    /// this one with a new head, or a new root's that still embodies what
    /// this one did, where the root does not say otherwise. Keeping what it
    /// embodied keeps the notices and the stale-write refusals true: the
    /// summary was written from those versions.
    ///
    /// Onto a checkpoint, what the replaced branch learned is now learned
    /// at the carry: the summary was written from it, so the context still
    /// embodies it, until a move leaves the carry.
    fn compacted(&self, onto: &Onto, head: EntryId, world: &State) -> Context {
        let mut context = match onto {
            Onto::Checkpoint(at) => {
                let mut context = self.clone();
                let mut before: Vec<(ResourceKey, Option<Known>)> = Vec::new();
                while context
                    .learned
                    .last()
                    .is_some_and(|l| !world.descends(*at, l.head))
                {
                    let Some(learned) = context.learned.pop_back() else {
                        break;
                    };
                    // Popped latest first, so a later pop's value is the
                    // earlier one.
                    for (resource, old) in learned.before {
                        before.retain(|(k, _)| *k != resource);
                        before.push((resource, old));
                    }
                }
                if !before.is_empty() {
                    context.learned.push_back(Learned { head, before });
                }
                context
            }
            Onto::Fresh(start) => {
                let mut rooted = Context::rooted(start);
                rooted.knows = rooted.knows.union(self.knows.clone());
                rooted
            }
        };
        context.head = head;
        context
    }

    /// Whether `change` moves something this context embodies that it has
    /// not been told of.
    fn moved_by(&self, change: &Change) -> bool {
        self.knows
            .get(&change.resource)
            .is_some_and(|known| known.has != change.version && known.told != Some(change.version))
    }

    /// The context now embodies these versions.
    fn observe(
        &mut self,
        observations: &[Observation],
        before: &mut Vec<(ResourceKey, Option<Known>)>,
    ) {
        for o in observations {
            let old = self.knows.insert(
                o.resource.clone(),
                Known {
                    has: o.version,
                    told: None,
                },
            );
            if !before.iter().any(|(k, _)| *k == o.resource) {
                before.push((o.resource.clone(), old));
            }
        }
    }

    /// The context has been told these resources moved, without seeing them.
    fn tell(
        &mut self,
        observations: &[Observation],
        before: &mut Vec<(ResourceKey, Option<Known>)>,
    ) {
        for o in observations {
            if let Some(known) = self.knows.get_mut(&o.resource) {
                let old = *known;
                known.told = Some(o.version);
                if !before.iter().any(|(k, _)| *k == o.resource) {
                    before.push((o.resource.clone(), Some(old)));
                }
            }
        }
    }

    /// Move the head to `to`, on this path or another, and embody exactly
    /// what the context embodied there: undo what the requests and
    /// compactions it leaves learned, back to where the paths part, then
    /// learn again what those of `task` on the path to `to` learned.
    fn move_to(&mut self, to: EntryId, task: TaskId, world: &State) -> Result<(), Rejected> {
        self.head = to;
        while self
            .learned
            .last()
            .is_some_and(|l| !world.descends(to, l.head))
        {
            let Some(learned) = self.learned.pop_back() else {
                break;
            };
            for (resource, before) in learned.before.into_iter().rev() {
                match before {
                    Some(known) => self.knows.insert(resource, known),
                    None => self.knows.remove(&resource),
                };
            }
        }
        let known = self.learned.last().map_or(self.start, |l| l.head);
        for (entry, at) in world.path(task, to, &|e| e == known).into_iter().rev() {
            if let Some(renders) = world.rendered(task, entry) {
                learn(self, renders, at, world)?;
            }
        }
        Ok(())
    }
}

/// What a task's context knows of one resource.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Known {
    /// The version the context embodies.
    pub has: Version,
    /// The newest version it has been told of, if any: a notice already
    /// said "this changed after turn N", and need not say it again.
    pub told: Option<Version>,
}

/// What one request changed in what its context knows: each resource's
/// value before it.
#[derive(Clone, Debug, PartialEq)]
struct Learned {
    head: EntryId,
    before: Vec<(ResourceKey, Option<Known>)>,
}

/// Who owns a task: the nursery it lives in.
///
/// A scope is not stored as an identity of its own. A spawn call opens one
/// and owns it, and its members are the tasks whose scope it is. The call
/// settles, with every member's outcome, when the last member is done, and
/// that settlement is the scope's close. Members can add siblings while they
/// run, so the moment the last one finishes nobody is left who could add
/// another: completion and growth cannot race. No task outlives its scope
/// and no scope outlives its call.
///
/// A member's failure is one of the outcomes its parent reads, not an
/// exception that cancels its siblings: an agent's failure is information
/// the parent weighs (Trio's nurseries, without cancel-on-first-error).
///
/// **Existing harnesses.** OpenCode's `subagent` tool blocks by default, one
/// level deep (`ref-opencode/packages/core/src/tool/plugin/subagent.ts:129`);
/// Oh My Pi's `task` and Codex's `spawn_agent` return at once and deliver
/// results later (`ref-oh-my-pi/packages/coding-agent/src/tools/settings.ts:892`);
/// Goose's `delegate` cannot nest; Pi ships none. In none can a child add a
/// sibling, and an asynchronous child can outlive the turn that started it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Scope {
    /// Max's own scope. It never closes: the tasks he starts live in it, and
    /// so do resumes and standing tasks such as the attention scheduler.
    Max,
    /// The scope the spawn call opened. Its members are the tasks the call
    /// created, the siblings they added, and any aside Max opened on the
    /// task waiting on it.
    Spawn(CallId),
}

/// How a task came to be. It decides its environment, where its first
/// context starts, how the first request is framed, and how the tree shows
/// the task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Origin {
    /// A new task in an environment, with a limb for each of its places. It
    /// starts on a new root ([`Move::Root`]), then its attachments, its
    /// shared brief and its own mandate. Created by an agent, only in an
    /// environment its own [starts](crate::environment::Environment::starts).
    Fresh {
        environment: EnvironmentId,
        /// By plugin kind, one for each of the environment's places.
        places: BTreeMap<String, LimbId>,
    },
    /// The parent's spawning response, the source's head when the fork is
    /// made. The first request answers every tool use in it with the same
    /// neutral result, its own spawn included, so no fork waits on its
    /// parent's other work; the fork's mandate follows as user text in that
    /// message. The fork-framing spike measured this: a mandate put in a
    /// tool result does not override the identity Sonnet has from its
    /// parent's context, and user text does.
    Fork { from: TaskId },
    /// A finished task's last entry, continued with a new mandate. It
    /// inherits what its source had not yet rendered, such as Max's edits
    /// in the finished chat. Max resumes any finished task; an agent resumes
    /// a task of a scope it owned, so an orchestrator can ask its designer a
    /// follow-up with the designer's context intact.
    Resume { from: TaskId },
    /// The head of a task waiting on its children, opened by Max as a
    /// user-facing fork so he can talk to it meanwhile. It joins the scope
    /// the task waits on, as the one member Max talks to: it is told when
    /// every other member is done ([`Prompt::Unblocked`]), it is done when
    /// he marks it done, and its result reaches the task with theirs, so the
    /// task carries on with everything, the talk included.
    Aside { from: TaskId },
}

/// A new root, and what is decided with it: a fresh task's first context,
/// or the one a compaction starts afresh.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Start {
    pub root: Root,
    /// Made with the root's tools ([`Binding::new`]).
    pub binding: Binding,
    /// The versions the root embodies: the environment's source, each
    /// limb's instructions (an `AGENTS.md`, the skills index), each tool's
    /// schema.
    pub observes: Vec<Observation>,
    /// Attachments to run before the first request, shown to the task as
    /// tool calls it made: calls made by the harness ([`Caller::Meta`]),
    /// shared by every member that names them, so N siblings share one read
    /// and one cache prefix.
    pub awaits: Vec<CallId>,
}

impl Start {
    fn values(&self, out: &mut Vec<ValueId>) {
        let Start {
            root,
            binding: _,
            observes: _,
            awaits: _,
        } = self;
        let Root {
            model: _,
            system,
            tools,
        } = root;
        out.push(*system);
        out.push(*tools);
    }
}

/// What the end of a model turn means for this task.
///
/// The one difference between an autonomous subagent, a chat with Max, and
/// a standing agent such as the attention scheduler. Max marking a
/// user-facing task done makes it autonomous for one last turn, which writes
/// the result (with [`crate::delivery::Prompt::Done`], so the model has
/// seen it before it does).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// The end of a turn completes the task, once nothing it started is
    /// still running; its last message is the result.
    Autonomous,
    /// The end of a turn gives the move to Max.
    UserFacing,
    /// The end of a turn waits for something its context embodies to change,
    /// or a word from Max. Only in Max's scope, since a scope holding one
    /// would never close.
    Standing,
}

/// What a task was asked to do.
///
/// The chain of mandates up the owner chain is the task's place in the
/// overall work: "A, then A.1, then A.1.3". Rendering that chain at a fork,
/// as a reminder after each tool round, and in a compaction's carry is how
/// a deeply forked agent knows to stop after A.1.3.
///
/// A mandate Max gives is his words: the brain counts its text among the
/// words he has said, heard by the task he gave it to, so the task can quote
/// it on ([`State::heard`]). Who gave any other mandate is the caller of the
/// meta call that created the task ([`Task::by`]).
///
/// A fresh task's mandate also carries what every member of its spawn
/// shares, rendered before its own text and identical for each of them, so
/// N siblings are one cache prefix: the parent's brief, and Max's words the
/// parent quotes. A quote is the value of his words itself, so a brief
/// carries them verbatim by construction, the same words have the same
/// label wherever they are quoted, and the child can tell them from its
/// parent's paraphrase.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mandate {
    pub text: ValueId,
    /// What every member of the spawn is told first.
    pub brief: Option<ValueId>,
    /// Max's words, each one the creating task has heard: words he sent it,
    /// the mandate he gave it, his answer to its question, or a quote in its
    /// own mandate.
    pub quotes: Vec<ValueId>,
}

/// An input waiting for a request to render it. Whether it ends a wait is
/// what it is, never a flag beside it: only a few things drive a request
/// (`context-and-agent-loop.md`): the agent's own tool loop, Max's words, a
/// harness prompt, work the agent asked to hear back from, and for a
/// standing task a change to what it embodies. A round's results and Max's
/// actions in the chat ride on the next request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// A settled call of the last round. The next request must render every
    /// one, since each tool use needs its result first.
    Result(CallId),
    /// A background call the task started has settled. Wakes it.
    Background(CallId),
    /// The real outcome of a call first settled as unknown. Wakes it.
    Late(CallId),
    /// One of Max's actions in its chat has settled. Rides along.
    MaxAction(CallId),
    /// A member passed it one of its running background calls
    /// ([`crate::call::CallEvent::Passed`]), which is its own from now on.
    /// Rides along.
    Passed(CallId),
    /// Max's words, or a turn the harness asks for. Wakes it.
    Delivery(DeliveryId),
}

impl Input {
    /// Whether it ends a wait, and keeps a turn from completing the task.
    pub fn wakes(&self) -> bool {
        match self {
            Input::Background(_) | Input::Late(_) | Input::Delivery(_) => true,
            Input::Result(_) | Input::MaxAction(_) | Input::Passed(_) => false,
        }
    }

    fn is(&self, rendered: &Rendered) -> bool {
        match (self, rendered) {
            (
                Input::Result(a) | Input::Background(a) | Input::Late(a) | Input::MaxAction(a),
                Rendered::Call(b),
            ) => a == b,
            (Input::Delivery(a), Rendered::Delivery(b)) => a == b,
            (Input::Passed(a), Rendered::Note(Note::Passed { call: b })) => a == b,
            _ => false,
        }
    }
}

/// The outstanding calls of one round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// The response whose tool uses these are; `None` for a fresh context's
    /// attachments, whose tool uses the harness writes when it renders them.
    pub response: Option<EntryId>,
    pub outstanding: Vec<CallId>,
}

/// What a task does once nothing is outstanding, instead of carrying on: the
/// fact that asks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Drain {
    /// Max interrupted: the move becomes his, whatever the mode. Its round's
    /// calls are stopped; its background work runs on. It never passes
    /// through a spawn call (that would stop the scope), so it is refused
    /// while the task's round waits on a scope with members still running
    /// ([`Rejected::Scoped`], and [`State::scoped`] beforehand); to talk to
    /// it then, Max opens an aside.
    ///
    /// It names the turn he interrupts by one of its requests, the one he
    /// was watching: an interrupt arriving once that turn has ended is
    /// refused ([`Rejected::Finished`]), so a late one never stops the
    /// next turn.
    ToMax { turn: ModelCallId },
    /// The work is no longer wanted: abandon the task and everything it
    /// owns ([`crate::cancel`]), which must have exited `within`
    /// milliseconds of this fact. Its request is cut, its calls, background
    /// work and members are stopped with less time, and it takes one tidy-up
    /// turn ([`crate::delivery::Prompt::Tidy`]), if there is time, before it
    /// ends [`Outcome::Abandoned`]. A second abandon can only bring the
    /// deadline forward; `within: 0` is a hard stop, with no turn. It
    /// outranks a pause: the tidy-up turn runs paused or not.
    #[serde(alias = "Stop")]
    Abandon { by: Abandoner, within: u64 },
    /// The brain cannot carry the task on: the provider rejects its context,
    /// the model refused, a limb it needs cannot be made, retries are
    /// exhausted.
    Fail(String),
}

/// A drain under way on a live task: what its [`Drain`] facts asked, with
/// what has happened since.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Draining {
    ToMax,
    Abandoning(Abandoning),
    Failing(String),
}

/// A task being abandoned, and how far through the stop protocol it is
/// ([`crate::cancel`]): first everything it owns stops, then its tidy-up
/// turn, then it ends; or, from its deadline, nothing more of its own runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Abandoning {
    pub by: Abandoner,
    /// When it must have exited: the earliest deadline it has been given.
    pub deadline: Timestamp,
    /// It has been asked for its tidy-up turn.
    pub told: bool,
    /// Its tidy-up turn has begun: the calls it makes now are its cleanup,
    /// and run.
    pub tidying: bool,
    /// Nothing more of its own runs, and everything it owns is stopped now:
    /// its deadline came ([`TaskEvent::Expired`], or a stop with no time
    /// left), or its tidy-up turn cannot be had ([`Drain::Fail`]).
    pub hard: bool,
}

impl Abandoning {
    fn new(by: Abandoner, at: Timestamp, within: u64) -> Abandoning {
        Abandoning {
            by,
            deadline: crate::cancel::deadline(at, within),
            told: false,
            tidying: false,
            hard: within == 0,
        }
    }

    /// Bring the deadline forward to `within` of `at`, if that is sooner.
    fn tighten(&mut self, at: Timestamp, within: u64) {
        self.deadline = self.deadline.min(crate::cancel::deadline(at, within));
        self.hard |= at >= self.deadline;
    }
}

/// Who abandoned a task: only Max, or the abandoning of its owner. No agent
/// can abandon a task it does not own, and a parent waiting on its scope
/// makes no calls at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Abandoner {
    Max,
    /// The task that owns its scope is being abandoned.
    Owner,
}

/// How a task ended. Each reaches its owner as this member's result; an
/// abandonment says who, so a parent does not relaunch what Max abandoned.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    /// Its last message, the assistant entry `entry`.
    Result {
        entry: EntryId,
    },
    Failed {
        reason: String,
    },
    /// Done, but abandoned: nobody wants the work any more. `report` is the
    /// last message of its tidy-up turn: where it left things. `None` when
    /// it had no turn: abandoned before it started, or at or past its
    /// deadline.
    Abandoned {
        by: Abandoner,
        report: Option<EntryId>,
    },
}

/// A fact only the task itself owns. Everything else that moves it is a
/// fact of another identity, folded in by `Task::apply`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskEvent {
    Created(Box<Created>),
    ModeSet(Mode),
    Renamed(String),
    /// Max sets the model and effort its next requests run at
    /// ([`crate::environment::Tuning`]), the whole setting at once.
    Tuned(crate::environment::Tuning),
    Moved(Move),
    Drain(Drain),
    /// The deadline of the task's abandonment has come: from now nothing
    /// more of its own runs, and everything it owns is stopped at once. The
    /// brain states it at or after the deadline; Max's equivalent is an
    /// abandon with no time left.
    Expired,
    /// Max pauses or resumes the task and everything under it ([`Pause`]).
    #[serde(alias = "Held")]
    Pause(Pause),
}

/// Max's control over whether a subtree's agent loops run, as he last set
/// it on one task. It applies to the task and everything under it, and
/// along any chain the newest word wins ([`State::pause`]), so he can pause
/// a whole tree and then resume one branch of it, or pause one branch
/// inside a tree he is parking.
///
/// **Concept.** Pausing is not ending. A paused task's work is still
/// wanted; nothing it owns is stopped, and nothing is lost. Its first click
/// is [`Pause::Now`], the second [`Pause::Park`].
///
/// **Vision.** Max stays in charge of his usage and his attention without
/// cost to the work: near the usage limit he freezes a tree at once and
/// spends nothing more on it; leaving for the day he parks it, so each
/// agent leaves its work safe and says where it stands; either resumes from
/// exactly where it was. Background work is the agent's to keep or stop,
/// since its life is not over; only an abandon closes everything.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Pause {
    /// Resume: the loops run. A task that parked is told it has been
    /// resumed ([`crate::delivery::Prompt::Resume`]); any other carries on
    /// as if nothing happened.
    Off,
    /// Nothing new starts under it: no request is sent and no call is
    /// dispatched. Nothing running is cut: a response in flight finishes and
    /// its tool uses wait undispatched, running calls finish and their
    /// results wait, background work runs on. No turn is spent, and the
    /// model is never told.
    Now,
    /// "Pause for now, please": as [`Pause::Now`], but each task under it
    /// that is in the middle of its own loop is asked to bring its work to
    /// a safe point ([`crate::delivery::Prompt::Park`]) and runs as if not
    /// paused until that turn ends, when it is [`Phase::Parked`]. A task
    /// waiting on Max, on its members or on background work is already at
    /// rest and spends no turn.
    Park,
}

impl<'de> Deserialize<'de> for Pause {
    /// Also reads the `Held(bool)` facts an earlier build stated.
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Pause, D::Error> {
        #[derive(Deserialize)]
        enum Named {
            Off,
            Now,
            Park,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Held(bool),
            Named(Named),
        }
        Ok(match Wire::deserialize(d)? {
            Wire::Held(false) | Wire::Named(Named::Off) => Pause::Off,
            Wire::Held(true) | Wire::Named(Named::Now) => Pause::Now,
            Wire::Named(Named::Park) => Pause::Park,
        })
    }
}

/// What Max last said about pausing a task, and when: the newest word on a
/// chain of tasks wins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Paused {
    pub pause: Pause,
    pub at: Timestamp,
}

impl TaskEvent {
    pub(crate) fn values(&self, out: &mut Vec<ValueId>) {
        match self {
            TaskEvent::Created(created) => {
                let Created {
                    name: _,
                    by: _,
                    scope: _,
                    origin: _,
                    mode: _,
                    mandate,
                    tuning: _,
                } = &**created;
                let Mandate {
                    text,
                    brief,
                    quotes,
                } = mandate;
                out.push(*text);
                out.extend(*brief);
                out.extend(quotes);
            }
            TaskEvent::Moved(Move::Root(start)) => start.values(out),
            TaskEvent::Moved(Move::Compact(compaction)) => {
                let Compaction {
                    by: _,
                    onto,
                    carry,
                    renders: _,
                } = &**compaction;
                if let Onto::Fresh(start) = onto {
                    start.values(out);
                }
                out.push(*carry);
            }
            TaskEvent::Moved(Move::Back { to: _ })
            | TaskEvent::Tuned(_)
            | TaskEvent::ModeSet(_)
            | TaskEvent::Renamed(_)
            | TaskEvent::Drain(_)
            | TaskEvent::Expired
            | TaskEvent::Pause(_) => {}
        }
    }
}

/// A task begins. Its environment and places, and for a fork, resume or
/// aside its context, follow from its origin.
///
/// Who may create one is checked by the brain's fold. Max creates in his own
/// scope, with `by` empty, and his mandate is his words. The brain creates
/// only as the effect of an agent's meta call, `by`: in the scope that call
/// opens, or as a sibling in its caller's own spawn scope, and only what the
/// caller's environment can start. So the tree records who started each
/// task, and the brain cannot put a task in Max's scope that he did not
/// start.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Created {
    pub name: String,
    /// The meta call that created it; `None` when Max did.
    pub by: Option<CallId>,
    pub scope: Scope,
    pub origin: Origin,
    pub mode: Mode,
    pub mandate: Mandate,
    /// The model and effort it runs at, over its environment's: chosen by
    /// Max in the card that starts a chat. A fork, an aside or a resume
    /// takes its source's where this says nothing.
    #[serde(default)]
    pub tuning: crate::environment::Tuning,
}

/// The head moved to an entry that does not extend it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Move {
    /// A starting task's first root, once every limb it reaches has
    /// described itself. A task has one: every later context is a
    /// [compaction](Move::Compact).
    Root(Box<Start>),
    Compact(Box<Compaction>),
    /// Max puts the task back at an entry it has been at: its start, or an
    /// entry one of its own requests sent or received since. Retry, edit,
    /// going back and choosing between versions of a reply are each this
    /// one move to a different entry; the path it leaves stays in the tree,
    /// where a face shows it as another version. What follows is read off
    /// the entry, so nothing beside it says what kind of move it was:
    ///
    /// - a response that ended a turn: Max's move ([`Phase::AwaitingMax`]),
    ///   whatever the mode. Going back, or picking another version.
    /// - the head of one of its requests: the model's move. The next
    ///   request extends it with nothing new unless something is pending,
    ///   so it is another sample of that response: a retry. On the head of
    ///   the request that rendered Max's message, the whole turn is retried.
    /// - a response with tool uses: the model's move, its round's results
    ///   rendered again from their settlements, not run again.
    /// - its start: the model's move, from the mandate.
    ///
    /// Only the context moves; the world keeps everything that happened.
    /// The context embodies again exactly what it embodied at the entry, so
    /// a file the left path read or wrote is a notice at the next request
    /// like any other change. Whatever a request rendered on the path it
    /// leaves that the path it lands on has not is pending again (a
    /// background result, Max's action in the chat, a harness prompt),
    /// except Max's own words, which he is leaving, and what came of calls
    /// started on the left path. When the left path changed the world (its
    /// calls wrote files, its background work still runs, it started
    /// tasks), the next request says so ([`Note::Moved`]); a retry in a
    /// chat that touched nothing is the same request as the first. Its
    /// background work runs on, the task's to keep or stop, as through a
    /// pause.
    ///
    /// Refused while a request or round is outstanding
    /// ([`Rejected::Busy`]): Max stops the task first. (The target is a
    /// move that stops the round itself, as an interrupt does, and lands
    /// once it has exited; it waits on the redesign of stopping.) Refused
    /// too while the task is starting, abandoning or failing, and once it
    /// is done.
    ///
    /// **Vision.** Max as a participant who "very, very frequently" changes
    /// his mind: retry, edit and revert as in other chat apps, with nothing
    /// lost, since every version stays in the tree and he can move back to
    /// it; and a retry exposing the model's nondeterminism as another
    /// record on one input ([`crate::model_call::ModelCall`]).
    ///
    /// **Existing harnesses.** claude.ai keeps a reply's versions as
    /// siblings and an edit as a new branch. Claude Code's rewind truncates
    /// the conversation to before a message and offers to restore the files
    /// its own edit tools changed; OpenCode's revert restores a git snapshot
    /// of the worktree. Here the files stay as they are and the context is
    /// told; restoring them is a separate act a workspace limb could offer
    /// Max, so going back to keep the code or to drop it are both one act.
    #[serde(alias = "Rewind")]
    Back {
        to: EntryId,
    },
}

/// The task moves between phases of its work without losing itself: what
/// it did since its latest [checkpoint](crate::state::State::checkpoint) is
/// replaced by what it carries forward.
///
/// **Concept.** A compaction is a move of the head that appends. The head
/// goes back to where the context should resume ([`Onto`]), and one user
/// message is appended there: the carry, which holds the agent's summary,
/// Max's words from what it replaced, verbatim, the output of every call of
/// the round whose tool uses it replaced, and the background work the task
/// still owns. The agent carries on from the carry in the same turn: with
/// its next step, or, when its work is done, with its report, which ends
/// the turn as any answer does. So a report written after a compaction
/// follows the carry, it is what Max reads and an autonomous task's result,
/// and it says only what the agent still knows. Nothing is deleted: what was replaced is the branch that ended at
/// the old head, still in the tree for Max and the face to read, and the
/// carry's renders say where each block came from.
///
/// Compacting onto the latest checkpoint is the ordinary case. Every
/// earlier checkpoint stays verbatim (the first message of the context, the
/// message that began each model turn, each earlier carry), so in a long
/// chat Max's messages and the agent's reports stay word for word and only
/// the tool calls between them go; each summary covers one phase, so its
/// cost is that phase's and it never summarises a summary; and the prefix
/// up to the checkpoint is already cached. Compacting onto a fresh root is
/// the reset, when even that has grown long, or to pick up a changed role,
/// model, instructions or tool set.
///
/// **Vision.** "The agent can be considered the same despite the
/// compaction" (Max, 2026-10-17): the carry is the agent's own note, never
/// a handover to someone else. The fork-per-phase want ("the main context
/// is left with only the summaries … rather than many, many tool calls")
/// without having to foresee the phases: a compaction leaves the same tree
/// a returning fork does, a side branch summarised at its branch point.
/// Cache-aware compaction (`compaction.md`): the prefix is kept, not
/// re-sent. "Compaction never destroys the transcript" (critique §3). The
/// testimony's silent mutation: what the context embodied is kept through
/// it, so notices and stale-write refusals still hold.
///
/// **Existing harnesses.** Oh My Pi, Pi and OpenCode compact by replacing
/// the conversation with a summary of all of it plus a replayed tail
/// (`ref-oh-my-pi/packages/coding-agent/src/session/session-context.ts:355`,
/// `ref-opencode/packages/schema/src/session-message.ts:255`): each
/// compaction summarises the last summary, the user's words survive only as
/// the summariser retold them, and the cache starts cold.
///
/// **Bx.** Shared: a compaction is a record whose input is the old head and
/// whose output is the new one, both named by hash; the replaced branch is
/// never deleted, so it stays addressable; provenance is in the record
/// ([`Compaction::renders`]), not in the bytes. Not shared: a summary is
/// written by a model, so the mapping from old head to new is neither
/// deterministic nor lossless, and is recorded rather than re-derivable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Compaction {
    /// The task's own compaction call, settled, once the round that made it
    /// is over. `None` when the harness compacts: a context the provider no
    /// longer accepts, or a task with no compaction tool.
    pub by: Option<CallId>,
    pub onto: Onto,
    /// The content of the user message appended after `onto`.
    pub carry: ValueId,
    /// What `carry`'s blocks were rendered from, in block order.
    pub renders: Vec<Span>,
}

/// Where a compaction resumes the context.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Onto {
    /// The task's latest checkpoint: everything up to it is kept verbatim.
    Checkpoint(EntryId),
    /// A new root, rendered from the environment's current version, where a
    /// changed role, model, instructions or tool set takes effect. It has
    /// no attachments of its own: they are in the carry.
    Fresh(Box<Start>),
}

impl Compaction {
    /// The entry the carry is: a user message after `onto`.
    pub fn carry_entry(&self) -> Entry {
        Entry::Message {
            parent: match &self.onto {
                Onto::Checkpoint(at) => *at,
                Onto::Fresh(start) => start.root.id(),
            },
            message: Message {
                role: Role::User,
                content: self.carry,
            },
        }
    }
}

impl Task {
    /// The task its creation describes, reading its source from the
    /// brain's state. A fresh task starts with no context; any other
    /// continues its source's.
    pub(crate) fn new(
        id: TaskId,
        created: &Created,
        mark: Mark,
        world: &State,
    ) -> Result<Task, Rejected> {
        let mut pending = Vec::new();
        let mut max_actions = Vec::new();
        let mut tuning = created.tuning.clone();
        let (environment, places, life) = match &created.origin {
            Origin::Fresh {
                environment,
                places,
            } => (*environment, places.clone(), Life::Starting),
            Origin::Fork { from } | Origin::Resume { from } | Origin::Aside { from } => {
                let source = world.task(*from).ok_or(Rejected::Unknown)?;
                tuning = source.tuning.or(&created.tuning);
                let resume = matches!(created.origin, Origin::Resume { .. });
                match (resume, source.is_done()) {
                    (true, false) => return Err(Rejected::NotFinished),
                    (false, true) => return Err(Rejected::Finished),
                    _ => {}
                }
                let context = source.context().ok_or(Rejected::Starting)?;
                if resume {
                    pending = source.pending.clone();
                    max_actions = source.max_actions.clone();
                }
                let life = Life::Live(Live::new(context.continued()));
                (source.environment, source.places.clone(), life)
            }
        };
        Ok(Task {
            id,
            name: created.name.clone(),
            environment,
            places,
            scope: created.scope,
            origin: created.origin.clone(),
            by: created.by,
            mode: created.mode,
            mandate: created.mandate.clone(),
            tuning,
            pause: None,
            created: mark,
            since: mark.at,
            pending,
            background: Vec::new(),
            max_actions,
            life,
        })
    }

    /// Its live part, once it has started and until it is done.
    pub fn live(&self) -> Option<&Live> {
        match &self.life {
            Life::Live(live) => Some(live),
            Life::Starting | Life::Done { .. } => None,
        }
    }

    /// Its phase, while it is live.
    pub fn phase(&self) -> Option<&Phase> {
        self.live().map(|l| &l.phase)
    }

    /// The context it works in, or finished in.
    pub fn context(&self) -> Option<&Context> {
        match &self.life {
            Life::Live(live) => Some(&live.context),
            Life::Done { context, .. } => context.as_ref(),
            Life::Starting => None,
        }
    }

    /// Where its context ends now, once it has one.
    pub fn head(&self) -> Option<EntryId> {
        self.context().map(|c| c.head)
    }

    pub fn is_done(&self) -> bool {
        matches!(self.life, Life::Done { .. })
    }

    pub fn outcome(&self) -> Option<&Outcome> {
        match &self.life {
            Life::Done { outcome, .. } => Some(outcome),
            Life::Starting | Life::Live(_) => None,
        }
    }

    /// The drain pending on it, if any.
    pub fn drain(&self) -> Option<&Draining> {
        self.live().and_then(|l| l.drain.as_ref())
    }

    /// Whether it is being stopped or failed: nothing new may start under
    /// it, and it ends once what it owns has.
    pub fn ends(&self) -> bool {
        ends(self.drain())
    }

    /// Since when Max has owed it a word, if he does.
    /// Whether it is in the middle of its own loop, so parking it costs a
    /// turn: a request in flight, a round of calls running, or results to
    /// answer. A round waiting only on running members is not its own work
    /// ([`State::scoped`] says so; the caller checks it).
    pub fn mid_loop(&self) -> bool {
        match self.phase() {
            Some(Phase::Thinking(_) | Phase::Acting(_)) => true,
            Some(Phase::Ready) => self.pending.iter().any(|p| matches!(p, Input::Result(_))),
            _ => false,
        }
    }

    pub fn owed_since(&self) -> Option<Timestamp> {
        self.live().and_then(|l| l.owed_since)
    }

    /// The calls whose settlement its fold reads: its round's, its
    /// background work, and Max's actions in its chat. This is the whole of
    /// "who reads a call", derived rather than indexed.
    pub fn awaited(&self) -> impl Iterator<Item = CallId> + '_ {
        let round = match self.phase() {
            Some(Phase::Acting(step)) => step.outstanding.as_slice(),
            _ => &[],
        };
        round
            .iter()
            .chain(&self.background)
            .chain(&self.max_actions)
            .copied()
    }

    pub fn awaits(&self, call: CallId) -> bool {
        self.awaited().any(|c| c == call)
    }

    fn live_mut(&mut self) -> Result<&mut Live, Rejected> {
        match &mut self.life {
            Life::Live(live) => Ok(live),
            Life::Starting => Err(Rejected::Starting),
            Life::Done { .. } => Err(Rejected::Finished),
        }
    }

    /// Fold one fact that concerns this task, or refuse it. The caller folds
    /// into a copy and keeps it only on success.
    ///
    /// This is the whole agent loop. Three properties hold by construction:
    /// no request is sent while a drain is pending or a tool use of the last
    /// response is unanswered; a task is done only when nothing it started
    /// is outstanding and nothing waking is unread; and Max's words are never
    /// lost, to a turn that ends first or to a compaction.
    pub(crate) fn apply(&mut self, record: &Stated, world: &State) -> Result<(), Rejected> {
        let at = record.at;
        if self.is_done() {
            return self.after_done(record);
        }
        match &record.fact {
            Fact::Task(id, event) if *id == self.id => self.event(event, at, world),
            Fact::ModelCall(id, event) => self.model_call(*id, event, at, world),
            Fact::Call(id, event) => self.call(*id, event, at, world),
            Fact::Delivery(delivery) if delivery.to == self.id => {
                self.deliver(delivery, &record.by, at)
            }
            Fact::Change(change) => {
                if self.woken_by(change) {
                    self.enter(Phase::Ready, at);
                }
                Ok(())
            }
            _ => Err(Rejected::NotForTask),
        }
    }

    /// A finished task folds only a new name, and Max's actions in its chat,
    /// which ride into a resume of it. Provisional: Max's choice between
    /// that and refusing them is open.
    fn after_done(&mut self, record: &Stated) -> Result<(), Rejected> {
        match &record.fact {
            Fact::Task(id, TaskEvent::Renamed(name)) if *id == self.id => {
                self.name = name.clone();
                Ok(())
            }
            Fact::Call(id, CallEvent::Made(call)) if matches!(call.caller, Caller::Max { chat: Some(t), .. } if t == self.id) =>
            {
                self.max_actions.push(*id);
                Ok(())
            }
            Fact::Call(id, CallEvent::Settled(_)) => {
                let i = self
                    .max_actions
                    .iter()
                    .position(|c| c == id)
                    .ok_or(Rejected::Finished)?;
                self.max_actions.remove(i);
                self.pending.push(Input::MaxAction(*id));
                Ok(())
            }
            _ => Err(Rejected::Finished),
        }
    }

    fn event(&mut self, event: &TaskEvent, at: Timestamp, world: &State) -> Result<(), Rejected> {
        match event {
            TaskEvent::Created(_) => return Err(Rejected::Exists),
            TaskEvent::ModeSet(mode) => {
                if *mode == Mode::Standing && self.scope != Scope::Max {
                    return Err(Rejected::WrongScope);
                }
                self.mode = *mode;
                if let Life::Live(live) = &mut self.life {
                    if *mode != Mode::UserFacing {
                        live.owed_since = None;
                    }
                    // A waiting task waits for what its new mode waits for.
                    // It never completes here: the model has not seen the
                    // change.
                    if matches!(live.phase, Phase::AwaitingMax | Phase::Listening)
                        && live.drain.is_none()
                    {
                        self.wait(at, world);
                        // An autonomous task left waiting on nothing would
                        // never move again: mark it done instead, or say
                        // something with the change.
                        if *mode == Mode::Autonomous
                            && self.phase() == Some(&Phase::Listening)
                            && self.background.is_empty()
                        {
                            return Err(Rejected::Idle);
                        }
                    }
                }
            }
            TaskEvent::Renamed(name) => self.name = name.clone(),
            TaskEvent::Tuned(tuning) => self.tuning = tuning.clone(),
            TaskEvent::Moved(Move::Root(start)) => {
                if self.life != Life::Starting {
                    return Err(Rejected::Exists);
                }
                self.root(start, at, world)?;
            }
            TaskEvent::Moved(Move::Compact(compaction)) => {
                self.compact(compaction, world)?;
            }
            TaskEvent::Moved(Move::Back { to }) => self.back(*to, at, world)?,
            TaskEvent::Drain(drain) => match &mut self.life {
                Life::Starting => match drain {
                    Drain::ToMax { .. } => return Err(Rejected::Starting),
                    Drain::Abandon { by, .. } => {
                        let outcome = Outcome::Abandoned {
                            by: *by,
                            report: None,
                        };
                        self.finish(outcome, at);
                    }
                    Drain::Fail(reason) => self.finish(
                        Outcome::Failed {
                            reason: reason.clone(),
                        },
                        at,
                    ),
                },
                Life::Live(live) => {
                    live.drain = Some(match (live.drain.take(), drain) {
                        // A stop under way can only be made harder.
                        (Some(Draining::Abandoning(mut s)), Drain::Abandon { within, .. }) => {
                            s.tighten(at, *within);
                            Draining::Abandoning(s)
                        }
                        // Its tidy-up turn cannot be had.
                        (Some(Draining::Abandoning(mut s)), Drain::Fail(_)) => {
                            s.hard = true;
                            Draining::Abandoning(s)
                        }
                        // An ending outranks an interrupt; the first ending
                        // stands.
                        (Some(ending @ (Draining::Abandoning(_) | Draining::Failing(_))), _) => {
                            ending
                        }
                        (_, Drain::ToMax { .. }) => Draining::ToMax,
                        (_, Drain::Abandon { by, within }) => {
                            Draining::Abandoning(Abandoning::new(*by, at, *within))
                        }
                        (_, Drain::Fail(reason)) => Draining::Failing(reason.clone()),
                    });
                    if !matches!(live.phase, Phase::Thinking(_) | Phase::Acting(_)) {
                        self.drained(at);
                    }
                }
                Life::Done { .. } => return Err(Rejected::Finished),
            },
            TaskEvent::Expired => {
                let live = self.live_mut()?;
                let Some(Draining::Abandoning(stopping)) = &mut live.drain else {
                    return Err(Rejected::NotForTask);
                };
                if at < stopping.deadline {
                    return Err(Rejected::Early);
                }
                stopping.hard = true;
                if !matches!(live.phase, Phase::Thinking(_) | Phase::Acting(_)) {
                    self.drained(at);
                }
            }
            TaskEvent::Pause(pause) => {
                self.pause = Some(Paused { pause: *pause, at });
            }
        }
        Ok(())
    }

    /// Begin the task's first context at `start`'s root, with its
    /// attachments to run first.
    fn root(&mut self, start: &Start, at: Timestamp, world: &State) -> Result<(), Rejected> {
        let mut live = Live::new(Context::rooted(start));
        if !start.awaits.is_empty() {
            live.phase = Phase::Acting(Step {
                response: None,
                outstanding: start.awaits.clone(),
            });
        }
        self.life = Life::Live(live);
        self.since = at;
        for call in &start.awaits {
            if world
                .call(*call)
                .ok_or(Rejected::Unknown)?
                .settled
                .is_some()
            {
                self.settled(*call, at)?;
            }
        }
        Ok(())
    }

    /// Replace what the task did since `compaction.onto` with its carry.
    /// The agent compacts once the round of its compaction call is over,
    /// carrying every other result of that round; the harness compacts a
    /// task between turns. Neither while it is stopping: its tidy-up turn
    /// is its last, with no next context. The task carries on, or keeps
    /// waiting.
    fn compact(&mut self, compaction: &Compaction, world: &State) -> Result<(), Rejected> {
        let Task { pending, life, .. } = self;
        let Life::Live(live) = life else {
            return Err(Rejected::Starting);
        };
        if live.drain.is_some() {
            return Err(Rejected::Draining);
        }
        match compaction.by {
            Some(call) if live.phase != Phase::Ready || !pending.contains(&Input::Result(call)) => {
                return Err(Rejected::Busy);
            }
            None if !waiting(&live.phase) => return Err(Rejected::Busy),
            _ => {}
        }
        // Nothing of the round is dropped unseen: its tool uses go, so
        // their outputs are carried. The compaction call's own is the
        // compaction.
        let carried = |p: &Input| compaction.renders.iter().any(|r| p.is(&r.from));
        let dropped = pending.iter().any(|p| match p {
            Input::Result(call) => Some(*call) != compaction.by && !carried(p),
            _ => false,
        });
        if dropped {
            return Err(Rejected::Unanswered);
        }
        let carry = compaction.carry_entry().id();
        let mut context = live.context.compacted(&compaction.onto, carry, world);
        learn(&mut context, &compaction.renders, carry, world)?;
        live.context = context;
        pending.retain(|p| !matches!(p, Input::Result(_)));
        Ok(())
    }

    fn model_call(
        &mut self,
        id: ModelCallId,
        event: &ModelCallEvent,
        at: Timestamp,
        world: &State,
    ) -> Result<(), Rejected> {
        match event {
            ModelCallEvent::Sent(call) => {
                if call.task != self.id {
                    return Err(Rejected::NotForTask);
                }
                let Task { pending, life, .. } = self;
                let Life::Live(live) = life else {
                    return Err(Rejected::Starting);
                };
                match &mut live.drain {
                    None => {}
                    // Its tidy-up turn, once it has been asked for it.
                    Some(Draining::Abandoning(s)) if s.told && !s.hard => s.tidying = true,
                    Some(_) => return Err(Rejected::Draining),
                }
                if live.phase != Phase::Ready {
                    return Err(Rejected::Busy);
                }
                if live.context.head != call.base {
                    return Err(Rejected::Detached);
                }
                let rendered = |p: &Input| call.renders.iter().any(|r| p.is(&r.from));
                if pending
                    .iter()
                    .any(|p| matches!(p, Input::Result(_)) && !rendered(p))
                {
                    return Err(Rejected::Unanswered);
                }
                if call.renders.iter().any(|r| {
                    matches!(r.from, Rendered::Delivery(d) if !pending.contains(&Input::Delivery(d)))
                }) {
                    return Err(Rejected::NotForTask);
                }
                if let Some(from) = live.context.moved
                    && !call
                        .renders
                        .iter()
                        .any(|r| r.from == Rendered::Note(Note::Moved { from }))
                {
                    return Err(Rejected::Unanswered);
                }
                let head = call.head();
                learn(&mut live.context, &call.renders, head, world)?;
                pending.retain(|p| !rendered(p));
                live.context.head = head;
                live.context.moved = None;
                self.enter(Phase::Thinking(id), at);
            }
            ModelCallEvent::FirstByte => {}
            ModelCallEvent::Ended { ending, .. } => {
                let live = self.live_mut()?;
                if live.phase != Phase::Thinking(id) {
                    return Err(Rejected::Busy);
                }
                match ending {
                    Ending::Responded { content, stop } => {
                        let entry = Message {
                            role: Role::Assistant,
                            content: *content,
                        }
                        .id_after(live.context.head);
                        live.context.head = entry;
                        live.failures = 0;
                        let calls = world
                            .model_call(id)
                            .map(|m| m.calls.clone())
                            .unwrap_or_default();
                        if !calls.is_empty() {
                            self.enter(
                                Phase::Acting(Step {
                                    response: Some(entry),
                                    outstanding: calls,
                                }),
                                at,
                            );
                        } else if *stop == Stop::EndTurn {
                            self.turn_ended(entry, at, world);
                        } else {
                            // Stopped short of ending its turn: the brain
                            // carries on with a continue note, roots a
                            // context too long anew, or fails a refusal.
                            self.drained(at);
                        }
                    }
                    Ending::Failed { .. } => {
                        live.failures += 1;
                        self.drained(at);
                    }
                    Ending::Limited { .. } | Ending::Cut { .. } | Ending::Lost => {
                        self.drained(at);
                    }
                }
            }
        }
        Ok(())
    }

    fn call(
        &mut self,
        id: CallId,
        event: &CallEvent,
        at: Timestamp,
        world: &State,
    ) -> Result<(), Rejected> {
        match event {
            CallEvent::Made(call) => match &call.caller {
                Caller::Background { task, .. } if *task == self.id => {
                    if self.live_mut()?.drain.is_some() {
                        return Err(Rejected::Draining);
                    }
                    self.background.push(id);
                    Ok(())
                }
                Caller::Max {
                    chat: Some(task), ..
                } if *task == self.id => {
                    self.max_actions.push(id);
                    Ok(())
                }
                _ => Err(Rejected::NotForTask),
            },
            CallEvent::Settled(_) if world.call(id).is_some_and(|c| c.settled.is_some()) => {
                self.pending.push(Input::Late(id));
                if self.drain().is_none() {
                    self.woken(at);
                }
                Ok(())
            }
            CallEvent::Settled(_) => self.settled(id, at),
            CallEvent::Passed { to, .. } => {
                if let Some(i) = self.background.iter().position(|c| *c == id) {
                    self.background.remove(i);
                } else if *to == Owner::Task(self.id) {
                    if self.drain().is_some() {
                        return Err(Rejected::Draining);
                    }
                    self.background.push(id);
                    self.pending.push(Input::Passed(id));
                } else {
                    return Err(Rejected::NotForTask);
                }
                Ok(())
            }
            CallEvent::Dispatched | CallEvent::Stopping { .. } | CallEvent::Abandon => {
                Err(Rejected::NotForTask)
            }
        }
    }

    /// A call this task waits on has settled: one of its round, its
    /// background work, or one of Max's actions in its chat.
    fn settled(&mut self, call: CallId, at: Timestamp) -> Result<(), Rejected> {
        if let Life::Live(live) = &mut self.life
            && let Phase::Acting(step) = &mut live.phase
            && let Some(i) = step.outstanding.iter().position(|c| *c == call)
        {
            step.outstanding.remove(i);
            let done = step.outstanding.is_empty();
            self.pending.push(Input::Result(call));
            if done {
                self.drained(at);
            }
            return Ok(());
        }
        if let Some(i) = self.background.iter().position(|c| *c == call) {
            self.background.remove(i);
            self.pending.push(Input::Background(call));
            if self.drain().is_some() {
                // Abandoning: the last background call to settle lets it finish.
                if self.phase() == Some(&Phase::Listening) && self.background.is_empty() {
                    self.drained(at);
                }
            } else {
                self.woken(at);
            }
            return Ok(());
        }
        let i = self
            .max_actions
            .iter()
            .position(|c| *c == call)
            .ok_or(Rejected::NotForTask)?;
        self.max_actions.remove(i);
        self.pending.push(Input::MaxAction(call));
        Ok(())
    }

    fn deliver(&mut self, delivery: &Delivery, by: &Author, at: Timestamp) -> Result<(), Rejected> {
        match delivery.content {
            Content::Prompt(Prompt::Tidy) => match &mut self.live_mut()?.drain {
                Some(Draining::Abandoning(s)) if !s.told && !s.hard => s.told = true,
                _ => return Err(Rejected::NotForTask),
            },
            Content::Prompt(Prompt::Park) => {
                let live = self.live_mut()?;
                if live.drain.is_some() || live.parking.is_some() || live.phase == Phase::Parked {
                    return Err(Rejected::NotForTask);
                }
                live.parking = Some(delivery.id);
            }
            Content::Prompt(Prompt::Resume) => {
                let live = self.live_mut()?;
                if live.parking.is_none() && live.phase != Phase::Parked {
                    return Err(Rejected::NotForTask);
                }
                live.parking = None;
                self.pending.push(Input::Delivery(delivery.id));
                if self.phase() == Some(&Phase::Parked) {
                    self.enter(Phase::Ready, at);
                }
                return Ok(());
            }
            _ => {}
        }
        self.pending.push(Input::Delivery(delivery.id));
        // Marking a task done hands it its last turn: it finishes when that
        // turn ends.
        if delivery.content == Content::Prompt(Prompt::Done) {
            self.mode = Mode::Autonomous;
            if let Life::Live(live) = &mut self.life {
                live.owed_since = None;
            }
        }
        if matches!(delivery.content, Content::Words(_))
            && matches!(by, Author::Max)
            && let Life::Live(live) = &mut self.life
        {
            live.owed_since = None;
        }
        if self.drain().is_none() {
            self.woken(at);
        }
        Ok(())
    }

    /// Something that wakes arrived: if the task is waiting, it is its move
    /// again.
    fn woken(&mut self, at: Timestamp) {
        if matches!(self.phase(), Some(Phase::AwaitingMax | Phase::Listening)) {
            self.enter(Phase::Ready, at);
        }
    }

    fn wakes(&self) -> bool {
        self.pending.iter().any(Input::wakes)
    }

    /// Whether `change` ends this task's wait: a standing task listens to
    /// everything its context embodies, any task to news
    /// ([`Change::wakes`]). Read off the change itself, so the fold of a
    /// change never depends on whether the world has recorded it yet.
    pub(crate) fn woken_by(&self, change: &Change) -> bool {
        self.live().is_some_and(|l| {
            let waiting = match l.phase {
                Phase::Listening => true,
                Phase::AwaitingMax => change.wakes,
                _ => false,
            };
            waiting
                && (change.wakes || self.mode == Mode::Standing)
                && l.drain.is_none()
                && l.context.moved_by(change)
        })
    }

    /// Whether something its context embodies has moved in a way that ends
    /// a wait: news, or anything for a standing task.
    fn news(&self, world: &State) -> bool {
        world
            .stale(self)
            .iter()
            .any(|s| s.wakes || self.mode == Mode::Standing)
    }

    /// The model ended its turn at `entry`: complete, wait, or carry on.
    fn turn_ended(&mut self, entry: EntryId, at: Timestamp, world: &State) {
        match self.drain() {
            // The tidy-up turn is over: its last message is the report.
            Some(Draining::Abandoning(s)) if s.tidying && !s.hard && self.background.is_empty() => {
                let outcome = Outcome::Abandoned {
                    by: s.by,
                    report: Some(entry),
                };
                return self.finish(outcome, at);
            }
            Some(_) => return self.drained(at),
            None => {}
        }
        // The turn that read the request to park ends parked.
        if let Life::Live(live) = &mut self.life
            && let Some(asked) = live.parking
            && !self.pending.contains(&Input::Delivery(asked))
        {
            live.parking = None;
            return self.enter(Phase::Parked, at);
        }
        if self.wakes() || self.news(world) {
            // Max's words, a prompt, or news arrived during the turn.
            return self.enter(Phase::Ready, at);
        }
        if self.mode == Mode::Autonomous && self.background.is_empty() {
            return self.finish(Outcome::Result { entry }, at);
        }
        self.wait(at, world);
    }

    /// Enter the wait the mode asks for, unless something already ends it.
    fn wait(&mut self, at: Timestamp, world: &State) {
        if self.news(world) {
            return self.enter(Phase::Ready, at);
        }
        if self.mode == Mode::UserFacing {
            return self.await_max(at);
        }
        let phase = if self.wakes() {
            Phase::Ready
        } else {
            Phase::Listening
        };
        self.enter(phase, at);
    }

    fn await_max(&mut self, at: Timestamp) {
        if self.wakes() {
            return self.enter(Phase::Ready, at);
        }
        if let Life::Live(live) = &mut self.life {
            live.owed_since.get_or_insert(at);
        }
        self.enter(Phase::AwaitingMax, at);
    }

    /// Nothing in the round is outstanding: carry on, or do what the drain
    /// says. A stopping task waits for its background work to stop, then is
    /// ready for its tidy-up turn, or ends if its deadline has come.
    fn drained(&mut self, at: Timestamp) {
        let Life::Live(live) = &mut self.life else {
            return;
        };
        match live.drain.clone() {
            None => self.enter(Phase::Ready, at),
            Some(Draining::ToMax) => {
                live.drain = None;
                live.parking = None;
                self.await_max(at);
            }
            Some(Draining::Abandoning(_) | Draining::Failing(_)) if !self.background.is_empty() => {
                self.enter(Phase::Listening, at);
            }
            Some(Draining::Abandoning(s)) if s.hard => {
                let outcome = Outcome::Abandoned {
                    by: s.by,
                    report: None,
                };
                self.finish(outcome, at);
            }
            Some(Draining::Abandoning(_)) => self.enter(Phase::Ready, at),
            Some(Draining::Failing(reason)) => self.finish(Outcome::Failed { reason }, at),
        }
    }

    fn finish(&mut self, outcome: Outcome, at: Timestamp) {
        let context = match std::mem::replace(&mut self.life, Life::Starting) {
            Life::Live(live) => Some(live.context),
            Life::Starting => None,
            Life::Done { context, .. } => context,
        };
        self.life = Life::Done { outcome, context };
        self.since = at;
    }

    fn enter(&mut self, phase: Phase, at: Timestamp) {
        if let Life::Live(live) = &mut self.life
            && live.phase != phase
        {
            live.phase = phase;
            self.since = at;
        }
    }

    /// Max puts the task at `to` ([`Move::Back`]), which the brain's state
    /// has checked it has been at. What follows is read off the entry.
    fn back(&mut self, to: EntryId, at: Timestamp, world: &State) -> Result<(), Rejected> {
        let id = self.id;
        let Task { pending, life, .. } = self;
        let Life::Live(live) = life else {
            return Err(Rejected::Finished);
        };
        if live.drain.is_some() {
            return Err(Rejected::Draining);
        }
        // Stopping the round first is Draining::Moving, not built: Max
        // stops a busy task himself before moving it.
        if !waiting(&live.phase) {
            return Err(Rejected::Busy);
        }
        let context = &mut live.context;
        let at_start = to == context.start;
        let response = world
            .response(to)
            .filter(|m| m.call.task == id && !at_start);
        // A response with tool uses: its round's results, rendered again.
        let round = response.map(|m| m.calls.clone()).unwrap_or_default();
        if round
            .iter()
            .any(|c| world.call(*c).and_then(|c| c.outcome()).is_none())
        {
            return Err(Rejected::Busy);
        }
        let ended_turn = round.is_empty()
            && response.is_some_and(|m| {
                matches!(
                    m.ended.as_ref().map(|e| &e.ending),
                    Some(Ending::Responded {
                        stop: Stop::EndTurn,
                        ..
                    })
                )
            });

        // What the path it lands on rendered, and what the path it leaves
        // rendered after the two part.
        let landed = renders(world, id, to, None);
        let left = renders(world, id, context.head, Some(to));
        pending.retain(|p| !matches!(p, Input::Result(_)) && !landed.iter().any(|r| p.is(r)));
        for r in &left {
            if landed.contains(r) {
                continue;
            }
            if let Some(input) = requeue(world, r, to, &round)
                && !pending.contains(&input)
            {
                pending.push(input);
            }
        }
        pending.extend(round.iter().map(|c| Input::Result(*c)));

        let from = context.moved.unwrap_or(context.head);
        context.move_to(to, id, world)?;
        context.moved = (!world.left(id, from, to).is_empty()).then_some(from);
        live.failures = 0;
        if live.phase == Phase::Parked {
            return Ok(());
        }
        if ended_turn {
            self.await_max(at);
        } else {
            self.enter(Phase::Ready, at);
        }
        Ok(())
    }
}

/// Whether a live task is waiting for a move rather than running one.
fn waiting(phase: &Phase) -> bool {
    matches!(
        phase,
        Phase::Ready | Phase::AwaitingMax | Phase::Listening | Phase::Parked
    )
}

/// Whether a drain ends the task, rather than handing the move to Max.
pub(crate) fn ends(drain: Option<&Draining>) -> bool {
    matches!(drain, Some(Draining::Abandoning(_) | Draining::Failing(_)))
}

/// Update what a context embodies from what a request newly included,
/// remembering what it was before.
fn learn(
    context: &mut Context,
    renders: &[Span],
    head: EntryId,
    world: &State,
) -> Result<(), Rejected> {
    let mut before = Vec::new();
    for Span { from, .. } in renders {
        match from {
            Rendered::Call(call) => {
                let outcome = world
                    .call(*call)
                    .ok_or(Rejected::Unknown)?
                    .outcome()
                    .ok_or(Rejected::Busy)?;
                if let CallOutcome::Refused { current, .. } = outcome {
                    context.tell(current, &mut before);
                }
                if let Some(seen) = outcome.seen() {
                    context.observe(&seen.observes, &mut before);
                }
            }
            Rendered::Delivery(_) => {}
            Rendered::Note(Note::Kept { words }) => {
                world.said(*words).ok_or(Rejected::Unknown)?;
            }
            Rendered::Note(Note::Stale { told, shown }) => {
                context.tell(told, &mut before);
                context.observe(shown, &mut before);
            }
            Rendered::Note(_) => {}
        }
    }
    if !before.is_empty() {
        context.learned.push_back(Learned { head, before });
    }
    Ok(())
}

/// What the requests and compactions of `task` on the path to `head`
/// rendered, back to where it meets the path to `stop` (or to its root),
/// counting each carry's replaced branch as on the path.
fn renders(world: &State, task: TaskId, head: EntryId, stop: Option<EntryId>) -> Vec<Rendered> {
    let stop = |e: EntryId| stop.is_some_and(|s| world.descends(s, e));
    world
        .path(task, head, &stop)
        .into_iter()
        .filter_map(|(entry, _)| world.rendered(task, entry))
        .flat_map(|renders| renders.iter().map(|s| s.from.clone()))
        .collect()
}

/// What a request on a path Max left rendered becomes when the path he
/// lands on (at `to`) has not: pending again, except his own words, which he
/// is leaving, what came of work started on the left path, and the task's
/// pause, which the move does not change.
fn requeue(world: &State, rendered: &Rendered, to: EntryId, round: &[CallId]) -> Option<Input> {
    match rendered {
        Rendered::Call(call) => {
            if round.contains(call) {
                return None;
            }
            let origin = world.origin(*call);
            if origin.is_some_and(|response| !world.descends(to, response)) {
                return None;
            }
            Some(match world.call(*call)?.call.caller {
                Caller::Max { .. } => Input::MaxAction(*call),
                // A tool use on the path it lands on, whose real outcome
                // came late.
                Caller::Agent { .. } if origin.is_some() => Input::Late(*call),
                Caller::Background { .. } => Input::Background(*call),
                _ => return None,
            })
        }
        Rendered::Delivery(id) => match world.delivery(*id)?.delivery.content {
            Content::Words(_) | Content::Prompt(Prompt::Park | Prompt::Resume) => None,
            Content::Prompt(_) => Some(Input::Delivery(*id)),
        },
        Rendered::Note(Note::Passed { call }) => Some(Input::Passed(*call)),
        Rendered::Note(_) => None,
    }
}
