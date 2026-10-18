//! What a request renders, and the wording of everything the harness says to
//! a model. Policy: the core fixes what must be rendered (every result of
//! the last round) and leaves the rest here.

use harness_core::call::{Caller, Outcome, Output, Owner};
use harness_core::context::{Entry, EntryId, Note, Rendered, Role, Root, Span};
use harness_core::delivery::{Content, Delivery, Prompt, Read};
use harness_core::environment::{Binding, Description};
use harness_core::fact::Author;
use harness_core::id::{CallId, Digest, LimbId, Timestamp, ValueId};
use harness_core::model_call::{Ending, Stop};
use harness_core::resource::Observation;
use harness_core::state::{Delivered, Stale, State};
use harness_core::task::{Abandoner, Draining, Input, Origin, Scope, Task};
use harness_core::value::Bytes;
use serde_json::{Value, json};

/// The bytes of a stored value.
pub type Values<'a> = &'a dyn Fn(ValueId) -> Vec<u8>;

/// Where Max's words fall in the agent's work: during a turn, the tool
/// calls it made after the brain time his face had shown him, and
/// whether he asked them to wait for its turn's end.
fn when(state: &State, task: &Task, delivered: &Delivered, mid_turn: bool) -> String {
    let (Author::Max, Some(seen)) = (delivered.by, delivered.seen) else {
        return String::new();
    };
    let mut unseen: Vec<_> = state
        .calls()
        .filter_map(|(_, c)| match &c.call.caller {
            Caller::Agent { task: t, tool_use } if *t == task.id && c.made.seq.n > seen.n => {
                Some((c.made.seq, tool_use.0.as_str()))
            }
            _ => None,
        })
        .collect();
    unseen.sort();
    let mut when = match unseen.as_slice() {
        [] if mid_turn => " while you were working".to_string(),
        [] => String::new(),
        [(_, one)] => format!(" while you were working, before your tool call `{one}`"),
        [(_, first), ..] => format!(
            " while you were working, before your last {} tool calls (from `{first}` on)",
            unseen.len()
        ),
    };
    if delivered.delivery.read == Read::TurnEnd {
        when.push_str(", asking that it wait for the end of your turn");
    }
    when
}

/// A new root in an environment: the identity, the role, then each limb's
/// instructions in `system`; the tools its binding routes.
pub struct NewRoot {
    pub root: Root,
    pub binding: Binding,
    /// The versions the root embodies.
    pub observes: Vec<Observation>,
    /// The bytes of `root.system` and `root.tools`.
    pub values: Vec<Bytes>,
}

pub fn root(
    identity: &str,
    model: &str,
    role: &str,
    limbs: &[(LimbId, &Description)],
) -> Result<NewRoot, String> {
    let mut system = vec![text(identity), text(role)];
    let mut observes = Vec::new();
    for (_, description) in limbs {
        for instruction in &description.instructions {
            system.push(text(&instruction.text));
            observes.push(instruction.observes.clone());
        }
    }
    let (binding, tools) =
        Binding::new(limbs).map_err(|tool| format!("two limbs offer the tool `{tool}`"))?;
    let system = blocks(&system);
    let tools = Bytes::from(tools);
    Ok(NewRoot {
        root: Root {
            model: model.into(),
            system: system.id(),
            tools: tools.id(),
        },
        binding,
        observes,
        values: vec![system, tools],
    })
}

/// What every fresh member of a spawn shares, as its first message: the
/// brief, then Max's quoted words verbatim, each under its label. The same
/// bytes for every member, so the same entry and one cache prefix.
pub fn brief(said: Option<&str>, quotes: &[(ValueId, Vec<String>)]) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(said) = said {
        out.push(text(&format!(
            "[harness] The brief every agent started with you was given:\n\n{said}"
        )));
    }
    for (id, words) in quotes {
        out.push(text(&format!(
            "[harness] Max's own words, quoted verbatim (`{}`):",
            label(*id)
        )));
        out.extend(words.iter().cloned());
    }
    out
}

/// How an agent names Max's words to quote them in a brief.
pub fn label(words: ValueId) -> String {
    format!("max-{}", &words.to_string()[..8])
}

/// Max's words as content blocks: his messages and answers are blocks
/// already; a mandate he gave is plain text.
pub fn words(bytes: &[u8]) -> Vec<String> {
    match serde_json::from_slice::<Vec<Value>>(bytes) {
        Ok(blocks) => blocks.iter().map(Value::to_string).collect(),
        Err(_) => vec![text(&String::from_utf8_lossy(bytes))],
    }
}

