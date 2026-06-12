//! Toy limb: owns tool declarations and tool execution in its working
//! directory. No provider access, no credentials, no agent loop.

use serde_json::{Value, json};
use std::path::PathBuf;

pub struct Limb {
    root: PathBuf,
}

impl Limb {
    pub fn new() -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Limb { root }
    }

    pub fn tool_defs(&self) -> Vec<Value> {
        vec![
            json!({
                "type": "function",
                "function": {
                    "name": "list_files",
                    "description": "List files in a directory (default: working directory).",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Directory to list" }
                        }
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "read_file",
                    "description": "Read a text file.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "File to read" }
                        },
                        "required": ["path"]
                    }
                }
            }),
            json!({
                "type": "function",
                "function": {
                    "name": "bash",
                    "description": "Run a bash command and return its output.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "command": { "type": "string", "description": "Command to run" }
                        },
                        "required": ["command"]
                    }
                }
            }),
        ]
    }

    pub fn execute(&self, name: &str, arguments_json: &str) -> String {
        let args: Value = serde_json::from_str(arguments_json).unwrap_or_else(|_| json!({}));
        match name {
            "list_files" => {
                let path = args["path"].as_str().unwrap_or(".");
                match std::fs::read_dir(self.root.join(path)) {
                    Ok(entries) => {
                        let mut names: Vec<String> = entries
                            .filter_map(|e| e.ok())
                            .map(|e| e.file_name().to_string_lossy().into_owned())
                            .collect();
                        names.sort();
                        names.join("\n")
                    }
                    Err(e) => format!("error listing {path}: {e}"),
                }
            }
            "read_file" => {
                let Some(path) = args["path"].as_str() else {
                    return "error: missing required argument 'path'".to_string();
                };
                match std::fs::read_to_string(self.root.join(path)) {
                    Ok(content) => content,
                    Err(e) => format!("error reading {path}: {e}"),
                }
            }
            "bash" => {
                let Some(command) = args["command"].as_str() else {
                    return "error: missing required argument 'command'".to_string();
                };
                match std::process::Command::new("bash")
                    .arg("-c")
                    .arg(command)
                    .current_dir(&self.root)
                    .output()
                {
                    Ok(output) => {
                        let mut result = String::new();
                        result.push_str(&String::from_utf8_lossy(&output.stdout));
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        if !stderr.is_empty() {
                            result.push_str("\n[stderr]\n");
                            result.push_str(&stderr);
                        }
                        if !output.status.success() {
                            result.push_str(&format!("\n[exit status: {}]", output.status));
                        }
                        result
                    }
                    Err(e) => format!("error running command: {e}"),
                }
            }
            other => format!("error: unknown tool '{other}'"),
        }
    }
}
