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
async fn main() {
    let config = brain::Config::from_env();
    face::print_help(&config.base_url, &config.model);

    // Face → brain: user events. Brain → everyone: the session event stream.
    // The session log is shared directly in this co-located deployment
    // (brain writes, face reads); see the TODO on Context about the mutex.
    let (user_tx, user_rx) = tokio::sync::mpsc::channel(64);
    let (bus_tx, bus_rx) = tokio::sync::broadcast::channel(256);
    let recorder_rx = bus_tx.subscribe();
    let context = std::sync::Arc::new(std::sync::Mutex::new(context::Context::new()));

    let recorder_task = tokio::spawn(recorder::run(recorder::path_from_env(), recorder_rx));
    let face_task = tokio::spawn(face::run(user_tx, bus_rx, context.clone()));

    brain::Session::run(config, limb::Limb::new(), bus_tx, user_rx, context).await;
    // The session dropped its bus sender; consumers drain and finish.
    let _ = face_task.await;
    let _ = recorder_task.await;
    // tokio's stdin reader is a blocking thread that may still be parked in
    // read(); exit explicitly rather than waiting on it.
    std::process::exit(0);
}
