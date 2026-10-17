//! The brain: what makes the harness's crates a harness.
//!
//! It holds its [`Journal`](harness_core::journal::Journal) and works
//! through [`State::todo`](harness_core::state::State::todo): it sends
//! requests through the provider, dispatches calls to limbs, runs the meta
//! limb's calls itself, and settles scopes. Every fact goes through
//! [`Journal::commit`](harness_core::journal::Journal::commit); nothing else
//! is kept that a restart would need. A relaunch is no special case: the
//! to-do list of the folded journal says each unfinished request was lost
//! and sends every unsettled call to the workspace again, which answers from
//! its own record.
//!
//! Each plugin limb runs as a process of its own, `harness-limb-<kind>`
//! from beside the brain's executable, spoken to in [`ToLimb`] and
//! [`FromLimb`] lines over its stdio ([`process`]). A limb whose process
//! ends is recorded as lost and started again when a task needs it.
//!
//! [`ToLimb`]: harness_core::environment::ToLimb
//! [`FromLimb`]: harness_core::environment::FromLimb
//!
//! The face is served from the same process. After each commit the brain
//! hands it the state, a persistent value it holds while the brain moves
//! on, so Max watches exactly what the brain appended; the bytes facts name
//! it reads over a connection of its own, since a stored value never
//! changes. Everything
//! he does there (his words, answering an ask, interrupting, stopping or
//! holding a task, opening an aside, marking a chat done, reordering his
//! queue, focusing a task) reaches the brain as a batch of his
//! facts, which it commits as authored by him, as far as his page had seen,
//! or refuses back to him with the core's reason. Each request's
//! streamed events and each command's output as it prints go to the face's
//! live feed, never to the journal.
//!
//! **Policy** lives in two modules, and nowhere else:
//! - [`policy`]: retries and backoff, breakpoint placement, fan-out
//!   gating on first byte, and where a clone goes;
//! - [`render`]: what a request renders beyond the results it must, and the
//!   wording of everything the harness says to a model, including the
//!   fork framing the fork-framing spike measured.
//!
//! ```text
//! harness-brain --journal J [--port 4321] [--sample WAV]
//!               [--workspace DIR --mandate FILE [--name NAME]]
//! ```
//! A new journal needs the workspace and the mandate; an existing one is
//! continued and they are ignored. The task they start is a chat with Max,
//! like every task he starts: each turn's end is his move, and it is done
//! when he says. Its mandate is his words, so its briefs can quote it.
//! `--sample` offers the page a recording to dictate, for a machine with no
//! microphone.

mod brain;
mod max;
mod meta;
mod policy;
mod process;
mod render;
mod roles;
mod web;

use std::net::{IpAddr, TcpListener};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Mutex, mpsc};
use std::thread;

use harness_core::id::ValueId;
use harness_face_web::{Dictation, Face, Feed, Source, Wiring};
use harness_provider::{Client, Login};
use harness_store::Store;

use crate::brain::{Brain, Inbox, Start};

/// The values the face shows, read over its own connection to the store.
struct Values(Mutex<Store>);

impl Source for Values {
    fn value(&self, id: ValueId) -> Option<Vec<u8>> {
        let store = self.0.lock().unwrap_or_else(|e| e.into_inner());
        store.value(id).ok().flatten()
    }
}

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
        (None, 4321u16, None, None, None);
    let (mut roles, mut role) = (None, "orchestrator".to_string());
    let mut sample = None;
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--journal" => journal = Some(PathBuf::from(value()?)),
            "--port" => port = value()?.parse().map_err(|e| format!("--port: {e}"))?,
            "--workspace" => workspace = Some(PathBuf::from(value()?)),
            "--mandate" => mandate = Some(PathBuf::from(value()?)),
            "--name" => name = Some(value()?),
            "--roles" => roles = Some(PathBuf::from(value()?)),
            "--role" => role = value()?,
            "--sample" => sample = Some(PathBuf::from(value()?)),
            _ => return Err(format!("unknown argument {flag}")),
        }
    }
    let journal_path = journal.ok_or("--journal is required")?;
    let roles = roles.ok_or("--roles is required")?;
    let start = match (workspace, mandate) {
        (Some(workspace), Some(mandate)) => Some(Start {
            workspace,
            name: name.unwrap_or_else(|| role.clone()),
            role,
            mandate: std::fs::read_to_string(&mandate)
                .map_err(|e| format!("the mandate {mandate:?}: {e}"))?,
        }),
        _ => None,
    };
    let state_dir = journal_path.with_extension("workspace-state");

    let store = Store::open(&journal_path).map_err(|e| format!("the journal opens: {e}"))?;
    let values = Store::open(&journal_path).map_err(|e| format!("the journal opens: {e}"))?;
    let (commits, followed) = mpsc::channel();

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
    let feed = Feed::default();
    let mut dictation = Dictation::new(login()?);
    if let Some(sample) = sample {
        dictation = dictation.with_sample(&sample)?;
    }
    let face = Face::with(
        Values(Mutex::new(values)),
        Wiring {
            act: Some(Box::new(inbox.max())),
            dictation: Some(dictation),
            feed: feed.clone(),
        },
    );
    // The Windows host, read again for each connection: DHCP can move it.
    let allow = |peer: IpAddr| Some(peer) == harness_face_web::default_gateway().map(IpAddr::V4);
    let client = Client::new(login()?);
    let mut brain = Brain::launch(
        store, commits, client, state_dir, &roles, start, inbox, feed,
    )?;
    // The brain and the face it serves run until the process is killed.
    thread::scope(|s| {
        s.spawn(|| face.follow(followed));
        s.spawn(|| {
            if let Err(e) = harness_face_web::serve(&face, listener, tls, allow) {
                eprintln!("harness-brain: the face stopped serving: {e}");
            }
        });
        println!("face: https://mx-hv-mj.local:{port}");
        brain.run();
    });
    Ok(())
}
