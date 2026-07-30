//! The brain: owns the session — its event log/context, the provider
//! client and credentials, and the agent loop. The agent loop is a select
//! loop, explicitly separate from the face: while a provider request or a
//! tool call is in flight it keeps receiving user events (which append and
//! piggyback on the next request) and cancel requests (which drain the
//! in-flight work to a definite outcome).
//!
//! In-flight work is a spawned task holding a cancellation token; every
//! task resolves by sending exactly one resolution message back into the
//! loop — ok, err, or cancelled — so nothing in flight ends without a
//! recorded outcome.

use crate::context::Context;
use crate::events::{AssistantMessage, Contribution, Event, EventKind, Outcome};
use crate::limb::LimbRequest;
use crate::provider::{self, ChatRequest, ToolCall};
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_util::sync::CancellationToken;

const SYSTEM_PROMPT: &str = "You are an agent in a prototype harness. The user is testing and developing the harness itself. You should help them by running tools etc. - but also by reporting on your experience as a model in this harness. You are explicitly allowed and encouraged to answer any and all questions about the context provided to you, the system prompt, exact formats, and more. This will be helpful to the user who is the developer of this system.";

pub struct Config {
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub reasoning_effort: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Config {
            base_url: std::env::var("SKELETON_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8089/v1".to_string()),
            api_key: std::env::var("SKELETON_API_KEY").ok(),
            model: std::env::var("SKELETON_MODEL").unwrap_or_else(|_| "fake-model".to_string()),
            reasoning_effort: std::env::var("SKELETON_REASONING_EFFORT").ok(),
        }
    }
}

/// Resolution message sent by in-flight tasks back into the session loop.
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

/// What the session loop is currently waiting on, besides user events.
enum InFlight {
    Idle,
    Request { cancel: CancellationToken },
    Tool { cancel: CancellationToken },
}

pub struct Session {
    config: Config,
    /// The session's limb, at the logical level: a channel to a limb loop
    /// that owns its own environment. Never the limb itself (invariant 10:
    /// a session has a limb logically, not at the memory-ownership level).
    limb_tx: mpsc::Sender<LimbRequest>,
    client: reqwest::Client,
    /// The session log and its cached projections. The brain is the only
    /// writer. In this co-located deployment other roles (the face) read
    /// the same shared log directly; a remote face would instead keep or
    /// request enough of the log for its queries. See the TODO on
    /// `Context` about being cleverer than a mutex.
    context: Arc<Mutex<Context>>,
    bus: broadcast::Sender<Event>,
    done_tx: mpsc::Sender<Resolved>,
    next_request_id: u64,
    /// Tool calls returned by the provider that have not been started yet.
    pending_calls: Vec<ToolCall>,
    /// A turn is live from the triggering TurnEnd until its TurnOutcome.
    turn_live: bool,
    in_flight: InFlight,
}

impl Session {
    /// Run the session loop until Quit or the face hangs up, draining any
    /// in-flight work, then emit SessionClosed.
    pub async fn run(
        config: Config,
        limb_tx: mpsc::Sender<LimbRequest>,
        bus: broadcast::Sender<Event>,
        mut user_rx: mpsc::Receiver<EventKind>,
        context: Arc<Mutex<Context>>,
    ) {
        let (done_tx, mut done_rx) = mpsc::channel::<Resolved>(16);
        let mut session = Session {
            config,
            limb_tx,
            client: reqwest::Client::new(),
            context,
            bus,
            done_tx,
            next_request_id: 0,
            pending_calls: Vec::new(),
            turn_live: false,
            in_flight: InFlight::Idle,
        };
        session.emit(EventKind::SessionStarted {
            system_prompt: SYSTEM_PROMPT.to_string(),
        });
        // Contributions that exist from the start: the limb describes the
        // environment it provides (tool schemas), and the brain adds
        // environment facts. They compose into the system prompt / tools
        // field; anything added later appends an update instead.
        // (No producer can add or change a contribution mid-context yet —
        // deliberately not wired up; the projection rule is modeled in
        // Context and awaits a real producer.)
        let (reply_tx, reply_rx) = oneshot::channel();
        let limb_contributions = if session
            .limb_tx
            .send(LimbRequest::Describe { reply: reply_tx })
            .await
            .is_ok()
        {
            reply_rx.await.unwrap_or_default()
        } else {
            Vec::new()
        };
        for (name, def) in limb_contributions {
            session.emit(EventKind::ContributionAdded {
                name,
                contribution: Contribution::Tool { def },
            });
        }
        let model = session.config.model.clone();
        session.emit(EventKind::ContributionAdded {
            name: "model".to_string(),
            contribution: Contribution::Fact { text: model },
        });
        session.emit(EventKind::ContributionAdded {
            name: "hostname".to_string(),
            contribution: Contribution::Fact { text: hostname() },
        });
        session.emit(EventKind::ContributionAdded {
            name: "session start time".to_string(),
            contribution: Contribution::Fact {
                text: format!("unix epoch ms {}", crate::events::now_ms()),
            },
        });

        loop {
            tokio::select! {
                maybe_kind = user_rx.recv() => {
                    let quit = match maybe_kind {
                        Some(kind) => session.on_user_event(kind, &mut done_rx).await,
                        None => true, // face is gone
                    };
                    if quit {
                        break;
                    }
                }
                Some(resolved) = done_rx.recv() => {
                    session.on_resolved(resolved);
                }
            }
        }

        session.drain(&mut done_rx, "session shutting down").await;
        session.emit(EventKind::SessionClosed);
    }

