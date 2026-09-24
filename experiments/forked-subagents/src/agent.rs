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
use crate::framing::{self, Cut, Words};
use crate::limb::Limb;
use crate::record::Recorder;
use crate::wire::{self, ChatRequest, Message, ToolCall};

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Instant;
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
    pub cut: Cut,
    pub words: Words,
    pub mode: Mode,
    pub max_cost: f64,
    pub max_depth: usize,
    pub max_turns: usize,
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
}

impl Outcome {
    pub fn label(&self) -> String {
        match self {
            Outcome::Completed => "completed".to_string(),
            Outcome::Cancelled => "cancelled".to_string(),
            Outcome::Faulted(reason) => format!("faulted: {reason}"),
            Outcome::Suspended => "suspended".to_string(),
        }
    }

    pub fn short(&self) -> &'static str {
        match self {
            Outcome::Completed => "completed",
            Outcome::Cancelled => "cancelled",
            Outcome::Faulted(_) => "faulted",
            Outcome::Suspended => "suspended",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ToolCallRecord {
    pub name: String,
    pub arguments: String,
}

#[derive(Clone)]
pub struct AgentRecord {
    pub path: String,
    pub depth: usize,
    pub fresh: bool,
    pub state: String,
    pub requests: usize,
    pub cached_in: u64,
    pub written_in: u64,
    pub uncached_in: u64,
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
    pub fn reads(&self) -> Vec<String> {
        self.tool_calls
            .iter()
            .filter(|call| call.name == "read_file")
            .filter_map(|call| {
                serde_json::from_str::<serde_json::Value>(&call.arguments)
                    .ok()?
                    .get("path")?
                    .as_str()
                    .map(|s| s.to_string())
            })
            .collect()
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "path": self.path,
            "depth": self.depth,
            "fresh": self.fresh,
            "state": self.state,
            "requests": self.requests,
            "cached_in": self.cached_in,
            "written_in": self.written_in,
            "uncached_in": self.uncached_in,
            "out": self.out,
            "cost": self.cost,
            "millis": self.millis,
            "children": self.children,
            "tool_calls": self.tool_calls.iter().map(|c| serde_json::json!({
                "name": c.name, "arguments": c.arguments
            })).collect::<Vec<_>>(),
            "task": self.task,
            "assignment": self.assignment,
            "handoff": self.handoff,
        })
    }
}

