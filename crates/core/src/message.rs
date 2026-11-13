//! Messages and watches: the two relationships between tasks that own
//! nothing.
//!
//! **Concept.** Owning is the one relationship tasks have: an agent hears
//! from what it started, when it ends. A message lets one live agent tell
//! another something now, and a watch lets an agent be told when work it
//! did not start ends. Neither owns: a message's call settles at once, and
//! a watched task's end is only reported to the watcher.
//!
//! **Vision.** "Two relationships that own nothing: messages and watches"
//! (`vision.md`). "We need an ability for a parent agent to pass messages to
//! its sub-agent without waiting for it to in turn" (Max, 12 Nov). "The three
//! of you will potentially have to collaborate, so I suggest we implement
//! messaging somehow. But messaging, I find it interesting because I feel
//! like it should show on the graph" (Max, 8 Nov, of Harness Features,
//! Harness UI and a third chat). The records on 12 and 13 Nov: Max's words
//! for a designer that reached it 30 minutes and one more resume late; four
//! stops of running agents in 25 hours only to add an instruction; peer
//! chats relaunching the brain over each other eleven times, with no way to
//! tell each other; and six times Max carried words between chats by hand.
//!
//! **Built.** The `meta` tool `message` states a message
//! ([`crate::delivery::Content::Message`]) in the batch that settles its
//! call, and a [`crate::call::Wait`] with `on` empty waits for words.
//! **Not yet folded.** `CallEvent::Watches(TaskId)` in
//! [`crate::call::CallEvent`]; `Dep::Said(DeliveryId)` and `Legs::said`
//! holding each said line on the map. Each new record kind is read by a
//! brain deployed before any brain writes it, so a relaunch back never
//! meets a record it can't fold.

use serde::{Deserialize, Serialize};

use crate::id::{CallId, TaskId, ValueId};

/// The `meta` tool one agent tells another with.
pub const MESSAGE: &str = "message";

/// The `history` tool an agent watches work it did not start with.
pub const WATCH: &str = "watch";

/// How long, in milliseconds, a message waits for its recipient's round to
/// end by itself, and each call of it has had since it was made, before the
/// round moves to the background. Ten times [`crate::state::HEARD_AFTER`]:
/// no one is waiting live, and a round that ends by itself costs the
/// recipient no extra request.
pub const TOLD_AFTER: u64 = 30_000;

/// Words one agent tells another: the content of a delivery the brain
/// states in the batch that settles the sender's `message` call, so a
/// restart never sends one twice.
///
/// **Concept.** A message lands as Max's words do, because it is the same
/// thing from another author. It is the recipient's pending input from the
/// moment it is stated until a request renders it, and it wakes a task that
/// waits: a `wait` settles at once, a chat waiting on Max takes a turn, and
/// a standing task listening wakes. Nothing reaches a context mid-round,
/// since the provider forbids it, so a message comes straight after the
/// round's results; a round still running [`TOLD_AFTER`] after both it
/// began and the message came moves to the background, so a correction
/// reaches a child in a long build or a foreground spawn in about half a
/// minute ([`crate::call::CallEvent::Backgrounded`]). A starting recipient
/// reads it after its mandate; a parked or paused one when Max resumes it,
/// since his controls win.
///
/// It renders as user text, so it can redirect: "\[harness\] `Harness
/// features`, which started you, wrote this while you were working, before
/// your tool call `bash` (`call-…`):", then the words, then each quote by
/// its label. The recipient has heard the quotes, so it can quote them on
/// ([`crate::task::Mandate::quotes`]). The relation carries the weight
/// ([`Kin`]): an owner's message is instructions, amending the mandate, so a
/// compaction keeps it verbatim with its quotes, as it keeps Max's words; a
/// member's is a report or a request, and a peer's a request it may
/// decline. Those are summarised like the rest.
///
/// The call settles at once, saying whether the recipient was working, was
/// woken or is parked: it owns nothing, and no one waits on it. Read is
/// derived, from the request that rendered it. An autonomous task can't
/// complete over an unread message; one that comes while it streams its
/// last response goes up to its owner with its result, as Max's late words
/// do. Only its owner can be that late: a member that tells it something
/// is still running, so it is too, and it has no peers.
///
/// An agent that can't go on without an answer waits for one with a
/// `wait` naming nothing, which any words end, Max's or a message. The one
/// circle that messages and owning can close, an owner waiting on a member
/// that waits to hear from it, never stands: a wait on such a member
/// settles at once and says so, and a member beginning such a wait settles
/// its owner's wait on it. A chat woken only by a message keeps its place
/// in Max's queue ([`crate::task::Live::owed_since`]), so no agent makes
/// work for him.
///
/// **Vision.** The parent passing words to its running sub-agent, and the
/// three Harness chats collaborating, above. "S5. Anything I tell an agent
/// lands on the graph. Not sure about that" (Max, 31 Oct): an agent's
/// message does, as a join on the map; his own words still
/// don't.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// The sender's `message` call. Who sent it, from which response and
    /// when are the call's, so a message can't claim a sender.
    pub call: CallId,
    /// What it says: text.
    pub words: ValueId,
    /// Max's words it hands on verbatim, by the `max-…` labels its sender
    /// heard.
    pub quotes: Vec<ValueId>,
}

