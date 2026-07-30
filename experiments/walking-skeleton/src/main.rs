//! Walking skeleton: face + brain + limb as logical roles in one process,
//! communicating by events. Two select loops — the face loop (user input
//! in, event rendering out) and the brain's session loop (user events,
//! in-flight provider requests, in-flight tool calls) — connected by
//! channels. The recorder is a third, passive consumer of the same event
//! stream.

mod brain;
mod context;
mod events;
mod face;
mod limb;
mod provider;
mod recorder;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let config = brain::Config::from_env();
    face::print_help(&config.base_url, &config.model);

    // Face → brain: user events. Brain → limb: execution requests.
    // Brain → everyone: the session event stream. The session log is
    // shared directly in this co-located deployment (brain writes, face
    // reads); see the TODO on Context about the mutex.
    let (user_tx, user_rx) = tokio::sync::mpsc::channel(64);
    let (limb_tx, limb_rx) = tokio::sync::mpsc::channel(8);
    let (bus_tx, bus_rx) = tokio::sync::broadcast::channel(256);
    let recorder_rx = bus_tx.subscribe();
    let context = std::sync::Arc::new(std::sync::Mutex::new(context::Context::new()));

    // Main is the supervisor. The brain's lifetime is the session's
    // lifetime; the auxiliaries end structurally *after* it (the brain
    // drops its bus sender and limb channel on return). An auxiliary
    // finishing during a live session is therefore a bug — supervised by
    // asking the brain to shut down gracefully (it still drains in-flight
    // work and the limb still cleans up its processes) rather than
    // hanging the session or abandoning it. "During a live session" is
    // decided by the session's own log, not by task timing: SessionClosed
    // is emitted *before* the brain returns, so an auxiliary can finish
    // orderly a beat before the brain's join handle reads done.
    // Residual: if the *face* died, its stdin thread may stay blocked on
    // a read until process exit — a blocked stdin read cannot be
    // interrupted; everything the process owns is drained regardless.
    let mut closed_watch = SessionClosedWatch {
        rx: bus_tx.subscribe(),
        seen: false,
    };
    let supervisor_tx = user_tx.clone();
    let mut auxiliaries = tokio::task::JoinSet::new();
    auxiliaries.spawn(async move {
        recorder::run(recorder::path_from_env(), recorder_rx).await;
        "recorder"
    });
    let face_context = context.clone();
    auxiliaries.spawn(async move {
        face::run(user_tx, bus_rx, face_context).await;
        "face"
    });
    auxiliaries.spawn(async move {
        limb::run(limb::Limb::new(), limb_rx).await;
        "limb"
    });

    let mut brain = tokio::spawn(brain::Session::run(
        config, limb_tx, bus_tx, user_rx, context,
    ));
    let mut failed = false;
    loop {
        tokio::select! {
            // Biased: a finished brain is checked first. The auxiliaries
            // begin terminating the moment the brain returns, so an
            // unbiased poll could misread their orderly exit as early.
            biased;
            result = &mut brain => {
                if let Err(e) = result {
                    eprintln!("brain task failed: {e}");
                    failed = true;
                }
                break;
            }
            Some(early) = auxiliaries.join_next() => {
                match early {
                    Ok(_) if closed_watch.session_closed() => {
                        // Orderly: the session already closed; this task
                        // finishing is the shutdown proceeding.
                    }
                    Ok(name) => {
                        eprintln!("{name} task ended during a live session; shutting down");
                        failed = true;
                        let _ = supervisor_tx.send(events::EventKind::Quit).await;
                    }
                    Err(e) => {
                        eprintln!("auxiliary task failed: {e}; shutting down");
                        failed = true;
                        let _ = supervisor_tx.send(events::EventKind::Quit).await;
                    }
                }
            }
        }
    }
    // The brain is done: the remaining auxiliaries terminate structurally
    // and every one of them is joined.
    while let Some(result) = auxiliaries.join_next().await {
        if let Err(e) = result {
            eprintln!("auxiliary task failed: {e}");
            failed = true;
        }
    }
    if failed {
        std::process::ExitCode::FAILURE
    } else {
        std::process::ExitCode::SUCCESS
    }
}

/// Watches the session's own log for SessionClosed. The fact is
/// monotonic — once the session has closed it stays closed — so the watch
/// must be too: reading the event out of the bus receiver *remembers* it.
/// (A consuming check here once misclassified the second of two orderly
/// auxiliary exits as a mid-session death: the first check swallowed the
/// event. Rare, racy, and structurally excluded by `seen`.)
struct SessionClosedWatch {
    rx: tokio::sync::broadcast::Receiver<events::Event>,
    seen: bool,
}

impl SessionClosedWatch {
    /// Has the session's log recorded SessionClosed yet? SessionClosed is
    /// the final event ever emitted, so lagging cannot skip past it.
    fn session_closed(&mut self) -> bool {
        while !self.seen {
            match self.rx.try_recv() {
                Ok(event) if matches!(event.kind, events::EventKind::SessionClosed) => {
                    self.seen = true;
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {}
                Err(_) => break,
            }
        }
        self.seen
    }
}
