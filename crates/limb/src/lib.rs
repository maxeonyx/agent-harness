//! A plugin limb as a server on its host
//! ([`Host`](harness_core::environment::Host)): what every kind of limb
//! shares, so a kind is only what it does with its rows.
//!
//! `harness-limb-<kind>` is one executable with two commands:
//! - `attach <limb>` is how the brain reaches the limb, and the only one it
//!   runs. It starts the limb if no instance holds its state, then carries
//!   the link over its own stdin and stdout until either end closes. What it
//!   prints on stderr when it fails is why ([`attach`]).
//! - `serve <limb>` is the limb itself, started by `attach` and never by
//!   hand: it holds the limb's state, listens on its socket, and runs the
//!   kind for whichever brain links to it ([`serve`]).
//!
//! A limb's state is one directory on its host, named by its id
//! ([`State`]): the lock one instance holds while it runs, the socket it
//! listens on, its records, and the log of what it printed. So the state is
//! the limb's address and its lock at once.
//!
//! An [`Instance`] holds the brain's rows for the limb and its records,
//! which a kind says through its [`Voice`]; a kind that runs calls runs
//! them with [`Calls`].
//!
//! # A limb's setup
//!
//! How the brain comes to use a limb, from needing it to the limb's description, which makes it reached (`State::reached`) so that a task's first root can be made. A limb outlives the brain that started it, so after a brain's restart only the second branch of the `alt` runs. Every kind goes through the same steps: a kind differs only in what its `Kind::open` and `Kind::apply` do.
//!
//! ```mermaid
//! ---
//! title: "A limb's setup: from Todo::Connect to a limb the brain can use"
//! ---
//! sequenceDiagram
//!   participant brain as The brain
//!   participant attach as attach LIMB
//!   participant serve as serve LIMB, the limb
//!   participant kind as The kind, a Workspace
//!   Note over brain: a task needs a limb that is not connected<br/>State::todo: Todo::Connect, as Brain::connect
//!   brain->>attach: start the link, whose stdin and stdout are the brain's end<br/>Process::attach runs harness-limb-KIND attach LIMB
//!   attach->>attach: look for the limb's socket<br/>attach::run: State::enter, connect
//!   alt nothing is listening
//!     attach->>serve: start the limb, in a systemd scope if there is a user manager<br/>attach::start: serve LIMB
//!     serve->>serve: take the lock, detach, listen on the socket<br/>serve::run: setsid, subreaper
//!     attach->>serve: connect, tried every 50 ms for up to 30 s
//!   else a limb already runs, as it does after a brain restart
//!     attach->>serve: connect
//!   end
//!   serve->>brain: the limb says Hello<br/>Hello, through attach::relay, read by link::follow
//!   Note over brain: the link is kept, and the brain ships the limb a whole view<br/>Brain::joined, then Brain::publish with LimbRow::Definition
//!   brain->>serve: the view<br/>LimbDown::View
//!   Note over serve: that connection is now the link, and the limb opens its instance<br/>Served::apply: Instance::open
//!   serve->>kind: open the kind: clone the repository if the definition has one to make, end an earlier instance's orphans<br/>Kind::open: Workspace::open, git::clone
//!   serve->>kind: tell the kind the link is up<br/>Instance::apply: Kind::apply, with connected
//!   kind->>brain: the kind describes itself, a record on the link<br/>Voice::describe: Fact::Described
//!   Note over brain: the brain notes the connection first, then includes the record<br/>Brain::heard: Reach::Connected<br/>State::reached is now true, so Todo::Root can come
//! ```

mod attach;
mod calls;
mod serve;
mod voice;

pub use calls::Calls;
pub use voice::{CallRow, Instance, Report, Rows, Voice, definition, instant, lock, now, settings};

use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use harness_core::environment::{Limb, LimbUp};
use harness_core::id::LimbId;

/// How long a limb runs on with no brain linked: long enough for a crashed
/// brain to be restarted by hand, so a restart never costs a limb its
/// running calls.
pub const ABSENT: Duration = Duration::from_secs(15 * 60);

/// How long its calls get to stop when the limb shuts down without being
/// told how long: no brain for [`ABSENT`], or a signal.
pub const SHUTDOWN: Duration = Duration::from_secs(5);

/// What a kind ships, in order: to the link while there is one.
pub type Out = Box<dyn Fn(LimbUp) + Send + Sync>;

