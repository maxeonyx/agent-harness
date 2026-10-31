//! The brain: what makes the harness's crates a harness.
//!
//! It is the decider of the substrate ([`harness_core::schema::Brain`]):
//! every replica's records reach it, and it folds each into its state or
//! records why not. It works through
//! [`State::todo`](harness_core::state::State::todo): it sends requests
//! through the provider, dispatches calls to limbs, runs the calls of its
//! own tool sets itself, and settles scopes. It reads nothing from disk but
//! its database, its own executable and the limb's beside it, whose digests
//! are the builds it records launching ([`harness_core::build::Launched`]),
//! the role files on a thread of their own ([`roles`]) (and, given
//! `--page`, the page's files, for an edit to show on reload), and keeps
//! nothing a
//! restart would need anywhere else. A relaunch is no special case: the to-do list of the folded database says
//! each unfinished request was lost, and each limb that reconnects is
//! shipped the calls it holds rows for.
//!
//! The brain is a limb: at launch it defines its own limb, which offers its
//! own tool sets (`meta`, `web`, `roles`), and the limb on its machine,
//! which offers `workspace`, `user`, `browser` and `builds`, and states a
//! new version of either whose offers changed. The limb on the machine is
//! one process however many directories its agents work in, which the
//! brain reaches by running `harness-limb attach <limb>` from beside its
//! own executable: that starts the limb if it is not running, and its stdin
//! and stdout are the link ([`link`], [`process`]). A limb outlives the
//! brain that started it, and serves the next one. A limb from before limbs
//! were per machine is attached to only while a call to it is unsettled,
//! and only if it is running (`harness-limb attach --existing <limb>`).
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
//! wording of everything the harness says to a model, the `meta` and `web`
//! sets' tools, and the policy the core leaves open live in
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
//!
//! # Processes and seams
//!
//! What runs where, and what crosses each seam. The brain's own limb is the
//! brain itself, and its `web` set runs inside the provider's responses.
//!
//! ```mermaid
//! flowchart TB
//!   subgraph tab["Max's tabs, PC or phone"]
//!     page["The page, running harness-face-tab as wasm<br/>its own records in IndexedDB"]
//!   end
//!   subgraph brain["harness-brain, one process"]
//!     face["Face<br/>HTTPS and WebSockets"]
//!     fold["Decider<br/>folds every replica's records"]
//!     todo["To-do list<br/>requests, calls, scopes"]
//!     roles["Role files reader, on a thread"]
//!   end
//!   db[("The brain's SQLite file<br/>records and values")]
//!   subgraph limbs["The limb on each machine: harness-limb serve, one process"]
//!     ws["workspace: shells and files<br/>in each agent's working directory"]
//!     other["user, browser, builds"]
//!     ldb[("Its state directory<br/>lock, socket, SQLite records")]
//!   end
//!   api["Anthropic<br/>Messages API, speech-to-text"]
//!   page <-->|"/link WebSocket: records up; rows, live progress down"| face
//!   face <-->|"records in; folded state out"| fold
//!   face -.->|value bytes| db
//!   fold <--> db
//!   roles -->|"each new reading, over a channel"| fold
//!   fold --> todo
//!   todo <-->|"attach: stdin and stdout"| limbs
//!   todo -->|"requests, a socket each"| api
//!   face -->|dictation| api
//! ```
//!
//! # A real turn
//!
//! A task's turn from Max's words to its end, with one tool call. No step is a script: each is what [`State::todo`](harness_core::state::State::todo) reads off the state after the one before. `Brain::send` may first compact the task (`Brain::compact`) or hold the request (`policy::hold`); neither is drawn.
//!
//! ```mermaid
//! ---
//! title: "A real turn: Max's words, a tool call, the end of the turn"
//! ---
//! sequenceDiagram
//!   participant page as Max's page
//!   participant face as Face
//!   participant brain as The brain
//!   participant req as Request thread
//!   participant api as Anthropic
//!   participant limb as A limb
//!   page->>face: Max presses Send: a Draft Send record goes up the link<br/>/link, from compose.js submit
//!   face->>brain: the record becomes a brain event<br/>Event::Tab: Brain::include
//!   Note over brain: the words are folded in as a delivery, and the waiting task is Ready<br/>State::send_words, Task::deliver<br/>Brain::work: State::todo says Todo::Send
//!   brain->>brain: record that a request is sent<br/>Brain::send: ModelCallEvent::Sent
//!   brain->>req: start a thread to make the request<br/>thread::spawn: Client::send
//!   req->>api: POST /v1/messages, streaming
//!   api-->>req: message_start
//!   req->>brain: the first byte has arrived<br/>Event::FirstByte: Brain::first_byte
//!   api-->>req: the response, as it is written
//!   req-->>face: the words as they come, never a fact<br/>Live::Model chunks
//!   req->>brain: the response is complete<br/>Event::Finished: Brain::finished
//!   Note over brain: one batch: a call is made for each tool use, then the request ends and the task is Acting<br/>CallEvent::Made (Brain::tool_uses), ModelCallEvent::Ended
//!   brain->>limb: send the call to its limb<br/>Todo::Dispatch: Brain::dispatch, CallEvent::Dispatched<br/>the call's row goes down the link in Brain::publish
//!   limb->>limb: run the call<br/>Calls::call
//!   limb->>brain: the call's result<br/>Voice::settle: CallEvent::Settled, in at Brain::heard
//!   Note over brain: the round is over, and the task is Ready<br/>Task::settled
//!   brain->>req: send the next request, with the result<br/>Todo::Send: Brain::send
//!   req->>brain: the model ends its turn<br/>Brain::finished: Stop::EndTurn
//!   Note over brain: the task waits for Max<br/>Task::turn_ended
//!   brain-->>face: publish the folded state<br/>Brain::publish
//!   face-->>page: the rows that changed<br/>/link
//! ```
//!
//! # Stopping a task
//!
//! What happens when Max abandons a task: the stop that reaches furthest, from the task's request through its calls to the processes of a shell, and down to the members of the scopes it owns. Work that is durable is stopped by facts (the drain, `CallEvent::Stopping`), so a restart carries a stop on; work that is only a process is stopped by [`harness_runtime::Cancel`], whose deadline only moves forward. The three branches are the three things a task can have running, and it may have all of them. A pause and a park wait for a safe point and cut nothing, so they are not drawn.
//!
//! ```mermaid
//! ---
//! title: "Max abandons a task"
//! ---
//! sequenceDiagram
//!   participant page as Max's page
//!   participant brain as The brain
//!   participant req as Request thread
//!   participant limb as A limb
//!   participant tree as The call's processes
//!   page->>brain: Max stops the task, giving it two minutes<br/>TaskEvent::Drain, Drain::Abandon by Max
//!   Note over brain: the task is draining, with a deadline<br/>Task::event: Draining::Abandoning, and State::todo reads the stop off it
//!   par its request in flight, Phase::Thinking
//!     brain->>req: cut the request now<br/>Todo::Cut: Cancel::now
//!     req->>brain: the request ends, cut<br/>Brain::finished: Ending::Cut
//!   and each call it waits on, in its round or in the background
//!     brain->>limb: ask the limb to stop the call, by the deadline<br/>Todo::Interrupt: Brain::interrupt, CallEvent::Stopping
//!     limb->>tree: end the call's processes: SIGTERM now, SIGKILL at the deadline<br/>Calls::call: Cancel::request, shell::run: Group::end
//!     limb->>brain: the call settles as interrupted<br/>Voice::settle: Outcome::Interrupted
//!   and each member of a scope it owns
//!     brain->>brain: stop the member, giving it three quarters of the time left<br/>Todo::Abandon: Brain::stop_member, Drain::Abandon by Owner
//!     Note over brain: each member then goes through this same diagram
//!     brain->>brain: settle the calls its scope still holds<br/>Todo::Settle: Brain::settle_scope
//!   end
//!   Note over brain: nothing of its round is left<br/>Task::drained
//!   brain->>brain: ask for a tidy-up turn<br/>Todo::Tidy: Prompt::Tidy
//!   brain->>req: send that turn, its one last request<br/>Todo::Send
//!   req->>brain: the model ends the turn<br/>Brain::finished: Stop::EndTurn
//!   Note over brain: the task ends as abandoned, with its report<br/>Task::turn_ended: Outcome::Abandoned<br/>At the deadline instead, Todo::Expire: TaskEvent::Expired ends it with no report<br/>"End now" is the same stop with no time left, Drain::Abandon within 0: no tidy-up turn, no report
//! ```
//!
//! # A usage fetch
//!
//! Max's click on the usage gauges. The same numbers arrive for free in the headers of every response (the quota of `ModelCallEvent::Ended`), and the view shows whichever is newer.
//!
//! ```mermaid
//! ---
//! title: "A usage fetch: Max asks for the plan's usage"
//! ---
//! sequenceDiagram
//!   participant page as Max's page
//!   participant brain as The brain
//!   participant th as Reading thread
//!   participant api as Anthropic
//!   page->>brain: Max clicks the gauges: a call to the usage tool on the brain's limb<br/>CallEvent::Made: Caller::Max, chrome.js refreshUsage
//!   Note over brain: the call is due, and the brain runs it itself<br/>State::todo: Todo::Dispatch, by its tool's name
//!   brain->>brain: record it as dispatched<br/>Brain::dispatch: Brain::read_usage: CallEvent::Dispatched
//!   brain->>th: start a thread to ask Anthropic<br/>thread::spawn: Client::usage
//!   th->>api: GET /api/oauth/usage, with the login's bearer token
//!   api-->>th: each window's utilization and resets_at
//!   th->>brain: the reading has arrived<br/>Event::Usage: Brain::read
//!   Note over brain: the values and the call's result are recorded in one batch, or the call fails and says why<br/>web::usage and CallEvent::Settled, or Brain::fail_call
//!   brain-->>page: the view's usage<br/>Brain::publish: view.usage, from face-web's view::usage
//!   Note over page: chrome.js renderUsage
//! ```

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
mod gate;
mod link;
mod process;
mod roles;

