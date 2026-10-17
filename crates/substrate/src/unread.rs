//! Records a build cannot read.

/// A record this build cannot read: what it says, as it was stored, and
/// why this build's schema does not parse it.
///
/// **Concept.** The substrate's half of "every build opens every
/// database". A record's envelope (its time, its clock, whether it is said
/// or a receipt, how long its writer declared it is kept) is the
/// substrate's and parses in every build. What it says is the schema's, and
/// a newer build may say something an older one has no word for. Such a
/// record is held exactly as stored: never dropped, never rewritten, kept
/// whatever durability its writer declared (this build cannot tell what
/// supersedes it). Its time still counts, so the place's next record comes
/// after it and a receipt of it still includes it. The fold skips it, and
/// the decider reports it beside its divergences. A build that reads it
/// folds it in its place the next time it opens the database.
///
/// **Vision.** "We want live editing, we don't want live bricking": going
/// back to the build before a bad one always opens what the bad one wrote,
/// and going forward again loses nothing. The database is the one source
/// of truth across builds, with no migrations.
///
/// **Existing harnesses.** A migration runs forward once, and an older
/// build then refuses the database (Rails, Django). Or every reader skips
/// unknown fields by convention, and silently drops them when it writes
/// the row back (protobuf).
///
/// **Different here.** By construction: no build can fail to open the
/// database because of what another build wrote, records are never written
/// back so nothing is lost by a build that cannot read them, and what was
/// skipped is a value Max and agents see, not a log line.
///
/// Not wired yet: [`crate::What`] gains `Unread(Unread)`, a store parses
/// the envelope and the payload apart, and [`crate::Decider::open`] skips
/// it in the fold and returns it with its divergences.
/// `harness-relaunch-spike` carries it inside its schema's vocabulary
/// until then.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unread {
    /// What the record says, as it was stored.
    pub bytes: Vec<u8>,
    /// Why this build does not parse it.
    pub why: String,
}
