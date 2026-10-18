//! The meta limb: the lifecycle tools the brain itself implements, as calls.
//!
//! `spawn` opens a scope: one call whose members are forks of the caller,
//! fresh agents in the roles its environment starts, or resumes of its
//! finished children, and which settles with every member's result once
//! the last is done. `add_sibling` adds one task to the caller's own scope
//! and returns at once. `compact_and_continue` compacts the caller's
//! context once its round is over ([`harness_core::task::Compaction`]), and
//! `compact_and_report` does too and ends its turn with its report. `attach`
//! gives back the recorded outputs of calls the caller names, so a
//! compaction carries them and a report is followed by them. `background` makes a named call
//! to one of the caller's other tools that no round waits on: its result
//! arrives as a message, and its owner cannot finish until it has. `stop`
//! abandons one of those calls, and `pass` hands one up the tree
//! ([`harness_core::call::Owner`]). Their effects (the members' `Created` facts, with a
//! limb for each new clone) are appended in the same batch as the
//! `Dispatched`, so a crash leaves both or neither, and a restart never
//! spawns twice.

use harness_core::call::{Call, Caller};
use harness_core::context::Rendered;
use harness_core::delivery::Content;
use harness_core::environment::{Audience, Description, Instruction, Limb, LimbKind, Tool};
use harness_core::id::{CallId, Digest, LimbId, TaskId, ValueId};
use harness_core::resource::{Observation, ResourceKey, Version};
use harness_core::state::State;
use harness_core::task::{Context, Task};
use harness_core::value::Bytes;
use serde_json::{Value, json};

use crate::render::{Values, blocks, call_label, carried, text};

pub const SPAWN: &str = "spawn";
pub const ADD_SIBLING: &str = "add_sibling";
pub const COMPACT_AND_CONTINUE: &str = "compact_and_continue";
pub const COMPACT_AND_REPORT: &str = "compact_and_report";
/// Whether `tool` is a compaction tool.
pub fn compacts(tool: &str) -> bool {
    [COMPACT_AND_CONTINUE, COMPACT_AND_REPORT].contains(&tool)
}

pub const BACKGROUND: &str = "background";
pub const STOP: &str = "stop";
pub const PASS: &str = "pass";
pub const ATTACH: &str = "attach";

/// The most of one attached call's text that is shown: about what a cut
/// tool output shows.
const ATTACHED: usize = 16 * 1024;

pub fn definition(id: LimbId) -> Limb {
    Limb {
        id,
        name: "meta".into(),
        kind: LimbKind::Meta,
    }
}