/// The next request's new content: the blocks of the user message that
/// carries it (empty when there is nothing new), and what each run of them
/// was rendered from.
#[derive(Default)]
pub struct Turn {
    pub renders: Vec<Span>,
    pub blocks: Vec<String>,
}

impl Turn {
    fn push(&mut self, from: Rendered, blocks: Vec<String>) {
        self.renders.push(Span {
            from,
            blocks: blocks.len(),
        });
        self.blocks.extend(blocks);
    }

    fn rendered(&self) -> impl Iterator<Item = &Rendered> {
        self.renders.iter().map(|s| &s.from)
    }

    fn extend(&mut self, other: Turn) {
        self.renders.extend(other.renders);
        self.blocks.extend(other.blocks);
    }
}

/// When a request is rendered: the local time as the agent is told it, and
/// how many tokens the task's model holds.
pub struct Now<'a> {
    pub time: &'a str,
    pub window: u64,
}

/// Everything the task's next request includes, in the order the provider
/// needs: tool results first, then text. `forked` are the calls of the
/// response a fork's first request answers.
pub fn turn(
    state: &State,
    values: Values,
    task: &Task,
    first: bool,
    forked: &[CallId],
    now: &Now,
) -> Turn {
    let mut turn = Turn::default();
    let mut texts = Turn::default();

    // A fork's first message answers every tool use of the forked response
    // with a neutral result, and gives its mandate as user text.
    for call in forked {
        turn.push(
            Rendered::Note(Note::Elsewhere { call: *call }),
            vec![elsewhere(state, values, task, *call)],
        );
    }
    // Does this request carry on the model's turn (a tool round's results,
    // or a response cut short) rather than start a new one?
    // A carry carries on the turn of the compaction it ends.
    let mid_turn = !forked.is_empty()
        || task.pending.iter().any(|p| matches!(p, Input::Result(_)))
        || task.head().is_some_and(|h| state.compacted(h).is_some())
        || matches!(
            task.head()
                .and_then(|h| state.response(h))
                .and_then(|m| m.ended.as_ref())
                .map(|e| &e.ending),
            Some(Ending::Responded {
                stop: Stop::MaxTokens | Stop::PauseTurn,
                ..
            })
        );
    // Max moved the context here, off a path that changed the world.
    if let Some(from) = task.context().and_then(|c| c.moved) {
        texts.push(
            Rendered::Note(Note::Moved { from }),
            vec![text(&moved(state, task, from))],
        );
    }
    for pending in &task.pending {
        match *pending {
            Input::Result(call) => {
                turn.push(
                    Rendered::Call(call),
                    vec![result(state, values, task, call)],
                );
            }
            Input::Background(call) | Input::Late(call) | Input::MaxAction(call) => {
                texts.push(Rendered::Call(call), vec![activity(state, values, call)]);
            }
            Input::Passed(call) => {
                texts.push(
                    Rendered::Note(Note::Passed { call }),
                    vec![passed(state, task, call)],
                );
            }
            Input::Delivery(id) => {
                let Some(delivered) = state.delivery(id) else {
                    continue;
                };
                let Delivery { content, read, .. } = &delivered.delivery;
                if *read == Read::TurnEnd && mid_turn {
                    continue;
                }
                let mut said = Vec::new();
                // Max's words are labelled, so a brief can quote them, and
                // say where in the agent's work he wrote them.
                if let Content::Words(words) = content {
                    said.push(text(&format!(
                        "[harness] Max wrote this{} (`{}` quotes it in a brief):",
                        when(state, task, delivered, mid_turn),
                        label(*words)
                    )));
                }
                said.extend(delivery(values, task, content, delivered.at.at));
                texts.push(Rendered::Delivery(id), said);
            }
        }
    }

    let stale: Vec<_> = state
        .stale(task)
        .into_iter()
        .filter(|s| !observed_now(state, turn.rendered().chain(texts.rendered()), s))
        .collect();
    if !stale.is_empty() {
        // A role and a limb's instructions are in the root: they change
        // only when a fresh root is made.
        let in_root = |s: &&Stale| s.resource.kind == "role" || s.resource.kind == "instructions";
        let (rooted, names): (Vec<_>, Vec<_>) = stale.iter().partition(in_root);
        let mut said = Vec::new();
        if rooted.iter().any(|s| s.resource.kind == "role") {
            said.push(text(
                "[harness] Your role's definition changed. Your request parameters follow it now; \
its prompt and tools take effect when you next compact with `fresh: true`.",
            ));
        }
        let limbs: Vec<_> = rooted
            .iter()
            .filter(|s| s.resource.kind == "instructions")
            .map(|s| {
                state
                    .limb(s.resource.limb)
                    .map_or_else(|| s.resource.limb.to_string(), |l| format!("`{}`", l.name))
            })
            .collect();
        if !limbs.is_empty() {
            said.push(text(&format!(
                "[harness] The instructions of {} changed. They take effect when you next compact with `fresh: true`.",
                limbs.join(", ")
            )));
        }
        if !names.is_empty() {
            let names: Vec<_> = names.iter().map(|s| s.resource.name.as_str()).collect();
            said.push(text(&format!(
                "[harness] Changed since you last saw them: {}. Read them again before relying on what you saw.",
                names.join(", ")
            )));
        }
        // What a limb said to show of a change, rather than announce it.
        for s in stale.iter().filter(|s| !in_root(s)) {
            if let Some(content) = s.content {
                said.push(text(&format!(
                    "[harness] {}: {}",
                    s.resource.name,
                    self::said(values, content)
                )));
            }
        }
        let told = stale
            .iter()
            .map(|s| Observation {
                resource: s.resource.clone(),
                version: s.now,
            })
            .collect();
        texts.push(
            Rendered::Note(Note::Stale {
                told,
                shown: Vec::new(),
            }),
            said,
        );
    }

    if first {
        texts.push(
            Rendered::Note(Note::Mandate),
            mandated(state, values, task, false),
        );
    }

    let after_response = matches!(
        task.head().and_then(|h| state.entry(h)),
        Some(Entry::Message { message, .. }) if message.role == Role::Assistant
    );
    if turn.blocks.is_empty() && texts.blocks.is_empty() && after_response {
        // The last response stopped short of ending its turn.
        texts.push(
            Rendered::Note(Note::Continue),
            vec![text("[harness] Your response was cut off. Carry on.")],
        );
    }
    // The clock starts what the harness says; mid-turn, it comes alone
    // only to warn of a context nearly full.
    let (clock, full) = clock(state, task, now);
    if !texts.blocks.is_empty() || full {
        turn.push(Rendered::Note(Note::Now), vec![text(&clock)]);
    }
    turn.extend(texts);
    turn
}

