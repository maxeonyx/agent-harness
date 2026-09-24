//! Agents as structured concurrency.
//!
//! One agent is one loop: send a request, run the tool calls it came back
//! with, repeat until it writes a message with no tool calls. That last
//! message is its report.
//!
//! A `task` call is a scope. The caller stops inside the call while its
//! children run at the same time, and the call returns one tool result
//! holding every child's report. A forked child's context is the parent's
//! `Vec<Message>` cloned, so it serializes to the same bytes and the provider
//! serves the whole prefix from cache; only its own tail is new.

use crate::face::Face;
use crate::framing::{self, Cut, Framing};
use crate::limb::Limb;
use crate::record::Recorder;
use crate::wire::{self, ChatRequest, Message, ToolCall};

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// HTTP attempts per request for an ordinary transient failure, each
/// separately gated by the spend cap and by cancellation. A rate limit is not
/// counted against this: it is waited out against `--rate-limit-patience`
/// instead.
const ATTEMPTS: usize = 4;

/// Why the harness could not get past something. The distinction matters to
/// the benchmark: a run the provider broke says nothing about the model,
/// while a run that hit its spend cap or its turn limit says the tree ran
/// away, which is exactly what is being measured.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FaultKind {
    /// The provider or the transport. Nothing to do with the model's choices.
    Provider,
    /// The spend cap.
    Budget,
    /// The turn limit.
    Runaway,
    /// A bug in the harness.
    Panic,
}

impl FaultKind {
    pub fn name(self) -> &'static str {
        match self {
            FaultKind::Provider => "provider",
            FaultKind::Budget => "budget",
            FaultKind::Runaway => "runaway",
            FaultKind::Panic => "panic",
        }
    }

    /// Whether a trial that ended this way can be read as evidence about the
    /// model.
    pub fn is_the_models_doing(self) -> bool {
        matches!(self, FaultKind::Budget | FaultKind::Runaway)
    }
}

#[derive(Clone, Debug)]
pub struct Fault {
    pub kind: FaultKind,
    pub reason: String,
}

impl Fault {
    pub fn provider(reason: impl Into<String>) -> Fault {
        Fault {
            kind: FaultKind::Provider,
            reason: reason.into(),
        }
    }
}

pub enum RequestEnd {
    Faulted(Fault),
    Cancelled,
}
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Honour each `task` entry's own `fresh` flag.
    Declared,
    Fork,
    Fresh,
}

