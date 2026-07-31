//! The face: a participant owning one external world — the TUI. The
//! terminal (stdin/stdout) is conceptually separate from the face process
//! itself: rendering is an output port to that world, not face loop
//! logic, and reading it happens on a dumb line-pump thread. The
//! distinction is what makes synchronous takeover coherent — when /dump
//! hands the tty to an editor, that is the face's *owned in-flight work*
//! (as is the /open file read): the loop keeps selecting the whole time,
//! buffering display items while the editor owns the terminal (flushed
//! when it returns), deferring further input, and still observing
//! session shutdown. The face is never blocked blind on its own world.

use crate::protocol::{BrainCommand, BrainMsg, DisplayItem, Outcome};
use crate::state::{DumpSnapshot, SessionState};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

pub fn print_help(base_url: &str, model: &str) {
    println!("walking-skeleton — provider {base_url} model {model}");
    println!("  <text>        stage a user message (appends, never triggers)");
    println!("  /open <path>  simulate user file-open activity (appends, never triggers)");
    println!("  /end          end the turn (triggers inference)");
    println!("  /cancel       cancel in-flight work (request or tool call)");
    println!("  /rebuild      rebuild the context from the event log");
    println!("  /dump         open the model view (markdown) in $EDITOR, default nano");
    println!("  /quit         exit");
}

enum Input {
    Line(String),
    Eof,
    Failed(String),
}

enum FaceWork {
    Dump(tokio::task::JoinHandle<Result<std::process::ExitStatus, String>>),
    Open {
        path: String,
        task: tokio::task::JoinHandle<Result<OpenedFile, String>>,
    },
}

struct OpenedFile {
    bytes: usize,
    head: String,
}

enum FaceWorkResult {
    Dump(Result<Result<std::process::ExitStatus, String>, tokio::task::JoinError>),
    Open {
        path: String,
        result: Result<Result<OpenedFile, String>, tokio::task::JoinError>,
    },
}

struct LineResult {
    input_finished: bool,
    work: Option<FaceWork>,
}

pub async fn run(
    brain_tx: mpsc::Sender<BrainMsg>,
    mut display_rx: mpsc::UnboundedReceiver<DisplayItem>,
    state: Arc<Mutex<SessionState>>,
) -> Result<(), String> {
    let (input_tx, mut input_rx) = mpsc::channel(8);
    let (resume_tx, resume_rx) = std::sync::mpsc::channel::<()>();
    let input_thread = std::thread::spawn(move || {
        use std::io::BufRead;
        for line in std::io::stdin().lock().lines() {
            match line {
                Ok(line) => {
                    let is_dump = line.trim() == "/dump";
                    let is_quit = line.trim() == "/quit";
                    if input_tx.blocking_send(Input::Line(line)).is_err() {
                        return;
                    }
                    if is_quit {
                        // The session is ending by user request: finish so
                        // the face can join this thread. (Only a shutdown
                        // that does NOT come through stdin leaves this
                        // thread blocked on the tty — the documented
                        // residual; process exit reaps it.)
                        return;
                    }
                    if is_dump && resume_rx.recv().is_err() {
                        return;
                    }
                }
                Err(error) => {
                    let _ = input_tx.blocking_send(Input::Failed(error.to_string()));
                    return;
                }
            }
        }
        let _ = input_tx.blocking_send(Input::Eof);
    });
    let mut input_finished = false;
    let mut deferred = VecDeque::new();
    let mut work = None;
    let mut buffered_display = VecDeque::new();
    let mut session_closed = false;

    loop {
        if work.is_none()
            && let Some(input) = deferred.pop_front()
        {
            apply_input(input, &brain_tx, &state, &mut input_finished, &mut work).await?;
            continue;
        }
        let work_in_flight = work.is_some();
        tokio::select! {
            input = input_rx.recv(), if !input_finished && !session_closed => {
                if work_in_flight {
                    if let Some(input) = input {
                        deferred.push_back(input);
                    } else {
                        input_finished = true;
                    }
                } else {
                    apply_input(
                        input.unwrap_or(Input::Eof),
                        &brain_tx,
                        &state,
                        &mut input_finished,
                        &mut work,
                    ).await?;
                }
            }
            item = display_rx.recv(), if !session_closed => {
                match item {
                    Some(DisplayItem::SessionClosed) => {
                        if work_in_flight {
                            session_closed = true;
                        } else {
                            println!("[brain] session closed");
                            break;
                        }
                    }
                    Some(item) if matches!(work, Some(FaceWork::Dump(_))) => {
                        buffered_display.push_back(item);
                    }
                    Some(item) => render(item),
                    None if work_in_flight => session_closed = true,
                    None => break,
                }
            }
            result = join_face_work(&mut work), if work_in_flight => {
                work = None;
                let resume_input = complete_face_work(result, &state)?;
                while let Some(item) = buffered_display.pop_front() {
                    render(item);
                }
                if resume_input {
                    let _ = resume_tx.send(());
                }
                if session_closed {
                    println!("[brain] session closed");
                    break;
                }
            }
        }
    }

    if input_thread.is_finished() {
        input_thread
            .join()
            .map_err(|_| "terminal input thread panicked".to_string())?;
    }
    Ok(())
}

