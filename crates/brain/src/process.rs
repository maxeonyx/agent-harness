//! A plugin limb the brain started: `harness-limb-<kind>`, found beside the
//! brain's own executable, spoken to in JSON lines over its stdin and
//! stdout. The brain knows nothing of a limb but its definition and this
//! protocol ([`ToLimb`], [`FromLimb`]).
//!
//! The limb is the leader of a process group of its own, so Ctrl-C in the
//! brain's terminal reaches only the brain, which shuts its limbs down in
//! order ([`ToLimb::Shutdown`]). Everything the limb starts carries its
//! mark, so killing it ([`Process::kill`]) ends the whole tree, whatever
//! left its group.

use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::process::{ChildStdin, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use harness_core::environment::{FromLimb, Limb, LimbKind, ToLimb};
use harness_runtime::{Cancel, Ended, Group};

/// A limb's process tree, and the line to it.
pub struct Process {
    group: Group,
    stdin: ChildStdin,
}

impl Process {
    /// Start the limb `definition` defines, keeping its record of calls in
    /// `state`. Each thing it says goes to `said`, then `None` once it has
    /// stopped talking.
    pub fn start(
        definition: &Limb,
        state: &Path,
        said: impl Fn(Option<FromLimb>) + Send + 'static,
    ) -> io::Result<Process> {
        let LimbKind::Plugin { kind, .. } = &definition.kind else {
            return Err(io::Error::other("only a plugin limb is a process"));
        };
        let program = std::env::current_exe()?.with_file_name(format!("harness-limb-{kind}"));
        let mut command = Command::new(&program);
        command
            .arg(state)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        let mut group = Group::spawn(&mut command, &format!("limb-{}", definition.id))
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", program.display())))?;
        let (Some(mut stdin), Some(stdout)) = group.pipes() else {
            unreachable!("both are piped")
        };
        let line = serde_json::to_string(definition).expect("a limb serialises");
        writeln!(stdin, "{line}")?;
        let name = definition.name.clone();
        // Detached: it ends when the last holder of the limb's stdout does.
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                match serde_json::from_str(&line) {
                    Ok(message) => said(Some(message)),
                    Err(e) => {
                        eprintln!("limb {name} said something unreadable ({e}): {line}");
                        break;
                    }
                }
            }
            said(None);
        });
        Ok(Process { group, stdin })
    }

    /// Say one thing to the limb. If it has gone, its reader reports that.
    pub fn send(&self, message: &ToLimb) {
        let line = serde_json::to_string(message).expect("a message serialises");
        let _ = writeln!(&self.stdin, "{line}");
    }

    /// Ask the limb to stop every call and exit, killing what has not
    /// stopped `within` from now.
    pub fn shutdown(&self, within: Duration) {
        let within = u64::try_from(within.as_millis()).unwrap_or(u64::MAX);
        self.send(&ToLimb::Shutdown { within });
    }

    /// Kill the limb and everything it started, now: its owner's hard kill,
    /// past the deadline it was given.
    pub fn kill(&mut self) {
        let cancel = Cancel::new();
        let Ok(mut waiter) = cancel.waiter() else {
            return;
        };
        match self.group.end(&mut waiter, Instant::now(), None, || None) {
            Ok(Ended::Killed { survivors }) if !survivors.is_empty() => {
                eprintln!("limb processes {survivors:?} survived SIGKILL");
            }
            Ok(_) => {}
            Err(e) => eprintln!("killing a limb: {e}"),
        }
    }
}

/// A limb that is let go is killed with whatever it left: by now it has
/// exited, or it is past its deadline.
impl Drop for Process {
    fn drop(&mut self) {
        self.kill();
    }
}
