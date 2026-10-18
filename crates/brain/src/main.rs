//! The brain: what makes the harness's crates a harness.
//!
//! It is the decider of the substrate ([`harness_core::schema::Brain`]):
//! every replica's records reach it, and it folds each into its state or
//! records why not. It works through
//! [`State::todo`](harness_core::state::State::todo): it sends requests
//! through the provider, dispatches calls to limbs, runs the meta limb's
//! calls itself, and settles scopes. It reads nothing from disk but its
//! database and its own executable, whose digest is the build it records
//! launching ([`harness_core::build::Launched`]) (and, given `--page`, the
//! page's files, for an edit to show on reload), and keeps nothing a
//! restart would need anywhere else. A relaunch is no special case: the to-do list of the folded database says
//! each unfinished request was lost, and each limb that reconnects is
//! shipped the calls it holds rows for.
//!
//! Each plugin limb runs as a server of its own, which the brain reaches
//! by running `harness-limb-<kind> attach <limb>` from beside its own
//! executable: that starts the limb if it is not running, and its stdin and
//! stdout are the link ([`link`], [`process`]). A limb outlives the brain
//! that started it, and serves the next one. The roles
//! limb, which states the environments read from role files, runs on a
//! thread of the brain, linked by a channel ([`roles`]).
//!
//! Ctrl-C, `SIGTERM` or `SIGHUP` shuts the brain down: nothing new starts,
//! the requests in flight have [`harness_program::policy::DRAIN`] to finish, and the process
//! exits once none of them is left. Its limbs run on, with their calls, for
//! the next brain. A second signal cuts everything now. Tasks are
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
//! wording of everything the harness says to a model, the meta and web limbs'
//! tools, and the policy the core leaves open live in
//! `harness-program`, which does no I/O and builds for wasm32; the brain
//! only calls it. What is left here is I/O: the database, the provider,
//! limb links, the face, signals, and where a new clone
//! goes on disk.
//!
//! Its flags are in [`USAGE`], which `harness-brain --help` prints.
//!
//! At launch the brain reads the models on offer from the provider once;
//! a role's alias (`opus`) means the newest of its family, and a new model
//! reaches the roles at the next launch.

/// Print a line to stdout, dropping it once nothing reads it: `println!`
/// panics on a closed pipe, and losing the reader of the brain's output
/// must never stop the brain, least of all from shutting down.
macro_rules! outln {
    ($($line:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stdout(), $($line)*);
    }};
}

/// Print a line to stderr, dropping it once nothing reads it, as `outln!`.
macro_rules! errln {
    ($($line:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), $($line)*);
    }};
}

mod brain;
mod check;
mod link;
mod process;
mod roles;

use std::net::{IpAddr, TcpListener};
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Mutex, mpsc};
use std::thread;

use harness_core::environment::UnknownEffort;
use harness_core::id::ValueId;
use harness_face_web::{Dictation, Face, Feed, Source, Wiring};
use harness_provider::{Client, Login};
use harness_runtime::Cancel;
use harness_store::Store;

use crate::brain::{Brain, Inbox};

/// The values the face shows, read over its own connection to the store.
struct Values(Mutex<Store>);

impl Source for Values {
    fn value(&self, id: ValueId) -> Option<Vec<u8>> {
        let store = self.0.lock().unwrap_or_else(|e| e.into_inner());
        store.value(id).ok().flatten()
    }
}

/// The commit this brain was built from, `+dirty` if the tree had
/// changes: what `--version`, the launch and the face say.
const SOURCE: &str = env!("HARNESS_SOURCE");

/// What `harness-brain --help` prints. `--model scripted` is
/// [`harness_provider::SCRIPTED`].
const USAGE: &str = "\
usage: harness-brain --db DB --port PORT [--roles DIR] [--sample WAV] [--model MODEL[/EFFORT]] [--page DIR]
       harness-brain --check DB
       harness-brain --version

  --db DB          the database; a new one is made if none is there
  --port PORT      where the face is served, over HTTPS
  --roles DIR      the role files the roles limb reads; a new database needs
                   it, and on an existing one it moves the limb to DIR
  --model MODEL[/EFFORT]
                   run every role on MODEL (and at EFFORT) over its file's:
                   `sonnet/low`; `scripted` costs nothing (it answers from a
                   script in the chat)
  --sample WAV     a recording the page offers to dictate, for a machine with
                   no microphone
  --page DIR       serve the page's files from DIR (crates/face-web/src/page)
                   as they are on disk, so an edit shows on reload, instead of
                   the ones built in
  --check DB       whether this build can take over the brain whose database
                   DB is a copy of: fold it, have the provider count each live
                   session's next request, stream one short response; print a
                   JSON report, and fail if anything did (a few real tokens)
  --version        print the commit it was built from
  --help           print this

