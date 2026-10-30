//! A limb as a server on its machine ([`Host`](harness_core::limb::Host)):
//! one process per brain, machine and user, running the tool sets its
//! definition offers.
//!
//! `harness-limb` is one executable (`crates/limb-machine`) with two
//! commands for the limb, besides the builds' own:
//! - `attach [--existing] <limb>` is how the brain reaches the limb, and the
//!   only one it runs. It starts the limb if no instance holds its state
//!   (unless `--existing`: then it only reaches one that runs, as for a limb
//!   from before limbs were per machine, linked only to settle its calls),
//!   then carries the link over its own stdin and stdout until either end
//!   closes. What it prints on stderr when it fails is why ([`attach`]).
//! - `serve <limb>` is the limb itself, started by `attach` and never by
//!   hand: it holds the limb's state, listens on its socket, and runs its
//!   tool sets for whichever brain links to it ([`serve`]).
//!
//! A limb's state is one directory on its host, named by its id
//! ([`State`]): the lock one instance holds while it runs, the socket it
//! listens on, its records, the log of what it printed, and a directory
//! per tool set. So the state is the limb's address and its lock at once.
//!
//! An [`Instance`] holds the brain's rows for the limb, its records, which
//! it says through its [`Voice`], and its tool sets, each a [`ToolSet`]
//! opened from one of the definition's offers on its own, so one that
//! cannot open fails only itself. It runs every call with [`Calls`],
//! dispatched to the set that has its tool by name, in the order each agent
//! wrote them. What a call runs on the machine runs in a systemd scope of
//! its own ([`scope`]).
//!
//! Max: "I'm not interested in the crash. I think that that's bad code if
//! we crash." The limb crates deny `unwrap`, `expect`, `panic!` and
//! `unreachable!`; a panic in a tool set all the same fails only the call
//! or the set it was in.
//!
//! # A limb's setup
//!
//! How the brain comes to use a limb, from needing it to the limb's description, which makes it reached (`State::reached`) so that a task's first root can be made. A limb outlives the brain that started it, so after a brain's restart only the second branch of the `alt` runs.
//!
//! ```mermaid
//! ---
//! title: "A limb's setup: from Todo::Connect to a limb the brain can use"
//! ---
//! sequenceDiagram
//!   participant brain as The brain
//!   participant attach as attach LIMB
//!   participant serve as serve LIMB, the limb
//!   participant sets as Its tool sets
//!   Note over brain: a task needs a limb that is not connected<br/>State::todo: Todo::Connect
//!   brain->>attach: start the link, whose stdin and stdout are the brain's end<br/>harness-limb attach LIMB
//!   attach->>attach: look for the limb's socket<br/>attach::run: State::enter, connect
//!   alt nothing is listening
//!     attach->>serve: start the limb in harness-limb-LIMB.scope, if there is a user manager<br/>attach::start: serve LIMB
//!     serve->>serve: take the lock, detach, open its records, listen on the socket<br/>serve::run: setsid, subreaper, Instance::open
//!     attach->>serve: connect, tried every 50 ms for up to 30 s
//!   else a limb already runs, as it does after a brain restart
//!     attach->>serve: connect
//!   end
//!   serve->>brain: the limb says Hello<br/>Hello, through attach::relay
//!   brain->>serve: a whole view, with LimbRow::Definition and a LimbRow::Workdir per directory
//!   Note over serve: that connection is now the link<br/>Instance::apply
//!   serve->>sets: open each offer's set on its own; hand every set the rows<br/>Instance::define, ToolSet::apply
//!   sets->>brain: the workspace prepares each directory on a thread of its own, and says what it found<br/>Fact::Workdir
//!   serve->>brain: the limb describes its sets, a record on the link<br/>Fact::Described
//!   Note over brain: State::reached is now true, and a task whose directory is Ready can be rooted
//! ```

mod attach;
mod calls;
pub mod scope;
mod serve;
mod set;
mod voice;

pub use calls::{Calls, Limit};
pub use set::{Open, Opening, Running, ToolSet, offer, settings};
pub use voice::{CallRow, Instance, Report, Rows, Voice, instant, lock, now};

use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use harness_core::id::LimbId;
use harness_core::limb::LimbUp;

/// How long a limb runs on with no brain linked: long enough for a crashed
/// brain to be restarted by hand, so a restart never costs a limb its
/// running calls.
pub const ABSENT: Duration = Duration::from_secs(15 * 60);

/// How long its calls get to stop when the limb shuts down without being
/// told how long: no brain for [`ABSENT`], or a signal.
pub const SHUTDOWN: Duration = Duration::from_secs(5);

/// What a limb ships, in order: to the link while there is one.
pub type Out = Box<dyn Fn(LimbUp) + Send + Sync>;

/// Run the limb's own commands, `attach [--existing] <limb>` or
/// `serve <limb>`, from `args` (the arguments after the program's name),
/// with the tool sets `sets` it can open, by name.
pub fn main(args: &[&str], sets: &'static [(&'static str, Open)]) -> ExitCode {
    let ran = match args {
        ["attach", limb] => parse(limb).and_then(|l| attach::run(l, true)),
        ["attach", "--existing", limb] => parse(limb).and_then(|l| attach::run(l, false)),
        ["serve", limb] => parse(limb).and_then(|l| serve::run(l, sets)),
        _ => Err("usage: harness-limb attach [--existing] <limb> | serve <limb>".into()),
    };
    match ran {
        Ok(code) => code,
        // What `attach` says is the limb's own last words, or its own.
        Err(e) if args.first() == Some(&"attach") => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("harness-limb: {e}");
            ExitCode::FAILURE
        }
    }
}

fn parse(limb: &str) -> Result<LimbId, String> {
    limb.parse().map_err(|e| format!("the limb {limb}: {e:?}"))
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
        let from = usize::try_from(from).unwrap_or(usize::MAX);
        let text = String::from_utf8_lossy(text.get(from..).unwrap_or_default());
        match text.lines().rev().find(|l| !l.trim().is_empty()) {
            Some(line) => line.trim().to_string(),
            None => format!("it ended saying nothing; see {}", self.log().display()),
        }
    }
}

/// What a panic said, for a call or a set that it failed.
pub fn panicked(payload: &(dyn std::any::Any + Send)) -> String {
    match payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
    {
        Some(message) => format!("it panicked: {message}"),
        None => "it panicked".into(),
    }
}

/// Run `f`, a panic in it becoming an error that says so.
pub fn guard<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|p| panicked(&*p))
}
