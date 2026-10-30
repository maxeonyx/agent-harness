//! Running a process in a systemd scope of its own, so what it and
//! everything it starts use counts as theirs, not the limb's, and stops
//! with them.
//!
//! Max (29 Oct): the limb "could create a scope for an agent who has bash
//! callability. And then that agent gets a scope per agent." Each command
//! is `harness-call-<call>.scope` in its agent's `harness-<task>.slice`, a
//! browser the limb launches is a scope of its own, and the limb itself is
//! `harness-limb-<limb>.scope`, so it stays a few MiB in `systemd-cgtop`
//! however much its agents run.
//!
//! `systemd-run --scope` makes the scope and then becomes the process, so
//! the process is still the limb's child, with the limb's descriptors.
//! Whether it got that far is told on a pipe of its own: a shell between
//! them says so on it before it runs the program with the real output.
//! Where `systemd-run` fails (no user manager, a unit of that name already
//! running), the program runs without a scope, and the caller says so.

use std::ffi::{OsStr, OsString};
use std::io::{self, PipeReader, Read};
use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

/// What the shell between `systemd-run` and the program says once the
/// scope is made.
const STARTED: &str = "harness-scope-started";

/// The shell between them: it says the scope is made, then runs the
/// program (`"$@"`) with the output the caller gave, on descriptor 3.
const WRAPPER: &str = "printf '%s\\n' harness-scope-started >&2; exec \"$@\" >&3 2>&3 3>&-";

/// A systemd scope to run a process in.
pub struct Scope {
    /// Its unit's name, without `.scope`.
    pub unit: String,
    /// The slice it is in, with `.slice`; the user manager's own when none.
    pub slice: Option<String>,
    /// What `systemctl` and `systemd-cgtop` show it as.
    pub description: String,
}

/// Whether a systemd user manager runs this user's units.
pub fn user_manager() -> bool {
    std::env::var_os("XDG_RUNTIME_DIR")
        .is_some_and(|dir| Path::new(&dir).join("systemd/private").exists())
}

/// A command to spawn, and how to know whether it is in its scope.
pub struct Spawn {
    pub command: Command,
    /// Why it runs without the scope it was given, if it does from the
    /// start.
    without: Option<String>,
    /// The pipe the shell says the scope is made on, while the scope is
    /// still to be confirmed.
    told: Option<PipeReader>,
    /// The output, kept open until the command has spawned.
    _out: Option<OwnedFd>,
}

/// `program args` as a command, its stdout and stderr both `out` (or
/// nothing), and its stdin nothing: in `scope` if it is given and there is
/// a user manager, otherwise as it is. The caller sets the rest (its
/// directory, its environment), spawns [`Spawn::command`], then asks
/// [`Spawn::scoped`] whether it is in its scope.
pub fn command(
    program: &OsStr,
    args: &[OsString],
    scope: Option<&Scope>,
    out: Option<OwnedFd>,
) -> io::Result<Spawn> {
    let without = match scope {
        Some(_) if !user_manager() => Some("there is no systemd user manager".to_string()),
        _ => None,
    };
    let Some(scope) = scope.filter(|_| without.is_none()) else {
        let mut command = Command::new(program);
        command.args(args).stdin(Stdio::null());
        match &out {
            Some(out) => command.stdout(out.try_clone()?).stderr(out.try_clone()?),
            None => command.stdout(Stdio::null()).stderr(Stdio::null()),
        };
        return Ok(Spawn {
            command,
            without,
            told: None,
            _out: out,
        });
    };
    let (told, tell) = io::pipe()?;
    let mut command = Command::new("systemd-run");
    command
        .args(["--user", "--scope", "--quiet", "--collect"])
        // Its arguments are the program's, as given, not `$VAR`s for it to expand.
        .arg("--expand-environment=no")
        .arg(format!("--unit={}", scope.unit))
        .arg(format!("--description={}", scope.description));
    if let Some(slice) = &scope.slice {
        command.arg(format!("--slice={slice}"));
    }
    command
        .args(["--", "sh", "-c", WRAPPER, "sh"])
        .arg(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(tell.try_clone()?)
        .stderr(tell);
    let out_fd: Option<RawFd> = out.as_ref().map(AsRawFd::as_raw_fd);
    // SAFETY: between fork and exec, only dup2, fcntl and open, which are
    // async-signal-safe, on descriptors this process holds open.
    unsafe {
        command.pre_exec(move || {
            let fd = match out_fd {
                Some(fd) => fd,
                None => {
                    let null = libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY);
                    if null < 0 {
                        return Err(io::Error::last_os_error());
                    }
                    null
                }
            };
            let done = if fd == 3 {
                libc::fcntl(3, libc::F_SETFD, 0)
            } else {
                libc::dup2(fd, 3)
            };
            if done < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Ok(Spawn {
        command,
        without: None,
        told: Some(told),
        _out: out,
    })
}

/// Where a spawned command runs.
pub enum Scoped {
    /// In its scope, or it was given none.
    In,
    /// Without its scope, and why: it runs all the same.
    Without(String),
    /// `systemd-run` ended without running anything, saying this.
    Failed(String),
}

impl Spawn {
    /// Once spawned: where it runs.
    pub fn scoped(self) -> Scoped {
        let Spawn {
            command,
            without,
            told,
            _out,
        } = self;
        // The command holds the pipe's write ends: the pipe ends only once
        // they are closed here and in the process.
        drop(command);
        let Some(mut told) = told else {
            return without.map_or(Scoped::In, Scoped::Without);
        };
        let mut said = String::new();
        if let Err(e) = told.read_to_string(&mut said) {
            return Scoped::Failed(format!("reading what systemd-run said: {e}"));
        }
        match said.lines().any(|l| l == STARTED) {
            true => Scoped::In,
            false if said.trim().is_empty() => {
                Scoped::Failed("systemd-run ended saying nothing".into())
            }
            false => Scoped::Failed(said.trim().to_string()),
        }
    }
}

/// Kill whatever is still in the scope `unit` (without `.scope`), which
/// then ends: what a command started that left its process group and
/// cleared its environment, and so escaped the limb's own ending of it.
/// Nothing, quietly, where there is no such scope.
pub fn kill(unit: &str) {
    if !user_manager() {
        return;
    }
    let _ = Command::new("systemctl")
        .args(["--user", "kill", "--signal=SIGKILL", "--quiet"])
        .arg(format!("{unit}.scope"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}