/// What the meta limb tells a root in an environment that starts `roles`,
/// each a name and its one-line description; `member` when the task is in
/// a scope it can add siblings to.
pub fn description(limb: LimbId, roles: &[(&str, &str)], member: bool) -> Description {
    let mut said = String::from(
        "All the calls of one response start together, and you get all of their results together, once the last \
has finished. So no call sees another's result from the same response: a call that needs one goes in your next \
response. Calls to your workspace or your browser are the exception: each starts once the calls you wrote before it \
to the same place have ended, so you can write a file and commit it in one response.\n\n\
You can start other agents with `spawn`. One call starts a scope: a list of tasks that run at the same time, \
and its result is each one's result (its last message), once every one of them has finished. Your other calls in \
the same response run alongside it, and their results wait with its. While they run you wait, so put everything \
that can run in parallel into one call.\n\n\
Fork for parts that can run at the same time, and for sequential steps too: a step whose details you will not \
need afterwards, only its result (an exploration, a search, a build and its checks), is a fork of one task, and your \
own context keeps just that result. Do a step yourself when what you learn in it carries into the next. Start a \
fresh agent for a larger task, or for work in another role.\n\n\
Each task is a fork of you, a fresh agent, or a resume of an agent you started before that has finished. \
A fork continues from this exact point of your context: it knows everything you know, so its mandate can be short, \
and it shares your cache, so it is cheap. It works in your directory with you. \
A resume continues that agent's own context with a new mandate, so a follow-up question keeps everything it learned. \
A fresh agent starts from nothing but its role, the shared `brief`, the `quotes` and its mandate, so say everything it needs. \
Each of Max's messages, mandates and answers is labelled `max-…`: name it in `quotes` to give his words to every fresh agent verbatim, rather than retelling them.\n\n\
A fork can itself spawn, and start the roles you can. `spawn` cannot run under `background`: you always wait for \
your scope.",
    );
    if roles.is_empty() {
        said.push_str("\n\nYou cannot start fresh agents, only forks of yourself.");
    } else {
        said.push_str("\n\nThe roles you can start fresh:\n");
        for (name, what) in roles {
            said.push_str(&format!("\n- `{name}`: {what}"));
        }
        said.push_str(
            "\n\nA fresh agent works in your directory (or the one `from` names), as a fork does; given a `branch`, it \
gets its own clone of that repository instead, beside it, on that branch, made from `base`, and commits and pushes there.",
        );
    }
    said.push_str(
        "\n\nYour context keeps checkpoints: its first message, each message that began one of your turns (Max's \
words, or whatever woke you), and each compaction. `compact_and_continue` replaces everything after the latest \
checkpoint with a summary you write, and you carry on from it in the same turn. Use it whenever you finish a step of \
your work and move to the next: your context stays short and cheap, and nothing up to the checkpoint is lost, so \
summarise only the step: what you did, what you found and decided and why, what is left, and anything you will need \
to look at again. What you compact away is gone, so the summary must also carry everything your report will need: \
the friction you hit, the evidence you will show (attach the calls whose real output it is alongside the compaction, or keep their labels to attach later), \
and your decisions and why, including those carried by earlier summaries. Compact when you really switch from one task to another, or when your context has grown \
long, even if it is mostly results already. Max's words after the checkpoint, the results of the other calls in the \
same response, and your running background work are carried with it as they are, so do not retell them: to take a \
file into the compacted context as it is now, read it in the same response.\n\n\
When your work is done, call `compact_and_report` instead, with your summary and your report: it compacts and ends \
your turn, and the report is your last message in the new context, right after the summary. Max reads it, or, if you \
were started by another agent, it is your result, and what you read alongside it reaches that agent with it. Put \
everything the reader needs in the report: they never see the summary.\n\n\
With `fresh: true` either tool starts a new context instead, from your role, your mandate, Max's words and your \
summary alone. Use it when even your checkpoints have grown long; the summary must then carry everything else you \
need. A change to your role's definition takes effect then. A `compact_and_continue` right after another compaction, with no other \
call since or alongside it, is refused: there is nothing to replace.\n\n\
Each result of a call you make ends with its label, such as `[call-3fa2c1]`. `attach` gives back the outputs of the \
calls you name by label, exactly as the harness recorded them, so whoever reads your work sees the real output (the \
log line, the command's output) rather than your retelling of it. Attach alongside `compact_and_report`, or alone \
right before a report you end your turn with, and the outputs follow your report to whoever reads it; attach \
alongside `compact_and_continue` to keep them in your compacted context. You can attach your own calls, and the calls your agents attached to their \
results.",
    );
    said.push_str(
        "\n\n`background` runs one of your other tools without waiting for it, under a `name` you give it: it returns \
at once, and the tool's result reaches you as a message when it settles. You cannot finish while background work you \
own still runs. `stop` it when you no longer need it, or `pass` it to the agent that started you (to Max, if he \
started you), who owns it from then on: do that only for what someone will still use after you finish, such as a \
dev server or a browser tab, and say why.",
    );
    if member {
        said.push_str(
            "\n\n`add_sibling` adds one task to the scope you are in, as a fork of you or in a role you can start, \
and returns at once: the agent that started you gets its result with yours.",
        );
    }
    let task = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string", "description": "Short and descriptive, such as `wordcount`." },
            "mandate": { "type": "string", "description": "What this agent must do, and how to check it is done." },
            "role": { "type": "string", "enum": roles.iter().map(|(name, _)| name).collect::<Vec<_>>(), "description": "Start a fresh agent in this role. Omit, with `resume`, to fork yourself." },
            "resume": { "type": "string", "description": "The name of a finished agent you started, to continue it." },
            "branch": { "type": "string", "description": "A fresh agent's own branch, for its own clone. Omit it to have it work in your directory." },
            "base": { "type": "string", "description": "What its branch is made from if it does not exist yet. Default `main`." },
            "from": { "type": "string", "description": "A directory, relative to your workspace, that a fresh agent works in or, with a `branch`, clones and clones beside. Default your workspace itself." }
        },
        "required": ["name", "mandate"]
    });
    let quotes = json!({
        "type": "array",
        "items": { "type": "string" },
        "description": "Labels of Max's words (`max-…`) to give every fresh agent verbatim, after the brief."
    });
    let spawn = json!({
        "name": SPAWN,
        "description": "Start agents to do parts of your work, together or a step at a time, and wait for all of their results.",
        "input_schema": {
            "type": "object",
            "properties": {
                "tasks": { "type": "array", "minItems": 1, "items": task },
                "brief": { "type": "string", "description": "What every fresh agent in this call is given before its own mandate." },
                "quotes": quotes
            },
            "required": ["tasks"]
        }
    });
    let summary = json!({ "type": "string", "description": "What you did since the checkpoint, what you found and decided and why, and what is left: everything you need to carry on, and everything your report will need (friction, evidence, decisions), including what earlier summaries carried." });
    let fresh = json!({ "type": "boolean", "description": "Start a new context instead, from your role, your mandate, Max's words and your summary alone." });
    let compact_and_continue = json!({
        "name": COMPACT_AND_CONTINUE,
        "description": "Replace your work since your latest checkpoint with your summary of it, and carry on from the summary in the same turn, with your next step. The results of other calls in the same response are carried with it: read a file alongside it to keep it as it is now.",
        "input_schema": {
            "type": "object",
            "properties": { "summary": summary, "fresh": fresh },
            "required": ["summary"]
        }
    });
    let compact_and_report = json!({
        "name": COMPACT_AND_REPORT,
        "description": "When your work is done: replace your work since your latest checkpoint with your summary of it, and end your turn with your report as your last message, after the summary. The results of other calls in the same response are carried with it.",
        "input_schema": {
            "type": "object",
            "properties": {
                "summary": summary,
                "report": { "type": "string", "description": "Your report: what the reader (Max, or the agent that started you) needs to know. It is your last message; they never see the summary." },
                "fresh": fresh
            },
            "required": ["summary", "report"]
        }
    });
    let attach = json!({
        "name": ATTACH,
        "description": "Show the real outputs of calls you name, as the harness recorded them: alongside `compact_and_report` or right before your report, they reach whoever reads your report; alongside `compact_and_continue`, they stay in your compacted context.",
        "input_schema": {
            "type": "object",
            "properties": {
                "calls": { "type": "array", "minItems": 1, "items": { "type": "string" }, "description": "The calls' labels, such as `call-3fa2c1`, from the end of each one's result." }
            },
            "required": ["calls"]
        }
    });
    let background = json!({
        "name": BACKGROUND,
        "description": "Run one of your other tools without waiting for its result, which comes to you as a message.",
        "input_schema": {
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "What to call it, unique among your running background work, such as `dev-server`." },
                "tool": { "type": "string", "description": "The tool's name." },
                "input": { "type": "object", "description": "The tool's input, as you would give it directly." }
            },
            "required": ["name", "tool", "input"]
        }
    });
    let stop = json!({
        "name": STOP,
        "description": "Stop one of your running background calls. Its outcome still comes to you as a message.",
        "input_schema": {
            "type": "object",
            "properties": {
                "call": { "type": "string", "description": "The background call's name." }
            },
            "required": ["call"]
        }
    });
    let pass = json!({
        "name": PASS,
        "description": "Hand one of your running background calls to the agent that started you, or to Max if he started you. It keeps running, it is theirs to use and stop, and its outcome goes to them.",
        "input_schema": {
            "type": "object",
            "properties": {
                "call": { "type": "string", "description": "The background call's name." },
                "why": { "type": "string", "description": "What it is for, and what its new owner should do with it." }
            },
            "required": ["call", "why"]
        }
    });
    let mut tools = vec![
        tool(SPAWN, spawn),
        tool(COMPACT_AND_CONTINUE, compact_and_continue),
        tool(COMPACT_AND_REPORT, compact_and_report),
        tool(ATTACH, attach),
        tool(BACKGROUND, background),
        tool(STOP, stop),
        tool(PASS, pass),
    ];
    if member {
        let sibling = json!({
            "name": ADD_SIBLING,
            "description": "Add one task to the scope you are in, and return at once.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "task": task,
                    "brief": { "type": "string", "description": "What a fresh agent is given before its mandate." },
                    "quotes": quotes
                },
                "required": ["task"]
            }
        });
        tools.push(tool(ADD_SIBLING, sibling));
    }
    Description {
        instructions: vec![Instruction {
            observes: Observation {
                resource: ResourceKey {
                    limb,
                    kind: "instructions".into(),
                    name: String::new(),
                },
                version: Version::Is(Digest::of(said.as_bytes())),
            },
            text: said,
        }],
        tools,
    }
}

