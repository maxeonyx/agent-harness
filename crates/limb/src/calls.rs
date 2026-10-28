//! Running a kind's calls, each at most once.
//!
//! A call is run on its own thread, after the calls its task made before it
//! on the same limb have ended: a model writes one response's tool uses in
//! the order it means them to happen (write a file, then commit it), so
//! they run in that order. Other tasks' calls run alongside.
//!
//! Each call is marked on disk as started before it has any effect, and
//! settled in the log before it stops running. So a call row runs at most
//! once, however often a crash of the brain or of the limb makes it arrive:
//! it is left to the settlement the log holds, left running, run for the
//! first time, rerun if its tool allows, or settled as
//! [`Outcome::Unknown`] when the instance running it was cut off. Never a
//! failure that did not happen.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, PipeReader};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use harness_core::call::{Caller, Outcome, Owner};
use harness_core::environment::LimbKey;
use harness_core::id::{CallId, TaskId};
use harness_core::value::Bytes;
use harness_runtime::Cancel;

use crate::{CallRow, Report, Rows, Voice, instant, lock};

/// A kind's running calls, and its marks of those it started.
pub struct Calls {
    voice: Arc<Voice>,
    /// A file per call started and not yet known settled by the brain.
    started: PathBuf,
    running: Arc<Mutex<HashMap<CallId, Running>>>,
    /// For each task, the end of its latest call: it hangs up when that
    /// call ends, and the task's next call waits for that.
    latest: Mutex<HashMap<TaskId, PipeReader>>,
    /// Shutting down: no new call runs, and a stopped one says why.
    shutting_down: AtomicBool,
    /// Run when the last running call has ended.
    idle: fn(),
}

struct Running {
    cancel: Cancel,
    limit: Limit,
    thread: Option<JoinHandle<()>>,
}

impl Calls {
    /// The calls of a kind that says them through `voice`, marked in
    /// `started`. `idle` runs each time no call is left running.
    pub fn open(voice: Arc<Voice>, started: &Path, idle: fn()) -> io::Result<Calls> {
        fs::create_dir_all(started)?;
        Ok(Calls {
            voice,
            started: started.to_path_buf(),
            running: Arc::default(),
            latest: Mutex::default(),
            shutting_down: AtomicBool::new(false),
            idle,
        })
    }

    /// One call row: run it at most once, however often it arrives, and
    /// stop it when the row says so, and lift its time limit once it is
    /// passed to Max ([`Limit`]). `rerun`: running its tool again after an
    /// instance was cut off running it is the same as running it once.
    /// `run` runs it on its input, once the task's earlier calls have
    /// ended, to the report and outcome it settles with.
    pub fn call(
        &self,
        row: CallRow,
        rerun: bool,
        run: impl FnOnce(&Bytes, &Cancel, &Limit, &mut Report) -> Outcome + Send + 'static,
    ) {
        let CallRow {
            id,
            call,
            input,
            stopping,
            owner,
            earlier,
        } = row;
        let kept = owner == Some(Owner::Max) && matches!(call.caller, Caller::Background { .. });
        let voice = &self.voice;
        let mut running = lock(&self.running);
        if let Some(running) = running.get(&id) {
            if let Some(by) = stopping {
                running.cancel.request(instant(by));
            }
            if kept {
                running.limit.lift();
            }
            return;
        }
        // Checked after `running`: a call's thread settles before it stops
        // running.
        if voice.settled(id) {
            return;
        }
        let mark = self.started.join(id.to_string());
        let started = earlier || mark.exists();
        if stopping.is_some() {
            let outcome = if started {
                Outcome::Unknown
            } else {
                Outcome::Interrupted { partial: None }
            };
            return voice.settle(Report::new(Some(id)), outcome);
        }
        // A limb shutting down starts nothing; the next instance runs it.
        if self.shutting_down() {
            return;
        }
        // Cut off while an earlier instance ran it.
        if started && !rerun {
            return voice.settle(Report::new(Some(id)), Outcome::Unknown);
        }
        let mut report = Report::new(Some(id));
        let Some(input) = input.cloned() else {
            let outcome = report.failed("the brain sent the call without its input");
            return voice.settle(report, outcome);
        };
        if let Err(e) = File::create(&mark).and_then(|f| f.sync_all()) {
            let outcome = report.failed(&format!("cannot record the call: {e}"));
            return voice.settle(report, outcome);
        }
        let cancel = Cancel::new();
        let limit = Limit::default();
        if kept {
            limit.lift();
        }
        // This call's end: the writer, held until it has settled. The
        // task's next call waits on the reader.
        let (before, ended) = match (&call.caller, io::pipe()) {
            (Caller::Agent { task, .. }, Ok((reader, writer))) => {
                (lock(&self.latest).insert(*task, reader), Some(writer))
            }
            _ => (None, None),
        };
        let thread = {
            let cancel = cancel.clone();
            let limit = limit.clone();
            let voice = voice.clone();
            let idle = self.idle;
            let running = self.running.clone();
            thread::spawn(move || {
                let outcome = match after(before, &cancel) {
                    Ok(()) => run(&input, &cancel, &limit, &mut report),
                    Err(_) => Outcome::Interrupted { partial: None },
                };
                voice.settle(report, outcome);
                drop(ended);
                let mut running = lock(&running);
                running.remove(&id);
                if running.is_empty() {
                    idle();
                }
            })
        };
        running.insert(
            id,
            Running {
                cancel,
                limit,
                thread: Some(thread),
            },
        );
    }

