use crate::protocol::{AssistantMessage, Contribution, Outcome};
use crate::provider::{ToolCall, WireMessage};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write as IoWrite};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JournalRecord {
    seq: u64,
    ts_ms: u128,
    #[serde(flatten)]
    entry: StateEntry,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "entry", rename_all = "snake_case")]
enum StateEntry {
    SystemPrompt {
        text: String,
    },
    Contribution {
        name: String,
        contribution: Contribution,
    },
    Transcript {
        item: TranscriptEntry,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TranscriptEntry {
    UserMessage {
        text: String,
        arrived_during_exchange: bool,
    },
    UserActivity {
        path: String,
        bytes: usize,
        head: String,
        arrived_during_exchange: bool,
    },
    TurnEnd,
    CancelRequest,
    RebuildRequest,
    Quit,
    RequestAttempt {
        request_id: u64,
        model: String,
    },
    RequestOutcome {
        request_id: u64,
        outcome: Outcome<AssistantMessage>,
    },
    ToolStarted {
        call: ToolCall,
    },
    ToolOutcome {
        call_id: String,
        outcome: Outcome<String>,
    },
    TurnOutcome {
        outcome: Outcome<()>,
    },
    SessionClosed,
}

#[derive(Clone)]
struct TranscriptRecord {
    seq: u64,
    item: TranscriptEntry,
}

pub struct SessionState {
    journal_path: PathBuf,
    journal: File,
    next_seq: u64,
    system_prompt: String,
    contributions: Vec<(String, Contribution)>,
    transcript: Vec<TranscriptRecord>,
    open_call_ids: Vec<String>,
}

impl SessionState {
    pub fn create(path: PathBuf, system_prompt: String) -> Result<Self, String> {
        let journal = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&path)
            .map_err(|e| format!("failed to create session journal {}: {e}", path.display()))?;
        let mut state = Self {
            journal_path: path,
            journal,
            next_seq: 0,
            system_prompt: String::new(),
            contributions: Vec::new(),
            transcript: Vec::new(),
            open_call_ids: Vec::new(),
        };
        state.append_entry(StateEntry::SystemPrompt {
            text: system_prompt,
        })?;
        Ok(state)
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let file = File::open(path)
            .map_err(|e| format!("failed to open session journal {}: {e}", path.display()))?;
        let mut records = Vec::new();
        for (line_index, line) in BufReader::new(file).lines().enumerate() {
            let line = line.map_err(|e| {
                format!(
                    "failed to read line {} from session journal {}: {e}",
                    line_index + 1,
                    path.display()
                )
            })?;
            let record = serde_json::from_str::<JournalRecord>(&line).map_err(|e| {
                format!(
                    "failed to parse line {} from session journal {}: {e}",
                    line_index + 1,
                    path.display()
                )
            })?;
            records.push(record);
        }
        let journal = OpenOptions::new()
            .append(true)
            .open(path)
            .map_err(|e| format!("failed to reopen session journal {}: {e}", path.display()))?;
        let mut state = Self {
            journal_path: path.to_path_buf(),
            journal,
            next_seq: 0,
            system_prompt: String::new(),
            contributions: Vec::new(),
            transcript: Vec::new(),
            open_call_ids: Vec::new(),
        };
        for record in records {
            if record.seq != state.next_seq {
                return Err(format!(
                    "session journal {} has sequence {} where {} was expected",
                    path.display(),
                    record.seq,
                    state.next_seq
                ));
            }
            state.next_seq += 1;
            state.apply(record.seq, record.entry);
        }
        Ok(state)
    }

    pub fn journal_path(&self) -> PathBuf {
        self.journal_path.clone()
    }

    /// Did the limb record that this call actually began executing?
    /// (Execution facts are the limb's; a call without one is an
    /// unexecuted proposal and must never receive an outcome.)
    pub fn tool_started(&self, call_id: &str) -> bool {
        self.transcript.iter().any(|record| {
            matches!(&record.item,
                TranscriptEntry::ToolStarted { call } if call.id == call_id)
        })
    }

    /// Has any earlier provider response already used this tool-call id?
    /// (The projection's executed/open bookkeeping is keyed by id, so a
    /// reused id would corrupt it; the brain rejects such responses.)
    pub fn knows_tool_call(&self, call_id: &str) -> bool {
        self.transcript.iter().any(|record| match &record.item {
            TranscriptEntry::RequestOutcome {
                outcome: Outcome::Ok { value },
                ..
            } => value.tool_calls.iter().any(|call| call.id == call_id),
            _ => false,
        })
    }

    pub fn append_contribution(
        &mut self,
        name: String,
        contribution: Contribution,
    ) -> Result<(), String> {
        self.append_entry(StateEntry::Contribution { name, contribution })
    }

    pub fn append_user_message(&mut self, text: String) -> Result<(), String> {
        self.append_transcript(TranscriptEntry::UserMessage {
            text,
            arrived_during_exchange: !self.open_call_ids.is_empty(),
        })
    }

    pub fn append_user_activity(
        &mut self,
        path: String,
        bytes: usize,
        head: String,
    ) -> Result<(), String> {
        self.append_transcript(TranscriptEntry::UserActivity {
            path,
            bytes,
            head,
            arrived_during_exchange: !self.open_call_ids.is_empty(),
        })
    }

    pub fn append_turn_end(&mut self) -> Result<(), String> {
        self.append_transcript(TranscriptEntry::TurnEnd)
    }

    pub fn append_cancel_request(&mut self) -> Result<(), String> {
        self.append_transcript(TranscriptEntry::CancelRequest)
    }

    pub fn append_rebuild_request(&mut self) -> Result<(), String> {
        self.append_transcript(TranscriptEntry::RebuildRequest)
    }

    pub fn append_quit(&mut self) -> Result<(), String> {
        self.append_transcript(TranscriptEntry::Quit)
    }

    pub fn append_request_attempt(&mut self, request_id: u64, model: String) -> Result<(), String> {
        self.append_transcript(TranscriptEntry::RequestAttempt { request_id, model })
    }

    pub fn append_request_outcome(
        &mut self,
        request_id: u64,
        outcome: Outcome<AssistantMessage>,
    ) -> Result<(), String> {
        self.append_transcript(TranscriptEntry::RequestOutcome {
            request_id,
            outcome,
        })
    }

    pub fn append_tool_started(&mut self, call: ToolCall) -> Result<(), String> {
        self.append_transcript(TranscriptEntry::ToolStarted { call })
    }

    pub fn append_tool_outcome(
        &mut self,
        call_id: String,
        outcome: Outcome<String>,
    ) -> Result<(), String> {
        self.append_transcript(TranscriptEntry::ToolOutcome { call_id, outcome })
    }

    pub fn append_turn_outcome(&mut self, outcome: Outcome<()>) -> Result<(), String> {
        self.append_transcript(TranscriptEntry::TurnOutcome { outcome })
    }

    pub fn append_session_closed(&mut self) -> Result<(), String> {
        self.append_transcript(TranscriptEntry::SessionClosed)
    }

    pub fn request_parts(&self) -> RequestParts {
        self.project().parts
    }

    pub fn wire_message_count(&self) -> usize {
        self.request_parts().messages.len()
    }

    pub fn dump(&self) -> DumpSnapshot {
        let projection = self.project();
        DumpSnapshot {
            parts: projection.parts,
            entries: projection.entries,
            records: self.transcript.clone(),
            represented: projection.represented,
            annotations: projection.annotations,
            unexecuted: projection.unexecuted,
        }
    }

    fn append_transcript(&mut self, item: TranscriptEntry) -> Result<(), String> {
        self.append_entry(StateEntry::Transcript { item })
    }

    fn append_entry(&mut self, entry: StateEntry) -> Result<(), String> {
        let record = JournalRecord {
            seq: self.next_seq,
            ts_ms: now_ms(),
            entry,
        };
        let mut line = serde_json::to_vec(&record)
            .map_err(|e| format!("failed to serialize session journal record: {e}"))?;
        line.push(b'\n');
        self.journal.write_all(&line).map_err(|e| {
            format!(
                "failed to append session journal {}: {e}",
                self.journal_path.display()
            )
        })?;
        self.journal.flush().map_err(|e| {
            format!(
                "failed to flush session journal {}: {e}",
                self.journal_path.display()
            )
        })?;
        self.next_seq += 1;
        self.apply(record.seq, record.entry);
        Ok(())
    }

    fn apply(&mut self, seq: u64, entry: StateEntry) {
        match entry {
            StateEntry::SystemPrompt { text } => self.system_prompt = text,
            StateEntry::Contribution { name, contribution } => {
                self.contributions.push((name, contribution));
            }
            StateEntry::Transcript { item } => {
                match &item {
                    TranscriptEntry::RequestOutcome {
                        outcome: Outcome::Ok { value },
                        ..
                    } => {
                        self.open_call_ids
                            .extend(value.tool_calls.iter().map(|call| call.id.clone()));
                    }
                    TranscriptEntry::ToolOutcome { call_id, .. } => {
                        self.open_call_ids.retain(|id| id != call_id);
                    }
                    TranscriptEntry::TurnOutcome { .. } => self.open_call_ids.clear(),
                    _ => {}
                }
                self.transcript.push(TranscriptRecord { seq, item });
            }
        }
    }

    fn project(&self) -> Projection {
        let executed: HashSet<&str> = self
            .transcript
            .iter()
            .filter_map(|record| match &record.item {
                TranscriptEntry::ToolStarted { call } => Some(call.id.as_str()),
                _ => None,
            })
            .collect();
        let mut messages = vec![WireMessage {
            role: "system".to_string(),
            content: Some(self.composed_system()),
            tool_calls: None,
            tool_call_id: None,
        }];
        let mut entries = Vec::new();
        let mut held = Vec::new();
        let mut represented = HashSet::new();
        let mut annotations = HashSet::new();
        let mut unexecuted = Vec::new();
        let mut open = HashSet::new();

        for record in &self.transcript {
            match &record.item {
                TranscriptEntry::UserMessage {
                    text,
                    arrived_during_exchange,
                } => {
                    let entry = ProjectedEntry {
                        seq: record.seq,
                        message: user_message(text.clone()),
                    };
                    represented.insert(record.seq);
                    if *arrived_during_exchange {
                        annotations.insert(record.seq);
                    }
                    if open.is_empty() {
                        entries.push(entry);
                    } else {
                        held.push(entry);
                    }
                }
                TranscriptEntry::UserActivity {
                    path,
                    head,
                    arrived_during_exchange,
                    ..
                } => {
                    let entry = ProjectedEntry {
                        seq: record.seq,
                        message: user_message(format!(
                            "[user activity] opened file {path}; first lines:\n{head}"
                        )),
                    };
                    represented.insert(record.seq);
                    if *arrived_during_exchange {
                        annotations.insert(record.seq);
                    }
                    if open.is_empty() {
                        entries.push(entry);
                    } else {
                        held.push(entry);
                    }
                }
                TranscriptEntry::RequestOutcome {
                    outcome: Outcome::Ok { value },
                    ..
                } => {
                    let calls: Vec<ToolCall> = value
                        .tool_calls
                        .iter()
                        .filter(|call| executed.contains(call.id.as_str()))
                        .cloned()
                        .collect();
                    let omitted: Vec<ToolCall> = value
                        .tool_calls
                        .iter()
                        .filter(|call| !executed.contains(call.id.as_str()))
                        .cloned()
                        .collect();
                    if !omitted.is_empty() {
                        unexecuted.push((record.seq, omitted));
                    }
                    if value.text.as_ref().is_some_and(|text| !text.is_empty()) || !calls.is_empty()
                    {
                        represented.insert(record.seq);
                        open.extend(calls.iter().map(|call| call.id.clone()));
                        entries.push(ProjectedEntry {
                            seq: record.seq,
                            message: WireMessage {
                                role: "assistant".to_string(),
                                content: value.text.clone(),
                                tool_calls: (!calls.is_empty()).then_some(calls),
                                tool_call_id: None,
                            },
                        });
                    }
                }
                TranscriptEntry::ToolOutcome { call_id, outcome }
                    if executed.contains(call_id.as_str()) =>
                {
                    represented.insert(record.seq);
                    entries.push(ProjectedEntry {
                        seq: record.seq,
                        message: WireMessage {
                            role: "tool".to_string(),
                            content: Some(render_tool_outcome(outcome)),
                            tool_calls: None,
                            tool_call_id: Some(call_id.clone()),
                        },
                    });
                    open.remove(call_id);
                    if open.is_empty() {
                        entries.append(&mut held);
                    }
                }
                TranscriptEntry::TurnOutcome { .. } => {
                    open.clear();
                    entries.append(&mut held);
                }
                _ => {}
            }
        }
        entries.append(&mut held);
        messages.extend(entries.iter().map(|entry| entry.message.clone()));
        Projection {
            parts: RequestParts {
                messages,
                tools: self
                    .latest_contributions()
                    .into_iter()
                    .filter_map(|(_, contribution)| match contribution {
                        Contribution::Tool { def } => Some(def.clone()),
                        Contribution::Fact { .. } => None,
                    })
                    .collect(),
            },
            entries,
            represented,
            annotations,
            unexecuted,
        }
    }

    fn latest_contributions(&self) -> Vec<(&str, &Contribution)> {
        let mut latest = Vec::new();
        for (name, contribution) in &self.contributions {
            if let Some((_, existing)) = latest
                .iter_mut()
                .find(|(existing_name, _)| *existing_name == name.as_str())
            {
                *existing = contribution;
            } else {
                latest.push((name.as_str(), contribution));
            }
        }
        latest
    }

    fn composed_system(&self) -> String {
        let mut text = self.system_prompt.clone();
        let facts: Vec<_> = self
            .latest_contributions()
            .into_iter()
            .filter_map(|(name, contribution)| match contribution {
                Contribution::Fact { text } => Some((name, text)),
                Contribution::Tool { .. } => None,
            })
            .collect();
        if !facts.is_empty() {
            text.push_str("\n\n[environment]\n");
            for (name, value) in facts {
                let _ = writeln!(text, "- {name}: {value}");
            }
        }
        text
    }
}

fn user_message(content: String) -> WireMessage {
    WireMessage {
        role: "user".to_string(),
        content: Some(content),
        tool_calls: None,
        tool_call_id: None,
    }
}

fn render_tool_outcome(outcome: &Outcome<String>) -> String {
    match outcome {
        Outcome::Ok { value } => value.clone(),
        Outcome::Err { error } => format!("[tool error] {error}"),
        Outcome::Cancelled { reason } => format!("[tool cancelled] {reason}"),
        Outcome::Panicked { payload } => format!("[tool panicked] {payload}"),
    }
}

#[derive(Clone)]
pub struct RequestParts {
    pub messages: Vec<WireMessage>,
    pub tools: Vec<serde_json::Value>,
}

#[derive(Clone)]
struct ProjectedEntry {
    seq: u64,
    message: WireMessage,
}

struct Projection {
    parts: RequestParts,
    entries: Vec<ProjectedEntry>,
    represented: HashSet<u64>,
    annotations: HashSet<u64>,
    unexecuted: Vec<(u64, Vec<ToolCall>)>,
}

pub struct DumpSnapshot {
    parts: RequestParts,
    entries: Vec<ProjectedEntry>,
    records: Vec<TranscriptRecord>,
    represented: HashSet<u64>,
    annotations: HashSet<u64>,
    unexecuted: Vec<(u64, Vec<ToolCall>)>,
}

impl DumpSnapshot {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("# walking-skeleton context dump — the model view\n\n");
        out.push_str("<!-- Everything in HTML comments is invisible to the model. -->\n\n");
        if let Some(system) = self.parts.messages.first() {
            render_message(&mut out, system);
        }
        if !self.parts.tools.is_empty() {
            out.push_str("## tools\n\n");
            out.push_str("<!-- sent as the request's `tools` field -->\n");
            out.push_str("```json\n");
            out.push_str(&serde_json::to_string_pretty(&self.parts.tools).unwrap_or_default());
            out.push_str("\n```\n\n");
        }