/// The time, and how full the context was at the task's last response,
/// unless a compaction has replaced that context since (the head is its
/// carry, or the report after it); and whether it
/// is full enough to warn the agent to compact ([`crate::policy::WARN_AT`]).
fn clock(state: &State, task: &Task, now: &Now) -> (String, bool) {
    let used = state
        .last_turn(task.id)
        .and_then(|e| e.usage)
        .filter(|_| {
            !task
                .head()
                .is_some_and(|h| state.compacted(h).is_some() || state.reported(h))
        })
        .map(|u| u.input + u.cache_read + u.cache_write_short + u.cache_write_long + u.output);
    let Some(used) = used else {
        return (format!("[harness] {}.", now.time), false);
    };
    let mut said = format!(
        "[harness] {}. Your context: {} of {} tokens.",
        now.time,
        tokens(used),
        tokens(now.window)
    );
    let full = used * 100 >= now.window * crate::policy::WARN_AT;
    if full {
        said.push_str(&format!(
            " It is {}% full: wrap up the step you are on soon, and then compact with `compact_and_continue`, \
or `compact_and_report` if you are done. If it fills, the \
harness compacts it for you, with no summary of yours, and what is only in your context is lost.",
            used * 100 / now.window
        ));
    }
    (said, full)
}

/// A token count, short: `48k`, `1M`.
fn tokens(n: u64) -> String {
    if n >= 1_000_000 && n.is_multiple_of(1_000_000) {
        format!("{}M", n / 1_000_000)
    } else {
        format!("{}k", (n + 500) / 1000)
    }
}

/// The task's mandate as its first request says it, or as a carry onto a
/// fresh root says it again (`carried`).
fn mandated(state: &State, values: Values, task: &Task, carried: bool) -> Vec<String> {
    // Max's words are a block of their own after the harness's framing,
    // so a face shows them as he wrote them.
    let framing = match (task.origin.clone(), task.scope) {
        (Origin::Aside { .. }, _) => Some(format!(
            "[harness] This is an aside: Max forked you to talk while the agents you started work. You are \
told when they are all done. When Max marks this aside done, you summarise what came of the talk, and that \
goes back to the context you were forked from with their results. He wrote (`{}` quotes it in a brief):",
            label(task.mandate.text)
        )),
        (Origin::Resume { .. }, Scope::Max) if task.by.is_none() => Some(format!(
            "[harness] Max has reopened this chat after it finished. He wrote (`{}` quotes it in a brief):",
            label(task.mandate.text)
        )),
        (_, Scope::Max) => Some(format!(
            "[harness] Max wrote this (`{}` quotes it in a brief):",
            label(task.mandate.text)
        )),
        (_, Scope::Spawn(_)) => None,
    };
    match framing {
        Some(framing) => vec![text(&framing), text(&said(values, task.mandate.text))],
        None => vec![text(&mandate(state, values, task, carried))],
    }
}

