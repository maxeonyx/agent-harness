mod brain;
mod face;
mod limb;
mod protocol;
mod provider;
mod state;

use protocol::{BrainCommand, BrainMsg};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, watch};

#[tokio::main]
async fn main() -> std::process::ExitCode {
    match run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let config = brain::Config::from_env();
    face::print_help(&config.base_url, &config.model);
    let journal_path = std::env::var("SKELETON_RECORD")
        .unwrap_or_else(|_| "skeleton-session.jsonl".to_string())
        .into();
    let mut initial_state =
        state::SessionState::create(journal_path, brain::SYSTEM_PROMPT.to_string())?;
    let limb = limb::Limb::new();
    for (name, contribution) in limb.contributions() {
        initial_state.append_contribution(name, contribution)?;
    }
    for (name, contribution) in brain::initial_contributions(&config) {
        initial_state.append_contribution(name, contribution)?;
    }
    let state = Arc::new(Mutex::new(initial_state));

    let (brain_tx, brain_rx) = mpsc::channel(64);
    let (limb_tx, limb_rx) = mpsc::channel(8);
    // Unbounded: rendering is an output port — the brain and limb must
    // never be flow-controlled by the user's terminal, or a full display
    // channel plus a full brain inbox becomes a cyclic backpressure
    // deadlock (face awaiting brain send, brain awaiting display send).
    let (display_tx, display_rx) = mpsc::unbounded_channel();
    let (session_over_tx, session_over_rx) = watch::channel(false);

    let mut tasks = tokio::task::JoinSet::new();
    let face_state = state.clone();
    let face_brain_tx = brain_tx.clone();
    tasks.spawn(async move {
        (
            "face",
            face::run(face_brain_tx, display_rx, face_state).await,
        )
    });
    let limb_brain_tx = brain_tx.clone();
    let limb_display_tx = display_tx.clone();
    let limb_state = state.clone();
    tasks.spawn(async move {
        (
            "limb",
            limb::run(limb, limb_rx, limb_brain_tx, limb_display_tx, limb_state).await,
        )
    });
    let brain_state = state.clone();
    tasks.spawn(async move {
        (
            "brain",
            brain::Session::run(
                config,
                brain_state,
                limb_tx,
                display_tx,
                brain_rx,
                session_over_tx,
            )
            .await,
        )
    });

    let supervisor_tx = brain_tx;
    let mut failed = false;
    while let Some(joined) = tasks.join_next().await {
        match joined {
            Ok((name, result)) => {
                let over = *session_over_rx.borrow();
                if let Err(error) = result {
                    eprintln!("{name} task failed: {error}");
                    failed = true;
                    let _ = supervisor_tx
                        .send(BrainMsg::Command(BrainCommand::Quit))
                        .await;
                } else if name != "brain" && !over {
                    eprintln!("{name} task ended during a live session; shutting down");
                    failed = true;
                    let _ = supervisor_tx
                        .send(BrainMsg::Command(BrainCommand::Quit))
                        .await;
                }
            }
            Err(error) => {
                eprintln!("participant task panicked: {error}; shutting down");
                failed = true;
                let _ = supervisor_tx
                    .send(BrainMsg::Command(BrainCommand::Quit))
                    .await;
            }
        }
    }
    if failed {
        Err("one or more participants failed".to_string())
    } else {
        Ok(())
    }
}
