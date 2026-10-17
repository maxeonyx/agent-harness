//! Attention: Max's queue, the order it is in, and what he is looking at.

use serde::{Deserialize, Serialize};

use crate::id::{CallId, TaskId, Timestamp};

/// The order in which waiting tasks should get Max's attention, as last
/// decided, with a reason for each.
///
/// **Concept.** Max's queue is not stored. It is derived
/// ([`crate::state::State::queue`]): a task is in it while Max owes it a
/// word ([`crate::task::Task::owed_since`]: a user-facing turn ended, or he
/// interrupted) or while it has an outstanding `ask` of his limb mid-turn.
/// Each keeps the time it started waiting, so a task keeps its place
/// through a harness turn (a handover before its cache expires) and leaves
/// only when Max answers.
///
/// The ranking only orders. It is keyed by task, so it never goes stale
/// because a task asked a second question, and a ranked task that no longer
/// waits drops out without anyone removing it. Tasks it has not placed come
/// after the ranked ones, oldest first, so the next item exists the instant
/// Max finishes one, and a slow, failed or usage-limited scheduler degrades
/// to a plain first-in first-out queue marked unranked.
///
/// The scheduler is an ordinary standing task whose environment holds only
/// the [attention limb](crate::environment::LimbKind::Attention), which
/// shows it the queue (a [resource](crate::resource::ResourceKey) it
/// observes, so a change to the queue wakes it, and changes that arrive
/// while it thinks fold into its next turn) and gives it one effect: a new
/// ranking. Max can reorder by hand with the same fact.
///
/// **Vision.** The attention scheduler; "the harness schedules the user"
/// (Max is the scarce resource every waiting task competes for); several
/// tasks needing him while he watches the tree; [`Focus`], so nothing moves
/// the task he is in from under him.
///
/// **Existing harnesses.** OpenCode, Codex, Pi and Oh My Pi each put one
/// user-facing session in front of Max at a time; a subagent that needs him
/// surfaces through its parent's session or not at all. None has a queue
/// across sessions, or an agent that orders one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ranking {
    pub order: Vec<Ranked>,
}

/// One task's place in the ranking, with the reason Max sees.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ranked {
    pub task: TaskId,
    pub why: String,
}

/// Which task the face stating it shows Max, if any. Faces differ: the
/// phone can be on the queue while the desktop is in a chat. The face is the
/// replica that states it, so no face can state another's. Only the newest per
/// face is kept ([`crate::schema::Key::Focus`]): clicking around all day
/// does not grow the database.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Focus {
    pub task: Option<TaskId>,
}

/// A task in Max's queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Queued {
    pub task: TaskId,
    /// Since when it has waited on him.
    pub since: Timestamp,
    /// Its outstanding question, when it asked one mid-turn.
    pub ask: Option<CallId>,
    /// The ranking's reason; `None` while unranked.
    pub why: Option<String>,
}

impl Ranking {
    /// The queue in order: ranked tasks first, then the rest oldest first.
    pub fn order(&self, mut queued: Vec<Queued>) -> Vec<Queued> {
        queued.sort_by_key(|q| q.since);
        let mut ordered = Vec::with_capacity(queued.len());
        for ranked in &self.order {
            if let Some(i) = queued.iter().position(|q| q.task == ranked.task) {
                let mut q = queued.remove(i);
                q.why = Some(ranked.why.clone());
                ordered.push(q);
            }
        }
        ordered.extend(queued);
        ordered
    }
}
