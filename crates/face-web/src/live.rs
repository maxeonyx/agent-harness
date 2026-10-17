//! Live progress beside the facts: what a running request has streamed so
//! far, kept until it ends so a tab that reconnects loses none of it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use harness_core::id::{CallId, ModelCallId};
use harness_core::live::Live;

/// What a [`Live`] is the progress of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Running {
    Model(ModelCallId),
    Call(CallId),
}

impl Running {
    pub fn of(live: &Live) -> Running {
        match live {
            Live::Model { call, .. } => Running::Model(*call),
            Live::Call { call, .. } => Running::Call(*call),
        }
    }
}

/// Every running thing's progress so far, in arrival order.
///
/// The brain [starts](Feed::start) one before its first fact is appended,
/// [pushes](Feed::push) each piece as it arrives, and [ends](Feed::end) it
/// once the record that completes it is stored, which holds what
/// matters of it. Nothing here is a fact, and a restart loses it all.
#[derive(Clone, Default)]
pub struct Feed(Arc<Mutex<HashMap<Running, Vec<String>>>>);

impl Feed {
    pub fn start(&self, running: Running) {
        self.lock().entry(running).or_default();
    }

    pub fn push(&self, live: Live) {
        let running = Running::of(&live);
        let (Live::Model { chunk: piece, .. } | Live::Call { chunk: piece, .. }) = live;
        if let Some(pieces) = self.lock().get_mut(&running) {
            pieces.push(piece);
        }
    }

    pub fn end(&self, running: Running) {
        self.lock().remove(&running);
    }

    /// Every piece not yet in `shown`, which counts how many of each
    /// running thing's pieces a link has shipped, and forgets what ended.
    pub(crate) fn since(&self, shown: &mut HashMap<Running, usize>) -> Vec<Live> {
        let pieces = self.lock();
        shown.retain(|running, _| pieces.contains_key(running));
        let mut new = Vec::new();
        for (running, all) in pieces.iter() {
            let after = shown.entry(*running).or_default();
            for chunk in &all[(*after).min(all.len())..] {
                new.push(match *running {
                    Running::Model(call) => Live::Model {
                        call,
                        chunk: chunk.clone(),
                    },
                    Running::Call(call) => Live::Call {
                        call,
                        chunk: chunk.clone(),
                    },
                });
            }
            *after = all.len();
        }
        new
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<Running, Vec<String>>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}
