//! Toy limb: a participant shaped like the face and brain — an inbox, a
//! select loop, and owned in-flight work — owning one external world: an
//! environment (here: a working directory, its filesystem, and the
//! processes tools spawn). No provider access, no agent loop.
//!
//! Both brain and limb record facts about tools, split by ownership: the
//! limb is in charge of the actual execution (or not) of tool calls, so
//! it appends the execution facts — `ToolStarted`, synchronously, to the
//! shared co-located session state — while the brain records the context
//! facts (a call detected in a response, a result entering the model
//! view). A call the limb never started is exactly a call with no
//! execution facts: unexecuted, omitted from the wire, resumable.
//!
//! Cancellation is a message, not a shared token: the brain sends
//! `LimbMsg::Cancel`; the limb cancels its own in-flight execution (the
//! token below never leaves this module), drains it — kills and reaps its
//! process tree — and reports the `Cancelled` outcome back. Never an
//! abandoned child, never a missing outcome.

use crate::protocol::{BrainMsg, Contribution, DisplayItem, LimbMsg, Outcome};
use crate::state::SessionState;
use serde_json::{Value, json};
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// The limb loop: select over the inbox and the owned in-flight
/// execution. Ends when the inbox closes (the brain drops its sender
/// after draining), after draining any execution still in flight.
///
/// Process ownership is structural: every child the limb spawns belongs
/// to the in-flight execution this loop owns (identity + cancellation
/// token + join handle together, always joined), and every exit path
/// drains it. The graceful shutdown chain is: brain sends Cancel and
/// waits for the outcome *before* dropping its sender — so the
/// closed-inbox exit normally has nothing left to clean up, and if the
/// brain vanished mid-execution the drain below still kills and reaps
/// the process tree. Cleanup is always an explicit async drain, never a
/// Drop side effect.
pub async fn run(
    limb: Limb,
    mut rx: mpsc::Receiver<LimbMsg>,
    brain_tx: mpsc::Sender<BrainMsg>,
    display_tx: mpsc::UnboundedSender<DisplayItem>,
    state: Arc<Mutex<SessionState>>,
) -> Result<(), String> {
    let mut in_flight: Option<(
        crate::provider::ToolCall,
        CancellationToken,
        tokio::task::JoinHandle<Outcome<String>>,
    )> = None;
    loop {
        // A closed inbox with work still in flight means the brain is gone
        // without draining us: cancel our own work (idempotent) so the
        // in-flight branch can join it and we can exit.
        if rx.is_closed()
            && let Some((_, cancel, _)) = &in_flight
        {
            cancel.cancel();
        }
        tokio::select! {
            // Biased, completion first: a finished execution is a real
            // result and gets recorded (ties must also resolve
            // deterministically — flakes are bugs). A cancel that ties
            // with completion did not prevent anything: the brain still
            // finalizes the turn cancelled and starts no new work.
            biased;
            outcome = join_execution(&mut in_flight), if in_flight.is_some() => {
                let (call_id, outcome) = outcome;
                in_flight = None;
                brain_tx.send(BrainMsg::ToolOutcome { call_id, outcome }).await
                    .map_err(|_| "brain inbox closed before the limb could deliver a tool outcome".to_string())?;
                if rx.is_closed() {
                    break;
                }
            }
            message = rx.recv(), if in_flight.is_none() || !rx.is_closed() => match message {
                Some(LimbMsg::Execute { call }) if in_flight.is_none() => {
                    with_state(&state, |state| state.append_tool_started(call.clone()))?;
                    let cancel = CancellationToken::new();
                    let name = call.function.name.clone();
                    let arguments = call.function.arguments.clone();
                    let root_limb = limb.clone();
                    let token = cancel.clone();
                    let _ = display_tx.send(DisplayItem::ToolStarted {
                        name: name.clone(),
                        arguments: arguments.clone(),
                    });
                    let task = tokio::spawn(async move {
                        root_limb.execute(&name, &arguments, token).await
                    });
                    in_flight = Some((call, cancel, task));
                }
                Some(LimbMsg::Execute { .. }) => {
                    return Err("limb received a second tool call while one was already in flight".to_string());
                }
                Some(LimbMsg::Cancel) => {
                    if let Some((_, cancel, _)) = &in_flight {
                        cancel.cancel();
                    }
                }
                None if in_flight.is_none() => break,
                None => {
                    // Inbox closed mid-execution (the brain died without
                    // draining us): drain our own work — cancel it and let
                    // the in-flight branch above join it and report. The
                    // select guard keeps this arm from spinning on the
                    // already-closed channel meanwhile.
                    if let Some((_, cancel, _)) = &in_flight {
                        cancel.cancel();
                    }
                }
            },
        }
    }
    Ok(())
}