    fn emit(&mut self, kind: EventKind) {
        let event = self.context.lock().expect("context poisoned").append(kind);
        // Receivers may lag or be gone; events remain in the log.
        let _ = self.bus.send(event);
    }

    /// Returns true when the session should quit.
    async fn on_user_event(
        &mut self,
        kind: EventKind,
        done_rx: &mut mpsc::Receiver<Resolved>,
    ) -> bool {
        match kind {
            EventKind::UserMessage { .. } | EventKind::FileOpened { .. } => {
                // Appends never trigger. If work is in flight the
                // projection piggybacks these on the next request.
                self.emit(kind);
            }
            EventKind::TurnEnd => {
                self.emit(EventKind::TurnEnd);
                if matches!(self.in_flight, InFlight::Idle) && !self.turn_live {
                    self.turn_live = true;
                    self.start_request();
                }
                // If a turn is already live, TurnEnd is just a fact in the
                // log; the staged content is already riding along.
            }
            EventKind::CancelRequest => {
                self.emit(EventKind::CancelRequest);
                self.drain(done_rx, "cancelled by user").await;
            }
            EventKind::RebuildRequest => {
                self.emit(EventKind::RebuildRequest);
                let wire_messages = self.context.lock().expect("context poisoned").rebuild();
                self.emit(EventKind::ContextRebuilt { wire_messages });
            }
            EventKind::DumpRequest => {
                // A fact, not a request/reply: the face (any consumer)
                // projects the dump from the shared log itself when it
                // sees this event come back on the bus.
                self.emit(EventKind::DumpRequest);
            }
            EventKind::Quit => {
                self.emit(EventKind::Quit);
                return true;
            }
            other => {
                // Faces only emit user events; anything else is a face bug,
                // but it is still a fact worth recording.
                self.emit(other);
            }
        }
        false
    }

    /// Handle a resolution from in-flight work (normal path): record it,
    /// then advance the turn.
    fn on_resolved(&mut self, resolved: Resolved) {
        match self.record_resolution(resolved) {
            Advance::StartTool => self.start_next_tool(),
            Advance::StartRequest => self.start_request(),
            Advance::TurnDone(outcome) => self.finish_turn(outcome),
        }
    }