use std::net::{IpAddr, TcpListener};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use harness_core::build::{Pathway, Start};
use harness_core::schema;

use harness_core::id::ValueId;
use harness_core::role::UnknownEffort;
use harness_face_web::{Dictation, Face, Feed, Folded, Source, Wiring};
use harness_provider::{Client, Login};
use harness_runtime::Cancel;
use harness_store::Store;

use crate::brain::{Brain, Inbox, Opening};

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
usage: harness-brain --db DB --port PORT [--candidate] [--login DB] [--roles DIR] [--sample WAV] [--model MODEL[/EFFORT]] [--page DIR] [--limbs DIR]
       harness-brain --check DB [--login DB] [--roles DIR] [--port PORT]
       harness-brain --version

  --db DB          the database; a new one is made if none is there
  --port PORT      where the face is served, over HTTPS
  --login DB       the database whose claude.ai login to use: the home
                   brain's (~/.local/share/agent-harness/home/brain.sqlite)
                   unless named, so one sign-in serves every brain
  --roles DIR      the role files the brain reads; a new database needs it,
                   and on an existing one it moves the `roles` set to DIR
  --model MODEL[/EFFORT]
                   run every role on MODEL (and at EFFORT) over its file's:
                   `sonnet/low`; `scripted` costs nothing (it answers from a
                   script in the chat)
  --sample WAV     a recording the page offers to dictate, for a machine with
                   no microphone
  --page DIR       serve the page's files from DIR (crates/face-web/src/page)
                   as they are on disk, so an edit shows on reload, instead of
                   the ones built in
  --limbs DIR      start the harness-limb in DIR for a machine's limb, not
                   the one beside this executable: a rehearsal's scratch
                   brain meeting the running build's limb
  --candidate      open DB only once the gate has passed on it, writing
                   nothing before: then point the installed link (`build`
                   beside DB) at this build, and launch; otherwise exit 1
  --check DB       run the gate on DB, a copy of a brain's database: fold
                   it, decide without acting, have the provider count each
                   live session's next request, stream one short response
                   using a tool, bind the port (any, unless named) and serve
                   the face read-only; print `signed in: yes` or `no` as it
                   reads the login, each pathway that passed, and fail with a
                   line for each that did not (a few real tokens); once it
                   passes, serve read-only until SIGTERM
  --version        print the commit it was built from
  --help           print this

