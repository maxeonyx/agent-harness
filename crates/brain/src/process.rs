//! A plugin limb the brain started: `harness-limb-<kind>`, found beside the
//! brain's own executable, spoken to in JSON lines over its stdin and
//! stdout. The brain knows nothing of a limb but its definition and this
//! protocol ([`ToLimb`], [`FromLimb`]).

use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::thread::{self, JoinHandle};

use harness_core::environment::{FromLimb, Limb, LimbKind, ToLimb};

/// A limb's process, and the thread that reads what it says.
pub struct Process {
    child: Child,
    stdin: ChildStdin,
    reader: Option<JoinHandle<()>>,
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
        let mut child = Command::new(&program)
            .arg(state)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", program.display())))?;
        let mut stdin = child.stdin.take().expect("a piped stdin");
        let stdout = child.stdout.take().expect("a piped stdout");
        let line = serde_json::to_string(definition).expect("a limb serialises");
        writeln!(stdin, "{line}")?;
        let name = definition.name.clone();
        let reader = thread::spawn(move || {
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
        Ok(Process {
            child,
            stdin,
            reader: Some(reader),
        })
    }

    /// Say one thing to the limb. If it has gone, its reader reports that.
    pub fn send(&self, message: &ToLimb) {
        let line = serde_json::to_string(message).expect("a message serialises");
        let _ = writeln!(&self.stdin, "{line}");
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
