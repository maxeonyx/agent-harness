//! A plugin limb the brain started: `harness-limb-<kind>`, found beside the
//! brain's own executable, given its state directory, the brain's socket
//! and its definition. It connects back over the socket ([`crate::link`]);
//! the brain knows nothing of a limb but its definition and its link.
//!
//! The limb is the leader of a process group of its own, so Ctrl-C in the
//! brain's terminal reaches only the brain, and a brain that exits or dies
//! leaves its limbs running, to reconnect to the next. Everything the limb
//! starts carries its mark, so the whole tree is found whatever left its
//! group. What it prints goes to `log` in its state directory, not to the
//! brain's terminal, which it outlives.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use harness_core::environment::{Limb, LimbKind};
use harness_runtime::Group;

/// A limb's process tree.
pub struct Process {
    group: Group,
    log: PathBuf,
    /// Where in `log` this process began.
    from: usize,
}

impl Process {
    /// Start the limb `definition` defines, keeping its log in `state`, to
    /// connect to `socket`.
    pub fn start(definition: &Limb, state: &Path, socket: &Path) -> io::Result<Process> {
        let LimbKind::Plugin { kind, .. } = &definition.kind else {
            return Err(io::Error::other("only a plugin limb is a process"));
        };
        let program = std::env::current_exe()?.with_file_name(format!("harness-limb-{kind}"));
        let log = state.join("log");
        let at = |e: io::Error| io::Error::new(e.kind(), format!("{}: {e}", log.display()));
        std::fs::create_dir_all(state).map_err(at)?;
        let out = File::options()
            .create(true)
            .append(true)
            .open(&log)
            .map_err(at)?;
        let from = out.metadata()?.len() as usize;
        let mut command = Command::new(&program);
        command
            .arg(state)
            .arg(socket)
            .arg(serde_json::to_string(definition).expect("a limb serialises"))
            .stdin(Stdio::null())
            .stdout(out.try_clone()?)
            .stderr(out);
        let group = Group::spawn(&mut command, &format!("limb-{}", definition.id))
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", program.display())))?;
        Ok(Process { group, log, from })
    }

    /// The last thing it printed, which for a limb that exited without
    /// connecting is why.
    pub fn last_words(&self) -> String {
        let text = std::fs::read(&self.log).unwrap_or_default();
        let text = String::from_utf8_lossy(text.get(self.from..).unwrap_or_default());
        let last = text.lines().rev().find(|l| !l.trim().is_empty());
        match last {
            Some(line) => line.trim().to_string(),
            None => format!("it exited saying nothing; see {}", self.log.display()),
        }
    }

    /// Whether its process has exited.
    pub fn exited(&mut self) -> bool {
        self.group.exited().is_some()
    }
}