/// What a compaction carries forward, and why.
pub struct Carry<'a> {
    /// The agent's summary; `None` for a compaction the harness made.
    pub summary: Option<&'a str>,
    /// Who compacted, and onto what.
    pub why: Why,
    /// Max's words from what it replaced ([`kept`]).
    pub words: &'a [ValueId],
    /// Settled calls whose tool uses it replaced: the round's other results.
    pub calls: &'a [CallId],
}

/// Who compacted, and onto what.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// The agent, onto its latest checkpoint.
    Checkpoint,
    /// The agent, onto a fresh root.
    Fresh,
    /// The harness: the context no longer fit the model's window.
    Full,
}

/// A compaction's carry: one user message, in the agent's own story. On a
/// fresh root it opens with the mandate; then the framing and the summary,
/// with the background work the task still owns; then Max's words verbatim;
/// then the outputs of the calls whose tool uses it replaced.
pub fn carry(state: &State, values: Values, task: &Task, carry: &Carry) -> Turn {
    let mut turn = Turn::default();
    if carry.why != Why::Checkpoint {
        let said = mandated(state, values, task, true);
        if let Some(brief) = briefing(values, task) {
            turn.push(Rendered::Note(Note::Brief), brief);
        }
        turn.push(Rendered::Note(Note::Mandate), said);
    }
    let mut framing = match carry.why {
        Why::Checkpoint => "[harness] You compacted your work after this point.".to_owned(),
        Why::Fresh => "[harness] You compacted your work into this new context.".to_owned(),
        Why::Full => "[harness] Your context no longer fit the model's window, so the harness started it \
again from your role and your mandate, without a summary. Check your workspace for what you had already done before \
carrying on."
            .to_owned(),
    };
    if !carry.words.is_empty() {
        framing.push_str(" Max's words from that work follow, as he wrote them.");
    }
    if !carry.calls.is_empty() {
        framing.push_str(" Then the results of your calls that had not yet reached you.");
    }
    let mut said = vec![text(&framing)];
    if let Some(summary) = carry.summary {
        said.push(text(&format!("Your summary:\n\n{summary}")));
    }
    let running: Vec<String> = task
        .background
        .iter()
        .filter_map(|c| state.call(*c))
        .map(|c| match &c.call.caller {
            Caller::Background { name, .. } => format!("`{name}` (`{}`)", c.call.tool),
            _ => format!("`{}`", c.call.tool),
        })
        .collect();
    if !running.is_empty() {
        said.push(text(&format!(
            "[harness] Still running in the background, yours to `stop` or `pass`: {}.",
            running.join(", ")
        )));
    }
    turn.push(Rendered::Note(Note::Compacted), said);
    for words in carry.words {
        let mut said = vec![text(&format!(
            "[harness] Max wrote this (`{}` quotes it in a brief):",
            label(*words)
        ))];
        said.extend(self::words(&values(*words)));
        turn.push(Rendered::Note(Note::Kept { words: *words }), said);
    }
    for call in carry.calls {
        turn.push(
            Rendered::Call(*call),
            carried(state, values, "[harness] Your", *call),
        );
    }
    turn
}

/// The brief and Max's quoted words a fresh agent starts with, if its
/// spawner gave any.
pub fn briefing(values: Values, task: &Task) -> Option<Vec<String>> {
    if !matches!(task.origin, Origin::Fresh { .. })
        || (task.mandate.brief.is_none() && task.mandate.quotes.is_empty())
    {
        return None;
    }
    let said = task
        .mandate
        .brief
        .map(|b| String::from_utf8_lossy(&values(b)).into_owned());
    let quotes: Vec<_> = task
        .mandate
        .quotes
        .iter()
        .map(|q| (*q, words(&values(*q))))
        .collect();
    Some(brief(said.as_deref(), &quotes))
}