    /// Forget the marks of calls the brain no longer asks for, which it
    /// holds settled.
    pub fn forget(&self, rows: &Rows) {
        let running = lock(&self.running);
        let Ok(entries) = fs::read_dir(&self.started) else {
            return;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let Some(call) = entry.file_name().to_str().and_then(|n| n.parse().ok()) else {
                continue;
            };
            if !running.contains_key(&call) && !rows.rows.contains_key(&LimbKey::Call(call)) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    /// Whether a call runs.
    pub fn any(&self) -> bool {
        !lock(&self.running).is_empty()
    }

    pub fn shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    /// Stop every running call, each to be gone `within` from now, and run
    /// no new one. Asked again with less time, it makes each stop harder.
    pub fn stop_all(&self, within: Duration) {
        self.shutting_down.store(true, Ordering::SeqCst);
        let deadline = Instant::now() + within;
        for running in lock(&self.running).values() {
            running.cancel.request(deadline);
        }
    }

    /// Wait until every call has settled.
    pub fn join(&self) {
        loop {
            let thread = lock(&self.running)
                .values_mut()
                .find_map(|r| r.thread.take());
            let Some(thread) = thread else {
                return;
            };
            let _ = thread.join();
        }
    }
}

/// Wait until the call `before` ends (its pipe hangs up), unless this call
/// is asked to stop first.
fn after(before: Option<PipeReader>, cancel: &Cancel) -> io::Result<()> {
    let Some(before) = before else {
        return Ok(());
    };
    let mut waiter = cancel.waiter()?;
    loop {
        if cancel.is_requested() {
            return Err(harness_runtime::Cancelled::error());
        }
        if waiter.wait(&[(before.as_raw_fd(), libc::POLLIN)], None)? == harness_runtime::Wake::Ready
        {
            return Ok(());
        }
    }
}

/// Whether a call is still held to its own time limit (a command's
/// `timeout_ms`). Clones share it.
///
/// A time limit is for someone waiting on the call's result. A background
/// call passed to Max ([`harness_core::call::CallEvent::Passed`]) is kept
/// for its lifetime instead, a dev server he goes on using, so its limit is
/// lifted and it runs until he stops it. A call passed to an agent keeps
/// its limit: that agent still waits on its result.
#[derive(Clone, Default)]
pub struct Limit(Arc<AtomicBool>);

impl Limit {
    pub fn lift(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_lifted(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}