fn with_state<T>(
    state: &Arc<Mutex<SessionState>>,
    operation: impl FnOnce(&mut SessionState) -> Result<T, String>,
) -> Result<T, String> {
    let mut state = state
        .lock()
        .map_err(|_| "session state lock poisoned in limb".to_string())?;
    operation(&mut state)
}

async fn join_execution(
    in_flight: &mut Option<(
        crate::provider::ToolCall,
        CancellationToken,
        tokio::task::JoinHandle<Outcome<String>>,
    )>,
) -> (String, Outcome<String>) {
    let (call, _, task) = in_flight.as_mut().expect("guarded by select condition");
    let outcome = match task.await {
        Ok(outcome) => outcome,
        Err(error) if error.is_panic() => Outcome::Panicked {
            payload: error.to_string(),
        },
        Err(_) => Outcome::Cancelled {
            reason: "tool execution task aborted".to_string(),
        },
    };
    (call.id.clone(), outcome)
}

#[derive(Clone)]
pub struct Limb {
    root: PathBuf,
}

impl Limb {
    pub fn new() -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Limb { root }
    }

    /// The limb's environment contributions: named tool schemas, plus
    /// facts about the machine this environment lives on.
    pub fn contributions(&self) -> Vec<(String, Contribution)> {
        let mut contributions: Vec<(String, Contribution)> = self
            .tool_defs()
            .into_iter()
            .map(|def| {
                let name = def["function"]["name"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();
                (name, Contribution::Tool { def })
            })
            .collect();
        contributions.push((
            "hostname".to_string(),
            Contribution::Fact { text: hostname() },
        ));
        contributions
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
        // The group id, captured while the shell is provably alive (its
        // pid is None after a successful wait). The group's lifetime is
        // the *operation's* lifetime: whichever way the call resolves —
        // completed, failed, or cancelled — the group dies with it, so a
        // tool that backgrounds a child cannot leak it past its own
        // resolution.
        let group = child.id();
        let kill_group = || {
            if let Some(pid) = group {
                // SAFETY: plain non-blocking syscall on the group we
                // created for this call. If every member is already dead
                // it is a harmless ESRCH.
                unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
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
            // Biased: a child that has already exited is a real result;
            // record it rather than letting a tying cancel discard it.
            biased;
            result = child.wait() => {
                // The shell resolved the operation; end the whole group
                // *before* draining the pipes, so a backgrounded
                // descendant can neither outlive the call nor hold the
                // pipes open indefinitely.
                kill_group();
                match result {
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
                Err(e) => {
                    // Even this failure path owns its reader task: the
                    // group kill above closed the pipe writers, so the
                    // readers finish and are joined, never detached.
                    let _ = readers.await;
                    Outcome::Err {
                        error: format!("error waiting for command: {e}"),
                    }
                }
            }},
            _ = cancel.cancelled() => {
                // Drain: kill the group (non-blocking syscall), reap the
                // shell asynchronously, and *join* the reader task — the
                // group kill closed every pipe writer, so the readers hit
                // EOF and finish; nothing is detached. Grandchildren were
                // ours to kill via the group; init reaps them.
                kill_group();
                let _ = child.kill().await;
                let _ = child.wait().await;
                let _ = readers.await;
                Outcome::Cancelled {
                    reason: "cancelled by user; child process tree killed".to_string(),
                }
            }
        }
    }
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| {
            std::fs::read_to_string("/proc/sys/kernel/hostname")
                .ok()
                .map(|value| value.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
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
    let mut task = tokio::spawn(body);
    tokio::select! {
        // Biased: a body that already finished produced a real result.
        biased;
        result = &mut task => result.unwrap_or_else(|e| Outcome::Panicked {
            payload: format!("tool task failed: {e}"),
        }),
        _ = cancel.cancelled() => {
            task.abort();
            let _ = task.await;
            Outcome::Cancelled {
                reason: format!("{name} cancelled"),
            }
        },
    }
}
