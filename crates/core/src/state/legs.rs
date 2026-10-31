//! The fold's index of legs ([`Legs`]), kept as the facts that begin them
//! apply. Each hook runs after its fact has been checked, so none refuses.

use imbl::{OrdSet, Vector};

use super::State;
use crate::call::Caller;
use crate::context::{Began, Dep, Entry, EntryId, Leg, LegId, Legs, Rendered, Work};
use crate::id::{CallId, TaskId, Timestamp};
use crate::model_call::ModelCall;
use crate::task::{Origin, Scope, Task};

impl State {
    /// A task's legs: how its context began, compacted, joined and was
    /// edited ([`Leg`]).
    pub fn legs(&self, task: TaskId) -> Option<&Legs> {
        self.legs.get(&task)
    }

    /// A new task's first leg, on what it continues or was spawned from.
    pub(super) fn leg_start(&mut self, task: &Task, creator: Option<TaskId>, at: Timestamp) {
        let on = match task.origin {
            Origin::Fork { from } | Origin::Resume { from } | Origin::Aside { from } => self
                .legs(from)
                .zip(task.head())
                .map(|(l, at)| Dep::Leg { leg: l.head, at }),
            Origin::Fresh { .. } => creator.and_then(|c| {
                let spawned = task.by.and_then(|by| self.origin(by));
                let at = spawned.or_else(|| self.task(c)?.head())?;
                Some(Dep::Leg {
                    leg: self.legs(c)?.head,
                    at,
                })
            }),
        };
        let start = Leg {
            began: Began::Start,
            at,
            first: task.head(),
            on: on.into_iter().collect(),
            last: task.head(),
        };
        let legs = Legs {
            head: start.id(task.id),
            legs: Vector::unit(start),
            work: Vector::new(),
            back: false,
            taken: OrdSet::new(),
        };
        self.legs.insert(task.id, legs);
    }

    /// A fresh task's first root is where its start leg begins.
    pub(super) fn leg_root(&mut self, task: TaskId, root: EntryId) {
        if let Some(start) = self.legs.get_mut(&task).and_then(|l| l.legs.front_mut()) {
            start.first.get_or_insert(root);
            start.last = Some(root);
        }
    }

    /// A compaction from `from`, whose carry is `carry`, ending at `last`.
    pub(super) fn leg_carry(
        &mut self,
        task: TaskId,
        carry: EntryId,
        from: EntryId,
        last: EntryId,
        at: Timestamp,
    ) {
        let Some(head) = self.diverged(task, from) else {
            return;
        };
        self.push_leg(
            task,
            Began::Carry(carry),
            at,
            vec![Dep::Leg {
                leg: head,
                at: from,
            }],
        );
        self.grow(task, last);
    }

    /// A request, before its entries are learned: a join when it takes in
    /// results not yet taken in, a new version when it diverges after a
    /// move back.
    pub(super) fn leg_request(&mut self, call: &ModelCall, at: Timestamp) {
        let task = call.task;
        let entries = call.entries();
        if entries.is_empty() {
            return;
        }
        let new = (entries.iter())
            .find(|(e, _)| !self.entries.contains_key(e))
            .map(|(e, entry)| (*e, entry.parent().unwrap_or(call.base)));
        let Some(legs) = self.legs.get(&task) else {
            return;
        };
        let mut diverged = None;
        if legs.back {
            match new {
                Some((e, parent)) => diverged = self.diverged(task, parent).map(|l| (e, l, parent)),
                None => self.rehead(task, call.head()),
            }
        }
        let Some(legs) = self.legs.get(&task) else {
            return;
        };
        let joined: Vec<CallId> = (call.renders.iter())
            .filter_map(|span| match span.from {
                Rendered::Call(c) => Some(c),
                _ => None,
            })
            .filter(|c| !legs.taken.contains(c) && self.joins(*c))
            .collect();
        if !joined.is_empty() {
            let mut on = vec![Dep::Leg {
                leg: legs.head,
                at: call.base,
            }];
            for c in &joined {
                if self.members(Scope::Spawn(*c)).next().is_none() {
                    on.push(Dep::Work(*c));
                }
                for member in self.members(Scope::Spawn(*c)) {
                    if let (Some(l), Some(at)) = (self.legs(member.id), member.head()) {
                        on.push(Dep::Leg { leg: l.head, at });
                    }
                }
            }
            if let Some(legs) = self.legs.get_mut(&task) {
                legs.taken.extend(joined);
            }
            self.push_leg(task, Began::Join(call.head()), at, on);
        } else if let Some((e, leg, at_entry)) = diverged {
            let on = vec![Dep::Leg { leg, at: at_entry }];
            self.push_leg(task, Began::Version(e), at, on);
        }
        self.grow(task, call.head());
    }

    /// A response, before it is learned: a new version when it is the
    /// first entry to diverge after a move back.
    pub(super) fn leg_response(&mut self, task: TaskId, response: &Entry, at: Timestamp) {
        let entry = response.id();
        if self.legs(task).is_some_and(|l| l.back) {
            let parent = response.parent();
            if self.entries.contains_key(&entry) {
                self.rehead(task, entry);
            } else if let Some(parent) = parent
                && let Some(leg) = self.diverged(task, parent)
            {
                let on = vec![Dep::Leg { leg, at: parent }];
                self.push_leg(task, Began::Version(entry), at, on);
            }
        }
        self.grow(task, entry);
    }

    /// Max moved the task back to `to`: its head is in the leg holding it,
    /// and the next entry that diverges splits that leg.
    pub(super) fn leg_back(&mut self, task: TaskId, to: EntryId) {
        let head = self.holding(task, to);
        if let (Some(legs), Some(head)) = (self.legs.get_mut(&task), head) {
            legs.head = head;
            legs.back = true;
        }
    }