/// The output of a call whose tool use a compaction replaced, as blocks
/// headed by what the call was, since a tool result needs its tool use: in
/// the carry, or after the report of a member whose last compaction carried
/// it.
pub fn carried(state: &State, values: Values, whose: &str, call: CallId) -> Vec<String> {
    let record = state.call(call).expect("a carried call is known");
    let outcome = record.outcome().expect("a carried call is settled");
    let (content, error) = outcome_content(values, outcome);
    let mut input = String::from_utf8_lossy(&values(record.call.input)).into_owned();
    if let Some((cut, _)) = input.char_indices().nth(300) {
        input.truncate(cut);
        input.push('…');
    }
    let blocks = content
        .as_array()
        .into_iter()
        .flatten()
        .map(Value::to_string);
    // An `attach` call's output heads each call it attaches already.
    if record.call.tool == crate::meta::ATTACH && !error {
        return blocks.collect();
    }
    let failed = if error { " failed" } else { "" };
    let named = if labelled(&record.call.tool) {
        format!(" (`{}`)", call_label(call))
    } else {
        String::new()
    };
    let mut said = vec![text(&format!(
        "{whose} `{}` call{named} {input}{failed}:",
        record.call.tool
    ))];
    said.extend(blocks);
    said
}

/// How an agent names one of its calls, or one its agents attached, to
/// `attach` it: shown with the call's result.
pub(crate) fn call_label(call: CallId) -> String {
    let digest = Digest::of(call.to_string().as_bytes()).to_string();
    format!("call-{}", &digest[..6])
}

/// Whether a call's result is worth attaching, and so shows its label:
/// every tool's but the meta tools that only manage the agent's own work.
fn labelled(tool: &str) -> bool {
    use crate::meta::*;
    !compacts(tool) && ![ATTACH, BACKGROUND, STOP, PASS, ADD_SIBLING].contains(&tool)
}

/// A call's label as the last block of its result, if it has one.
fn label_block(tool: &str, call: CallId) -> Option<Value> {
    labelled(tool).then(|| json!({ "type": "text", "text": format!("[{}]", call_label(call)) }))
}

/// Max's words in what a compaction replaces, in the order he said them:
/// what the requests on the task's chain after `since` rendered of his
/// (messages and answers), and what compactions there carried. Everything
/// up to `since` is kept, and his words with it; with no `since` (a new
/// root) nothing is, back to the chain's root, through the context a fork
/// or a resumed chat began with.
pub fn kept(state: &State, task: &Task, since: Option<EntryId>) -> Vec<ValueId> {
    let Some(head) = task.head() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut next = Some(head);
    while let Some(id) = next
        && Some(id) != since
    {
        let mut spans: Vec<&Rendered> = Vec::new();
        next = state.entry(id).and_then(Entry::parent);
        if let Some(compacted) = state.compacted(id) {
            spans.extend(compacted.renders.iter().map(|s| &s.from).rev());
        }
        if let Some(request) = state.request(id)
            && request.call.base != id
        {
            spans.extend(request.call.renders.iter().map(|s| &s.from).rev());
            next = Some(request.call.base);
        }
        for from in spans {
            let words = match from {
                Rendered::Delivery(d) => match state.delivery(*d).map(|d| &d.delivery.content) {
                    Some(Content::Words(words)) => Some(*words),
                    _ => None,
                },
                Rendered::Call(c) => state
                    .call(*c)
                    .and_then(|c| c.outcome())
                    .and_then(Outcome::output)
                    .map(|o| o.model)
                    .filter(|m| state.said(*m).is_some()),
                Rendered::Note(Note::Kept { words }) => Some(*words),
                Rendered::Note(_) => None,
            };
            if let Some(words) = words.filter(|w| !found.contains(w)) {
                found.push(words);
            }
        }
    }
    found.reverse();
    found
}

/// What Max moving a task back left standing: what the path it left did
/// to the world, which the move did not undo.
fn moved(state: &State, task: &Task, from: EntryId) -> String {
    let Some(head) = task.head() else {
        return String::new();
    };
    let left = state.left(task.id, from, head);
    let tools = |calls: &[CallId]| {
        let mut names: Vec<_> = calls
            .iter()
            .filter_map(|c| state.call(*c))
            .map(|c| format!("`{}`", c.call.tool))
            .collect();
        names.dedup();
        names.join(", ")
    };
    let mut said = vec![
        "[harness] Max moved this conversation back to here. What followed is gone from your context, \
but what it did to the world stays:"
            .to_string(),
    ];
    if !left.wrote.is_empty() {
        said.push(format!(
            "- files changed by the tool calls {} are as those calls left them; read them again before relying on them",
            tools(&left.wrote)
        ));
    }
    if !left.running.is_empty() {
        said.push(format!(
            "- background work it started still runs: {}",
            tools(&left.running)
        ));
    }
    if !left.started.is_empty() {
        let names: Vec<_> = left
            .started
            .iter()
            .filter_map(|t| state.task(*t))
            .map(|t| format!("`{}`", t.name))
            .collect();
        said.push(format!("- tasks it started: {}", names.join(", ")));
    }
    said.join("\n")
}