impl Mode {
    pub fn parse(text: &str) -> Result<Mode, String> {
        match text {
            "fork" => Ok(Mode::Fork),
            "fresh" => Ok(Mode::Fresh),
            "declared" => Ok(Mode::Declared),
            other => Err(format!(
                "unknown --mode {other}; expected fork, fresh or declared"
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Mode::Declared => "declared",
            Mode::Fork => "fork",
            Mode::Fresh => "fresh",
        }
    }

    fn resolve(self, declared: bool) -> bool {
        match self {
            Mode::Declared => declared,
            Mode::Fork => false,
            Mode::Fresh => true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub model: String,
    pub provider: Option<String>,
    pub base_url: String,
    pub api_key: Option<String>,
    pub framing: Framing,
    pub max_cost: f64,
    pub max_depth: usize,
    pub max_turns: usize,
    /// How long to keep waiting out a rate limit before giving up on it.
    pub rate_limit_patience: Duration,
    /// Fault injection. Naming an agent's path makes that agent's task panic
    /// as it starts, which is the only way to watch the harness record a
    /// panicked agent without shipping a bug.
    pub panic_in: Option<String>,
    /// No timeout means a provider that stops answering hangs the whole tree
    /// for as long as it likes. A timed-out request is a transient failure
    /// and is retried.
    pub request_timeout: Duration,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Completed,
    Cancelled,
    /// The harness could not get past something — bad credentials, the spend
    /// cap, a provider that will not answer. Not completion: the agent's
    /// ancestors are blocked in exactly the same way.
    Faulted(String),
    /// An ancestor of a faulted agent: still inside its `task` call, never
    /// resumed.
    Suspended,
    /// The agent's task panicked. A bug, and an out-of-band fault like any
    /// other — but it still ends with a recorded outcome.
    Panicked(String),
}

impl Outcome {
    pub fn label(&self) -> String {
        match self {
            Outcome::Completed => "completed".to_string(),
            Outcome::Cancelled => "cancelled".to_string(),
            Outcome::Faulted(reason) => format!("faulted: {reason}"),
            Outcome::Suspended => "suspended".to_string(),
            Outcome::Panicked(reason) => format!("panicked: {reason}"),
        }
    }

    pub fn short(&self) -> &'static str {
        match self {
            Outcome::Completed => "completed",
            Outcome::Cancelled => "cancelled",
            Outcome::Faulted(_) => "faulted",
            Outcome::Suspended => "suspended",
            Outcome::Panicked(_) => "panicked",
        }
    }

    /// Both kinds of out-of-band fault: the harness cannot get past either,
    /// so neither resumes an ancestor.
    fn blocks_the_parent(&self) -> bool {
        matches!(
            self,
            Outcome::Faulted(_) | Outcome::Suspended | Outcome::Panicked(_)
        )
    }
}

/// Where an agent is. Every agent reaches `Ended` exactly once, including one
/// whose task panicked — a `state` that is still `Running` in `summary.json`
/// after the run is over means the harness lost track of an agent.
#[derive(Clone, Debug, PartialEq)]
pub enum AgentState {
    Waiting,
    Running,
    Suspended,
    Ended(Outcome),
}

impl AgentState {
    pub fn short(&self) -> &'static str {
        match self {
            AgentState::Waiting => "waiting",
            AgentState::Running => "running",
            AgentState::Suspended => "suspended",
            AgentState::Ended(outcome) => outcome.short(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ToolCallRecord {
    pub name: String,
    pub arguments: String,
    /// What the tool answered. A read that failed is not a read, and the
    /// benchmark must not score it as one, so the result is recorded next to
    /// the call rather than inferred from the arguments.
    pub result: Option<String>,
}

#[derive(Clone)]
pub struct AgentRecord {
    pub path: String,
    pub depth: usize,
    pub fresh: bool,
    pub state: AgentState,
    pub requests: usize,
    pub cached_in: u64,
    pub written_in: u64,
    pub uncached_in: u64,
    /// The first request only. For a child that is the fork itself: how much
    /// of the parent's prefix the provider served from cache.
    pub first_cached_in: u64,
    pub first_uncached_in: u64,
    pub out: u64,
    pub cost: f64,
    pub millis: u128,
    pub tool_calls: Vec<ToolCallRecord>,
    pub children: Vec<String>,
    /// The `task` text the parent wrote for this agent, before any framing.
    pub task: Option<String>,
    /// That text plus the framing under test, as the child actually saw it.
    pub assignment: Option<String>,
    pub handoff: String,
    pub messages: Vec<Message>,
}

impl AgentRecord {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "path": self.path,
            "depth": self.depth,
            "fresh": self.fresh,
            "state": self.state.short(),
            "requests": self.requests,
            "cached_in": self.cached_in,
            "written_in": self.written_in,
            "uncached_in": self.uncached_in,
            "first_cached_in": self.first_cached_in,
            "first_uncached_in": self.first_uncached_in,
            "out": self.out,
            "cost": self.cost,
            "millis": self.millis,
            "children": self.children,
            "tool_calls": self.tool_calls.iter().map(|c| serde_json::json!({
                "name": c.name, "arguments": c.arguments, "result": c.result
            })).collect::<Vec<_>>(),
            "task": self.task,
            "assignment": self.assignment,
            "handoff": self.handoff,
        })
    }
}

struct RunState {
    spent: f64,
    /// Requests sent and not yet answered, and the most any single request
    /// has cost so far. A scope puts many requests in the air at once, so a
    /// cap checked against `spent` alone is overshot by whatever they turn
    /// out to cost; charging each in-flight request the worst seen keeps the
    /// overshoot to about one request's worth.
    in_flight: usize,
    worst: f64,
    fault: Option<String>,
    fault_kind: Option<FaultKind>,
    agents: Vec<AgentRecord>,
    index: HashMap<String, usize>,
}

pub struct Run {
    pub config: Config,
    pub face: Face,
    pub recorder: Recorder,
    pub cancel: CancellationToken,
    client: reqwest::Client,
    limb: Limb,
    session_id: String,
    state: Mutex<RunState>,
}

pub struct AgentEnd {
    pub outcome: Outcome,
    pub handoff: String,
    pub messages: Vec<Message>,
}

impl Run {
    pub fn new(
        config: Config,
        limb: Limb,
        face: Face,
        recorder: Recorder,
        cancel: CancellationToken,
        session_id: String,
    ) -> Run {
        Run {
            face,
            recorder,
            cancel,
            client: reqwest::Client::builder()
                .timeout(config.request_timeout)
                .build()
                .expect("build HTTP client"),
            limb,
            session_id,
            config,
            state: Mutex::new(RunState {
                spent: 0.0,
                in_flight: 0,
                worst: 0.0,
                fault: None,
                fault_kind: None,
                agents: Vec::new(),
                index: HashMap::new(),
            }),
        }
    }

    pub fn register(
        &self,
        path: &str,
        depth: usize,
        fresh: bool,
        task: Option<String>,
        parent: Option<&str>,
    ) -> usize {
        let mut state = self.state();
        let index = state.agents.len();
        state.agents.push(AgentRecord {
            path: path.to_string(),
            depth,
            fresh,
            state: AgentState::Waiting,
            requests: 0,
            cached_in: 0,
            written_in: 0,
            uncached_in: 0,
            first_cached_in: 0,
            first_uncached_in: 0,
            out: 0,
            cost: 0.0,
            millis: 0,
            tool_calls: Vec::new(),
            children: Vec::new(),
            task,
            assignment: None,
            handoff: String::new(),
            messages: Vec::new(),
        });
        state.index.insert(path.to_string(), index);
        if let Some(parent) = parent {
            let parent_index = *state
                .index
                .get(parent)
                .unwrap_or_else(|| panic!("agent {path} registered under unknown parent {parent}"));
            state.agents[parent_index].children.push(path.to_string());
        }
        index
    }

    /// A panic while an agent's record is being written must not take the
    /// rest of the run's bookkeeping with it — losing `summary.json` is how
    /// you lose the evidence for a run you already paid for.
    fn state(&self) -> std::sync::MutexGuard<'_, RunState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn note(&self, index: usize, f: impl FnOnce(&mut AgentRecord)) {
        f(&mut self.state().agents[index]);
    }

    /// A panicked agent is a bug, and an out-of-band fault — but it still
    /// ends with a recorded outcome, so `summary.json` never claims an agent
    /// is still running after the run is over.
    pub fn record_panic(&self, path: &str, reason: &str) {
        self.raise_fault(
            path,
            &Fault {
                kind: FaultKind::Panic,
                reason: format!("agent task panicked: {reason}"),
            },
        );
        if let Some(index) = self.index_of(path) {
            self.note(index, |record| {
                record.state = AgentState::Ended(Outcome::Panicked(reason.to_string()))
            });
        }
    }

    pub fn index_of(&self, path: &str) -> Option<usize> {
        self.state().index.get(path).copied()
    }

    pub fn fault(&self) -> Option<String> {
        self.state().fault.clone()
    }

    pub fn fault_kind(&self) -> Option<FaultKind> {
        self.state().fault_kind
    }

    /// An out-of-band failure stops the whole run: nothing new starts
    /// anywhere, and every ancestor of the faulting agent stays suspended.
    fn raise_fault(&self, path: &str, fault: &Fault) {
        {
            let mut state = self.state();
            if state.fault.is_none() {
                state.fault = Some(format!("{path}: {}", fault.reason));
                state.fault_kind = Some(fault.kind);
            }
        }
        self.face
            .say(&format!("{path:<28} FAULT: {}", fault.reason));
        self.cancel.cancel();
    }

    pub fn agents(&self) -> Vec<AgentRecord> {
        self.state().agents.clone()
    }

    pub fn total_cost(&self) -> f64 {
        self.state().spent
    }

    /// How a request ended. Cancellation is not a fault: nothing went wrong,
    /// the run was stopped.
    async fn request(
        &self,
        path: &str,
        index: usize,
        messages: &[Message],
    ) -> Result<Message, RequestEnd> {
        let request = ChatRequest {
            model: self.config.model.clone(),
            messages: messages.to_vec(),
            tools: framing::tool_schemas(),
            cache_control: serde_json::json!({ "type": "ephemeral" }),
            provider: self
                .config
                .provider
                .as_ref()
                .map(|slug| serde_json::json!({ "order": [slug], "allow_fallbacks": false })),
            session_id: self.session_id.clone(),
        };
        let body = serde_json::to_value(&request)
            .map_err(|e| RequestEnd::Faulted(Fault::provider(e.to_string())))?;

        // Two kinds of patience. An ordinary transient failure gets a few
        // quick attempts; a rate limit gets waited out, because the provider
        // is asking us to come back rather than failing.
        let mut backoff = Duration::from_millis(500);
        let patience = self.config.rate_limit_patience;
        let mut rate_backoff = patience / 24;
        let mut waited_out = Duration::ZERO;
        let mut attempt = 0usize;
        loop {
            attempt += 1;
            if self.cancel.is_cancelled() {
                return Err(RequestEnd::Cancelled);
            }
            // Every attempt is a separate charge, so every attempt is gated.
            {
                let mut state = self.state();
                let committed = state.spent + state.in_flight as f64 * state.worst;
                if committed >= self.config.max_cost {
                    return Err(RequestEnd::Faulted(Fault {
                        kind: FaultKind::Budget,
                        reason: format!(
                            "spend cap reached: ${:.4} spent, {} request(s) in flight at up to ${:.4} each, cap ${:.4}",
                            state.spent, state.in_flight, state.worst, self.config.max_cost
                        ),
                    }));
                }
                state.in_flight += 1;
            }
            self.recorder.wire(path, "request", &body);
            self.face.line(
                path,
                &if attempt == 1 {
                    "request sent".to_string()
                } else {
                    format!("request sent (attempt {attempt})")
                },
            );
            let attempted = wire::send_once(
                &self.client,
                &self.config.base_url,
                self.config.api_key.as_deref(),
                &request,
            )
            .await;
            self.state().in_flight -= 1;

            let again = match attempted {
                wire::Attempt::Answered(sent) => {
                    self.recorder.wire(path, "response", &sent.body);
                    self.absorb(path, index, &sent);
                    return Ok(sent.response.choices.into_iter().next().unwrap().message);
                }
                wire::Attempt::Fatal(reason) => {
                    self.recorder
                        .wire(path, "failure", &serde_json::json!({ "reason": reason }));
                    return Err(RequestEnd::Faulted(Fault::provider(reason)));
                }
                wire::Attempt::Transient(again) => again,
            };
            self.recorder.wire(
                path,
                "failure",
                &serde_json::json!({
                    "reason": again.reason,
                    "rate_limited": again.rate_limited,
                }),
            );

            let delay = again.retry_after.unwrap_or(if again.rate_limited {
                rate_backoff
            } else {
                backoff
            });
            if again.rate_limited {
                if waited_out + delay > patience {
                    return Err(RequestEnd::Faulted(Fault::provider(format!(
                        "rate limited for longer than {:.0}s: {}",
                        patience.as_secs_f64(),
                        again.reason
                    ))));
                }
                waited_out += delay;
                rate_backoff *= 2;
                self.face.line(
                    path,
                    &format!(
                        "rate limited, waiting {:.1}s ({:.0}s of {:.0}s patience used)",
                        delay.as_secs_f64(),
                        waited_out.as_secs_f64(),
                        patience.as_secs_f64()
                    ),
                );
            } else {
                if attempt >= ATTEMPTS {
                    return Err(RequestEnd::Faulted(Fault::provider(format!(
                        "{attempt} attempts failed; last: {}",
                        again.reason
                    ))));
                }
                backoff *= 3;
                self.face.line(
                    path,
                    &format!("request failed, will retry: {}", again.reason),
                );
            }
            // A retry is new work. Cancellation must reach it while it waits.
            tokio::select! {
                _ = tokio::time::sleep(delay) => {}
                _ = self.cancel.cancelled() => return Err(RequestEnd::Cancelled),
            }
        }
    }

    /// Book one answered request against the run and the agent.
    fn absorb(&self, path: &str, index: usize, sent: &wire::Sent) {
        let usage = sent.response.usage.clone();
        let uncached = usage
            .prompt_tokens
            .saturating_sub(usage.prompt_tokens_details.cached_tokens);
        {
            let mut state = self.state();
            state.spent += usage.cost;
            state.worst = state.worst.max(usage.cost);
            let record = &mut state.agents[index];
            if record.requests == 0 {
                record.first_cached_in = usage.prompt_tokens_details.cached_tokens;
                record.first_uncached_in = uncached;
            }
            record.requests += 1;
            record.cached_in += usage.prompt_tokens_details.cached_tokens;
            record.written_in += usage.prompt_tokens_details.cache_write_tokens;
            record.uncached_in += uncached;
            record.out += usage.completion_tokens;
            record.cost += usage.cost;
        }
        self.face.line(
            path,
            &format!(
                "request returned   in {} (cached {}, written {})  out {}  ${:.4}  via {}",
                usage.prompt_tokens,
                usage.prompt_tokens_details.cached_tokens,
                usage.prompt_tokens_details.cache_write_tokens,
                usage.completion_tokens,
                usage.cost,
                sent.response.provider.as_deref().unwrap_or("?")
            ),
        );
    }

    pub fn snapshot(&self) -> String {
        let agents = self.agents();
        let mut text = String::from(
            "agent                            state      reqs   cached  written  uncached      out      cost     time\n",
        );
        let mut total = Tally::default();
        for agent in &agents {
            let name = format!(
                "{}{}",
                "  ".repeat(agent.depth),
                agent.path.rsplit(" › ").next().unwrap_or(&agent.path)
            );
            text.push_str(&format!(
                "{:<32} {:<10} {:>4} {:>8} {:>8} {:>9} {:>8} {:>9} {:>7.1}s\n",
                name,
                agent.state.short(),
                agent.requests,
                agent.cached_in,
                agent.written_in,
                agent.uncached_in,
                agent.out,
                format!("${:.4}", agent.cost),
                agent.millis as f64 / 1000.0,
            ));
            total.add(agent);
        }
        text.push_str(&format!(
            "{:<32} {:<10} {:>4} {:>8} {:>8} {:>9} {:>8} {:>9}\n",
            "TOTAL",
            "",
            total.requests,
            total.cached_in,
            total.written_in,
            total.uncached_in,
            total.out,
            format!("${:.4}", total.cost),
        ));
        text.push_str(&format!(
            "cache-read share of input: {:.1}%\n",
            total.cache_share() * 100.0
        ));
        text
    }
}

#[derive(Default)]
struct Tally {
    requests: usize,
    cached_in: u64,
    written_in: u64,
    uncached_in: u64,
    out: u64,
    cost: f64,
}

impl Tally {
    fn add(&mut self, agent: &AgentRecord) {
        self.requests += agent.requests;
        self.cached_in += agent.cached_in;
        self.written_in += agent.written_in;
        self.uncached_in += agent.uncached_in;
        self.out += agent.out;
        self.cost += agent.cost;
    }

    fn cache_share(&self) -> f64 {
        let input = self.cached_in + self.uncached_in;
        if input == 0 {
            0.0
        } else {
            self.cached_in as f64 / input as f64
        }
    }
}

/// Boxed so the recursion (an agent opens a scope, whose children are
/// agents) has a type.
pub fn run_agent(
    run: Arc<Run>,
    path: String,
    depth: usize,
    index: usize,
    messages: Vec<Message>,
) -> Pin<Box<dyn Future<Output = AgentEnd> + Send>> {
    Box::pin(async move {
        let started = Instant::now();
        run.note(index, |record| record.state = AgentState::Running);
        if run.config.panic_in.as_deref() == Some(path.as_str()) {
            panic!("--panic-in {path}");
        }
        let mut messages = messages;
        let mut handoff = String::new();
        let mut turns = 0usize;

        let end = |run: &Arc<Run>, outcome: Outcome, handoff: String, messages: Vec<Message>| {
            run.note(index, |record| {
                record.state = AgentState::Ended(outcome.clone());
                record.millis = started.elapsed().as_millis();
                record.handoff = handoff.clone();
                record.messages = messages.clone();
            });
            run.face
                .line(&path, &format!("agent finished: {}", outcome.label()));
            AgentEnd {
                outcome,
                handoff,
                messages,
            }
        };

        loop {
            if run.cancel.is_cancelled() {
                return end(&run, Outcome::Cancelled, handoff, messages);
            }
            turns += 1;
            if turns > run.config.max_turns {
                let fault = Fault {
                    kind: FaultKind::Runaway,
                    reason: format!("agent ran past {} turns", run.config.max_turns),
                };
                run.raise_fault(&path, &fault);
                return end(&run, Outcome::Faulted(fault.reason), handoff, messages);
            }

            let reply = match run.request(&path, index, &messages).await {
                Ok(reply) => reply,
                Err(RequestEnd::Cancelled) => {
                    return end(&run, Outcome::Cancelled, handoff, messages);
                }
                Err(RequestEnd::Faulted(fault)) => {
                    run.raise_fault(&path, &fault);
                    return end(&run, Outcome::Faulted(fault.reason), handoff, messages);
                }
            };
            let text = reply.content.clone().unwrap_or_default();
            let calls = reply.tool_calls.clone().unwrap_or_default();
            messages.push(reply);

            if calls.is_empty() {
                return end(&run, Outcome::Completed, text, messages);
            }
            if !text.trim().is_empty() {
                handoff = text;
            }
            if run.cancel.is_cancelled() {
                return end(&run, Outcome::Cancelled, handoff, messages);
            }

            // Sort the turn's calls once: the locals, and at most one scope.
            // Everything after this point knows which is which, so no later
            // step has to ask whether a slot was filled in.
            let mut locals: Vec<(usize, Local)> = Vec::new();
            let mut refusals: Vec<(usize, String)> = Vec::new();
            let mut scope_at: Option<usize> = None;
            for (i, call) in calls.iter().enumerate() {
                let arguments: serde_json::Value = serde_json::from_str(&call.function.arguments)
                    .unwrap_or(serde_json::Value::Null);
                let target = arguments
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                match call.function.name.as_str() {
                    "list_dir" => locals.push((i, Local::ListDir(target))),
                    "read_file" => locals.push((i, Local::ReadFile(target))),
                    "task" if scope_at.is_none() => scope_at = Some(i),
                    "task" => refusals.push((
                        i,
                        "Error: only one `task` call per turn. This one did not run.".to_string(),
                    )),
                    other => {
                        refusals.push((i, format!("Error: there is no tool called `{other}`.")))
                    }
                }
            }

            // The local tools run first, so a forked child's context can
            // carry their results alongside its own assignment and stay a
            // valid transcript.
            let mut results: Vec<Option<Message>> = vec![None; calls.len()];
            for (i, local) in &locals {
                let (label, answer) = match local {
                    Local::ListDir(target) => {
                        (format!("list_dir({target})"), run.limb.list_dir(target))
                    }
                    Local::ReadFile(target) => {
                        (format!("read_file({target})"), run.limb.read_file(target))
                    }
                };
                run.face.line(&path, &format!("tool {label}"));
                results[*i] = Some(Message::tool_result(&calls[*i].id, &answer));
            }
            for (i, refusal) in &refusals {
                results[*i] = Some(Message::tool_result(&calls[*i].id, refusal));
            }

            // Recorded with their results: a read that failed is not a read.
            run.note(index, |record| {
                for (i, call) in calls.iter().enumerate() {
                    record.tool_calls.push(ToolCallRecord {
                        name: call.function.name.clone(),
                        arguments: call.function.arguments.clone(),
                        result: results[i]
                            .as_ref()
                            .and_then(|message| message.content.clone()),
                    });
                }
            });

            if let Some(i) = scope_at {
                let turn = ParentTurn {
                    messages: messages.clone(),
                    calls: calls.clone(),
                    call_index: i,
                    results: results.clone(),
                };
                match scope(&run, &path, depth, index, turn).await {
                    Ok(text) => {
                        run.note(index, |record| {
                            if let Some(call) = record
                                .tool_calls
                                .iter_mut()
                                .rev()
                                .find(|call| call.name == "task" && call.result.is_none())
                            {
                                call.result = Some(text.clone());
                            }
                        });
                        results[i] = Some(Message::tool_result(&calls[i].id, &text));
                    }
                    Err(outcome) => return end(&run, outcome, handoff, messages),
                }
            }
            for result in results {
                messages.push(result.expect(
                    "the turn's calls were classified as locals, refusals and at most one scope, and every one of those was answered",
                ));
            }
        }
    })
}

/// The parent's turn, frozen at the moment it called `task`: its messages
/// through that assistant turn, the calls that turn made, and the results of
/// the ones that are not the scope. A forked child's context is cut from
/// exactly this.
struct ParentTurn {
    messages: Vec<Message>,
    calls: Vec<ToolCall>,
    call_index: usize,
    results: Vec<Option<Message>>,
}

enum Local {
    ListDir(String),
    ReadFile(String),
}

struct ChildSpec {
    name: String,
    task: String,
    after: Vec<String>,
    fresh: bool,
    raw: serde_json::Value,
}

struct ChildReport {
    outcome: Outcome,
    handoff: String,
}

/// A `task` call. Returns the one tool result the parent is resumed with, or
/// the outcome that stops the parent from being resumed at all.
async fn scope(
    run: &Arc<Run>,
    parent_path: &str,
    depth: usize,
    parent_index: usize,
    turn: ParentTurn,
) -> Result<String, Outcome> {
    if depth + 1 > run.config.max_depth {
        run.face
            .line(parent_path, "task refused: at the depth limit");
        return Ok(framing::depth_limit_error(run.config.max_depth));
    }
    let specs = match parse_children(&turn.calls[turn.call_index].function.arguments) {
        Ok(specs) => specs,
        Err(error) => {
            run.face
                .line(parent_path, &format!("task rejected: {error}"));
            return Ok(format!("Error: {error}"));
        }
    };

    let names: Vec<String> = specs.iter().map(|spec| spec.name.clone()).collect();
    run.face.line(
        parent_path,
        &format!("scope opened: {} — suspended", names.join(", ")),
    );
    run.note(parent_index, |record| record.state = AgentState::Suspended);

    let turn = Arc::new(turn);

    let mut senders = HashMap::new();
    let mut receivers: HashMap<String, watch::Receiver<Option<Arc<ChildReport>>>> = HashMap::new();
    for spec in &specs {
        let (tx, rx) = watch::channel(None);
        senders.insert(spec.name.clone(), tx);
        receivers.insert(spec.name.clone(), rx);
    }

    let mut handles = Vec::new();
    for spec in &specs {
        let child_path = format!("{parent_path} › {}", spec.name);
        let fresh = run.config.framing.mode.resolve(spec.fresh);
        let siblings: Vec<String> = names
            .iter()
            .filter(|name| *name != &spec.name)
            .cloned()
            .collect();
        let child_index = run.register(
            &child_path,
            depth + 1,
            fresh,
            Some(spec.task.clone()),
            Some(parent_path),
        );
        let dependencies: Vec<(String, watch::Receiver<Option<Arc<ChildReport>>>)> = spec
            .after
            .iter()
            .map(|name| (name.clone(), receivers[name].clone()))
            .collect();
        let sender = senders.remove(&spec.name).unwrap();

        let run = run.clone();
        let turn = turn.clone();
        let name = spec.name.clone();
        let task = spec.task.clone();
        let raw = spec.raw.clone();
        handles.push(tokio::spawn(async move {
            let mut dependency_reports = Vec::new();
            for (dependency, mut receiver) in dependencies {
                let report = match receiver.wait_for(|value| value.is_some()).await {
                    Ok(guard) => guard.clone().expect("dependency reported"),
                    Err(_) => Arc::new(ChildReport {
                        outcome: Outcome::Cancelled,
                        handoff: String::new(),
                    }),
                };
                dependency_reports.push((dependency, report.handoff.clone()));
            }
            if run.cancel.is_cancelled() {
                run.note(child_index, |record| {
                    record.state = AgentState::Ended(Outcome::Cancelled)
                });
                let report = Arc::new(ChildReport {
                    outcome: Outcome::Cancelled,
                    handoff: String::new(),
                });
                let _ = sender.send(Some(report.clone()));
                return report;
            }
            let assignment = framing::assignment(
                run.config.framing.words,
                &name,
                &task,
                &siblings,
                &dependency_reports,
            );
            let messages = child_context(run.config.framing.cut, fresh, &turn, &raw, &assignment);
            run.note(child_index, |record| {
                record.assignment = Some(assignment.clone())
            });
            let end = run_agent(run.clone(), child_path, depth + 1, child_index, messages).await;
            let report = Arc::new(ChildReport {
                outcome: end.outcome,
                handoff: end.handoff,
            });
            let _ = sender.send(Some(report.clone()));
            report
        }));
    }

    let mut reports = Vec::new();
    let mut blocked = false;
    for (spec, handle) in specs.iter().zip(handles) {
        let report = match handle.await {
            Ok(report) => report,
            Err(error) => {
                // A panicked agent still ends with a recorded outcome.
                let child_path = format!("{parent_path} › {}", spec.name);
                let reason = error.to_string();
                run.record_panic(&child_path, &reason);
                Arc::new(ChildReport {
                    outcome: Outcome::Panicked(reason),
                    handoff: String::new(),
                })
            }
        };
        if report.outcome.blocks_the_parent() {
            blocked = true;
        }
        reports.push((
            spec.name.clone(),
            report.outcome.short().to_string(),
            report.handoff.clone(),
        ));
    }

    if blocked {
        run.face.line(
            parent_path,
            "scope did not return; this agent stays suspended",
        );
        return Err(Outcome::Suspended);
    }
    run.face.line(parent_path, "scope returned — resuming");
    run.note(parent_index, |record| record.state = AgentState::Running);
    Ok(framing::scope_result(&reports))
}

fn parse_children(arguments: &str) -> Result<Vec<ChildSpec>, String> {
    let value: serde_json::Value = serde_json::from_str(arguments)
        .map_err(|e| format!("the arguments were not valid JSON: {e}"))?;
    let entries = value
        .get("agents")
        .and_then(|v| v.as_array())
        .ok_or("`agents` must be an array")?;
    if entries.is_empty() {
        return Err("`agents` was empty; name at least one agent".to_string());
    }
    let mut specs = Vec::new();
    let mut seen = HashSet::new();
    for entry in entries {
        let name = entry
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("every agent needs a `name`")?
            .trim()
            .to_string();
        if name.is_empty() {
            return Err("an agent had an empty `name`".to_string());
        }
        if !seen.insert(name.clone()) {
            return Err(format!("two agents were both named `{name}`"));
        }
        let task = entry
            .get("task")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("agent `{name}` needs a `task`"))?
            .to_string();
        let after = match entry.get("after") {
            None | Some(serde_json::Value::Null) => Vec::new(),
            Some(serde_json::Value::Array(items)) => items
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(|s| s.to_string())
                        .ok_or_else(|| format!("agent `{name}`: `after` must be a list of names"))
                })
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => return Err(format!("agent `{name}`: `after` must be a list of names")),
        };
        specs.push(ChildSpec {
            name,
            task,
            after,
            fresh: entry
                .get("fresh")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            raw: entry.clone(),
        });
    }
    let names: HashSet<&String> = specs.iter().map(|spec| &spec.name).collect();
    for spec in &specs {
        for dependency in &spec.after {
            if dependency == &spec.name {
                return Err(format!(
                    "agent `{}` was told to start after itself",
                    spec.name
                ));
            }
            if !names.contains(dependency) {
                return Err(format!(
                    "agent `{}` was told to start after `{dependency}`, which is not in this call",
                    spec.name
                ));
            }
        }
    }
    check_acyclic(&specs)?;
    Ok(specs)
}

