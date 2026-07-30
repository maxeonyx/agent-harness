//! Toy limb: its own loop, owning a particular environment (here: a
//! working directory) including the context it provides (tool schemas).
//! A session has a limb at the logical level, not at the memory-ownership
//! level: the brain holds a channel to it, never the limb itself. No
//! provider access, no agent loop. Execution is async and cancel-correct
//! in the request → drain → finalize sense: the brain requests
//! cancellation via a token; the limb drains (kills and reaps a running
//! child process) and resolves to a `Cancelled` outcome — never an
//! abandoned child, never a missing outcome.

use crate::events::Outcome;
use serde_json::{Value, json};
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

/// What a brain can ask of a limb. Every request carries a reply channel:
/// the limb always answers (dropping the reply is visible to the asker).
pub enum LimbRequest {
    /// The contributions this limb provides to the model's environment:
    /// (name, tool schema) pairs.
    Describe {
        reply: oneshot::Sender<Vec<(String, Value)>>,
    },
    Execute {
        name: String,
        arguments: String,
        cancel: CancellationToken,
        reply: oneshot::Sender<Outcome<String>>,
    },
}

/// The limb loop: executes requests sequentially in its own environment.
/// Ends when all request senders are dropped.
pub async fn run(limb: Limb, mut rx: mpsc::Receiver<LimbRequest>) {
    while let Some(request) = rx.recv().await {
        match request {
            LimbRequest::Describe { reply } => {
                let _ = reply.send(limb.contributions());
            }
            LimbRequest::Execute {
                name,
                arguments,
                cancel,
                reply,
            } => {
                let outcome = limb.execute(&name, &arguments, cancel).await;
                let _ = reply.send(outcome);
            }
        }
    }
}

pub struct Limb {
    root: PathBuf,
}

impl Limb {
    pub fn new() -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Limb { root }
    }

    /// The limb's environment contributions: named tool schemas.
    fn contributions(&self) -> Vec<(String, Value)> {
        self.tool_defs()
            .into_iter()
            .map(|def| {
                let name = def["function"]["name"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();
                (name, def)
            })
            .collect()
    }

    fn tool_defs(&self) -> Vec<Value> {
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

    /// Execute a tool call. Resolves to a definite outcome: `Ok` with the
    /// tool output, `Err` for tool-level failures, or `Cancelled` if the
    /// token fires first (after draining any child process).
    pub async fn execute(
        &self,
        name: &str,
        arguments_json: &str,
        cancel: CancellationToken,
    ) -> Outcome<String> {
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
                        Outcome::Ok {
                            value: names.join("\n"),
                        }
                    }
                    Err(e) => Outcome::Err {
                        error: format!("error listing {path}: {e}"),
                    },
                }
            }
            "read_file" => {
                let Some(path) = args["path"].as_str() else {
                    return Outcome::Err {
                        error: "missing required argument 'path'".to_string(),
                    };
                };
                match tokio::fs::read_to_string(self.root.join(path)).await {
                    Ok(content) => Outcome::Ok { value: content },
                    Err(e) => Outcome::Err {
                        error: format!("error reading {path}: {e}"),
                    },
                }
            }
            "bash" => {
                let Some(command) = args["command"].as_str() else {
                    return Outcome::Err {
                        error: "missing required argument 'command'".to_string(),
                    };
                };
                self.run_bash(command, cancel).await
            }
            other => Outcome::Err {
                error: format!("unknown tool '{other}'"),
            },
        }
    }

    async fn run_bash(&self, command: &str, cancel: CancellationToken) -> Outcome<String> {
        let mut child = match tokio::process::Command::new("bash")
            .arg("-c")
            .arg(command)
            .current_dir(&self.root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                return Outcome::Err {
                    error: format!("error running command: {e}"),
                };
            }
        };

        // Read both pipes truly concurrently: a child that fills one pipe
        // while we drain only the other would deadlock (it cannot exit
        // while blocked writing, and we would never see EOF).
        let mut stdout_pipe = child.stdout.take();
        let mut stderr_pipe = child.stderr.take();
        let readers = tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let read_stdout = async {
                if let Some(pipe) = stdout_pipe.as_mut() {
                    let _ = pipe.read_to_end(&mut stdout).await;
                }
            };
            let read_stderr = async {
                if let Some(pipe) = stderr_pipe.as_mut() {
                    let _ = pipe.read_to_end(&mut stderr).await;
                }
            };
            tokio::join!(read_stdout, read_stderr);
            (stdout, stderr)
        });

        tokio::select! {
            result = child.wait() => match result {
                Ok(status) => {
                    let (stdout, stderr) = readers.await.unwrap_or_default();
                    let mut text = String::new();
                    text.push_str(&String::from_utf8_lossy(&stdout));
                    let stderr = String::from_utf8_lossy(&stderr);
                    if !stderr.is_empty() {
                        text.push_str("\n[stderr]\n");
                        text.push_str(&stderr);
                    }
                    if !status.success() {
                        text.push_str(&format!("\n[exit status: {status}]"));
                    }
                    Outcome::Ok { value: text }
                }
                Err(e) => Outcome::Err {
                    error: format!("error waiting for command: {e}"),
                },
            },
            _ = cancel.cancelled() => {
                // Drain: kill the child and reap it before resolving.
                let _ = child.kill().await;
                let _ = child.wait().await;
                readers.abort();
                Outcome::Cancelled {
                    reason: "cancelled by user; child process killed".to_string(),
                }
            }
        }
    }
}
