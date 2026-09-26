//! What you watch while the tree runs: append-only, prefixed with the agent's
//! path in the tree.
//!
//! Two kinds of line. An event is what the harness did: a request sent, a
//! scope opened. A message is text entering an agent's context, and it is
//! shown exactly and whole: a bracketed header with its place in that
//! agent's context, then its text, every line behind a `│` so where it starts
//! and ends is never in doubt. A message is written in one go, so two agents
//! running at once cannot interleave halfway through one.

use crate::wire::Message;
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
        if self.verbose {
            self.say(&format!("{path:<28} {text}"));
        }
    }

    /// Message `n` of an agent's context. `tool` names the tool a tool
    /// result answers, which the message itself only knows by call id.
    pub fn message(&self, path: &str, n: usize, message: &Message, tool: Option<&str>) {
        if !self.verbose {
            return;
        }
        let role = &message.role;
        let mut parts = Vec::new();
        let text = message.content.as_deref().unwrap_or("");
        let calls = message.tool_calls.as_deref().unwrap_or_default();
        // Reasoning the model did before answering, which it reads back on
        // its next request. A provider may keep the words to itself.
        if let Some(reasoning) = message
            .reasoning_details
            .as_ref()
            .and_then(|value| value.as_array())
            .filter(|blocks| !blocks.is_empty())
        {
            let words: Vec<&str> = reasoning
                .iter()
                .filter_map(|b| b["thinking"].as_str().or(b["text"].as_str()))
                .filter(|words| !words.is_empty())
                .collect();
            parts.push(if words.is_empty() {
                format!("{path:<28} [{n} {role} thinking] (the provider did not show it)")
            } else {
                block(path, n, &format!("{role} thinking"), &words.join("\n"))
            });
        }
        if !text.is_empty() || calls.is_empty() {
            let header = match (&message.tool_call_id, tool) {
                (Some(id), Some(tool)) => format!("{role} {tool} {id}"),
                (Some(id), None) => format!("{role} {id}"),
                (None, _) => role.to_string(),
            };
            parts.push(block(path, n, &header, text));
        }
        for call in calls {
            let header = format!("{role} tool_use {} {}", call.function.name, call.id);
            parts.push(block(path, n, &header, &call.function.arguments));
        }
        self.say(&parts.join("\n"));
    }

    /// Something the user asked for, or must see: a tree snapshot, a fault, a
    /// benchmark trial. Never suppressed.
    pub fn say(&self, text: &str) {
        let mut out = self.out.lock().unwrap();
        let _ = writeln!(out, "{text}");
        let _ = out.flush();
    }
}

fn block(path: &str, n: usize, header: &str, text: &str) -> String {
    let mut block = format!("{path:<28} [{n} {header}]");
    if text.is_empty() {
        block.push_str(" (empty)");
    }
    for line in text.lines() {
        block.push_str("\n    │");
        if !line.is_empty() {
            block.push(' ');
            block.push_str(line);
        }
    }
    block
}