HARNESS_BROWSER    the harness's browser: an endpoint to attach to
                   (host:port, http://… or ws://…) or a browser to launch;
                   empty for Playwright's headless shell. A changed value is a
                   new version of the browser limb; unset keeps the one it has

Max starts a chat as he does at a tab: at the page, or from a script with
`target/release/examples/act PORT chat ROLE DIR|- TEXT`.
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            errln!("harness-brain: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut db, mut port, mut roles, mut sample, mut model) = (None, None, None, None, None);
    let mut page = None;
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--db" => db = Some(PathBuf::from(value()?)),
            "--port" => port = Some(value()?.parse().map_err(|e| format!("--port: {e}"))?),
            "--roles" => roles = Some(PathBuf::from(value()?)),
            "--sample" => sample = Some(PathBuf::from(value()?)),
            "--model" => model = Some(value()?),
            "--version" => {
                outln!("harness-brain {SOURCE}");
                return Ok(());
            }
            "--page" => page = Some(PathBuf::from(value()?)),
            "--check" => {
                let db = PathBuf::from(value()?);
                let login = Login::claude_code().map_err(|e| format!("the login: {}", e.0))?;
                let (report, ok) = check::check(&db, &Client::new(login), SOURCE)?;
                outln!("{report:#}");
                return if ok {
                    Ok(())
                } else {
                    Err("the check failed".into())
                };
            }
            "--help" | "-h" => {
                outln!("{}", USAGE.trim_end());
                return Ok(());
            }
            _ => return Err(format!("unknown argument {flag}\n{USAGE}")),
        }
    }
    let db = db.ok_or("--db is required")?;
    // `--model sonnet/low`: the model, and the effort after a slash.
    let (every, effort) = match model
        .as_deref()
        .map(|m| m.split_once('/').unwrap_or((m, "")))
    {
        None => (None, None),
        Some((model, effort)) => (
            Some(model.to_owned()).filter(|m| !m.is_empty()),
            match effort {
                "" => None,
                e => Some(
                    e.parse()
                        .map_err(|e: UnknownEffort| format!("--model: {e}"))?,
                ),
            },
        ),
    };
    outln!("harness-brain {SOURCE}");
    let port: u16 = port.ok_or("--port is required")?;
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
    let client = Client::new(login()?);
    let offered = client.models(&Cancel::new()).unwrap_or_else(|e| {
        errln!("harness-brain: the provider's models: {e}; aliases keep what they resolved to");
        vec![harness_provider::scripted()]
    });
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
            models: offered.clone(),
            source: SOURCE.into(),
            page,
        },
    );
    // The Windows host, read again for each connection: DHCP can move it.
    let allow = |peer: IpAddr| Some(peer) == harness_face_web::default_gateway().map(IpAddr::V4);
    let mut brain = Brain::launch(
        store,
        folds,
        client,
        roles::Models {
            offered,
            every,
            effort,
        },
        roles.as_deref(),
        inbox,
        feed,
    )?;
    // The face serves until the brain has shut down or panicked: the scope
    // would wait for the face forever, so the process exits from inside it.
    thread::scope(|s| {
        s.spawn(|| face.follow(followed));
        s.spawn(|| {
            if let Err(e) = harness_face_web::serve(&face, listener, tls, allow) {
                errln!("harness-brain: the face stopped serving: {e}");
            }
        });
        outln!("face: https://mx-hv-mj.local:{port}");
        let ran = panic::catch_unwind(AssertUnwindSafe(|| brain.run()));
        errln!("harness-brain: shut down");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        std::process::exit(if ran.is_ok() { 0 } else { 101 })
    })
}
