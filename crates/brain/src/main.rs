//! The brain: what makes the harness's crates a harness.
//!
//! It is the decider of the substrate ([`harness_core::schema::Brain`]):
//! every replica's records reach it, and it folds each into its state or
//! records why not. It works through
//! [`State::todo`](harness_core::state::State::todo): it sends requests
//! through the provider, dispatches calls to limbs, runs the meta limb's
//! calls itself, and settles scopes. It reads nothing from disk but its
//! database and its own executable, whose digest is the build it records
//! launching ([`harness_core::build::Launched`]), and keeps nothing a restart would need anywhere else. A
//! relaunch is no special case: the to-do list of the folded database says
//! each unfinished request was lost, and each limb that reconnects is
//! shipped the calls it holds rows for.
//!
//! Each plugin limb runs as a process of its own, `harness-limb-<kind>`
//! from beside the brain's executable, linked to the brain over a Unix
//! socket in its state directory ([`link`], [`process`]). A limb outlives
//! the brain that started it, and reconnects to the next one. The roles
//! limb, which states the environments read from role files, runs on a
//! thread of the brain, linked by a channel ([`roles`]).
//!
//! Ctrl-C, `SIGTERM` or `SIGHUP` shuts the brain down: nothing new starts,
//! the requests in flight have [`harness_program::policy::DRAIN`] to finish, the limbs stop
//! their commands by the same deadline and exit, and the process exits
//! once none of it is left. A second signal cuts everything now. Tasks are
//! not stopped: the next launch carries them on ([`harness_core::cancel`]).
//!
//! The face is served from the same process. After each change the brain
//! hands it the state, a persistent value it holds while the brain moves
//! on, with how far the brain has included each replica; the bytes facts
//! name it reads over a connection of its own, since a stored value never
//! changes. Each of Max's tabs is a replica: its records reach the brain,
//! which states that the tab is his the first time, and includes them.
//! Each request's streamed events and each command's output as it prints
//! go to the face's live feed, never to the database.
//!
//! **The session's program** is not here. What a request renders, the
//! wording of everything the harness says to a model, the meta, Max and web
//! limbs' tools, and the policy the core leaves open live in
//! `harness-program`, which does no I/O and builds for wasm32; the brain
//! only calls it. What is left here is I/O: the database, the provider,
//! limb processes and sockets, the face, signals, and where a new clone
//! goes on disk.
//!
//! ```text
//! harness-brain --db DB [--port 4321] [--roles DIR] [--sample WAV]
//!               [--workspace DIR --mandate FILE [--name NAME] [--role ROLE]]
//! ```
//! A new database needs `--roles`, the directory the roles limb reads; an
//! existing one keeps the roles limb it has. With a workspace and a
//! mandate, the brain starts a task in Max's scope as his: a chat with him,
//! like every task he starts, done when he says. Its mandate is his words,
//! so its briefs can quote it. `--sample` offers the page a recording to
//! dictate, for a machine with no microphone.

mod brain;
mod link;
mod process;
mod roles;

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
    let (mut db, mut port, mut workspace, mut mandate, mut name) =
        (None, 4321u16, None, None, None);
    let (mut roles, mut role) = (None, "orchestrator".to_string());
    let mut sample = None;
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--db" => db = Some(PathBuf::from(value()?)),
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
    let db = db.ok_or("--db is required")?;
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
    let state_dir = db.with_extension("limbs");

    let store = Store::open(&db).map_err(|e| format!("the database opens: {e}"))?;
    let values = Store::open(&db).map_err(|e| format!("the database opens: {e}"))?;
    let (folds, followed) = mpsc::channel();

    let certs = PathBuf::from(std::env::var("HOME").map_err(|e| e.to_string())?)
        .join(".config/dev-certs/shared-local");
    let tls = harness_face_web::tls(
        &certs.join("wildcard-local.crt"),
        &certs.join("wildcard-local.key"),
    )
    .map_err(|e| format!("the certificate: {e}"))?;
    let listener = TcpListener::bind(("0.0.0.0", port)).map_err(|e| format!("port {port}: {e}"))?;
    let inbox = Inbox::new();
    inbox.signals()?;
    let login = || Login::claude_code().map_err(|e| format!("the login: {}", e.0));
    let feed = Feed::default();
    let mut dictation = Dictation::new(login()?);
    if let Some(sample) = sample {
        dictation = dictation.with_sample(&sample)?;
    }
    let face = Face::with(
        Values(Mutex::new(values)),
        Wiring {
            include: Some(inbox.include()),
            dictation: Some(dictation),
            feed: feed.clone(),
        },
    );
    // The Windows host, read again for each connection: DHCP can move it.
    let allow = |peer: IpAddr| Some(peer) == harness_face_web::default_gateway().map(IpAddr::V4);
    let client = Client::new(login()?);
    let mut brain = Brain::launch(
        store,
        folds,
        client,
        state_dir,
        roles.as_deref(),
        start,
        inbox,
        feed,
    )?;
    // The face serves until the brain has shut down.
    thread::scope(|s| {
        s.spawn(|| face.follow(followed));
        s.spawn(|| {
            if let Err(e) = harness_face_web::serve(&face, listener, tls, allow) {
                eprintln!("harness-brain: the face stopped serving: {e}");
            }
        });
        println!("face: https://mx-hv-mj.local:{port}");
        brain.run();
        eprintln!("harness-brain: shut down");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        std::process::exit(0)
    })
}