/// One kind of limb: what it does with the rows a brain ships it.
pub trait Kind: Sized + Send + Sync + 'static {
    /// Open an instance on its definition, the first the brain ships: make
    /// what it works in, and say its records through `voice`. What it
    /// returns on failure is why the limb cannot start.
    fn open(definition: &Limb, voice: Arc<Voice>, state: &Path) -> Result<Self, String>;

    /// Act on the brain's rows as they now stand: run the calls it has no
    /// record of, stop those asked to stop, watch what is asked.
    /// `connected`: the rows begin a link, on which it describes itself.
    /// Never blocks on a call.
    fn apply(&self, rows: &Rows, connected: bool);

    /// Whether anything would die with the process: a running call, or what
    /// one left running (a server, a browser and its tabs). A superseded
    /// limb ([`harness_core::environment::LimbRow::Superseded`]) exits once nothing does.
    fn holds(&self) -> bool;

    /// Stop everything running, to be gone `within` from now, and start
    /// nothing new. Asked again with less time, stop harder.
    fn stop_all(&self, within: Duration);

    /// Wait until nothing it started is left.
    fn join(&self);
}

/// Run as `harness-limb-<kind>`: `attach <limb>` or `serve <limb>`.
pub fn main<K: Kind>(kind: &str) -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(command), Some(limb), None) = (args.next(), args.next(), args.next()) else {
        eprintln!("usage: harness-limb-{kind} attach|serve <limb>");
        return ExitCode::FAILURE;
    };
    let ran = limb
        .parse::<LimbId>()
        .map_err(|e| format!("the limb {limb}: {e:?}"))
        .and_then(|limb| match command.as_str() {
            "attach" => attach::run(kind, limb),
            "serve" => serve::run::<K>(kind, limb),
            _ => Err(format!("no command `{command}`: attach or serve")),
        });
    match ran {
        Ok(code) => code,
        // What `attach` says is the limb's own last words, or its own.
        Err(e) if command == "attach" => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("harness-limb-{kind}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// One limb's state on its host: `$XDG_STATE_HOME/harness/limbs/<limb>`.
pub struct State(PathBuf);

impl State {
    /// Make the limb's state, and work in it: its socket is named relative
    /// to it, since a socket's path is limited to 107 bytes and the state's
    /// is not.
    pub fn enter(limb: LimbId) -> io::Result<State> {
        let base = match std::env::var_os("XDG_STATE_HOME") {
            Some(dir) if !dir.is_empty() => PathBuf::from(dir),
            _ => {
                PathBuf::from(std::env::var_os("HOME").ok_or_else(|| io::Error::other("no $HOME"))?)
                    .join(".local/state")
            }
        };
        let dir = base.join("harness/limbs").join(limb.to_string());
        std::fs::create_dir_all(&dir)
            .and_then(|()| std::env::set_current_dir(&dir))
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", dir.display())))?;
        Ok(State(dir))
    }

    pub fn dir(&self) -> &Path {
        &self.0
    }

    /// Its socket, relative to the state, which is the working directory.
    pub fn socket(&self) -> &'static Path {
        Path::new("sock")
    }

    /// What the limb printed, every instance of it.
    pub fn log(&self) -> PathBuf {
        self.0.join("log")
    }

    /// Its records: the replica its state is.
    pub fn records(&self) -> PathBuf {
        self.0.join("limb.sqlite")
    }

    /// The lock one instance holds while it runs, if no instance holds it.
    pub fn lock(&self) -> io::Result<Option<File>> {
        let file = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.0.join("lock"))?;
        // SAFETY: flock on a descriptor this function owns.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return Ok(Some(file));
        }
        match io::Error::last_os_error() {
            e if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            e => Err(e),
        }
    }

    /// Whether an instance is running: it holds the lock.
    pub fn running(&self) -> bool {
        matches!(self.lock(), Ok(None))
    }

    /// The last thing printed after byte `from` of the log: why an instance
    /// that ended did.
    pub fn last_words(&self, from: u64) -> String {
        let text = std::fs::read(self.log()).unwrap_or_default();
        let text = String::from_utf8_lossy(text.get(from as usize..).unwrap_or_default());
        match text.lines().rev().find(|l| !l.trim().is_empty()) {
            Some(line) => line.trim().to_string(),
            None => format!("it ended saying nothing; see {}", self.log().display()),
        }
    }
}