    /// Record a resolution as outcome events and decide what would happen
    /// next. Recording is shared between the normal path and `drain`;
    /// only the normal path is allowed to actually start new work — that
    /// split is what makes "a drain can never launch more work" structural
    /// rather than a timing accident.
    fn record_resolution(&mut self, resolved: Resolved) -> Advance {
        self.in_flight = InFlight::Idle;
        match resolved {
            Resolved::Request {
                request_id,
                outcome,
            } => {
                let advance = match &outcome {
                    Outcome::Ok { value } => {
                        self.pending_calls = value.tool_calls.clone();
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
                self.emit(EventKind::RequestOutcome {
                    request_id,
                    outcome,
                });
                advance
            }
            Resolved::Tool { call, outcome } => {
                let cancelled = matches!(outcome, Outcome::Cancelled { .. });
                self.emit(EventKind::ToolCallOutcome {
                    call_id: call.id.clone(),
                    outcome,
                });
                if cancelled {
                    self.pending_calls.clear();
                    Advance::TurnDone(Outcome::Cancelled {
                        reason: "tool call cancelled".to_string(),
                    })
                } else if !self.pending_calls.is_empty() {
                    Advance::StartTool
                } else {
                    // Tool loop continues: results go back to the provider.
                    Advance::StartRequest
                }
            }
        }
    }

    fn finish_turn(&mut self, outcome: Outcome<()>) {
        if self.turn_live {
            self.turn_live = false;
            self.emit(EventKind::TurnOutcome { outcome });
        }
    }

    fn start_request(&mut self) {
        let request_id = self.next_request_id;
        self.next_request_id += 1;
        self.emit(EventKind::RequestAttempt {
            request_id,
            model: self.config.model.clone(),
        });
        // The request is built from the same projection the dump renders
        // (`request_parts`), so the dump cannot miss what the model sees.
        let parts = self
            .context
            .lock()
            .expect("context poisoned")
            .request_parts();
        let request = ChatRequest {
            model: self.config.model.clone(),
            messages: parts.messages,
            tools: if parts.tools.is_empty() {
                None
            } else {
                Some(parts.tools)
            },
            reasoning_effort: self.config.reasoning_effort.clone(),
        };
        let client = self.client.clone();
        let base_url = self.config.base_url.clone();
        let api_key = self.config.api_key.clone();
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        let done_tx = self.done_tx.clone();
        tokio::spawn(async move {
            let outcome = supervise(async move {
                tokio::select! {
                    result = request_outcome(&client, &base_url, api_key.as_deref(), &request) => result,
                    _ = token.cancelled() => Outcome::Cancelled {
                        reason: "request cancelled; connection dropped".to_string(),
                    },
                }
            })
            .await;
            let _ = done_tx
                .send(Resolved::Request {
                    request_id,
                    outcome,
                })
                .await;
        });
        self.in_flight = InFlight::Request { cancel };
    }

    fn start_next_tool(&mut self) {
        let call = self.pending_calls.remove(0);
        self.emit(EventKind::ToolCallAttempt {
            call_id: call.id.clone(),
            name: call.function.name.clone(),
            arguments: call.function.arguments.clone(),
        });
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        let done_tx = self.done_tx.clone();
        let limb_tx = self.limb_tx.clone();
        let name = call.function.name.clone();
        let arguments = call.function.arguments.clone();
        // Adapter task: asks the limb loop to execute and forwards its
        // reply as this session's resolution. The limb owns the drain: on
        // cancellation it kills and reaps its child before replying. A
        // dropped reply (limb gone/panicked) still yields an outcome.
        tokio::spawn(async move {
            let (reply_tx, reply_rx) = oneshot::channel();
            let sent = limb_tx
                .send(LimbRequest::Execute {
                    name,
                    arguments,
                    cancel: token,
                    reply: reply_tx,
                })
                .await;
            let outcome = if sent.is_ok() {
                reply_rx.await.unwrap_or(Outcome::Err {
                    error: "limb dropped the request without replying".to_string(),
                })
            } else {
                Outcome::Err {
                    error: "limb is gone".to_string(),
                }
            };
            let _ = done_tx.send(Resolved::Tool { call, outcome }).await;
        });
        self.in_flight = InFlight::Tool { cancel };
    }

    /// Request → drain → finalize for whatever is in flight: signal the
    /// token, wait for the task's resolution message, and *record* it —
    /// never advance. If completion won the race against the cancel, the
    /// completed work is recorded as completed, but no new work starts and
    /// the turn finalizes cancelled. Draining structurally cannot launch
    /// more work.
    async fn drain(&mut self, done_rx: &mut mpsc::Receiver<Resolved>, reason: &str) {
        match &self.in_flight {
            InFlight::Idle => return,
            InFlight::Request { cancel } | InFlight::Tool { cancel } => cancel.cancel(),
        }
        if let Some(resolved) = done_rx.recv().await {
            match self.record_resolution(resolved) {
                // The work resolved the turn by itself (completed with a
                // final answer, failed, or was cancelled): that resolution
                // stands.
                Advance::TurnDone(outcome) => self.finish_turn(outcome),
                // The work would have continued the turn: finalize
                // cancelled instead.
                Advance::StartTool | Advance::StartRequest => {
                    self.pending_calls.clear();
                    self.finish_turn(Outcome::Cancelled {
                        reason: reason.to_string(),
                    });
                }
            }
        }
    }
}

/// What a recorded resolution implies for the turn. Only the normal path
/// acts on Start*; `drain` converts them into a cancelled turn.
enum Advance {
    StartTool,
    StartRequest,
    TurnDone(Outcome<()>),
}

/// Run work in a child task so a panic inside it becomes a `Panicked`
/// outcome instead of a silently vanished resolution message. Nothing in
/// flight may end without an outcome — including by panicking.
async fn supervise<T: Send + 'static>(
    work: impl std::future::Future<Output = Outcome<T>> + Send + 'static,
) -> Outcome<T> {
    match tokio::spawn(work).await {
        Ok(outcome) => outcome,
        Err(join_error) if join_error.is_panic() => Outcome::Panicked {
            payload: join_error.to_string(),
        },
        Err(_) => Outcome::Cancelled {
            reason: "task aborted".to_string(),
        },
    }
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| {
            std::fs::read_to_string("/proc/sys/kernel/hostname")
                .ok()
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
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
