//! A plugin limb the brain started: `harness-limb-<kind>`, found beside the
//! brain's own executable, given its state directory, the brain's socket
//! and its definition. It connects back over the socket ([`crate::link`]);
//! the brain knows nothing of a limb but its definition and its link.
//!
//! The limb is the leader of a process group of its own, so Ctrl-C in the
//! brain's terminal reaches only the brain, and a brain that dies leaves
//! its limbs running, to reconnect to the next. Everything the limb starts
//! carries its mark, so killing it ([`Process::kill`]) ends the whole tree,
//! whatever left its group.

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

use harness_core::environment::{Limb, LimbKind};
use harness_runtime::{Cancel, Ended, Group};

/// A limb's process tree.
pub struct Process {
    group: Group,
}

impl Process {
    /// Start the limb `definition` defines, keeping its log in `state`, to
    /// connect to `socket`.
    pub fn start(definition: &Limb, state: &Path, socket: &Path) -> io::Result<Process> {
        let LimbKind::Plugin { kind, .. } = &definition.kind else {
            return Err(io::Error::other("only a plugin limb is a process"));
        };
        let program = std::env::current_exe()?.with_file_name(format!("harness-limb-{kind}"));
        let mut command = Command::new(&program);
        command
            .arg(state)
            .arg(socket)
            .arg(serde_json::to_string(definition).expect("a limb serialises"))
            .stdin(Stdio::null());
        let group = Group::spawn(&mut command, &format!("limb-{}", definition.id))
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", program.display())))?;
        Ok(Process { group })
    }

    /// Whether its process has exited.
    pub fn exited(&mut self) -> bool {
        self.group.exited().is_some()
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