fn tool(name: &str, definition: Value) -> Tool {
    Tool {
        name: name.into(),
        definition: definition.to_string(),
        audience: Audience::Agent,
    }
}

/// What a compaction call asks for.
pub struct Compact {
    pub summary: String,
    pub fresh: bool,
    /// `compact_and_report`'s report, which ends the turn.
    pub report: Option<String>,
}

/// Whether a compaction by `task` now would only repeat its last one: its
/// latest checkpoint is a compaction's carry that holds no call's output,
/// and every call it has made since is a compaction, so there is nothing
/// to replace. This is what an
/// agent compacting in a loop looks like; only `compact_and_continue` can
/// loop, since `compact_and_report` ends the turn.
pub fn repeats(state: &State, task: &Task) -> bool {
    let (Some(checkpoint), Some(head)) = (state.checkpoint(task), task.head()) else {
        return false;
    };
    state.compacted(checkpoint).is_some_and(|c| {
        !c.renders
            .iter()
            .any(|span| matches!(span.from, Rendered::Call(_)))
    }) && !state.calls().any(|(id, record)| {
        matches!(record.call.caller, Caller::Agent { task: t, .. } if t == task.id)
            && !compacts(&record.call.tool)
            && state.origin(id).is_some_and(|response| {
                response != checkpoint
                    && state.descends(head, response)
                    && state.descends(response, checkpoint)
            })
    })
}

