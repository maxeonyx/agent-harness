use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::decider::Standing;
use crate::record::{Live, Record, Schema};
use crate::time::Time;

/// What a follower holds of its decider: the rows it wants, as of one of
/// the decider's times, and its own standing there.
///
/// **Concept.** A replica of a derived relation. The decider derives each
/// follower's rows from its state (a tab's: the tasks on its map, the
/// session it has open, the drafts beside it; a limb's: the calls it must
/// run and stop), so a follower never folds what it cannot see in full.
/// `as_of` is the decider's time the rows are true at, and `standing` says
/// which of the follower's own records that time includes, so the follower
/// reads its acts' fate off the same view it shows. What Max had seen when
/// he acted is the `as_of` of the view on his screen, which his face states
/// as a fact of its own.
///
/// **Vision.** The face as a replica that renders derived state; "the
/// result is replicated back out to the clients"; "I don't count my newly
/// submitted state as valid [...] until I've received the results of my
/// streaming query *as at* a time strictly later".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "K: Serialize, R: Serialize, F: Serialize",
    deserialize = "K: Ord + Deserialize<'de>, R: Deserialize<'de>, F: Deserialize<'de>"
))]
pub struct View<K: Ord, R, F> {
    pub as_of: Time,
    pub standing: Standing<F>,
    pub rows: BTreeMap<K, R>,
}

/// A view moving on: the rows that changed since the follower's last
/// `as_of`, and its standing if that changed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Step<K, R, F> {
    pub as_of: Time,
    pub standing: Option<Standing<F>>,
    pub set: Vec<(K, R)>,
    pub gone: Vec<K>,
}

/// What a decider ships a follower: a whole view on connecting, then steps,
/// with [`Live`] data between them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "K: Serialize, R: Serialize, F: Serialize, L: Serialize",
    deserialize = "K: Ord + Deserialize<'de>, R: Deserialize<'de>, F: Deserialize<'de>, L: Deserialize<'de>"
))]
pub enum Down<K: Ord, R, F, L> {
    View(View<K, R, F>),
    Step(Step<K, R, F>),
    Live(Live<L>),
}

/// What a follower ships its decider: its own records, with [`Live`] data
/// between them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound(serialize = "L: Serialize", deserialize = "L: Deserialize<'de>"))]
pub enum Up<S: Schema, L> {
    Record(Record<S>),
    Live(Live<L>),
}

/// A decider's side of one follower's view: what it last shipped, so it
/// ships only what changed.
///
/// **Concept.** The link's whole state at the decider, and it is not
/// stored: a follower that reconnects, to this process or a restarted one,
/// gets a whole view, so nothing about a follower ever has to be kept for
/// it while it is away.
pub struct Publisher<K: Ord, R, F> {
    shipped: Option<View<K, R, F>>,
}

impl<K: Ord, R, F> Default for Publisher<K, R, F> {
    fn default() -> Self {
        Publisher { shipped: None }
    }
}

impl<K: Ord + Clone, R: Clone + PartialEq, F: Clone + PartialEq> Publisher<K, R, F> {
    /// What to ship for `view`, now true: the whole view the first time,
    /// then only what changed, or nothing.
    pub fn publish<L>(&mut self, view: View<K, R, F>) -> Option<Down<K, R, F, L>> {
        let Some(shipped) = &self.shipped else {
            self.shipped = Some(view.clone());
            return Some(Down::View(view));
        };
        let set: Vec<(K, R)> = view
            .rows
            .iter()
            .filter(|(k, r)| shipped.rows.get(*k) != Some(*r))
            .map(|(k, r)| (k.clone(), r.clone()))
            .collect();
        let gone: Vec<K> = shipped
            .rows
            .keys()
            .filter(|k| !view.rows.contains_key(*k))
            .cloned()
            .collect();
        let standing = (view.standing != shipped.standing).then(|| view.standing.clone());
        let step = Step {
            as_of: view.as_of,
            standing,
            set,
            gone,
        };
        self.shipped = Some(view);
        let empty = step.standing.is_none() && step.set.is_empty() && step.gone.is_empty();
        (!empty).then_some(Down::Step(step))
    }
}

/// A follower's side of its view: the rows it holds, as of the decider's
/// time it last heard of.
pub struct Mirror<K: Ord, R, F> {
    view: Option<View<K, R, F>>,
}

impl<K: Ord, R, F> Default for Mirror<K, R, F> {
    fn default() -> Self {
        Mirror { view: None }
    }
}

impl<K: Ord, R, F> Mirror<K, R, F> {
    /// The view held, once one has arrived.
    pub fn view(&self) -> Option<&View<K, R, F>> {
        self.view.as_ref()
    }

    /// Take in what the decider shipped. A step for a view this mirror
    /// does not hold is ignored: the decider ships a whole view on connect.
    pub fn apply<L>(&mut self, down: Down<K, R, F, L>) {
        match down {
            Down::View(view) => self.view = Some(view),
            Down::Step(step) => {
                if let Some(view) = &mut self.view {
                    view.as_of = step.as_of;
                    if let Some(standing) = step.standing {
                        view.standing = standing;
                    }
                    for key in step.gone {
                        view.rows.remove(&key);
                    }
                    view.rows.extend(step.set);
                }
            }
            Down::Live(_) => {}
        }
    }
}
