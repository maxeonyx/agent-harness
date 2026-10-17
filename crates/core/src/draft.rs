//! Max's drafts: his words before they are a delivery.

use serde::{Deserialize, Serialize};

use crate::delivery::Read;
use crate::fact::Rejected;
use crate::id::{DeliveryId, EventId, TaskId, ValueId};
use crate::state::Mark;

/// What Max does to a draft, from any of his faces.
///
/// **Concept.** A draft is a message from its first keystroke: the event
/// that opens it names it ([`DeliveryId`]), it is attached to a session
/// without being part of its context, and the brain holds it, so the phone
/// shows what the PC typed and either can send it. No draft event is ever
/// refused for racing another: Max's words are never lost to a race. The
/// first Send the brain includes delivers the draft; a later Send of the
/// same words has nothing left to do; a later Send of other words (typed on
/// the phone while the PC sent) opens a new draft holding them, named by
/// that Send. Whether a Send delivered, found the draft already sent, or
/// opened a new draft is read off the draft, the same on every device.
///
/// **Vision.** "A draft or message has identity from creation in the
/// browser"; "its sent status is derived from causality"; drafts that
/// persist in the brain, attached to a session but not part of it, with
/// phone and PC both editing and sending.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DraftEvent {
    /// A new draft beside `session`.
    Opened { session: TaskId },
    /// The whole text when he paused typing. Only the newest is kept
    /// ([`crate::schema::Key::Draft`]); keystrokes in between are
    /// [`harness_substrate::Live`] if anything shows them.
    Text(ValueId),
    /// Deliver exactly `text`: the words on his screen, whatever another
    /// device typed meanwhile, for the session to read when `read` says.
    Send {
        text: ValueId,
        #[serde(default)]
        read: Read,
    },
    /// He threw it away. Text typed into it afterwards, on another device
    /// that had not seen the discard, brings it back.
    Discarded,
}

/// A draft as the brain holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    pub id: DeliveryId,
    pub session: TaskId,
    /// The newest text, in the brain's order.
    pub text: Option<ValueId>,
    /// The Send that delivered it.
    pub sent: Option<Sent>,
    /// Why the newest Send did not deliver: the session cannot take Max's
    /// words now (it has finished, say). The words stay here.
    pub held: Option<Rejected>,
    pub discarded: bool,
    /// The draft whose Send opened this one, when a concurrent Send of
    /// other words did.
    pub from: Option<DeliveryId>,
}

/// The Send that delivered a draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sent {
    /// The Send itself: which face sent it, at which of its times.
    pub by: EventId,
    pub text: ValueId,
    pub at: Mark,
}

/// What became of one Send, read off the drafts. The same on every device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fate {
    /// It delivered the draft.
    Delivered,
    /// The draft had already been sent with these words.
    Duplicate,
    /// Another Send had delivered the draft first; these words are a new
    /// draft, named by this Send.
    Successor,
    /// It did not deliver; the draft holds the words and says why.
    Held,
}
