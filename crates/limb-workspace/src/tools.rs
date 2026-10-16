//! The tools the workspace offers, as a context's root declares them, and
//! the instructions that come with them.

use harness_core::context::Wire;
use harness_core::environment::{Audience, Tool};
use serde_json::json;

/// How to use the workspace, rendered into every root that reaches it.
pub const INSTRUCTIONS: &str = "\
You work in a directory on a machine through the tools `bash`, `read`, `write`, `edit` and `grep`. \
Relative paths are relative to the directory.

Every `bash` call starts a fresh shell there: no working directory, variables or background jobs carry over. \
Each command owns every process it starts, and they are all killed when it ends, is interrupted or reaches its time limit. \
A server you need across calls must leave the command's process group (`setsid`).

Long output is cut to its first and last lines. `bash` requires a `grep` pattern: lines matching it in the middle are shown too, with a little context, in addition to the head and tail, so name what you are looking for (errors, a test's name). \
An empty pattern adds nothing. Markers say how many lines and matches each gap hides. \
`read` numbers its lines; read a range with `offset` and `limit` to see a cut part.

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
            false,
        ),
        tool(
            "read",
            "Read a file, with numbered lines, or list a directory.",
            json!({
                "path": { "type": "string" },
                "offset": { "type": "integer", "description": "The first line to read, from 1." },
                "limit": { "type": "integer", "description": "How many lines to read." },
                "grep": { "type": "string", "description": "A regular expression for lines worth seeing from the middle of a long file." }
            }),
            &["path"],
            true,
        ),
        tool(
            "write",
            "Write a whole file, creating it and its directories if needed. Refused if the file has changed since you last saw it.",
            json!({
                "path": { "type": "string" },
                "content": { "type": "string" }
            }),
            &["path", "content"],
            true,
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
            false,
        ),
        tool(
            "grep",
            "Search the text files under a directory for a regular expression, skipping `.git`. Prints `path:line:text`.",
            json!({
                "pattern": { "type": "string" },
                "path": { "type": "string", "description": "Where to search. Default: the workspace directory." }
            }),
            &["pattern"],
            true,
        ),
    ]
}

fn tool(
    name: &str,
    description: &str,
    properties: serde_json::Value,
    required: &[&str],
    rerun: bool,
) -> Tool {
    let definition = json!({
        "name": name,
        "description": description,
        "input_schema": { "type": "object", "properties": properties, "required": required },
    });
    Tool {
        name: name.into(),
        definition: Wire(definition.to_string()),
        audience: Audience::Both,
        rerun,
        deferred: false,
    }
}
