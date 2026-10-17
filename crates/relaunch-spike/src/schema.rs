//! The spike's vocabulary: sessions with real contexts, and the launch
//! facts of `harness_core::build`. A stand-in for the harness schema, small
//! enough to change per build.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use harness_core::build::{BuildId, Failure, Handover, Launch, Launched, Proven, Relaunch};
use harness_core::context::{Entry, EntryId, Root};
use harness_core::id::{CallId, TaskId, ValueId};
use harness_core::model_call::{ModelCall, Usage};
use harness_substrate::{Durability, Folding, Schema, Time, Unread};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq)]
pub struct Spike;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Said {
    Launched(Launched),
    /// A session on `root`. Its task is this record's time.
    Opened {
        session: String,
        root: Root,
        values: Vec<Vec<u8>>,
    },
    /// A request that responded, the bytes it named, and what it cost.
    Turn {
        session: String,
        call: ModelCall,
        response: ValueId,
        values: Vec<Vec<u8>>,
        usage: Usage,
    },
    /// A request the provider refused.
    Failed {
        session: String,
        why: String,
    },
    /// `session` asked to move the harness to another build: the call it
    /// made is this record's time.
    Relaunching {
        session: String,
        relaunch: Relaunch,
    },
    /// The relaunch `call` asked for did not happen.
    NotRelaunched {
        call: CallId,
        why: Failure,
    },
    Handover(Handover),
    Proven(Proven),
    /// A fact only builds with `new-fact` have a word for.
    #[cfg(feature = "new-fact")]
    Note {
        text: String,
    },
    /// A record this build cannot read. Made by the store, never written.
    #[serde(skip)]
    Unread(Unread),
}

#[derive(Clone, Default)]
pub struct State {
    pub values: HashMap<ValueId, Vec<u8>>,
    pub entries: HashMap<EntryId, Entry>,
    pub sessions: BTreeMap<String, Session>,
    pub running: Option<BuildId>,
    pub proven: BTreeSet<BuildId>,
    pub relaunching: Option<(CallId, String, Relaunch)>,
    pub history: Vec<String>,
    pub unread: Vec<(Time, String)>,
    pub notes: Vec<String>,
}

#[derive(Clone)]
pub struct Session {
    pub task: TaskId,
    pub root: EntryId,
    pub head: EntryId,
    /// What happened to the harness since this session's last request: the
    /// dated notices its next request carries.
    pub notices: Vec<String>,
    pub turns: u32,
}

impl Schema for Spike {
    type Said = Said;
    type Refusal = String;
    type State = State;
    type Key = ();

    fn durability(_: Time, _: &Said) -> Durability<()> {
        Durability::Kept
    }

    fn fold(state: &mut State, folding: Folding<'_, Spike>) -> Result<(), String> {
        let at = folding.at;
        match folding.said {
            Said::Launched(launched) => {
                if let Launch::Relaunched { by: Some(call), .. } = &launched.how {
                    match state.relaunching.take() {
                        Some((asked, _, relaunch))
                            if asked == *call && relaunch.to == launched.build => {}
                        _ => return Err(format!("no relaunch to {} was asked", launched.build)),
                    }
                }
                state.running = Some(launched.build);
                state.event(at, describe(launched));
            }
            Said::Opened {
                session,
                root,
                values,
            } => {
                if state.sessions.contains_key(session) {
                    return Err(format!("{session} is already open"));
                }
                state.learn(values);
                let id = root.id();
                state.entries.insert(id, Entry::Root(root.clone()));
                let session_state = Session {
                    task: TaskId(at.event(0)),
                    root: id,
                    head: id,
                    notices: Vec::new(),
                    turns: 0,
                };
                state.sessions.insert(session.clone(), session_state);
            }
            Said::Turn {
                session,
                call,
                response,
                values,
                ..
            } => {
                state.learn(values);
                if !state.values.contains_key(response) {
                    return Err("the response's bytes are missing".into());
                }
                let Some(s) = state.sessions.get_mut(session) else {
                    return Err(format!("no session {session}"));
                };
                if call.base != s.head {
                    return Err(format!("{session} has moved on"));
                }
                let reply = call.response(*response);
                s.head = reply.id();
                s.notices.clear();
                s.turns += 1;
                state.entries.extend(call.entries());
                state.entries.insert(reply.id(), reply);
            }
            Said::Failed { .. } => {}
            Said::Relaunching { session, relaunch } => {
                if state.relaunching.is_some() {
                    return Err("a relaunch is already under way".into());
                }
                state.relaunching = Some((CallId(at.event(0)), session.clone(), relaunch.clone()));
            }
            Said::NotRelaunched { call, why } => {
                let Some((asked, _, relaunch)) = state.relaunching.take() else {
                    return Err("no relaunch is under way".into());
                };
                if asked != *call {
                    return Err(format!("{call} is not the relaunch under way"));
                }
                state.event(
                    at,
                    format!(
                        "relaunch to {} failed: {}",
                        relaunch.to.short(),
                        failure(why)
                    ),
                );
            }
            Said::Handover(handover) => {
                state
                    .history
                    .push(format!("{at}: handed over to {}", handover.to.short()));
            }
            Said::Proven(proven) => {
                state.proven.insert(proven.build);
                state
                    .history
                    .push(format!("{at}: {} proven", proven.build.short()));
            }
            #[cfg(feature = "new-fact")]
            Said::Note { text } => state.notes.push(text.clone()),
            Said::Unread(unread) => state.unread.push((at, unread.why.clone())),
        }
        Ok(())
    }
}

impl State {
    fn learn(&mut self, values: &[Vec<u8>]) {
        for bytes in values {
            self.values.insert(ValueId::of(bytes), bytes.clone());
        }
    }

    /// Something every session is told at its next request.
    fn event(&mut self, at: Time, line: String) {
        for session in self.sessions.values_mut() {
            session.notices.push(line.clone());
        }
        self.history.push(format!("{at}: {line}"));
    }

    /// The context ending at `head`, root first.
    pub fn context(&self, head: EntryId) -> Vec<(EntryId, &Entry)> {
        let mut chain = Vec::new();
        let mut next = Some(head);
        while let Some(id) = next {
            let entry = &self.entries[&id];
            chain.push((id, entry));
            next = entry.parent();
        }
        chain.reverse();
        chain
    }
}

fn describe(launched: &Launched) -> String {
    let build = launched.build.short();
    match &launched.how {
        Launch::Started => format!("the harness started on build {build}"),
        Launch::Relaunched { from, .. } => {
            format!("the harness moved from build {} to {build}", from.short())
        }
        Launch::RolledBack { from, why } => format!(
            "the harness went back to build {build}: build {} {}",
            from.short(),
            failure(why)
        ),
    }
}

pub fn failure(why: &Failure) -> String {
    match why {
        Failure::Stale { running } => format!(
            "replaces a build that is no longer running ({} is)",
            running.short()
        ),
        Failure::Missing => "is not installed".into(),
        Failure::Exited { code, .. } => format!("exited (code {code:?}) before it was proven"),
        Failure::Refused {
            status, message, ..
        } => {
            format!("had a request refused ({status:?}): {message}")
        }
        Failure::TimedOut => "was not ready in time".into(),
    }
}
