//! The tools the workspace offers, as a context's root declares them, and
//! the instructions that come with them.

use std::path::Path;

use harness_core::limb::{Audience, Branch, Tool};
use serde_json::json;

/// Where an agent works, rendered into every root of an agent there: a
/// working directory's own instructions; for a clone made for its agent,
/// that it is its own.
pub fn instructions(root: &Path, branch: Option<&Branch>) -> String {
    let own = match branch {
        Some(Branch { from, name, base }) => format!(
            " This directory is your own clone of `{from}`, on the branch `{name}`, made from `{base}`. \
Nobody else works in it. Commit your work here and push it: `git push -u origin {name}`."
        ),
        None => String::new(),
    };
    format!(
        "You work in the directory `{}` through the tools `bash`, `read`, `write`, `edit` and `grep`. \
Relative paths are relative to it.{own}",
        root.display()
    )
}

/// How to use the workspace tools, wherever an agent works.
pub const USE: &str = "\
Your calls to this workspace run one after another, in the order you write them, each once the one before has ended. \
Every `bash` call starts a fresh shell there: no working directory, variables or background jobs carry over. \
Each command owns every process it starts, and they are all killed when it ends, is interrupted or reaches its time limit. \
A process that starts a session of its own leaves the command: `setsid server > server.log 2>&1 &` keeps a server across calls. \
Stop it with `kill <pid>`; it is stopped anyway when the limb ends.

Long `bash` and `grep` output is cut to a view: its first and last 30 lines, with markers naming the lines each gap hides, and its longest lines cut to fit about 16 KiB; binary output is not shown. \
When anything is cut, the view ends by naming a file that holds the whole output: `read` or `grep` that file rather than running the command again. \
`bash` takes an optional `grep` pattern: lines matching it in the middle are shown too, with a little context, and a cut line keeps the part around its match; name what you look for (errors, a test's name) when the output may be long. \
`read` returns whole numbered lines, as many as you ask for up to 64 KiB, and says where to read on; it shows an image as an image.

This directory is shared: forks of your context and other agents may work in it too. \
When a file you have read changes, you are told at your next turn. \
A `write` or `edit` is refused, with no effect, if the file has changed since you last saw it through `read`, `write` or `edit`, or exists and you have never seen it so. \
The refusal shows the file as it is now, as `read` would, and that counts as reading it: check your change still applies, then retry. \
Your own calls never cause a refusal: the earlier calls of the same response, and files your commands changed. Seeing a file in a command's output does not count as seeing it. \
Changes made by commands are not checked in advance. \
To change several places in a file, give one `edit` several replacements rather than scripting it in `bash`: they land together or not at all, and the file stays checked.

`AGENTS.md` files below this directory apply to the work inside their own directory: read one before working there.";

/// The workspace's tools.
pub fn tools() -> Vec<Tool> {
    vec![
        tool(
            "bash",
            "Run a bash command in your working directory, with stdin closed and stdout and stderr merged. \
             Ends with the exit code. Long output is cut to its first and last lines plus the lines matching `grep`; \
             the whole of it is kept in a file the result names.",
            json!({
                "command": { "type": "string", "description": "The command, run with `bash -c`." },
                "grep": { "type": "string", "description": "A regular expression for the lines worth seeing from the middle of long output, besides its head and tail: errors, a test's name." },
                "timeout_ms": { "type": "integer", "description": "Stop the command after this long. Default 120000, at most 600000." }
            }),
            &["command"],
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
            "Replace exact text in a file: one or more replacements, applied in order, each to the text the ones before it left, \
             and written all at once or not at all. Each `old` must occur exactly once (or at least once with `all`); \
             if one does not, nothing is written, and the result names it and shows the file. \
             Refused if the file has changed since you last saw it.",
            json!({
                "path": { "type": "string" },
                "edits": {
                    "type": "array",
                    "description": "The replacements, in order. One is `[{\"old\": ..., \"new\": ...}]`.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "old": { "type": "string", "description": "The exact text to replace, including whitespace." },
                            "new": { "type": "string" },
                            "all": { "type": "boolean", "description": "Replace every occurrence." }
                        },
                        "required": ["old", "new"]
                    }
                }
            }),
            &["path", "edits"],
        ),
        tool(
            "grep",
            "Search a file, or the text files under a directory, for a regular expression, skipping `.git`. \
             Prints `path:line:text`; a long line shows the part around its match. \
             Many matches are cut like `bash` output, the whole list kept in a file the result names.",
            json!({
                "pattern": { "type": "string" },
                "path": { "type": "string", "description": "Where to search. Default: your working directory." }
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
