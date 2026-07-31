use crate::protocol::{
    AssistantMessage, BrainCommand, BrainMsg, Contribution, DisplayItem, LimbMsg, Outcome,
};
use crate::provider::{self, ChatRequest, ToolCall};
use crate::state::SessionState;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

pub const SYSTEM_PROMPT: &str = "You are an agent in a prototype harness. The user is testing and developing the harness itself. You should help them by running tools etc. - but also by reporting on your experience as a model in this harness. You are explicitly allowed and encouraged to answer any and all questions about the context provided to you, the system prompt, exact formats, and more. This will be helpful to the user who is the developer of this system.";

pub struct Config {
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub reasoning_effort: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            base_url: std::env::var("SKELETON_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8089/v1".to_string()),
            api_key: std::env::var("SKELETON_API_KEY").ok(),
            model: std::env::var("SKELETON_MODEL").unwrap_or_else(|_| "fake-model".to_string()),
            reasoning_effort: std::env::var("SKELETON_REASONING_EFFORT").ok(),
        }
    }
}

enum Resolved {
    Request {
        request_id: u64,
        outcome: Outcome<AssistantMessage>,
    },
    Tool {
        call: ToolCall,
        outcome: Outcome<String>,
    },
}

enum InFlight {
    Idle,
    Request {
        request_id: u64,
        cancel: CancellationToken,
        task: tokio::task::JoinHandle<Outcome<AssistantMessage>>,
    },
    Tool {
        call: ToolCall,
    },
}

pub struct Session {
    config: Config,
    state: Arc<Mutex<SessionState>>,
    limb_tx: mpsc::Sender<LimbMsg>,
    display_tx: mpsc::Sender<DisplayItem>,
    inbox: mpsc::Receiver<BrainMsg>,
    deferred: VecDeque<BrainMsg>,
    client: reqwest::Client,
    next_request_id: u64,
    pending_calls: VecDeque<ToolCall>,
    turn_live: bool,
}

