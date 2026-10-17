//! The data flow substrate: the one way state moves between the parts of
//! the harness.
//!
//! The brain, every limb process and every browser tab is a [`PlaceId`]: a
//! replica with one timeline of its own. What a place says is a
//! [`Record`]: a time at that place, its clock, and what happened. Nothing
//! else crosses between places. There are no requests, replies or
//! refusals on a wire: a place ships its own records to the place that
//! decides what they are about, and that place ships back a view of what
//! they amount to.
//!
//! - **Up: facts go to whoever decides.** A follower (a tab, a limb) keeps
//!   its own records in a [`Log`] until its decider has included them, and
//!   ships every one its decider has not.
//! - **The decider orders them, in band.** A [`Decider`] (the brain) takes
//!   each record in, folds it into its state with the schema's rules, and
//!   writes an [`Included`] record of its own saying so: "my record `n`
//!   includes your record `m`". A record that breaks a rule is still held
//!   and still included, with the refusal in the [`Included`]: an invalid
//!   act stays a fact. The decider's own timeline is the only order of
//!   concurrent acts there is, so every replica agrees on who sent first.
//! - **Down: views go to whoever looks.** The decider derives, for each
//!   follower, the rows it wants, as of one of the decider's times, with the
//!   follower's [`Standing`]: how far the decider has included it and what
//!   it refused. A [`Publisher`] ships the rows that changed; a [`Replica`]
//!   holds them. So a follower knows its own act is in when it holds a view
//!   as of a decider time that includes it: "sent" is derived, never told.
//! - **Never-stored data takes no time.** Streamed tokens and command
//!   output are [`Live`]: shipped on the same link after the record they
//!   follow, never stored, never included, superseded by the record that
//!   ends them.
//! - **Durability is declared per record by the schema**
//!   ([`Durability`]): kept, or only the latest per key. Collection follows
//!   it alone, so no follower, however long away, holds anything back: a
//!   follower that returns gets a whole view.
//!
//! The substrate knows nothing of tasks or calls: a [`Schema`] supplies what
//! a place says, how it folds, why a fold refuses, and how long a record
//! lasts. `harness-core` is one schema. The crate uses no threads, files,
//! clocks, randomness or database, so it builds for `wasm32` and anything
//! may embed it: the embedding supplies the clock, the entropy for a new
//! place, the transport and the [`Storage`].
//!
//! # Vision
//!
//! "A and B contain filtered replicas of each other [...] unidirectionally
//! synced and then A can do a reduction or derivation over both"; no message
//! protocol between processes; the browser is a replica; a record is "a
//! time and a place, and a fact"; which times include which is in band;
//! invalid actions stay facts; durability kinds declared in band; a draft
//! has identity from its creation in the browser and its sent status is
//! derived from causality. From Max's database notes: "derived views run on
//! the backend's copy of all clients synchronized records, producing derived
//! state as at some transaction timestamp; the result is replicated back out
//! to the clients", and "I don't count my newly submitted state as valid
//! [...] until I've received the results of my streaming query *as at* a
//! time strictly later".
//!
//! # Not built yet
//!
//! - Snapshots of a decider's state, which a "kept until processed"
//!   durability needs: without one, a record a decider has folded is its
//!   only memory of the effect, so it is kept.
//! - A follower that runs the decider's fold itself ("if desired, the
//!   derived part can run on any client"): the follower would hold the
//!   decider's records rather than its rows. The types allow it; nothing
//!   uses it.
//! - Incremental views: a [`Publisher`] diffs whole row sets.

mod decider;
mod log;
mod record;
mod storage;
mod time;
mod unread;
mod view;

pub use decider::{Decider, Divergence, Error, Standing, Status};
pub use log::{Log, Restored};
pub use record::{Durability, Folding, Included, Live, Record, Schema, What};
pub use storage::{Memory, Storage};
pub use time::{EventId, ParseError, PlaceId, Time, Timestamp};
pub use unread::Unread;
pub use view::{Down, Publisher, Replica, Step, Up, View};