/// Whether a stale resource is one a call this request renders observed: a
/// task's own write is not news to it.
fn observed_now<'a>(
    state: &State,
    mut renders: impl Iterator<Item = &'a Rendered>,
    stale: &Stale,
) -> bool {
    renders.any(|r| {
        let Rendered::Call(call) = r else {
            return false;
        };
        state
            .call(*call)
            .and_then(|c| c.outcome())
            .and_then(Outcome::seen)
            .is_some_and(|o| {
                o.observes
                    .iter()
                    .any(|ob| ob.resource == stale.resource && ob.version == stale.now)
            })
    })
}

/// A tool result for one of the task's own tool uses. A compaction call's
/// result reaches the model only when the compaction did not happen: the
/// task is stopping, or Max moved it back to before it.
fn result(state: &State, values: Values, task: &Task, call: CallId) -> String {
    let record = state.call(call).expect("a pending result is a known call");
    let Caller::Agent { tool_use, .. } = &record.call.caller else {
        unreachable!("a round's result answers a tool use")
    };
    let outcome = record.outcome().expect("a pending result is settled");
    let (mut content, error) = outcome_content(values, outcome);
    if crate::meta::compacts(&record.call.tool) && matches!(outcome, Outcome::Completed(_)) {
        let said = if state.has_compacted(call) {
            "Max moved you back to before this compaction, so your context is as it was here. Carry on from here."
        } else if task.ends() {
            "Not compacted: you are being stopped, and this is your last turn. End it with your report as your last message."
        } else {
            "Not compacted."
        };
        content = json!([{ "type": "text", "text": said }]);
    }
    if let (Some(output), Value::Array(blocks)) = (outcome.output(), &mut content)
        && state.said(output.model).is_some()
    {
        blocks.push(json!({ "type": "text", "text": format!("(Max's answer: `{}` quotes it in a brief.)", label(output.model)) }));
    }
    if let (Some(named), Value::Array(blocks)) =
        (label_block(&record.call.tool, call), &mut content)
    {
        blocks.push(named);
    }
    json!({
        "type": "tool_result",
        "tool_use_id": tool_use.0,
        "content": content,
        "is_error": error,
    })
    .to_string()
}

/// A tool result's content, and whether it is an error.
fn outcome_content(values: Values, outcome: &Outcome) -> (Value, bool) {
    let blocks = |id: ValueId| serde_json::from_slice::<Value>(&values(id)).unwrap_or(json!([]));
    let said = |s: &str| json!([{ "type": "text", "text": s }]);
    match outcome {
        Outcome::Completed(output) => (blocks(output.model), false),
        Outcome::Failed(output) => (blocks(output.model), true),
        Outcome::Refused { current, shown } => {
            let names: Vec<_> = current.iter().map(|o| o.resource.name.as_str()).collect();
            let why = format!(
                "Refused, with no effect: {} changed since you last saw it, or you had not seen it through `read`, `write` or `edit` (a command's output does not count).",
                names.join(", ")
            );
            let Some(shown) = shown else {
                return (said(&format!("{why} Read it, then retry.")), true);
            };
            let mut content = said(&format!(
                "{why} As it is now, which counts as having read it: retry if your change still applies."
            ));
            if let (Value::Array(items), Value::Array(more)) = (&mut content, blocks(shown.model)) {
                items.extend(more);
            }
            (content, true)
        }
        Outcome::Interrupted { partial } => {
            let mut content = partial.as_ref().map_or(json!([]), |o| blocks(o.model));
            if let Value::Array(items) = &mut content {
                items.push(json!({ "type": "text", "text": "[interrupted; it may have had partial effects]" }));
            }
            (content, true)
        }
        Outcome::Unknown => (
            said(
                "The outcome of this call is unknown: the harness stopped while it was running, and this tool is \
                 not safe to run again blind. It may or may not have had its effects. Check before relying on either.",
            ),
            true,
        ),
    }
}

