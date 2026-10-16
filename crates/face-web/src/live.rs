//! Live progress beside the journal: what a running request has streamed
//! so far, kept until it ends so a page that reconnects loses none of it.

use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use harness_core::context::Wire;
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
/// once the fact that completes it is appended, when the journal holds what
/// matters of it. Nothing here is a fact, and a restart loses it all.
#[derive(Clone, Default)]
pub struct Feed(Arc<Shared>);

#[derive(Default)]
struct Shared {
    pieces: Mutex<HashMap<Running, Vec<Wire>>>,
    grew: Condvar,
}

impl Feed {
    pub fn start(&self, running: Running) {
        self.lock().entry(running).or_default();
    }

    pub fn push(&self, live: Live) {
        let running = Running::of(&live);
        let (Live::Model { event: piece, .. } | Live::Call { chunk: piece, .. }) = live;
        if let Some(pieces) = self.lock().get_mut(&running) {
            pieces.push(piece);
            self.0.grew.notify_all();
        }
    }

    pub fn end(&self, running: Running) {
        self.lock().remove(&running);
        self.0.grew.notify_all();
    }

    /// Stream `running`'s pieces after the first `after`, each with its
    /// count as the event id, until it ends; then an `end` event.
    pub(crate) fn stream(
        &self,
        stream: &mut impl Write,
        running: Running,
        mut after: usize,
        heartbeat: Duration,
    ) -> io::Result<()> {
        loop {
            let pieces = self.lock();
            let (pieces, _) = self
                .0
                .grew
                .wait_timeout_while(pieces, heartbeat, |p| {
                    p.get(&running).is_some_and(|p| p.len() <= after)
                })
                .unwrap_or_else(|e| e.into_inner());
            let Some(new) = pieces
                .get(&running)
                .map(|p| p[after.min(p.len())..].to_vec())
            else {
                drop(pieces);
                stream.write_all(b"event: end\ndata:\n\n")?;
                return stream.flush();
            };
            drop(pieces);
            if new.is_empty() {
                stream.write_all(b": heartbeat\n\n")?;
            }
            for piece in new {
                after += 1;
                writeln!(stream, "id: {after}")?;
                for line in piece.0.lines() {
                    writeln!(stream, "data: {line}")?;
                }
                stream.write_all(b"\n")?;
            }
            stream.flush()?;
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<Running, Vec<Wire>>> {
        self.0.pieces.lock().unwrap_or_else(|e| e.into_inner())
    }
}