    /// A background call `call`, started by the task's call `from`.
    pub(super) fn leg_work(&mut self, task: TaskId, call: CallId, from: CallId) {
        let at = self.origin(from).or_else(|| self.task(task)?.head());
        if let (Some(legs), Some(at)) = (self.legs.get_mut(&task), at) {
            let leg = legs.head;
            legs.work.push_back(Work { call, leg, at });
        }
    }

    /// Whether a request taking in `call` begins a join: it is a background
    /// call, or a spawn with members.
    fn joins(&self, call: CallId) -> bool {
        self.calls
            .get(&call)
            .is_some_and(|c| matches!(c.call.caller, Caller::Background { .. }))
            || self.members(Scope::Spawn(call)).next().is_some()
    }

    /// The leg of `task` holding `entry`: the nearest leg to begin at it or
    /// above it.
    fn holding(&self, task: TaskId, entry: EntryId) -> Option<LegId> {
        let legs = self.legs.get(&task)?;
        let mut next = Some(entry);
        while let Some(id) = next {
            if let Some(leg) = legs.legs.iter().find(|l| l.first == Some(id)) {
                return Some(leg.id(task));
            }
            next = self.entries.get(&id).and_then(Entry::parent);
        }
        legs.legs.front().map(|l| l.id(task))
    }

    /// The head moved along entries the tree already holds.
    fn rehead(&mut self, task: TaskId, head: EntryId) {
        let leg = self.holding(task, head);
        if let (Some(legs), Some(leg)) = (self.legs.get_mut(&task), leg) {
            legs.head = leg;
        }
    }

    /// The task's head leg, once what follows `at` diverges from it: after
    /// a move back, the leg holding `at` splits there, and the stretch it
    /// leaves behind becomes an old version of its own.
    fn diverged(&mut self, task: TaskId, at: EntryId) -> Option<LegId> {
        let legs = self.legs.get(&task)?;
        if !legs.back {
            return Some(legs.head);
        }
        let held = self.holding(task, at)?;
        let i = legs.legs.iter().position(|l| l.id(task) == held)?;
        let leg = &legs.legs[i];
        let tail = leg.last.filter(|last| *last != at).and_then(|last| {
            let mut e = last;
            while let Some(parent) = self.entries.get(&e).and_then(Entry::parent) {
                if parent == at {
                    let began = self.began_at(e).unwrap_or(leg.at);
                    return Some((e, last, began));
                }
                e = parent;
            }
            None
        });
        if let Some((first, last, began)) = tail {
            let split = Leg {
                began: Began::Version(first),
                at: began,
                first: Some(first),
                on: vec![Dep::Leg { leg: held, at }],
                last: Some(last),
            };
            let old = split.id(task);
            self.repoint(held, old, at, first);
            let legs = self.legs.get_mut(&task)?;
            legs.legs[i].last = Some(at);
            legs.legs.insert(i + 1, split);
        }
        let legs = self.legs.get_mut(&task)?;
        legs.back = false;
        legs.head = held;
        Some(held)
    }

    /// When the entry `e` was first sent or received.
    fn began_at(&self, e: EntryId) -> Option<Timestamp> {
        let sent = self.request(e).map(|m| m.sent.at);
        let received = || self.response(e)?.ended.as_ref().map(|x| x.at.at);
        sent.or_else(received)
    }

    /// What depended on `from` at its tail, after `split` from `first` on,
    /// now depends on `to`, in every task.
    fn repoint(&mut self, from: LegId, to: LegId, split: EntryId, first: EntryId) {
        let names =
            |leg: LegId, at: EntryId| leg == from && at != split && self.descends(at, first);
        let touched: Vec<TaskId> = (self.legs.iter())
            .filter(|(_, l)| {
                let deps = l.legs.iter().flat_map(|l| &l.on);
                deps.clone()
                    .any(|d| matches!(*d, Dep::Leg { leg, at } if names(leg, at)))
                    || l.work.iter().any(|w| names(w.leg, w.at))
            })
            .map(|(t, _)| *t)
            .collect();
        let moves: Vec<(TaskId, Legs)> = (touched.into_iter())
            .filter_map(|t| {
                let mut legs = self.legs.get(&t)?.clone();
                for leg in legs.legs.iter_mut() {
                    for dep in &mut leg.on {
                        if let Dep::Leg { leg, at } = dep
                            && names(*leg, *at)
                        {
                            *leg = to;
                        }
                    }
                }
                for work in legs.work.iter_mut() {
                    if names(work.leg, work.at) {
                        work.leg = to;
                    }
                }
                Some((t, legs))
            })
            .collect();
        self.legs.extend(moves);
    }

    fn push_leg(&mut self, task: TaskId, began: Began, at: Timestamp, on: Vec<Dep>) {
        if let Some(legs) = self.legs.get_mut(&task) {
            let first = match began {
                Began::Carry(e) | Began::Join(e) | Began::Version(e) => Some(e),
                Began::Start => None,
            };
            let leg = Leg {
                began,
                at,
                first,
                on,
                last: first,
            };
            legs.head = leg.id(task);
            legs.legs.push_back(leg);
        }
    }

    /// The head leg grew to `last`.
    fn grow(&mut self, task: TaskId, last: EntryId) {
        if let Some(legs) = self.legs.get_mut(&task) {
            let head = legs.head;
            if let Some(leg) = legs.legs.iter_mut().find(|l| l.id(task) == head) {
                leg.last = Some(last);
            }
        }
    }
}