/// A compaction call's input, or why it cannot compact.
pub fn compaction(tool: &str, input: &[u8]) -> Result<Compact, String> {
    let value: Value = serde_json::from_slice(input).map_err(|e| format!("invalid input: {e}"))?;
    let field = |k: &str| {
        value[k]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let summary = field("summary").ok_or("`summary` must say what you need to carry on")?;
    let report = if tool == COMPACT_AND_REPORT {
        Some(field("report").ok_or("`report` must be your report")?)
    } else {
        None
    };
    Ok(Compact {
        summary,
        fresh: value["fresh"].as_bool().unwrap_or(false),
        report,
    })
}

/// An `attach` call's output: each call its input names, headed by what it
/// was, its text cut to [`ATTACHED`]. `task` may attach a settled call it or
/// any agent under it owns.
pub fn attach(state: &State, values: Values, task: &Task, input: &[u8]) -> Result<Bytes, String> {
    let value: Value = serde_json::from_slice(input).map_err(|e| format!("invalid input: {e}"))?;
    let labels: Vec<&str> = value["calls"]
        .as_array()
        .filter(|c| !c.is_empty())
        .and_then(|c| c.iter().map(Value::as_str).collect())
        .ok_or("`calls` must be a non-empty list of labels")?;
    let mut said = Vec::new();
    for label in labels {
        let call = state
            .calls()
            .find(|(id, record)| {
                record.outcome().is_some() && call_label(*id) == label && under(state, *id, task.id)
            })
            .map(|(id, _)| id)
            .ok_or(format!(
                "none of your settled calls, or your agents' attached ones, is labelled `{label}`"
            ))?;
        said.extend(cut(
            carried(state, values, "[harness] Attached: the", call),
            &call_label(call),
        ));
    }
    Ok(blocks(&said))
}

/// Whether `call` is owned by `task` or by an agent under it.
fn under(state: &State, call: CallId, task: TaskId) -> bool {
    let mut owner = state.call_owner(call);
    while let Some(t) = owner {
        if t == task {
            return true;
        }
        owner = state.owner(t);
    }
    false
}

/// An attached call's blocks with their text cut to [`ATTACHED`]: its head
/// and its tail, which names the file holding a cut tool output.
fn cut(said: Vec<String>, label: &str) -> Vec<String> {
    let mut left = ATTACHED;
    let total: usize = said.iter().map(String::len).sum();
    if total <= ATTACHED {
        return said;
    }
    let mut kept = Vec::new();
    for block in said {
        let Ok(mut value) = serde_json::from_str::<Value>(&block) else {
            continue;
        };
        let Some(text) = value["text"].as_str().map(str::to_owned) else {
            // An image or other block: kept whole.
            kept.push(block);
            continue;
        };
        if text.len() <= left {
            left -= text.len();
            kept.push(block);
            continue;
        }
        let head = floor(&text, left / 2);
        let tail = ceil(&text, text.len() - left / 2);
        value["text"] = Value::from(format!(
            "{}\n[… {} bytes of `{label}` cut here …]\n{}",
            &text[..head],
            tail - head,
            &text[tail..]
        ));
        left = 0;
        kept.push(value.to_string());
    }
    kept
}

fn floor(text: &str, mut at: usize) -> usize {
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

fn ceil(text: &str, mut at: usize) -> usize {
    while !text.is_char_boundary(at) {
        at += 1;
    }
    at
}

/// A background call's name, its tool and its input.
pub fn background(input: &[u8]) -> Result<(String, String, Bytes), String> {
    let value: Value = serde_json::from_slice(input).map_err(|e| format!("invalid input: {e}"))?;
    let name = value["name"]
        .as_str()
        .filter(|n| !n.is_empty())
        .ok_or("`name` must name the call")?;
    let tool = value["tool"].as_str().ok_or("`tool` must name a tool")?;
    if !value["input"].is_object() {
        return Err("`input` must be the tool's input object".into());
    }
    Ok((
        name.to_owned(),
        tool.to_owned(),
        Bytes::from(value["input"].to_string()),
    ))
}

/// The running background call a `stop` or `pass` names among those
/// `owner` owns, and a pass's reason.
pub fn owned(state: &State, owner: &Task, input: &[u8]) -> Result<(CallId, String), String> {
    let value: Value = serde_json::from_slice(input).map_err(|e| format!("invalid input: {e}"))?;
    let name = value["call"]
        .as_str()
        .ok_or("`call` must name a background call")?;
    let why = value["why"].as_str().unwrap_or_default().to_owned();
    let call = named(state, owner, name).ok_or(format!(
        "you have no running background call named `{name}`"
    ))?;
    match state.call(call) {
        Some(record) if record.abandoned || record.stopping.is_some() => {
            Err(format!("`{name}` is already stopping"))
        }
        _ => Ok((call, why)),
    }
}

/// The newest of `owner`'s running background calls named `name`.
fn named(state: &State, owner: &Task, name: &str) -> Option<CallId> {
    owner.background.iter().rev().copied().find(|c| {
        state.call(*c).is_some_and(
            |r| matches!(&r.call.caller, Caller::Background { name: n, .. } if n == name),
        )
    })
}

/// A spawn or add_sibling call's input.
pub struct Spawn {
    pub members: Vec<Member>,
    pub brief: Option<String>,
    /// Labels of Max's words to quote.
    pub quotes: Vec<String>,
}

pub struct Member {
    pub name: String,
    pub mandate: String,
    pub how: How,
}

pub enum How {
    Fork,
    Fresh {
        role: String,
        branch: Option<String>,
        base: String,
        /// The clone its clone is made from, if not the parent's workspace.
        from: Option<String>,
    },
    Resume(String),
}

pub fn parse(tool: &str, input: &[u8]) -> Result<Spawn, String> {
    let value: Value = serde_json::from_slice(input).map_err(|e| format!("invalid input: {e}"))?;
    let tasks: Vec<&Value> = if tool == ADD_SIBLING {
        vec![&value["task"]]
    } else {
        value["tasks"]
            .as_array()
            .filter(|t| !t.is_empty())
            .ok_or("`tasks` must be a non-empty list")?
            .iter()
            .collect()
    };
    let members = tasks
        .into_iter()
        .map(|t| {
            let field = |k: &str| t[k].as_str().map(str::to_owned);
            let how = match (field("role"), field("resume")) {
                (Some(_), Some(_)) => {
                    return Err("a task has a `role` or a `resume`, not both".into());
                }
                (Some(role), None) => How::Fresh {
                    role,
                    branch: field("branch"),
                    base: field("base").unwrap_or_else(|| "main".into()),
                    from: field("from"),
                },
                (None, Some(from)) => How::Resume(from),
                (None, None) => How::Fork,
            };
            Ok(Member {
                name: field("name").ok_or("each task needs a `name`")?,
                mandate: field("mandate").ok_or("each task needs a `mandate`")?,
                how,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let brief = value["brief"].as_str().map(str::to_owned);
    let quotes = match &value["quotes"] {
        Value::Null => Vec::new(),
        Value::Array(labels) => labels
            .iter()
            .map(|l| l.as_str().map(str::to_owned))
            .collect::<Option<_>>()
            .ok_or("`quotes` must be a list of labels")?,
        _ => return Err("`quotes` must be a list of labels".into()),
    };
    Ok(Spawn {
        members,
        brief,
        quotes,
    })
}

/// The task names a spawn's input gives, for the neutral answer a fork sees.
pub fn names(tool: &str, input: &[u8]) -> Vec<String> {
    parse(tool, input)
        .map(|s| s.members.into_iter().map(|m| m.name).collect())
        .unwrap_or_default()
}

/// A scope's result: each member's outcome under its name, as a tool
/// result's content.
pub fn result(members: &[(String, Vec<String>)]) -> Bytes {
    let body: Vec<String> = members
        .iter()
        .flat_map(|(name, said)| {
            std::iter::once(text(&format!("## {name}"))).chain(said.iter().cloned())
        })
        .collect();
    blocks(&body)
}

/// What a call to `limb` expects: the versions the context knows of its
/// resources.
pub fn expects(context: &Context, limb: Option<LimbId>) -> Vec<Observation> {
    context
        .knows
        .iter()
        .filter(|(r, _)| Some(r.limb) == limb)
        .map(|(resource, known)| Observation {
            resource: resource.clone(),
            version: known.has,
        })
        .collect()
}

/// A `background` call's input as the call it starts, with its input's
/// bytes: routed by the caller's binding, never to the meta limb.
pub fn background_call(
    state: &State,
    parent: &Task,
    from: CallId,
    meta: LimbId,
    input: &[u8],
) -> Result<(Call, Bytes), String> {
    let (name, tool, input) = background(input)?;
    if named(state, parent, &name).is_some() {
        return Err(format!(
            "you already have background work named `{name}` running"
        ));
    }
    let context = parent.context().ok_or("you have no context")?;
    let limb = context
        .binding
        .limb(&tool)
        .ok_or(format!("you have no tool `{tool}`"))?;
    if limb == meta {
        return Err(format!("`{tool}` cannot run in the background"));
    }
    let call = Call {
        caller: Caller::Background {
            task: parent.id,
            from,
            name,
        },
        expects: expects(context, Some(limb)),
        tool,
        input: input.id(),
    };
    Ok((call, input))
}

/// Max's words a label names, among those `parent` has heard: his messages,
/// the mandates he gave, his answers, and what briefs quoted.
pub fn quoted(state: &State, parent: &Task, label: &str) -> Result<ValueId, String> {
    let words = state.deliveries().filter_map(|d| match d.delivery.content {
        Content::Words(words) => Some(words),
        Content::Prompt(_) => None,
    });
    let mandates = state
        .tasks()
        .flat_map(|t| [t.mandate.text].into_iter().chain(t.mandate.quotes.clone()));
    let answers = state
        .calls()
        .filter_map(|(_, c)| c.outcome()?.output().map(|o| o.model));
    words
        .chain(mandates)
        .chain(answers)
        .filter(|v| state.heard(parent.id, *v))
        .find(|v| crate::render::label(*v) == label)
        .ok_or(format!(
            "no words of Max's you have heard are labelled `{label}`"
        ))
}