async fn apply_input(
    input: Input,
    brain_tx: &mpsc::Sender<BrainMsg>,
    state: &Arc<Mutex<SessionState>>,
    input_finished: &mut bool,
    work: &mut Option<FaceWork>,
) -> Result<(), String> {
    match input {
        Input::Line(line) => {
            let result = handle_line(&line, brain_tx, state).await?;
            *input_finished = result.input_finished;
            *work = result.work;
        }
        Input::Eof => {
            *input_finished = true;
            let _ = brain_tx.send(BrainMsg::Command(BrainCommand::Quit)).await;
        }
        Input::Failed(error) => {
            let _ = brain_tx.send(BrainMsg::Command(BrainCommand::Quit)).await;
            return Err(format!("failed to read terminal input: {error}"));
        }
    }
    Ok(())
}

async fn handle_line(
    line: &str,
    brain_tx: &mpsc::Sender<BrainMsg>,
    state: &Arc<Mutex<SessionState>>,
) -> Result<LineResult, String> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(LineResult {
            input_finished: false,
            work: None,
        });
    }
    let result = match line {
        "/quit" => {
            brain_tx
                .send(BrainMsg::Command(BrainCommand::Quit))
                .await
                .map_err(|_| "brain inbox closed before /quit was delivered".to_string())?;
            LineResult {
                input_finished: true,
                work: None,
            }
        }
        "/end" => {
            send_command(brain_tx, BrainCommand::TurnEnd).await?;
            LineResult {
                input_finished: false,
                work: None,
            }
        }
        "/cancel" => {
            println!("[face] cancel requested");
            send_command(brain_tx, BrainCommand::Cancel).await?;
            LineResult {
                input_finished: false,
                work: None,
            }
        }
        "/rebuild" => {
            send_command(brain_tx, BrainCommand::Rebuild).await?;
            LineResult {
                input_finished: false,
                work: None,
            }
        }
        "/dump" => {
            let snapshot = with_state_read(state, SessionState::dump)?;
            LineResult {
                input_finished: false,
                work: Some(start_dump(snapshot)),
            }
        }
        _ if line.starts_with("/open ") => {
            let path = line.trim_start_matches("/open ").to_string();
            LineResult {
                input_finished: false,
                work: Some(start_open(path)),
            }
        }
        _ => {
            with_state(state, |session| {
                session.append_user_message(line.to_string())
            })?;
            println!("[face] staged user message");
            LineResult {
                input_finished: false,
                work: None,
            }
        }
    };
    Ok(result)
}

async fn send_command(
    brain_tx: &mpsc::Sender<BrainMsg>,
    command: BrainCommand,
) -> Result<(), String> {
    brain_tx
        .send(BrainMsg::Command(command))
        .await
        .map_err(|_| "brain inbox closed before the command was delivered".to_string())
}

fn with_state<T>(
    state: &Arc<Mutex<SessionState>>,
    operation: impl FnOnce(&mut SessionState) -> Result<T, String>,
) -> Result<T, String> {
    let mut state = state
        .lock()
        .map_err(|_| "session state lock poisoned in face".to_string())?;
    operation(&mut state)
}

fn with_state_read<T>(
    state: &Arc<Mutex<SessionState>>,
    operation: impl FnOnce(&SessionState) -> T,
) -> Result<T, String> {
    let state = state
        .lock()
        .map_err(|_| "session state lock poisoned in face".to_string())?;
    Ok(operation(&state))
}