/// A call outside the round that has settled: the late outcome of a call
/// earlier said to be unknown, or something Max did in the chat.
fn activity(state: &State, values: Values, call: CallId) -> String {
    let Some(record) = state.call(call) else {
        return text("[harness] A call you made has settled.");
    };
    let Some(outcome) = record.outcome() else {
        return text("[harness] A call you made has settled.");
    };
    let (content, _) = outcome_content(values, outcome);
    let said: Vec<_> = content
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|b| b["text"].as_str())
        .collect();
    let what = match record.call.caller {
        Caller::Max { .. } => format!("Max ran `{}` here", record.call.tool),
        Caller::Background { .. } => format!(
            "Your background `{}` call (`{}`) has settled",
            record.call.tool,
            call_label(call)
        ),
        _ => format!(
            "Your earlier `{}` call, whose outcome was unknown, did finish",
            record.call.tool
        ),
    };
    text(&format!("[harness] {what}:\n{}", said.join("\n")))
}

/// A background call passed to `task`: whose it was, why, and that it is
/// the task's own now.
fn passed(state: &State, task: &Task, call: CallId) -> String {
    let Some(record) = state.call(call) else {
        return text("[harness] Background work was passed to you.");
    };
    let name = match &record.call.caller {
        Caller::Background { name, .. } => name.as_str(),
        _ => "",
    };
    let handed = record
        .passed
        .iter()
        .rev()
        .find(|h| h.to == Owner::Task(task.id));
    let (by, why) = handed.map_or(("an agent you started", ""), |h| {
        (
            state
                .task(h.by)
                .map_or("an agent you started", |t| t.name.as_str()),
            h.why.as_str(),
        )
    });
    text(&format!(
        "[harness] `{by}` passed you its background `{}` call `{name}`, which is still running: {why}\n\
It is yours now: its result comes to you as a message when it settles, and you cannot finish while it runs. \
`stop` it, or `pass` it up in turn.",
        record.call.tool
    ))
}

/// What a delivery made `at` says.
fn delivery(values: Values, task: &Task, content: &Content, at: Timestamp) -> Vec<String> {
    let aside = matches!(task.origin, Origin::Aside { .. });
    // A fork works in the same directory as the agent it was forked from.
    let shared = match task.origin {
        Origin::Fork { .. } | Origin::Aside { .. } => {
            " You share your working directory with the agent you were forked from and its other forks: \
change back only what you changed yourself."
        }
        Origin::Fresh { .. } | Origin::Resume { .. } => "",
    };
    let said = match content {
        Content::Words(words) => return words_of(values, *words),
        Content::Prompt(Prompt::Done) if aside => {
            "Max has marked this aside done. Summarise what came of your talk with him since it was opened, \
for the context it was opened on: what he said, what was decided, and what must now be done differently. \
This is your last message here."
                .to_owned()
        }
        Content::Prompt(Prompt::Done) => {
            "Max has marked this chat done. Write your result: what you did, what you learned, and what is \
left undone. This is your last message here."
                .to_owned()
        }
        Content::Prompt(Prompt::Lingering) => {
            "Your turn ended, but background work you own is still running. Its result wakes you when it \
settles, and you cannot finish before then. `stop` what you no longer need; `pass` up what someone will still use \
after you finish. If there is nothing else to do meanwhile, end your turn to wait."
                .to_owned()
        }
        Content::Prompt(Prompt::Tidy) => {
            let (by, left) = match task.drain() {
                Some(Draining::Abandoning(s)) => (s.by, (s.deadline.0 - at.0).max(0) / 1000),
                _ => (Abandoner::Max, 0),
            };
            let by = match by {
                Abandoner::Max => "Max has abandoned this task: the work is no longer needed",
                Abandoner::Owner => {
                    "The agent that started you has been abandoned, so this task is too: the work is no longer needed"
                }
            };
            format!(
                "{by}. Everything you had running, and every agent you started, has been stopped. \
You have about {left} seconds before you are killed. Use them to leave your work safe and understandable \
to whoever picks it up: nothing half-written, nothing left running.{shared} Then end your turn with a short \
report of where things stand: what is done, what is not, and anything left that someone must know about. \
Start nothing new. This is your last turn."
            )
        }
        Content::Prompt(Prompt::Park) => format!(
            "Max is pausing this work for now. Bring it to a safe point: finish or undo what is half done, so \
nothing is left broken or half-written, and start nothing new.{shared} Work you have running in the \
background is yours to keep or stop: keep what should still be running when Max comes back. If you have \
done much since your last checkpoint, end with `compact_and_report`, so you resume from a short context; otherwise end \
your turn. Either way, your last message is a short note of where things stand and what you will do next. You will wait, paused, until Max \
resumes you; this is not the end of your task."
        ),
        Content::Prompt(Prompt::Resume) => {
            "Max has resumed you. Carry on from where you left off.".to_owned()
        }
        Content::Prompt(Prompt::Unblocked) => {
            "Every other agent the context you were forked from started is done, and its results wait on this \
aside. Tell Max; he decides when this aside is done."
                .to_owned()
        }
    };
    vec![text(&format!("[harness] {said}"))]
}

