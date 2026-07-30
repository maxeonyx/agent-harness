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
use std::future::Future;
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
///
/// Process ownership is structural: every child the limb spawns lives
/// inside an `execute` call that this loop awaits inline, so the loop can
/// only reach the channel-closed exit with no live child. The graceful
/// shutdown chain is: brain drains in-flight work (cancel → the limb
/// kills and reaps its process tree → outcome) *before* dropping its
/// sender, so limb shutdown never has anything left to clean up. Cleanup
/// is always an explicit async drain, never a Drop side effect.
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
                let path = args["path"].as_str().unwrap_or(".").to_string();
                let dir = self.root.join(&path);
                cancellable(cancel, "list_files", async move {
                    let mut names = Vec::new();
                    match tokio::fs::read_dir(dir).await {
                        Ok(mut entries) => {
                            while let Ok(Some(entry)) = entries.next_entry().await {
                                names.push(entry.file_name().to_string_lossy().into_owned());
                            }
                            names.sort();
                            Outcome::Ok {
                                value: names.join("\n"),
                            }
                        }
                        Err(e) => Outcome::Err {
                            error: format!("error listing {path}: {e}"),
                        },
                    }
                })
                .await
            }
            "read_file" => {
                let Some(path) = args["path"].as_str() else {
                    return Outcome::Err {
                        error: "missing required argument 'path'".to_string(),
                    };
                };
                let path = path.to_string();
                let file = self.root.join(&path);
                cancellable(cancel, "read_file", async move {
                    match tokio::fs::read_to_string(file).await {
                        Ok(content) => Outcome::Ok { value: content },
                        Err(e) => Outcome::Err {
                            error: format!("error reading {path}: {e}"),
                        },
                    }
                })
                .await
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
        // The child gets its own process group (its pgid == its pid), so
        // the limb owns a whole tree it can address exactly: killing the
        // group reaches every descendant the shell forked, and nothing
        // else. No global process-table inspection, ever.
        let mut command_builder = std::process::Command::new("bash");
        command_builder
            .arg("-c")
            .arg(command)
            .current_dir(&self.root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        std::os::unix::process::CommandExt::process_group(&mut command_builder, 0);
        let mut child = match tokio::process::Command::from(command_builder)
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
                    let (stdout, stderr) = match readers.await {
                        Ok(output) => output,
                        Err(e) => {
                            return Outcome::Err {
                                error: format!("error reading command output: {e}"),
                            };
                        }
                    };
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
                // Drain: kill the whole process group we created (the
                // kill(2) syscall does not block), then reap the shell
                // asynchronously before resolving. Grandchildren were
                // ours to kill via the group; they reparent to init for
                // reaping.
                if let Some(pid) = child.id() {
                    // SAFETY: plain syscall; pid is our own live child's
                    // group id.
                    unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
                }
                let _ = child.kill().await;
                let _ = child.wait().await;
                readers.abort();
                Outcome::Cancelled {
                    reason: "cancelled by user; child process tree killed".to_string(),
                }
            }
        }
    }
}

/// Race a tool body against its cancellation token: the token winning
/// resolves to a definite `Cancelled` outcome. The abandoned body holds no
/// process or lock — filesystem tools may at worst leave an I/O operation
/// to finish in the background (a read blocked on a FIFO lingers on the
/// blocking pool until its writer appears; a documented spike limitation).
async fn cancellable(
    cancel: CancellationToken,
    name: &str,
    body: impl Future<Output = Outcome<String>> + Send + 'static,
) -> Outcome<String> {
    let task = tokio::spawn(body);
    tokio::select! {
        result = task => result.unwrap_or_else(|e| Outcome::Panicked {
            payload: format!("tool task failed: {e}"),
        }),
        _ = cancel.cancelled() => Outcome::Cancelled {
            reason: format!("{name} cancelled"),
        },
    }
}