        let mut rendered_comments = HashSet::new();
        for entry in &self.entries {
            for record in self.records.iter().filter(|record| record.seq < entry.seq) {
                self.render_comment(&mut out, record, &mut rendered_comments);
            }
            if self.annotations.contains(&entry.seq) {
                let _ = writeln!(
                    out,
                    "<!-- seq {}: arrived while a tool exchange was open; the model sees it here, after the exchange -->",
                    entry.seq
                );
            }
            render_message(&mut out, &entry.message);
        }
        for record in &self.records {
            self.render_comment(&mut out, record, &mut rendered_comments);
        }
        for (seq, calls) in &self.unexecuted {
            let _ = writeln!(
                out,
                "<!-- seq {seq}: unexecuted tool calls omitted from the model view: {} -->",
                serde_json::to_string(calls).unwrap_or_default()
            );
        }
        out
    }

    fn render_comment(
        &self,
        out: &mut String,
        record: &TranscriptRecord,
        rendered: &mut HashSet<u64>,
    ) {
        if self.represented.contains(&record.seq) || !rendered.insert(record.seq) {
            return;
        }
        let item = serde_json::to_string(&record.item).unwrap_or_default();
        let _ = writeln!(out, "<!-- seq {}: {} -->", record.seq, item);
    }
}

fn render_message(out: &mut String, message: &WireMessage) {
    match &message.tool_call_id {
        Some(id) => {
            let _ = writeln!(out, "## {} ({id})\n", message.role);
        }
        None => {
            let _ = writeln!(out, "## {}\n", message.role);
        }
    }
    if let Some(content) = &message.content
        && !content.is_empty()
    {
        out.push_str(content);
        out.push_str("\n\n");
    }
    if let Some(calls) = &message.tool_calls {
        out.push_str("```json tool_calls\n");
        out.push_str(&serde_json::to_string_pretty(calls).unwrap_or_default());
        out.push_str("\n```\n\n");
    }
}

pub fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}