fn words_of(values: Values, id: ValueId) -> Vec<String> {
    words(&values(id))
}

/// The neutral answer, in a fork, to one tool use of the forked response.
fn elsewhere(state: &State, values: Values, task: &Task, call: CallId) -> String {
    let record = state.call(call).expect("a forked response's call is known");
    let Caller::Agent { tool_use, .. } = &record.call.caller else {
        unreachable!("a response's call answers a tool use")
    };
    let tool = record.call.tool.as_str();
    let said = if tool == crate::meta::SPAWN || tool == crate::meta::ADD_SIBLING {
        let names = crate::meta::names(tool, &values(record.call.input));
        format!("Started {}.", names.join(", "))
    } else {
        let parent = state
            .owner(task.id)
            .and_then(|o| state.task(o))
            .map_or("the agent you were forked from", |o| o.name.as_str());
        format!(
            "Not run by you: `{parent}` made this call, and its result went to `{parent}` only, so it is not in your copy of the context."
        )
    };
    json!({
        "type": "tool_result",
        "tool_use_id": tool_use.0,
        "content": said,
    })
    .to_string()
}

/// A spawned task's mandate, as user text: its place in the work, its part,
/// whose its siblings' parts are, and when to stop.
fn mandate(state: &State, values: Values, task: &Task, carried: bool) -> String {
    let owner = state.owner(task.id).and_then(|o| state.task(o));
    let owner_name = owner.map_or("your owner", |o| o.name.as_str());
    let siblings: Vec<_> = state
        .members(task.scope)
        .filter(|t| t.id != task.id)
        .map(|t| t.name.as_str())
        .collect();
    let part = said(values, task.mandate.text);
    let mut said_ = match task.origin {
        Origin::Fork { .. } if carried => format!(
            "You are `{}`, forked from `{owner_name}` to do one part of its work, and only that part.\n\nYour part:\n{part}",
            task.name
        ),
        Origin::Fork { .. } => format!(
            "You are now `{}`, a fork of `{owner_name}`. Everything above is `{owner_name}`'s context; \
             from here you do one part of its work, and only that part.\n\nYour part:\n{part}",
            task.name
        ),
        _ => format!(
            "You are `{}`, started by `{owner_name}`.\n\nYour task:\n{part}",
            task.name
        ),
    };
    if let (Origin::Fork { .. }, Some(owner)) = (&task.origin, owner) {
        said_.push_str(&format!(
            "\n\n`{owner_name}`'s own mandate, for context only:\n{}",
            said(values, owner.mandate.text)
        ));
        if owner.by.is_some()
            && let Some(grand) = state.owner(owner.id).and_then(|g| state.task(g))
        {
            said_.push_str(&format!(" (given by `{}`)", grand.name));
        }
    }
    if !siblings.is_empty() {
        said_.push_str(&format!(
            "\n\nThe other parts ({}) are being done by other agents at the same time. They are not yours: \
             do not do them or start them again. `{owner_name}` combines the results after every part has returned.",
            siblings.join(", ")
        ));
    }
    // A root need not bind `compact_and_report`.
    let end = if task
        .context()
        .is_some_and(|c| c.binding.limb(crate::meta::COMPACT_AND_REPORT).is_some())
    {
        "end with `compact_and_report`. Its report is your result, and it"
    } else {
        "end your turn. Your last message is your result, and it"
    };
    said_.push_str(&format!(
        "\n\nWhen your part is done, {end} goes to `{owner_name}`."
    ));
    said_
}

/// A stored value read as text.
pub fn said(values: Values, id: ValueId) -> String {
    String::from_utf8_lossy(&values(id)).into_owned()
}

/// A text content block.
pub fn text(text: &str) -> String {
    json!({ "type": "text", "text": text }).to_string()
}

/// A content array of these blocks, as bytes.
pub fn blocks(blocks: &[String]) -> Bytes {
    Bytes::from(format!("[{}]", blocks.join(",")))
}

/// The visible text of an assistant entry's content.
pub fn visible(content: &[u8]) -> String {
    serde_json::from_slice::<Vec<Value>>(content)
        .unwrap_or_default()
        .iter()
        .filter(|b| b["type"] == "text")
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// A failed outcome saying why, and its output's bytes.
pub fn failed(said: &str) -> (Bytes, Outcome) {
    let model = blocks(&[text(said)]);
    let output = Output {
        model: model.id(),
        full: None,
        observes: Vec::new(),
    };
    (model, Outcome::Failed(output))
}