/// What another task is to this one, when they can tell each other things:
/// who can address whom, by construction.
///
/// **Concept.** A message's `to` is resolved among the live tasks that are
/// one of these to its sender, by name or task id. A name outside them is
/// not refused; it names no one there, as a wrong path does, and the call
/// fails saying whom the sender can reach. A recipient that has finished,
/// or is draining, fails the call with its outcome and "resume it, if it's
/// yours". Every relation runs both ways ([`Kin::inverse`]), so whoever
/// tells you something you can answer.
///
/// Siblings, grandchildren and other chats' agents are reached through
/// their owner, who coordinates them and so should hear. Max's chats are
/// the exception, because their owner is Max, and he is not a relay. Max
/// himself is no one's kin: a chat talks to him in its turn, an autonomous
/// agent through its owner, and a question is an ask ([`crate::ask`]).
/// A standing task is reached through its own tools, never told things,
/// so no agent lobbies the scheduler.
///
/// **Vision.** "If an agent has a tool, it should always be allowed to use
/// the tool" (Max, 12 Nov): nothing checks a message against a policy;
/// whom it can reach is what the tool means. "Only an agent that has Max
/// can hand to Max."
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kin {
    /// The agent that started this one.
    Owner,
    /// A live agent this one started, in any of its spawns, background ones
    /// and the chats it opened for Max among them.
    Member,
    /// Another live chat that faces Max, when this one faces him too.
    Peer,
}

impl Kin {
    /// What this task is to the other.
    pub fn inverse(self) -> Kin {
        match self {
            Kin::Owner => Kin::Member,
            Kin::Member => Kin::Owner,
            Kin::Peer => Kin::Peer,
        }
    }
}

/// The input of a `watch` call: be told when a task this agent did not
/// start ends.
///
/// **Concept.** A watch is background work of the watcher's, under the name
/// it gives, held until the watched task ends, and settled with what its
/// owner hears: its result, or that it was abandoned, with its report.
/// The brain states `CallEvent::Watches(task)` when it runs one, as it
/// states a wait ([`crate::call::Wait`]), so the fold knows what it waits
/// on. A watch on a task that can't end before the watcher does would
/// close a circle of waits, so it settles at once and says why: the
/// watcher is inside the watched task's work, or something inside it
/// watches the watcher, directly or through others. A finished task is
/// read, with `context`, rather than watched.
///
/// **Vision.** "The watch isn't ownership, so the watch just gets told it
/// was abandoned"; "deadlock prevention by directed acyclic graph" (Max,
/// 28 Oct). "Can an agent, like, join an already finished thing that isn't
/// one thing that it spawned? Like, form a connection in the graph that
/// isn't something that itself started [...] I think yes. As we said, it's
/// not ownership though" (Max, 2 Nov). On 13 Nov Harness uptime waited over
/// 70 minutes for a pull request Harness Features was to hand it when the
/// agent writing it finished: a watch on that agent would have woken it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Watch {
    pub task: TaskId,
}