impl Session {
    pub async fn run(
        config: Config,
        state: Arc<Mutex<SessionState>>,
        limb_tx: mpsc::Sender<LimbMsg>,
        display_tx: mpsc::Sender<DisplayItem>,
        inbox: mpsc::Receiver<BrainMsg>,
        session_over: watch::Sender<bool>,
    ) -> Result<(), String> {
        let mut session = Self {
            config,
            state,
            limb_tx,
            display_tx,
            inbox,
            deferred: VecDeque::new(),
            client: reqwest::Client::new(),
            next_request_id: 0,
            pending_calls: VecDeque::new(),
            turn_live: false,
        };
        session.append_initial_contributions()?;
        let mut in_flight = InFlight::Idle;

        let looped: Result<(), String> = async {
            loop {
                if let Some(message) = session.deferred.pop_front() {
                    if session.on_message(message, &mut in_flight).await? {
                        break;
                    }
                    continue;
                }
                let busy = matches!(in_flight, InFlight::Request { .. });
                tokio::select! {
                    // Biased: a ready user message (a cancel in particular)
                    // is processed before a ready completion — "the user
                    // has requested to finish" must not lose a scheduler
                    // coin toss and watch a tool call start anyway.
                    biased;
                    message = session.inbox.recv() => {
                        match message {
                            Some(message) => {
                                if session.on_message(message, &mut in_flight).await? {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                    resolved = join_request(&mut in_flight), if busy => {
                        session.on_resolved(resolved, &mut in_flight).await?;
                    }
                }
            }
            Ok(())
        }
        .await;

        // Drain whatever is in flight even when the loop failed: nothing
        // ends detached or without a recorded outcome. The first error
        // wins; drain errors on the failure path are secondary.
        let drained = session.drain(&mut in_flight, "session shutting down").await;
        looped?;
        drained?;
        session.with_state(|state| state.append_session_closed())?;
        // The monotonic session-over watch must be set BEFORE the face can
        // observe SessionClosed and exit, or the supervisor could join the
        // face while the watch still reads false and misclassify an
        // orderly shutdown as a mid-session death.
        let _ = session_over.send(true);
        session.send_display(DisplayItem::SessionClosed).await;
        Ok(())
    }

    /// The brain's own contributions: facts about the session and the
    /// provider world it owns. (Environment facts — hostname, tools — are
    /// the limb's contributions, appended by main at startup.)
    fn append_initial_contributions(&self) -> Result<(), String> {
        let model = self.config.model.clone();
        self.with_state(|state| {
            state.append_contribution("model".to_string(), Contribution::Fact { text: model })?;
            state.append_contribution(
                "session start time".to_string(),
                Contribution::Fact {
                    text: format!("unix epoch ms {}", crate::state::now_ms()),
                },
            )
        })
    }

    async fn on_message(
        &mut self,
        message: BrainMsg,
        in_flight: &mut InFlight,
    ) -> Result<bool, String> {
        match message {
            BrainMsg::Command(BrainCommand::TurnEnd) => {
                self.with_state(|state| state.append_turn_end())?;
                if matches!(in_flight, InFlight::Idle) && !self.turn_live {
                    self.turn_live = true;
                    self.start_request(in_flight).await?;
                }
            }
            BrainMsg::Command(BrainCommand::Cancel) => {
                self.with_state(|state| state.append_cancel_request())?;
                self.drain(in_flight, "cancelled by user").await?;
            }
            BrainMsg::Command(BrainCommand::Rebuild) => {
                // The entire rebuild happens under one lock: append the
                // request, replay the journal, swap. Releasing the lock
                // in between would let a concurrent face append land in
                // the old state after the replay read the file — lost
                // from memory, and a duplicate seq on the next append.
                let wire_messages = self.with_state(|state| {
                    state.append_rebuild_request()?;
                    let rebuilt = SessionState::load(&state.journal_path())?;
                    let wire_messages = rebuilt.wire_message_count();
                    *state = rebuilt;
                    Ok(wire_messages)
                })?;
                self.send_display(DisplayItem::ContextRebuilt { wire_messages })
                    .await;
            }
            BrainMsg::Command(BrainCommand::Quit) => {
                self.with_state(|state| state.append_quit())?;
                return Ok(true);
            }
            BrainMsg::ToolOutcome { call_id, outcome } => {
                let call = match std::mem::replace(in_flight, InFlight::Idle) {
                    InFlight::Tool { call } if call.id == call_id => call,
                    other => {
                        *in_flight = other;
                        return Err(format!(
                            "brain received outcome for tool call {call_id}, but that call is not in flight"
                        ));
                    }
                };
                self.on_resolved(Resolved::Tool { call, outcome }, in_flight)
                    .await?;
            }
        }
        Ok(false)
    }

    async fn on_resolved(
        &mut self,
        resolved: Resolved,
        in_flight: &mut InFlight,
    ) -> Result<(), String> {
        *in_flight = InFlight::Idle;
        let display = resolution_display(&resolved);
        let advance = self.record_resolution(resolved)?;
        self.send_display(display).await;
        match advance {
            Advance::StartTool => self.start_next_tool(in_flight).await?,
            Advance::StartRequest => self.start_request(in_flight).await?,
            Advance::TurnDone(outcome) => self.finish_turn(outcome).await?,
        }
        Ok(())
    }

    fn record_resolution(&mut self, resolved: Resolved) -> Result<Advance, String> {
        match resolved {
            Resolved::Request {
                request_id,
                outcome,
            } => {
                let advance = match &outcome {
                    Outcome::Ok { value } => {
                        self.pending_calls = value.tool_calls.clone().into();
                        if self.pending_calls.is_empty() {
                            Advance::TurnDone(Outcome::Ok { value: () })
                        } else {
                            Advance::StartTool
                        }
                    }
                    Outcome::Err { error } => Advance::TurnDone(Outcome::Err {
                        error: error.clone(),
                    }),
                    Outcome::Cancelled { reason } => Advance::TurnDone(Outcome::Cancelled {
                        reason: reason.clone(),
                    }),
                    Outcome::Panicked { payload } => Advance::TurnDone(Outcome::Panicked {
                        payload: payload.clone(),
                    }),
                };
                let stored = outcome.clone();
                self.with_state(|state| state.append_request_outcome(request_id, stored))?;
                Ok(advance)
            }
            Resolved::Tool { call, outcome } => {
                let cancelled = matches!(outcome, Outcome::Cancelled { .. });
                let call_id = call.id;
                self.with_state(|state| state.append_tool_outcome(call_id, outcome))?;
                if cancelled {
                    self.pending_calls.clear();
                    Ok(Advance::TurnDone(Outcome::Cancelled {
                        reason: "tool call cancelled".to_string(),
                    }))
                } else if self.pending_calls.is_empty() {
                    Ok(Advance::StartRequest)
                } else {
                    Ok(Advance::StartTool)
                }
            }
        }
    }

    async fn finish_turn(&mut self, outcome: Outcome<()>) -> Result<(), String> {
        if self.turn_live {
            self.turn_live = false;
            let stored = outcome.clone();
            self.with_state(|state| state.append_turn_outcome(stored))?;
            self.send_display(DisplayItem::TurnResolved { outcome })
                .await;
        }
        Ok(())
    }

    async fn start_request(&mut self, in_flight: &mut InFlight) -> Result<(), String> {
        let request_id = self.next_request_id;
        self.next_request_id += 1;
        let model = self.config.model.clone();
        self.with_state(|state| state.append_request_attempt(request_id, model.clone()))?;
        let parts = self.with_state_read(SessionState::request_parts)?;
        let request = ChatRequest {
            model,
            messages: parts.messages,
            tools: (!parts.tools.is_empty()).then_some(parts.tools),
            reasoning_effort: self.config.reasoning_effort.clone(),
        };
        self.send_display(DisplayItem::RequestStarted { request_id })
            .await;
        let client = self.client.clone();
        let base_url = self.config.base_url.clone();
        let api_key = self.config.api_key.clone();
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        let task = tokio::spawn(async move {
            tokio::select! {
                // Biased: a response that has already completed wins a tie
                // with the cancel token — a completed response is kept
                // (it cost money and is probably good; binding ruling),
                // even when the turn it belongs to finalizes cancelled.
                biased;
                outcome = request_outcome(&client, &base_url, api_key.as_deref(), &request) => outcome,
                _ = token.cancelled() => Outcome::Cancelled {
                    reason: "request cancelled; connection dropped".to_string(),
                },
            }
        });
        *in_flight = InFlight::Request {
            request_id,
            cancel,
            task,
        };
        Ok(())
    }

    async fn start_next_tool(&mut self, in_flight: &mut InFlight) -> Result<(), String> {
        let call = self
            .pending_calls
            .pop_front()
            .ok_or_else(|| "brain tried to start a tool with no pending call".to_string())?;
        if self
            .limb_tx
            .send(LimbMsg::Execute { call: call.clone() })
            .await
            .is_err()
        {
            // The limb is gone; the attempt still resolves — as Panicked,
            // recorded like any other outcome — and the turn ends with it.
            let payload = "limb disappeared before accepting the tool call".to_string();
            let outcome = Outcome::Panicked {
                payload: payload.clone(),
            };
            self.send_display(DisplayItem::ToolResolved {
                outcome: outcome.clone(),
            })
            .await;
            self.with_state(|state| state.append_tool_outcome(call.id, outcome))?;
            self.pending_calls.clear();
            self.finish_turn(Outcome::Panicked { payload }).await?;
        } else {
            *in_flight = InFlight::Tool { call };
        }
        Ok(())
    }

    async fn drain(&mut self, in_flight: &mut InFlight, reason: &str) -> Result<(), String> {
        let resolved = match in_flight {
            InFlight::Idle => return Ok(()),
            InFlight::Request { cancel, .. } => {
                cancel.cancel();
                join_request(in_flight).await
            }
            InFlight::Tool { call } => {
                let call = call.clone();
                if self.limb_tx.send(LimbMsg::Cancel).await.is_err() {
                    Resolved::Tool {
                        call,
                        outcome: Outcome::Panicked {
                            payload: "limb disappeared while cancelling the tool call".to_string(),
                        },
                    }
                } else {
                    loop {
                        match self.inbox.recv().await {
                            Some(BrainMsg::ToolOutcome { call_id, outcome })
                                if call_id == call.id =>
                            {
                                break Resolved::Tool { call, outcome };
                            }
                            Some(message) => self.deferred.push_back(message),
                            None => {
                                break Resolved::Tool {
                                    call,
                                    outcome: Outcome::Panicked {
                                        payload: "brain inbox closed while waiting for the limb to cancel the tool call".to_string(),
                                    },
                                };
                            }
                        }
                    }
                }
            }
        };
        *in_flight = InFlight::Idle;
        let display = resolution_display(&resolved);
        let advance = self.record_resolution(resolved)?;
        self.send_display(display).await;
        match advance {
            Advance::TurnDone(outcome) => self.finish_turn(outcome).await?,
            Advance::StartTool | Advance::StartRequest => {
                self.pending_calls.clear();
                self.finish_turn(Outcome::Cancelled {
                    reason: reason.to_string(),
                })
                .await?;
            }
        }
        Ok(())
    }

    fn with_state<T>(
        &self,
        operation: impl FnOnce(&mut SessionState) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "session state lock poisoned in brain".to_string())?;
        operation(&mut state)
    }

    fn with_state_read<T>(&self, operation: impl FnOnce(&SessionState) -> T) -> Result<T, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "session state lock poisoned in brain".to_string())?;
        Ok(operation(&state))
    }

    async fn send_display(&self, item: DisplayItem) {
        let _ = self.display_tx.send(item).await;
    }
}

enum Advance {
    StartTool,
    StartRequest,
    TurnDone(Outcome<()>),
}

fn resolution_display(resolved: &Resolved) -> DisplayItem {
    match resolved {
        Resolved::Request { outcome, .. } => DisplayItem::RequestResolved {
            outcome: outcome.clone(),
        },
        Resolved::Tool { outcome, .. } => DisplayItem::ToolResolved {
            outcome: outcome.clone(),
        },
    }
}

async fn join_request(in_flight: &mut InFlight) -> Resolved {
    match in_flight {
        InFlight::Request {
            request_id, task, ..
        } => {
            let outcome = match task.await {
                Ok(outcome) => outcome,
                Err(error) if error.is_panic() => Outcome::Panicked {
                    payload: error.to_string(),
                },
                Err(_) => Outcome::Cancelled {
                    reason: "request task aborted".to_string(),
                },
            };
            Resolved::Request {
                request_id: *request_id,
                outcome,
            }
        }
        _ => unreachable!("join_request called without a request in flight"),
    }
}

async fn request_outcome(
    client: &reqwest::Client,
    base_url: &str,
    api_key: Option<&str>,
    request: &ChatRequest,
) -> Outcome<AssistantMessage> {
    match provider::send(client, base_url, api_key, request).await {
        Ok(response) => match response.choices.into_iter().next() {
            Some(choice) => Outcome::Ok {
                value: AssistantMessage {
                    text: choice.message.content,
                    tool_calls: choice.message.tool_calls.unwrap_or_default(),
                },
            },
            None => Outcome::Err {
                error: "provider response had no choices".to_string(),
            },
        },
        Err(error) => Outcome::Err { error },
    }
}