fn check_acyclic(specs: &[ChildSpec]) -> Result<(), String> {
    let mut pending: HashMap<&str, HashSet<&str>> = specs
        .iter()
        .map(|spec| {
            (
                spec.name.as_str(),
                spec.after.iter().map(|s| s.as_str()).collect(),
            )
        })
        .collect();
    while !pending.is_empty() {
        let ready: Vec<&str> = pending
            .iter()
            .filter(|(_, deps)| deps.is_empty())
            .map(|(name, _)| *name)
            .collect();
        if ready.is_empty() {
            let mut stuck: Vec<&str> = pending.keys().copied().collect();
            stuck.sort();
            return Err(format!(
                "these agents wait on each other in a cycle: {}",
                stuck.join(", ")
            ));
        }
        for name in ready {
            pending.remove(name);
            for deps in pending.values_mut() {
                deps.remove(name);
            }
        }
    }
    Ok(())
}

/// The child's context. `full` and `own` are the fork: the parent's messages
/// byte for byte, through the assistant turn that called `task`, then the
/// tool results for that turn with this child's assignment in the `task`
/// slot. `own` differs only in that the child's copy of the `task` arguments
/// holds its own entry alone — which costs no extra cache, because every
/// child writes that turn afresh anyway.
fn child_context(
    cut: Cut,
    fresh: bool,
    turn: &ParentTurn,
    raw_entry: &serde_json::Value,
    assignment: &str,
) -> Vec<Message> {
    if fresh {
        return vec![turn.messages[0].clone(), Message::new("user", assignment)];
    }
    let turn_index = turn.messages.len() - 1;
    if cut == Cut::Before {
        let mut messages = turn.messages[..turn_index].to_vec();
        messages.push(Message::new("user", assignment));
        return messages;
    }
    let mut messages = turn.messages.clone();
    if cut == Cut::Own {
        let assistant = &mut messages[turn_index];
        if let Some(tool_calls) = assistant.tool_calls.as_mut() {
            tool_calls[turn.call_index].function.arguments =
                serde_json::json!({ "agents": [raw_entry] }).to_string();
        }
    }
    for (i, call) in turn.calls.iter().enumerate() {
        if i == turn.call_index {
            messages.push(Message::tool_result(&call.id, assignment));
        } else {
            messages.push(
                turn.results[i]
                    .clone()
                    .expect("only the scope's own slot is unanswered while the scope runs"),
            );
        }
    }
    messages
}

/// Each agent's final context, rendered so `diff` between a parent's file and
/// a forked child's file shows only the tail.
pub fn render_context(record: &AgentRecord) -> String {
    let mut text = format!(
        "# {}\n\n{} · {} · {} requests · ${:.4}\n",
        record.path,
        if record.fresh { "fresh" } else { "fork" },
        record.state.short(),
        record.requests,
        record.cost
    );
    for message in &record.messages {
        text.push_str(&format!("\n## {}\n\n", message.role));
        if let Some(id) = &message.tool_call_id {
            text.push_str(&format!("(answering {id})\n\n"));
        }
        if let Some(content) = &message.content
            && !content.is_empty()
        {
            text.push_str(content);
            text.push('\n');
        }
        for call in message.tool_calls.iter().flatten() {
            text.push_str(&format!(
                "\n-> {} {} {}\n",
                call.id, call.function.name, call.function.arguments
            ));
        }
    }
    text
}
