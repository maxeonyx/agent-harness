//! What you watch while the tree runs: append-only, prefixed with the agent's
//! path in the tree.
//!
//! Anything an agent said, and anything a tool said back, is part of that.
//! Watching a tree that only reports `request returned` tells you it is
//! moving, not what it is doing — and in `chat` the root's reply is the whole
//! point of having asked.
//!
//! A block of text is indented under its path rather than repeating the path
//! on every line, and is written in one go, so two agents running at the same
//! time cannot interleave halfway through one.

use std::io::Write;
use std::sync::Mutex;

/// How much of a tool's answer to show before summarising the rest. Enough to
/// see what came back.
const RESULT_LINES: usize = 5;

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
        if self.verbose {
            self.say(&format!("{:<28} {text}", path));
        }
    }

    /// Something an agent wrote, in full, under the agent that wrote it.
    pub fn block(&self, path: &str, label: &str, text: &str) {
        if self.verbose {
            self.say(&render(path, label, text.trim_end()));
        }
    }

    /// What a tool answered: the first few lines, and how many were left. An
    /// error is never abbreviated — it is usually the whole explanation, and
    /// the one Max needed was the line saying a path lay outside the run
    /// directory.
    pub fn result(&self, path: &str, label: &str, text: &str) {
        if self.verbose {
            self.say(&render(path, label, &abbreviate(text.trim_end())));
        }
    }

    /// Something the user asked for, or must see: a tree snapshot, a fault, a
    /// benchmark trial. Never suppressed.
    pub fn say(&self, text: &str) {
        let mut out = self.out.lock().unwrap();
        let _ = writeln!(out, "{text}");
        let _ = out.flush();
    }
}

fn render(path: &str, label: &str, text: &str) -> String {
    let mut block = format!("{path:<28} {label}");
    for line in text.lines() {
        block.push_str("\n    ");
        block.push_str(line);
    }
    block
}

fn abbreviate(text: &str) -> String {
    if text.starts_with("Error:") {
        return text.to_string();
    }
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= RESULT_LINES + 1 {
        return text.to_string();
    }
    let rest = lines.len() - RESULT_LINES;
    format!("{}\n… {rest} more lines", lines[..RESULT_LINES].join("\n"))
}