HARNESS_BROWSER    the harness's browser: an endpoint to attach to
                   (host:port, http://… or ws://…) or a browser to launch;
                   empty for Playwright's headless shell. A changed value is a
                   new version of the machine's limb; unset keeps the one it has

Max starts a chat as he does at a tab: at the page, or from a script with
`target/release/examples/act PORT chat ROLE DIR|- TEXT`.
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failed(lines)) => {
            for line in lines {
                errln!("harness-brain: {line}");
            }
            ExitCode::FAILURE
        }
    }
}

/// Why the brain did not run, one line each.
struct Failed(Vec<String>);

impl From<String> for Failed {
    fn from(why: String) -> Failed {
        Failed(vec![why])
    }
}

impl From<&str> for Failed {
    fn from(why: &str) -> Failed {
        Failed(vec![why.to_owned()])
    }
}

impl From<Vec<gate::Failure>> for Failed {
    fn from(failures: Vec<gate::Failure>) -> Failed {
        Failed(failures.iter().map(ToString::to_string).collect())
    }
}

/// Say that a pathway of the gate passed, and how long it took.
fn passed(pathway: Pathway, took: Duration) {
    outln!("gate: {pathway}: passed in {:.1}s", took.as_secs_f64());
}

fn run() -> Result<(), Failed> {
    let mut args = std::env::args().skip(1);
    let (mut db, mut port, mut roles, mut sample, mut model) = (None, None, None, None, None);
    let (mut page, mut login, mut check, mut start) = (None, None, None, Start::Installed);
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
            "--login" => login = Some(Login::at(PathBuf::from(value()?))),
            "--check" => check = Some(PathBuf::from(value()?)),
            "--limbs" => {
                let _ = process::LIMBS.set(PathBuf::from(value()?));
            }
            Start::FLAG => start = Start::Candidate,
            "--help" | "-h" => {
                outln!("{}", USAGE.trim_end());
                return Ok(());
            }
            _ => return Err(format!("unknown argument {flag}\n{USAGE}").into()),
        }
    }
    let login = match login {
        Some(login) => login,
        None => Login::home().map_err(|e| e.to_string())?,
    };
    let client = Client::new(login.clone()).map_err(|e| e.0)?;
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
    let db = match (&check, db) {
        (Some(copy), _) => copy.clone(),
        (None, db) => db.ok_or("--db is required")?,
    };
    let port = match check {
        Some(_) => port.unwrap_or(0),
        None => port.ok_or("--port is required")?,
    };
    if check.is_some() {
        let signed_in = (login.signed_in()).map_err(|e| format!("the login: {e}"))?;
        outln!("signed in: {}", if signed_in { "yes" } else { "no" });
    } else {
        outln!("harness-brain {SOURCE}");
    }
    let offered = client.models(&Cancel::new()).unwrap_or_else(|e| {
        errln!("harness-brain: the provider's models: {e}; aliases keep what they resolved to");
        vec![harness_provider::scripted()]
    });
    // An installed start makes a new database if none is there.
    let installed = match (&check, start) {
        (None, Start::Installed) => {
            Some(Store::open(&db).map_err(|e| format!("the database opens: {e}"))?)
        }
        _ => None,
    };
    // The face serves from the gate's Serve on, read-only until the brain
    // opens: what it found, while the rest is checked.
    let values = Store::read(&db).map_err(|e| format!("the database opens: {e}"))?;
    let feed = Feed::default();
    let mut dictation = Dictation::new(login.clone()).map_err(|e| e.0)?;
    if let Some(sample) = sample {
        dictation = dictation.with_sample(&sample)?;
    }
    let face = Face::with(
        Values(Mutex::new(values)),
        Wiring {
            dictation: Some(dictation),
            login: Some(login.clone()),
            feed: feed.clone(),
            models: offered.clone(),
            source: SOURCE.into(),
            page,
            report: Box::new(|error| errln!("harness-brain: {error}")),
        },
    );
    let (folds, followed) = mpsc::channel();
    thread::spawn({
        let face = Arc::clone(&face);
        move || face.follow(followed)
    });
    let serve = |decider: &schema::Brain<Store>, listener: TcpListener, tls| {
        let _ = folds.send(Folded {
            state: decider.state().clone(),
            as_of: decider.head(),
            standing: decider.standings().clone(),
        });
        let port = listener.local_addr().map_or(port, |a| a.port());
        outln!("face: https://mx-hv-mj.local:{port}");
        let face = Arc::clone(&face);
        // The Windows host, read again for each connection: DHCP can move it.
        let allow =
            |peer: IpAddr| Some(peer) == harness_face_web::default_gateway().map(IpAddr::V4);
        thread::spawn(move || {
            if let Err(e) = harness_face_web::serve(&face, listener, tls, allow) {
                errln!("harness-brain: the face stopped serving: {e}");
            }
        });
    };
    if check.is_some() {
        gate::gate(&db, &client, roles.as_deref(), port, passed, serve)?;
        outln!("gate: passed; serving read-only until stopped");
        loop {
            thread::park();
        }
    }
    let (decider, opening) = match installed {
        Some(store) => {
            let (decider, divergences) = schema::Brain::open(store.replica(), store)
                .map_err(|e| format!("the database folds: {e}"))?;
            for d in &divergences {
                errln!("harness-brain: skipping the record at {}: {}", d.at, d.why);
            }
            let opening = Opening::of(decider.state(), decider.next(), roles.as_deref())?;
            let (listener, tls) = gate::serve(port)?;
            serve(&decider, listener, tls);
            (decider, opening)
        }
        None => {
            let gate::Passed { decider, opening } =
                gate::gate(&db, &client, roles.as_deref(), port, passed, serve)?;
            (decider.storage().write()).map_err(|e| format!("the database writes: {e}"))?;
            install(&db)?;
            (decider, opening)
        }
    };
    let inbox = Inbox::new();
    inbox.signals()?;
    face.open(inbox.include());
    login.keep_fresh();
    let mut brain = Brain::launch(
        decider,
        opening,
        folds,
        client,
        roles::Models {
            offered,
            every,
            effort,
        },
        inbox,
        feed,
    );
    let ran = panic::catch_unwind(AssertUnwindSafe(|| brain.run()));
    errln!("harness-brain: shut down");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    std::process::exit(if ran.is_ok() { 0 } else { 101 })
}

/// Point the domain's installed link, `build` beside the database `db`, at
/// this build's directory, in one step: a restart of the domain runs this
/// build from now on.
fn install(db: &Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("the brain's own executable: {e}"))?;
    let build = (exe.parent().and_then(Path::parent))
        .ok_or(format!("{} is in no build's bin/", exe.display()))?;
    let at = db.with_file_name("build");
    let staging = at.with_extension("new");
    let _ = std::fs::remove_file(&staging);
    std::os::unix::fs::symlink(build, &staging)
        .and_then(|()| std::fs::rename(&staging, &at))
        .map_err(|e| format!("the installed link {}: {e}", at.display()))
}
