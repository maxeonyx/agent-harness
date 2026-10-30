//! A limb the brain is linked to: `harness-limb attach <limb>`, found
//! beside the brain's own executable, run as a child whose stdin and stdout
//! are the link ([`crate::link`]) and whose stderr is why, if it fails. It
//! starts the limb if it is not running, or with `--existing`, for a limb
//! from before limbs were per machine, only attaches to one that is; the
//! limb lives on its own after that, and the brain knows nothing of it but
//! its definition and its link.
//!
//! The child leads a process group of its own, so Ctrl-C in the brain's
//! terminal reaches only the brain. The brain exiting closes the child's
//! stdin, which ends the link and nothing else.

use std::io::{self, BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use harness_core::id::LimbId;

use crate::link::{self, Conn, Said};

/// One link to a limb, through the `attach` child carrying it.
pub struct Process {
    child: Child,
    /// The last line it printed on stderr.
    said: Arc<Mutex<String>>,
}

impl Process {
    /// Attach to `limb`, starting it unless `existing`, saying what the
    /// link says to `said` as connection `conn`.
    pub fn attach(
        limb: LimbId,
        existing: bool,
        conn: Conn,
        said: impl Fn(Conn, Said) + Send + 'static,
    ) -> io::Result<Process> {
        let program = std::env::current_exe()?.with_file_name("harness-limb");
        let mut child = Command::new(&program)
            .arg("attach")
            .args(existing.then_some("--existing"))
            .arg(limb.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", program.display())))?;
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            unreachable!("all three are piped");
        };
        let last = Arc::new(Mutex::new(String::new()));
        let heard = last.clone();
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if !line.trim().is_empty() {
                    errln!("limb {limb}: {line}");
                    *heard.lock().unwrap_or_else(|e| e.into_inner()) = line.trim().to_string();
                }
            }
        });
        thread::spawn(move || {
            if let Err(e) = link::follow(conn, stdout, stdin, &said) {
                errln!("limb {limb}'s link: {e}");
            }
            said(conn, Said::Left);
        });
        Ok(Process { child, said: last })
    }

    /// The last thing it printed: why, for one that failed.
    pub fn last_words(&self) -> String {
        let said = self.said.lock().unwrap_or_else(|e| e.into_inner());
        match said.as_str() {
            "" => "it ended saying nothing".into(),
            said => said.to_string(),
        }
    }

    /// How it ended, once it has.
    pub fn exited(&mut self) -> Option<ExitStatus> {
        self.child.try_wait().ok().flatten()
    }
}
