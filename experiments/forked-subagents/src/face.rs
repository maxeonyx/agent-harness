//! What you watch while the tree runs: one append-only line per thing that
//! happened, prefixed with the agent's path in the tree.

use std::io::Write;
use std::sync::Mutex;

pub struct Face {
    out: Mutex<std::io::Stdout>,
    verbose: bool,
}

impl Face {
    pub fn new(verbose: bool) -> Face {
        Face {
            out: Mutex::new(std::io::stdout()),
            verbose,
        }
    }

    /// An event about one agent. Suppressed when the face is quiet (the
    /// benchmark runs hundreds of these and reports per trial instead).
    pub fn line(&self, path: &str, text: &str) {
        if !self.verbose {
            return;
        }
        self.say(&format!("{:<28} {text}", path));
    }

    /// Something the user asked for, or must see: a tree snapshot, a fault,
    /// a benchmark trial. Never suppressed.
    pub fn say(&self, text: &str) {
        let mut out = self.out.lock().unwrap();
        let _ = writeln!(out, "{text}");
        let _ = out.flush();
    }
}
