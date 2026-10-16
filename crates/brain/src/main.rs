//! The brain: what makes the harness's crates a harness.
//!
//! It holds the [`State`](harness_core::state::State) folded from its
//! journal and works through [`State::todo`](harness_core::state::State::todo):
//! it sends requests through the provider, dispatches calls to limbs, runs
//! the meta limb's calls itself, and settles scopes. Every fact goes through
//! [`State::apply`](harness_core::state::State::apply) on a copy of the state
//! and is then appended to the store; nothing else is kept that a restart
//! would need. A relaunch folds the journal, says each unfinished request
//! was lost, asks the workspace how each dispatched call ended, and carries
//! on.
//!
//! The face is served from the same process over a second connection to the
//! same store, so Max watches exactly what the brain appended. Its compose
//! box sends through the brain: his words become a delivery he authored,
//! as far as his page had seen, or are refused back to him (the task has
//! finished meanwhile).
//!
//! **Policy** lives in two modules, and nowhere else:
//! - [`policy`]: the model and request parameters, retries and backoff,
//!   breakpoint placement, fan-out gating on first byte, and the agent
//!   kinds' role prompts;
//! - [`render`]: what a request renders beyond the results it must, and the
//!   wording of everything the harness says to a model, including the
//!   fork framing the fork-framing spike measured.
//!
//! ```text
//! harness-brain --journal J [--port 4321] [--workspace DIR --mandate FILE [--name NAME]]
//! ```
//! A new journal needs the workspace and the mandate; an existing one is
//! continued and they are ignored.

mod brain;
mod journal;
mod meta;
mod policy;
mod render;

use std::net::{IpAddr, TcpListener};
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;

use harness_core::id::FaceId;
use harness_face_web::{Compose, Dictation, Face};
use harness_provider::{Client, Login};
use harness_store::Store;

use crate::brain::{Brain, First, Inbox};
use crate::journal::Journal;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("harness-brain: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut journal, mut port, mut workspace, mut mandate, mut name) =
        (None, 4321u16, None, None, "orchestrator".to_string());
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--journal" => journal = Some(PathBuf::from(value()?)),
            "--port" => port = value()?.parse().map_err(|e| format!("--port: {e}"))?,
            "--workspace" => workspace = Some(PathBuf::from(value()?)),
            "--mandate" => mandate = Some(PathBuf::from(value()?)),
            "--name" => name = value()?,
            _ => return Err(format!("unknown argument {flag}")),
        }
    }
    let journal_path = journal.ok_or("--journal is required")?;
    let first = match (workspace, mandate) {
        (Some(workspace), Some(mandate)) => Some(First {
            workspace,
            name,
            mandate: std::fs::read_to_string(&mandate)
                .map_err(|e| format!("the mandate {mandate:?}: {e}"))?,
        }),
        _ => None,
    };
    let state_dir = journal_path.with_extension("workspace-state");

    let store = Store::open(&journal_path).map_err(|e| format!("the journal opens: {e}"))?;
    let journal = Journal::open(&journal_path).map_err(|e| format!("the journal opens: {e}"))?;

    let certs = PathBuf::from(std::env::var("HOME").map_err(|e| e.to_string())?)
        .join(".config/dev-certs/shared-local");
    let tls = harness_face_web::tls(
        &certs.join("wildcard-local.crt"),
        &certs.join("wildcard-local.key"),
    )
    .map_err(|e| format!("the certificate: {e}"))?;
    let listener = TcpListener::bind(("0.0.0.0", port)).map_err(|e| format!("port {port}: {e}"))?;
    let inbox = Inbox::new();
    let login = || Login::claude_code().map_err(|e| format!("the login: {}", e.0));
    let face = Face::with(
        journal.clone(),
        Compose {
            deliver: Some(Box::new(inbox.words(FaceId(brain::new_id())))),
            dictation: Some(Dictation::new(login()?)),
        },
    );
    let follower = face.clone();
    thread::spawn(move || follower.follow());
    // The Windows host, read again for each connection: DHCP can move it.
    let allow = |peer: IpAddr| Some(peer) == harness_face_web::default_gateway().map(IpAddr::V4);
    thread::spawn(move || harness_face_web::serve(face, listener, tls, allow));
    println!("face: https://mx-hv-mj.local:{port}");

    let client = Client::new(login()?);
    let mut brain = Brain::launch(store, journal, client, state_dir, first, inbox)?;
    brain.run();
    Ok(())
}
