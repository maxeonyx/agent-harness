use crate::protocol::{BrainCommand, BrainMsg, DisplayItem, Outcome};
use crate::state::{DumpSnapshot, SessionState};
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

pub async fn run(
    brain_tx: mpsc::Sender<BrainMsg>,
    mut display_rx: mpsc::Receiver<DisplayItem>,
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

    loop {
        tokio::select! {
            input = input_rx.recv(), if !input_finished => {
                match input {
                    Some(Input::Line(line)) => {
                        if handle_line(&line, &brain_tx, &state, &resume_tx).await? {
                            input_finished = true;
                        }
                    }
                    Some(Input::Eof) => {
                        input_finished = true;
                        let _ = brain_tx.send(BrainMsg::Command(BrainCommand::Quit)).await;
                    }
                    Some(Input::Failed(error)) => {
                        let _ = brain_tx.send(BrainMsg::Command(BrainCommand::Quit)).await;
                        return Err(format!("failed to read terminal input: {error}"));
                    }
                    None => input_finished = true,
                }
            }
            item = display_rx.recv() => {
                match item {
                    Some(DisplayItem::SessionClosed) => {
                        println!("[brain] session closed");
                        break;
                    }
                    Some(item) => render(item),
                    None => break,
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

async fn handle_line(
    line: &str,
    brain_tx: &mpsc::Sender<BrainMsg>,
    state: &Arc<Mutex<SessionState>>,
    resume_tx: &std::sync::mpsc::Sender<()>,
) -> Result<bool, String> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(false);
    }
    match line {
        "/quit" => {
            brain_tx
                .send(BrainMsg::Command(BrainCommand::Quit))
                .await
                .map_err(|_| "brain inbox closed before /quit was delivered".to_string())?;
            Ok(true)
        }
        "/end" => {
            send_command(brain_tx, BrainCommand::TurnEnd).await?;
            Ok(false)
        }
        "/cancel" => {
            println!("[face] cancel requested");
            send_command(brain_tx, BrainCommand::Cancel).await?;
            Ok(false)
        }
        "/rebuild" => {
            send_command(brain_tx, BrainCommand::Rebuild).await?;
            Ok(false)
        }
        "/dump" => {
            let snapshot = with_state_read(state, SessionState::dump)?;
            dump_into_editor(snapshot).await;
            println!("[face] returned from dump");
            let _ = resume_tx.send(());
            Ok(false)
        }
        _ if line.starts_with("/open ") => {
            let path = line.trim_start_matches("/open ");
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    let bytes = content.len();
                    let head = content.lines().take(20).collect::<Vec<_>>().join("\n");
                    with_state(state, |session| {
                        session.append_user_activity(path.to_string(), bytes, head)
                    })?;
                    println!("[face] opened {path} ({bytes} bytes)");
                }
                Err(error) => println!("[face] could not open {path}: {error}"),
            }
            Ok(false)
        }
        _ => {
            with_state(state, |session| {
                session.append_user_message(line.to_string())
            })?;
            println!("[face] staged user message");
            Ok(false)
        }
    }
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

async fn dump_into_editor(snapshot: DumpSnapshot) {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
    let result = tokio::task::spawn_blocking(move || {
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
    })
    .await;
    match result {
        Ok(Ok(status)) if status.success() => {}
        Ok(Ok(status)) => println!("[face] editor exited with {status}"),
        Ok(Err(error)) => println!("[face] {error}"),
        Err(error) => println!("[face] editor task failed: {error}"),
    }
}
