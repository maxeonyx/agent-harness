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

    // Face → brain: user events. Brain → limb: execution requests.
    // Brain → everyone: the session event stream. The session log is
    // shared directly in this co-located deployment (brain writes, face
    // reads); see the TODO on Context about the mutex.
    let (user_tx, user_rx) = tokio::sync::mpsc::channel(64);
    let (limb_tx, limb_rx) = tokio::sync::mpsc::channel(8);
    let (bus_tx, bus_rx) = tokio::sync::broadcast::channel(256);
    let recorder_rx = bus_tx.subscribe();
    let context = std::sync::Arc::new(std::sync::Mutex::new(context::Context::new()));

    let recorder_task = tokio::spawn(recorder::run(recorder::path_from_env(), recorder_rx));
    let face_task = tokio::spawn(face::run(user_tx, bus_rx, context.clone()));
    let limb_task = tokio::spawn(limb::run(limb::Limb::new(), limb_rx));

    brain::Session::run(config, limb_tx, bus_tx, user_rx, context).await;
    // The session dropped its bus sender and limb channel: every task's
    // termination follows structurally, and all of them are joined.
    face_task.await.expect("face task failed");
    recorder_task.await.expect("recorder task failed");
    limb_task.await.expect("limb task failed");
}