struct RunState {
    spent: f64,
    fault: Option<String>,
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
            config,
            face,
            recorder,
            cancel,
            client: reqwest::Client::new(),
            limb,
            session_id,
            state: Mutex::new(RunState {
                spent: 0.0,
                fault: None,
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
        let mut state = self.state.lock().unwrap();
        let index = state.agents.len();
        state.agents.push(AgentRecord {
            path: path.to_string(),
            depth,
            fresh,
            state: "waiting".to_string(),
            requests: 0,
            cached_in: 0,
            written_in: 0,
            uncached_in: 0,
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
        if let Some(parent) = parent
            && let Some(&parent_index) = state.index.get(parent)
        {
            state.agents[parent_index].children.push(path.to_string());
        }
        index
    }

    fn note(&self, index: usize, f: impl FnOnce(&mut AgentRecord)) {
        let mut state = self.state.lock().unwrap();
        f(&mut state.agents[index]);
    }

    pub fn fault(&self) -> Option<String> {
        self.state.lock().unwrap().fault.clone()
    }

    /// An out-of-band failure stops the whole run: nothing new starts
    /// anywhere, and every ancestor of the faulting agent stays suspended.
    fn raise_fault(&self, path: &str, reason: &str) {
        {
            let mut state = self.state.lock().unwrap();
            if state.fault.is_none() {
                state.fault = Some(format!("{path}: {reason}"));
            }
        }
        self.face.say(&format!("{path:<28} FAULT: {reason}"));
        self.cancel.cancel();
    }

    pub fn agents(&self) -> Vec<AgentRecord> {
        self.state.lock().unwrap().agents.clone()
    }

    pub fn total_cost(&self) -> f64 {
        self.state.lock().unwrap().spent
    }

    async fn request(
        &self,
        path: &str,
        index: usize,
        messages: &[Message],
    ) -> Result<Message, String> {
        {
            let state = self.state.lock().unwrap();
            if state.spent >= self.config.max_cost {
                return Err(format!(
                    "spend cap reached: ${:.4} of ${:.4}",
                    state.spent, self.config.max_cost
                ));
            }
        }
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
        let body = serde_json::to_value(&request).map_err(|e| e.to_string())?;
        self.recorder.wire(path, "request", &body);
        self.face.line(path, "request sent");
        let sent = wire::send(
            &self.client,
            &self.config.base_url,
            self.config.api_key.as_deref(),
            &request,
        )
        .await
        .map_err(|fault| fault.0)?;
        self.recorder.wire(path, "response", &sent.body);
        let usage = sent.response.usage.clone();
        let uncached = usage
            .prompt_tokens
            .saturating_sub(usage.prompt_tokens_details.cached_tokens);
        {
            let mut state = self.state.lock().unwrap();
            state.spent += usage.cost;
            let record = &mut state.agents[index];
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
        Ok(sent.response.choices.into_iter().next().unwrap().message)
    }

    pub fn snapshot(&self) -> String {
        let agents = self.agents();
        let mut text = String::from(
            "agent                            state      reqs   cached  written  uncached      out      cost     time\n",
        );
        let mut totals = (0usize, 0u64, 0u64, 0u64, 0u64, 0.0f64);
        for agent in &agents {
            let name = format!(
                "{}{}",
                "  ".repeat(agent.depth),
                agent.path.rsplit(" › ").next().unwrap_or(&agent.path)
            );
            text.push_str(&format!(
                "{:<32} {:<10} {:>4} {:>8} {:>8} {:>9} {:>8} {:>9} {:>7.1}s\n",
                name,
                agent.state,
                agent.requests,
                agent.cached_in,
                agent.written_in,
                agent.uncached_in,
                agent.out,
                format!("${:.4}", agent.cost),
                agent.millis as f64 / 1000.0,
            ));
            totals.0 += agent.requests;
            totals.1 += agent.cached_in;
            totals.2 += agent.written_in;
            totals.3 += agent.uncached_in;
            totals.4 += agent.out;
            totals.5 += agent.cost;
        }
        text.push_str(&format!(
            "{:<32} {:<10} {:>4} {:>8} {:>8} {:>9} {:>8} {:>9}\n",
            "TOTAL",
            "",
            totals.0,
            totals.1,
            totals.2,
            totals.3,
            totals.4,
            format!("${:.4}", totals.5),
        ));
        let cache_share = if totals.1 + totals.3 > 0 {
            totals.1 as f64 / (totals.1 + totals.3) as f64
        } else {
            0.0
        };
        text.push_str(&format!(
            "cache-read share of input: {:.1}%\n",
            cache_share * 100.0
        ));
        text
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
        run.note(index, |record| record.state = "running".to_string());
        let mut messages = messages;
        let mut handoff = String::new();
        let mut turns = 0usize;

        let end = |run: &Arc<Run>, outcome: Outcome, handoff: String, messages: Vec<Message>| {
            run.note(index, |record| {
                record.state = outcome.short().to_string();
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
                let reason = format!("agent ran past {} turns", run.config.max_turns);
                run.raise_fault(&path, &reason);
                return end(&run, Outcome::Faulted(reason), handoff, messages);
            }

            let reply = match run.request(&path, index, &messages).await {
                Ok(reply) => reply,
                Err(reason) => {
                    run.raise_fault(&path, &reason);
                    return end(&run, Outcome::Faulted(reason), handoff, messages);
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
            run.note(index, |record| {
                for call in &calls {
                    record.tool_calls.push(ToolCallRecord {
                        name: call.function.name.clone(),
                        arguments: call.function.arguments.clone(),
                    });
                }
            });
            if run.cancel.is_cancelled() {
                return end(&run, Outcome::Cancelled, handoff, messages);
            }

            // The local tools run first, so a forked child's context can
            // carry their results alongside its own assignment and stay a
            // valid transcript.
            let mut results: Vec<Option<Message>> = vec![None; calls.len()];
            let mut task_index: Option<usize> = None;
            for (i, call) in calls.iter().enumerate() {
                let arguments: serde_json::Value = serde_json::from_str(&call.function.arguments)
                    .unwrap_or(serde_json::Value::Null);
                let argument = |key: &str| {
                    arguments
                        .get(key)
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                };
                match call.function.name.as_str() {
                    "list_dir" => {
                        let target = argument("path");
                        run.face.line(&path, &format!("tool list_dir({target})"));
                        results[i] =
                            Some(Message::tool_result(&call.id, &run.limb.list_dir(&target)));
                    }
                    "read_file" => {
                        let target = argument("path");
                        run.face.line(&path, &format!("tool read_file({target})"));
                        results[i] =
                            Some(Message::tool_result(&call.id, &run.limb.read_file(&target)));
                    }
                    "task" if task_index.is_none() => task_index = Some(i),
                    "task" => {
                        results[i] = Some(Message::tool_result(
                            &call.id,
                            "Error: only one `task` call per turn. This one did not run.",
                        ));
                    }
                    other => {
                        results[i] = Some(Message::tool_result(
                            &call.id,
                            &format!("Error: there is no tool called `{other}`."),
                        ));
                    }
                }
            }

            if let Some(i) = task_index {
                let turn = ParentTurn {
                    messages: messages.clone(),
                    calls: calls.clone(),
                    call_index: i,
                    results: results.clone(),
                };
                match scope(&run, &path, depth, index, turn).await {
                    Ok(text) => results[i] = Some(Message::tool_result(&calls[i].id, &text)),
                    Err(outcome) => return end(&run, outcome, handoff, messages),
                }
            }
            for result in results {
                messages.push(result.expect("every tool call answered"));
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
    run.note(parent_index, |record| {
        record.state = "suspended".to_string()
    });

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
        let fresh = run.config.mode.resolve(spec.fresh);
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
                run.note(child_index, |record| record.state = "cancelled".to_string());
                let report = Arc::new(ChildReport {
                    outcome: Outcome::Cancelled,
                    handoff: String::new(),
                });
                let _ = sender.send(Some(report.clone()));
                return report;
            }
            let assignment = framing::assignment(
                run.config.words,
                &name,
                &task,
                &siblings,
                &dependency_reports,
            );
            let messages = child_context(run.config.cut, fresh, &turn, &raw, &assignment);
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
                let reason = format!("child agent task panicked: {error}");
                run.raise_fault(&format!("{parent_path} › {}", spec.name), &reason);
                Arc::new(ChildReport {
                    outcome: Outcome::Faulted(reason),
                    handoff: String::new(),
                })
            }
        };
        if matches!(report.outcome, Outcome::Faulted(_) | Outcome::Suspended) {
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
    run.note(parent_index, |record| record.state = "running".to_string());
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
                    .expect("tool call answered before the scope"),
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
        record.state,
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
