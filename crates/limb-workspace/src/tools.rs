//! The tools the workspace offers, as a context's root declares them, and
//! the instructions that come with them.

use std::path::Path;

use harness_core::environment::{Audience, Tool};

use crate::Clone;
use serde_json::json;

/// How to use the workspace in `root`, rendered into every root context
/// that reaches it; for a clone made for its agent, that it is its own.
pub fn instructions(root: &Path, clone: Option<&Clone>) -> String {
    let own = match clone {
        Some(Clone { from, branch, base }) => format!(
            "\n\nThis directory is your own clone of `{from}`, on the branch `{branch}`, made from `{base}`. \
Nobody else works in it. Commit your work here and push it: `git push -u origin {branch}`."
        ),
        None => String::new(),
    };
    format!(
        "You work in the directory `{}` through the tools `bash`, `read`, `write`, `edit` and `grep`. \
Relative paths are relative to it.{own}\n\n{USE}",
        root.display()
    )
}

const USE: &str = "\
The calls of one response run one after another, in the order you write them. \
Every `bash` call starts a fresh shell there: no working directory, variables or background jobs carry over. \
Each command owns every process it starts, and they are all killed when it ends, is interrupted or reaches its time limit. \
A process that starts a session of its own leaves the command: `setsid server > server.log 2>&1 &` keeps a server across calls. \
Stop it with `kill <pid>`; it is stopped anyway when the workspace closes.

Long `bash` and `grep` output is cut to its first and last 30 lines, and lines over 200 characters to their two ends. \
`bash` requires a `grep` pattern: lines matching it in the middle are shown too, with a little context, in addition to the head and tail, so name what you are looking for (errors, a test's name). \
An empty pattern adds nothing. Markers say how many lines and matches each gap hides. \
`read` returns whole numbered lines, as many as you ask for up to 64 KiB, and says where to read on; it shows an image as an image.

This directory is shared: forks of your context and other agents may work in it too. \
When a file you have read changes, you are told at your next turn. \
A `write` or `edit` is refused, with no effect, if the file has changed since you last saw it (or exists and you have never seen it): read it and try again. \
Changes made by commands are not checked in advance.

`AGENTS.md` files below this directory apply to the work inside their own directory: read one before working there.";

/// The workspace's tools.
pub fn tools() -> Vec<Tool> {
    vec![
        tool(
            "bash",
            "Run a bash command in the workspace directory, with stdin closed and stdout and stderr merged. \
             Ends with the exit code. Output is cut to its first and last lines plus the lines matching `grep`.",
            json!({
                "command": { "type": "string", "description": "The command, run with `bash -c`." },
                "grep": { "type": "string", "description": "A regular expression for the lines worth seeing from the middle of long output, in addition to the head and tail. Empty for none." },
                "timeout_ms": { "type": "integer", "description": "Stop the command after this long. Default 120000, at most 600000." }
            }),
            &["command", "grep"],
        ),
        tool(
            "read",
            "Read a file as whole numbered lines, or list a directory. Returns the lines asked for, \
             stopping before the line that would pass 64 KiB and saying the offset to read on from. \
             A PNG, JPEG, GIF or WebP image is shown as an image. To find lines in a file, `grep` it.",
            json!({
                "path": { "type": "string" },
                "offset": { "type": "integer", "description": "The first line to read, from 1." },
                "limit": { "type": "integer", "description": "How many lines to read. Default: to the end, within the budget." }
            }),
            &["path"],
        ),
        tool(
            "write",
            "Write a whole file, creating it and its directories if needed. Refused if the file has changed since you last saw it.",
            json!({
                "path": { "type": "string" },
                "content": { "type": "string" }
            }),
            &["path", "content"],
        ),
        tool(
            "edit",
            "Replace exact text in a file. `old` must occur exactly once unless `all` is set. Refused if the file has changed since you last saw it.",
            json!({
                "path": { "type": "string" },
                "old": { "type": "string", "description": "The exact text to replace, including whitespace." },
                "new": { "type": "string" },
                "all": { "type": "boolean", "description": "Replace every occurrence." }
            }),
            &["path", "old", "new"],
        ),
        tool(
            "grep",
            "Search a file, or the text files under a directory, for a regular expression, skipping `.git`. Prints `path:line:text`.",
            json!({
                "pattern": { "type": "string" },
                "path": { "type": "string", "description": "Where to search. Default: the workspace directory." }
            }),
            &["pattern"],
        ),
    ]
}

fn tool(name: &str, description: &str, properties: serde_json::Value, required: &[&str]) -> Tool {
    let definition = json!({
        "name": name,
        "description": description,
        "input_schema": { "type": "object", "properties": properties, "required": required },
    });
    Tool {
        name: name.into(),
        definition: definition.to_string(),
        audience: Audience::Both,
    }
}

/// Whether running `tool` again, after an instance was cut off while running
/// it, is the same as running it once: a read, a search, or a write of
/// whole content, which completes as a no-op once it has landed. A command
/// or an edit may not be.
pub fn rerun(tool: &str) -> bool {
    matches!(tool, "read" | "grep" | "write" | "skill")
}