fn render(item: DisplayItem) {
    match item {
        DisplayItem::RequestStarted { request_id } => {
            println!("[brain] request {request_id} in flight");
        }
        DisplayItem::RequestResolved { outcome } => match outcome {
            Outcome::Ok { value } => {
                if let Some(text) = value.text.filter(|text| !text.is_empty()) {
                    println!("[agent] {text}");
                }
            }
            Outcome::Err { error } => println!("[brain] request failed: {error}"),
            Outcome::Cancelled { reason } => println!("[brain] request cancelled: {reason}"),
            Outcome::Panicked { payload } => println!("[brain] request panicked: {payload}"),
        },
        DisplayItem::ToolStarted { name, arguments } => {
            println!("[limb] tool call: {name}({arguments})");
        }
        DisplayItem::ToolResolved { outcome } => match outcome {
            Outcome::Ok { value } => println!("[limb] tool result: {} bytes", value.len()),
            Outcome::Err { error } => println!("[limb] tool error: {error}"),
            Outcome::Cancelled { reason } => println!("[limb] tool cancelled: {reason}"),
            Outcome::Panicked { payload } => println!("[limb] tool panicked: {payload}"),
        },
        DisplayItem::TurnResolved { outcome } => match outcome {
            Outcome::Ok { .. } => println!("[brain] turn complete"),
            Outcome::Err { error } => println!("[brain] turn failed: {error}"),
            Outcome::Cancelled { reason } => println!("[brain] turn cancelled: {reason}"),
            Outcome::Panicked { payload } => println!("[brain] turn panicked: {payload}"),
        },
        DisplayItem::ContextRebuilt { wire_messages } => {
            println!("[brain] context rebuilt ({wire_messages} wire messages)");
        }
        DisplayItem::SessionClosed => unreachable!(),
    }
}

fn start_dump(snapshot: DumpSnapshot) -> FaceWork {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
    FaceWork::Dump(tokio::task::spawn_blocking(move || {
        let path = std::env::temp_dir().join(format!(
            "skeleton-dump-{}-{}.md",
            std::process::id(),
            crate::state::now_ms()
        ));
        std::fs::write(&path, snapshot.render())
            .map_err(|error| format!("failed to write dump {}: {error}", path.display()))?;
        println!("[face] dump written to {}", path.display());
        std::process::Command::new(&editor)
            .arg(&path)
            .status()
            .map_err(|error| format!("failed to launch editor: {error}"))
    }))
}

fn start_open(path: String) -> FaceWork {
    let task_path = path.clone();
    let task = tokio::task::spawn_blocking(move || {
        let content = std::fs::read_to_string(&task_path).map_err(|error| error.to_string())?;
        Ok(OpenedFile {
            bytes: content.len(),
            head: content.lines().take(20).collect::<Vec<_>>().join("\n"),
        })
    });
    FaceWork::Open { path, task }
}

async fn join_face_work(work: &mut Option<FaceWork>) -> FaceWorkResult {
    match work.as_mut().expect("guarded by select condition") {
        FaceWork::Dump(task) => FaceWorkResult::Dump(task.await),
        FaceWork::Open { path, task } => FaceWorkResult::Open {
            path: path.clone(),
            result: task.await,
        },
    }
}

fn complete_face_work(
    result: FaceWorkResult,
    state: &Arc<Mutex<SessionState>>,
) -> Result<bool, String> {
    match result {
        FaceWorkResult::Dump(result) => {
            match result {
                Ok(Ok(status)) if status.success() => {}
                Ok(Ok(status)) => println!("[face] editor exited with {status}"),
                Ok(Err(error)) => println!("[face] {error}"),
                Err(error) => println!("[face] editor task failed: {error}"),
            }
            println!("[face] returned from dump");
            Ok(true)
        }
        FaceWorkResult::Open { path, result } => match result {
            Ok(Ok(opened)) => {
                with_state(state, |session| {
                    session.append_user_activity(path.clone(), opened.bytes, opened.head)
                })?;
                println!("[face] opened {path} ({} bytes)", opened.bytes);
                Ok(false)
            }
            Ok(Err(error)) => {
                println!("[face] could not open {path}: {error}");
                Ok(false)
            }
            Err(error) => {
                println!("[face] could not open {path}: {error}");
                Ok(false)
            }
        },
    }
}
