//! The `meta` tool set: the lifecycle tools the brain itself implements,
//! as calls.
//!
//! `spawn` opens a scope: one call whose members are forks of the caller,
//! fresh agents in the roles its role starts, or resumes of its
//! finished children, and which settles with every member's result once
//! the last is done. `add_sibling` adds one task to the caller's own scope
//! and returns at once. `compact_and_continue` compacts the caller's
//! context once its round is over ([`harness_core::task::Compaction`]), and
//! `compact_and_report` does too and ends its turn with its report. `attach`
//! gives back the recorded outputs of calls the caller names, so a
//! compaction carries them and a report is followed by them. `background` makes a named call
//! to one of the caller's other tools, or a `spawn`, that no round waits on:
//! its result arrives as a message, and its owner cannot finish until it
//! has. `wait` waits in a round on some of that work, and its output is
//! their results ([`harness_core::call::Wait`]). `stop`
//! abandons one of those calls, and `pass` hands one up the tree
//! ([`harness_core::call::Owner`]). Their effects (the members' `Created`
//! facts), and those of the [`crate::max`] set's `open_chat`, are appended
//! in the same batch as the `Dispatched`, so a crash leaves both or
//! neither, and a restart never spawns twice.

use harness_core::call::{Call, Caller, Until, Wait};
use harness_core::context::Rendered;
use harness_core::delivery::Content;
use harness_core::fact::Author;
use harness_core::id::{CallId, Digest, LimbId, TaskId, ValueId};
use harness_core::limb::{Audience, Instruction, Tool, ToolSet};
use harness_core::resource::{Observation, ResourceKey, Version};
use harness_core::role::Role;
use harness_core::state::State;
use harness_core::task::{Context, Input, Task};
use harness_core::value::Bytes;
use serde_json::{Value, json};

use crate::render::{Values, blocks, carried, text};

pub const SPAWN: &str = "spawn";
pub const ADD_SIBLING: &str = "add_sibling";
pub const COMPACT_AND_CONTINUE: &str = "compact_and_continue";
pub const COMPACT_AND_REPORT: &str = "compact_and_report";
/// Whether `tool` is a compaction tool.
pub fn compacts(tool: &str) -> bool {
    [COMPACT_AND_CONTINUE, COMPACT_AND_REPORT].contains(&tool)
}

pub const BACKGROUND: &str = "background";
pub const WAIT: &str = "wait";
pub const STOP: &str = "stop";
pub const PASS: &str = "pass";
pub const ATTACH: &str = "attach";

/// The most of one attached call's text that is shown: about what a cut
/// tool output shows.
const ATTACHED: usize = 16 * 1024;

/// The tool set's name, as roles hold it.
pub const SET: &str = "meta";

/// The `meta` set the brain `limb` gives a root in a role that starts
/// `roles`; `dir` when the task has a working directory, which a fresh
/// agent works in unless told otherwise; `member` when the task is in a
/// scope it can add siblings to.
pub fn set(limb: LimbId, roles: &[&Role], dir: bool, member: bool) -> ToolSet {
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
Each member is a fork of you, a resume, or a fresh agent, and each kind has its own key. \
A fork (`fork`) continues from this exact point of your context: it knows everything you know, Max's words included, so its mandate can be short, \
and it shares your cache, so it is cheap. It works in your directory with you. \
A resume (`resume`) continues the context of an agent you started before that has finished, with a new mandate, so a follow-up question keeps everything it learned. \
A fresh agent (`start`, under its role) starts from nothing but its role, the `brief` and `quotes` its `start` shares, and its mandate, so say everything it needs. \
Each of Max's messages, mandates and answers is labelled `max-…`: name it in `quotes` to give his words verbatim, rather than retelling them, \
to fresh agents, or to a resume, which has not heard what he said to you since it finished.\n\n\
A fork can itself spawn, and start the roles you can. A spawn reports once, when its last task has finished, so \
start separately, each as its own background spawn, the pieces of work you will act on separately.",
    );
    if roles.is_empty() {
        said.push_str("\n\nYou cannot start fresh agents, only forks of yourself.");
    } else {
        said.push_str("\n\nThe roles you can start fresh, and the tool sets each holds:\n");
        for role in roles {
            let sets: Vec<String> = role.tools.iter().map(|t| format!("`{t}`")).collect();
            said.push_str(&format!(
                "\n- `{}` ({}): {}",
                role.name,
                sets.join(", "),
                role.description
            ));
        }
        said.push_str(match dir {
            true => "\n\nA fresh agent in a role that works in a directory works in yours, as a fork does, or in the `dir` you give it: \
relative to yours, absolute, or `~/…`. To give it a clone of its own, make the clone first and name it as its `dir`.",
            false => "\n\nA fresh agent in a role that works in a directory works in the `dir` you give it: absolute, or `~/…`.",
        });
    }
    said.push_str(
        "\n\nYour context keeps checkpoints: its first message, each message that began one of your turns (Max's \
words, or whatever woke you), and each compaction. `compact_and_continue` replaces everything after the latest \
checkpoint with what you write in `carry`, and you carry on from it in the same turn. Use it whenever you finish a step of \
your work and move to the next: your context stays short and cheap, and nothing up to the checkpoint is lost, so \
summarise only the step: what you did, what you found and decided and why, what is left, and anything you will need \
to look at again. What you compact away is gone, so what you carry must also hold everything your report will need: \
the friction you hit, the evidence you will show (attach the calls whose real output it is alongside the compaction, or keep their labels to attach later), \
and your decisions and why, including those carried by earlier compactions. Compact when you really switch from one task to another, or when your context has grown \
long, even if it is mostly results already. Max's words after the checkpoint, the results of the other calls in the \
same response, and your running background work are carried with it as they are, so do not retell them: to take a \
file into the compacted context as it is now, read it in the same response.\n\n\
When your work is done, call `compact_and_report` instead, with what you carry and your report: it compacts and ends \
your turn, and the report is your last message in the new context, right after what you carried. Max reads it, or, if you \
were started by another agent, it is your result, and what you read alongside it reaches that agent with it. Put \
everything the reader needs in the report: they never see what you carried.\n\n\
The harness decides where a compaction starts from: your latest checkpoint, or, when your role, your \
instructions or your model have changed or what your checkpoints keep has grown long, a new context from your role, \
your mandate, Max's words and what you carry alone. Your turn's opening line says when your next compaction will, so \
what you carry then holds everything else you need. A `compact_and_continue` right after another compaction, with no other \
call since or alongside it, is refused: there is nothing to replace.\n\n\
Each result of a call you make ends with its label, such as `[call-3fa2c1]`. `attach` gives back the outputs of the \
calls you name by label, exactly as the harness recorded them, so whoever reads your work sees the real output (the \
log line, the command's output) rather than your retelling of it. Attach alongside `compact_and_report`, or alone \
right before a report you end your turn with, and the outputs follow your report to whoever reads it; attach \
alongside `compact_and_continue` to keep them in your compacted context. You can attach your own calls, and the calls your agents attached to their \
results.",
    );
    said.push_str(
        "\n\n`background` runs one of your other tools, or a `spawn`, without waiting for it, under a `name` you give \
it: it returns at once. Say what you are starting. When you have nothing to do until it lands, call `wait`: it returns \
the first of your background work to land (`until: any`, the default) or all of it (`until: all`), each result whole \
and given you only there. Never `sleep` on your own work. A result that lands while you are not waiting reaches you \
as a message, and starts a new turn if yours has ended. Max's words end a wait, saying what still runs.\n\n\
End your turn when your work is done, with your result. You cannot finish while background work you own still \
runs: `wait` for it, and when Max's words end a wait, answer him and wait again. `stop` background work when you no longer need it, \
or `pass` it to the agent that started you (to Max, if he started you), who owns it from then on: do that only for \
what someone will still use after you finish, such as a dev server or a browser tab, and say why.",
    );
    if member {
        said.push_str(
            "\n\n`add_sibling` adds members to the scope you are in, given as `spawn` takes them, \
and returns at once: the agent that started you gets their results with yours.",
        );
    }
    let members = members_schema(roles, dir);
    let spawn = json!({
        "name": SPAWN,
        "description": "Start agents to do parts of your work, together or a step at a time, and wait for all of their results.",
        "input_schema": members
    });
    let carry = json!({ "type": "string", "description": "What you carry: what you did since the checkpoint, what you found and decided and why, and what is left: everything you need to carry on, and everything your report will need (friction, evidence, decisions), including what earlier compactions carried." });
    let compact_and_continue = json!({
        "name": COMPACT_AND_CONTINUE,
        "description": "Replace your work since your latest checkpoint with what you carry from it, and carry on from that in the same turn, with your next step. The results of other calls in the same response are carried with it: read a file alongside it to keep it as it is now.",
        "input_schema": {
            "type": "object",
            "properties": { "carry": carry },
            "required": ["carry"]
        }
    });
    let compact_and_report = json!({
        "name": COMPACT_AND_REPORT,
        "description": "When your work is done: replace your work since your latest checkpoint with what you carry from it, and end your turn with your report as your last message, after what you carried. The results of other calls in the same response are carried with it.",
        "input_schema": {
            "type": "object",
            "properties": {
                "carry": carry,
                "report": { "type": "string", "description": "Your report: what the reader (Max, or the agent that started you) needs to know. It is your last message; they never see what you carried." }
            },
            "required": ["carry", "report"]
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
        "description": "Run one of your other tools, or a `spawn`, without waiting for its result: `wait` for it, or it comes to you as a message.",
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
    let wait = json!({
        "name": WAIT,
        "description": "Wait for your own background work: until the first of it lands (`until: any`, the default) or all of it (`until: all`). The results come whole in this call's output. Max's words end it early, saying what still runs.",
        "input_schema": {
            "type": "object",
            "properties": {
                "on": { "type": "array", "items": { "type": "string" }, "description": "The background calls' names, or their labels (`call-…`). Default: all your running background work." },
                "until": { "type": "string", "enum": ["any", "all"], "description": "Return when any of them has landed, or all. Default `any`." }
            }
        }
    });
    let stop = json!({
        "name": STOP,
        "description": "Stop one of your running background calls. Its outcome still comes to you as a message.",
        "input_schema": {
            "type": "object",
            "properties": {
                "call": { "type": "string", "description": "The background call's name, or its label (`call-…`)." }
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
                "call": { "type": "string", "description": "The background call's name, or its label (`call-…`)." },
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
        tool(WAIT, wait),
        tool(STOP, stop),
        tool(PASS, pass),
    ];
    if member {
        let sibling = json!({
            "name": ADD_SIBLING,
            "description": "Add members to the scope you are in, and return at once.",
            "input_schema": members
        });
        tools.push(tool(ADD_SIBLING, sibling));
    }
    ToolSet {
        name: SET.into(),
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

/// The input schema of `spawn` and `add_sibling`: one key per kind of
/// member, each with exactly the fields that kind takes, so a field that
/// cannot apply cannot be written. `start` has one key per role in
/// `roles`, and a `dir` only for a role whose agents work in a directory,
/// required when the caller has none of its own to give them.
fn members_schema(roles: &[&Role], dir: bool) -> Value {
    let name =
        json!({ "type": "string", "description": "Short and descriptive, such as `wordcount`." });
    let mandate = json!({ "type": "string", "description": "What this agent must do, and how to check it is done." });
    let quotes = |whom: &str| {
        json!({
            "type": "array",
            "items": { "type": "string" },
            "description": format!("Labels of Max's words (`max-…`) to give {whom} verbatim.")
        })
    };
    let list = |description: String, properties: Value, required: Vec<&str>| {
        json!({
            "type": "array",
            "description": description,
            "items": {
                "type": "object",
                "properties": properties,
                "required": required,
                "additionalProperties": false
            }
        })
    };
    let mut properties = json!({
        "fork": list(
            "Forks of you, each continuing from this point of your context.".into(),
            json!({ "name": name, "mandate": mandate }),
            vec!["name", "mandate"],
        ),
        "resume": list(
            "Finished agents you started, each continued with a new mandate.".into(),
            json!({
                "agent": { "type": "string", "description": "The name of the finished agent you started." },
                "mandate": mandate,
                "quotes": quotes("it, before its new mandate")
            }),
            vec!["agent", "mandate"],
        ),
    });
    if !roles.is_empty() {
        let mut start = json!({
            "brief": { "type": "string", "description": "What every fresh agent in this call is given before its own mandate." },
            "quotes": quotes("every fresh agent, after the brief")
        });
        for role in roles {
            let mut fields = json!({ "name": name, "mandate": mandate });
            let mut required = vec!["name", "mandate"];
            if role.works_in_a_directory() {
                fields["dir"] = dir_field(dir, "Where it works");
                if !dir {
                    required.push("dir");
                }
            }
            start[&role.name] = list(format!("Fresh `{}` agents.", role.name), fields, required);
        }
        properties["start"] = json!({
            "type": "object",
            "description": "Fresh agents, under their roles, and what all of them share.",
            "properties": start,
            "additionalProperties": false
        });
    }
    json!({ "type": "object", "properties": properties })
}

/// A fresh agent's `dir` field, `what` it is: relative to the caller's
/// directory when `dir`, the caller has one.
pub(crate) fn dir_field(dir: bool, what: &str) -> Value {
    let description = match dir {
        true => format!(
            "{what}: relative to your directory, absolute, or `~/…`. Default: your directory."
        ),
        false => format!("{what}: absolute, or `~/…`."),
    };
    json!({ "type": "string", "description": description })
}

fn tool(name: &str, definition: Value) -> Tool {
    Tool {
        name: name.into(),
        definition: definition.to_string(),
        audience: Audience::Agent,
        held: false,
    }
}

/// What a compaction call asks for.
pub struct Compact {
    pub summary: String,
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
    // `summary` is what tool definitions recorded before `carry` named it.
    let summary = field("carry")
        .or_else(|| field("summary"))
        .ok_or("`carry` must say what you need to carry on")?;
    let report = if tool == COMPACT_AND_REPORT {
        Some(field("report").ok_or("`report` must be your report")?)
    } else {
        None
    };
    // A `fresh` an older root's tools offered is the agent's guess at what
    // the harness decides ([`crate::policy::reroot`]), so it goes unread.
    Ok(Compact { summary, report })
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
                record.outcome().is_some() && id.label() == label && under(state, *id, task.id)
            })
            .map(|(id, _)| id)
            .ok_or(format!(
                "none of your settled calls, or your agents' attached ones, is labelled `{label}`"
            ))?;
        said.extend(cut(
            carried(state, values, "[harness] Attached: the", call),
            &call.label(),
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
fn background(input: &[u8]) -> Result<(String, String, Bytes), String> {
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

/// The newest of `owner`'s running background calls named `name`, or the
/// one whose label it is (`call-…`, as a call moved to the background is
/// named).
pub(crate) fn named(state: &State, owner: &Task, name: &str) -> Option<CallId> {
    owner.background.iter().rev().copied().find(|c| {
        c.label() == name
            || state.call(*c).is_some_and(
                |r| matches!(&r.call.caller, Caller::Background { name: n, .. } if n == name),
            )
    })
}

/// What a `wait` by `owner` asks for.
pub enum Waiting {
    /// To wait on its running background calls.
    On(Wait),
    /// Nothing: what it names has landed, or nothing runs. What it says.
    Holds(String),
}

/// What a `wait` by `owner` asks for: the running background calls its
/// input names, or all of them; or, when what it names has landed already,
/// or nothing runs, to return at once.
pub fn wait(state: &State, owner: &Task, input: &[u8]) -> Result<Waiting, String> {
    let value: Value = serde_json::from_slice(input).map_err(|e| format!("invalid input: {e}"))?;
    let until = match value["until"].as_str() {
        None | Some("any") => Until::Any,
        Some("all") => Until::All,
        Some(other) => return Err(format!("`until` is `any` or `all`, not `{other}`")),
    };
    let (mut on, mut landed) = (Vec::new(), Vec::new());
    match &value["on"] {
        Value::Null => on.clone_from(&owner.background),
        Value::Array(names) => {
            for n in names {
                let n = n.as_str().ok_or("`on` must be a list of names")?;
                match named(state, owner, n) {
                    Some(call) => on.push(call),
                    None => landed.push(
                        settled(state, owner, n)
                            .ok_or(format!("you have no background call named `{n}`"))?,
                    ),
                }
            }
        }
        _ => return Err("`on` must be a list of names".into()),
    }
    on.dedup();
    if !on.is_empty() && (landed.is_empty() || until == Until::All) {
        return Ok(Waiting::On(Wait { on, until }));
    }
    let names = |calls: &[CallId]| -> String {
        let names: Vec<String> = calls.iter().map(|c| background_name(state, *c)).collect();
        names.join(", ")
    };
    if landed.is_empty() {
        return Ok(Waiting::Holds(
            "You have no background work running, so there is nothing to wait on.".into(),
        ));
    }
    let mut said = format!(
        "Already landed, and its result has reached you: {}.",
        names(&landed)
    );
    if !on.is_empty() {
        said.push_str(&format!(" Still running: {}.", names(&on)));
    }
    Ok(Waiting::Holds(said))
}

/// The newest of `owner`'s settled background calls named `name`, or the
/// one whose label it is.
fn settled(state: &State, owner: &Task, name: &str) -> Option<CallId> {
    (state.calls())
        .filter(|(c, r)| {
            r.settled.is_some()
                && matches!(&r.call.caller, Caller::Background { task, name: n, .. }
                    if *task == owner.id && (n == name || c.label() == name))
        })
        .map(|(c, _)| c)
        .max()
}

/// How a background call is named to its owner: its name and its label.
fn background_name(state: &State, call: CallId) -> String {
    match state.call(call).map(|r| &r.call.caller) {
        Some(Caller::Background { name, .. }) => format!("`{name}` (`{}`)", call.label()),
        _ => format!("`{}`", call.label()),
    }
}

/// A wait's output, and whether it completed: the whole result of each
/// call it waits on that has settled, then what still runs, and, for one
/// ended early, why. Only one its task's stopping ended is interrupted;
/// one that words, or a member waiting to hear from it, ended is an
/// ordinary result.
pub fn waited(state: &State, values: Values, call: CallId) -> Option<(Bytes, bool)> {
    let wait = state.call(call)?.waits.as_ref()?;
    let (landed, running) = state.waited(call)?;
    let holds = state.wait_holds(call, wait);
    let owner = state.call_owner(call).and_then(|t| state.task(t));
    let stopping = owner.is_some_and(|t| t.drain().is_some());
    let listening: Vec<String> = (wait.on.iter())
        .flat_map(|c| state.listening(*c))
        .map(|t| format!("`{}`", t.name))
        .collect();
    let mut said = Vec::new();
    if !holds {
        let then = match owner.is_some_and(Task::closes) {
            true => "then `wait` again.",
            false => {
                "and end your turn. What still runs keeps running, and its result starts a new turn."
            }
        };
        said.push(text(&match (stopping, owner) {
            (true, _) => "[harness] You are being stopped, so this wait ended.".to_owned(),
            (false, _) if !listening.is_empty() => format!(
                "[harness] {} waits to hear from you, so waiting on it would wait forever: tell it what it \
needs, {then}",
                listening.join(", ")
            ),
            (false, Some(owner)) => {
                format!("[harness] {} to you: answer, {then}", writers(state, owner))
            }
            (false, None) => format!("[harness] This wait ended: {then}"),
        }));
    }
    said.extend((landed.iter()).map(|c| crate::render::activity(state, values, *c)));
    if !running.is_empty() {
        let names: Vec<String> = (running.iter())
            .map(|c| background_name(state, *c))
            .collect();
        said.push(text(&format!(
            "[harness] Still running: {}.",
            names.join(", ")
        )));
    }
    if said.is_empty() {
        said.push(text(
            "[harness] Nothing this wait was on is yours any more.",
        ));
    }
    Some((blocks(&said), holds || !stopping))
}

/// Who has written to `task` since its last request: Max, and the agents
/// whose messages wait for it.
fn writers(state: &State, task: &Task) -> String {
    let mut who: Vec<String> = Vec::new();
    for p in &task.pending {
        let Input::Delivery(d) = p else { continue };
        let Some(delivered) = state.delivery(*d) else {
            continue;
        };
        let name = match (delivered.by, state.teller(delivered)) {
            (Author::Max, _) => "Max".to_owned(),
            (_, Some((from, _))) => {
                (state.task(from)).map_or("An agent".to_owned(), |t| format!("`{}`", t.name))
            }
            _ => continue,
        };
        if !who.contains(&name) {
            who.push(name);
        }
    }
    match who.as_slice() {
        [] => "Words came".to_owned(),
        [one] => format!("{one} wrote"),
        [rest @ .., last] => format!("{} and {last} wrote", rest.join(", ")),
    }
}

/// A spawn or add_sibling call's input.
pub struct Spawn {
    pub members: Vec<Member>,
    /// What its `start` gives every fresh member.
    pub brief: Option<String>,
    /// Labels of Max's words its `start` quotes to every fresh member.
    pub quotes: Vec<String>,
}

pub struct Member {
    pub name: String,
    pub mandate: String,
    pub how: How,
}

pub enum How {
    Fork,
    Resume {
        /// The name of the finished agent it continues.
        agent: String,
        /// Labels of Max's words to quote to it.
        quotes: Vec<String>,
    },
    Fresh {
        role: String,
        /// Where it works, as the caller named it, if not the caller's
        /// own directory.
        dir: Option<String>,
    },
}

/// The keys of `start` that are not roles.
const SHARED: [&str; 2] = ["brief", "quotes"];

/// A spawn or add_sibling call's input. A key that means nothing to it is
/// said back rather than dropped.
pub fn parse(tool: &str, input: &[u8]) -> Result<Spawn, String> {
    let value: Value = serde_json::from_slice(input).map_err(|e| format!("invalid input: {e}"))?;
    let object = value.as_object().ok_or("the input must be an object")?;
    if let Some(key) = object
        .keys()
        .find(|k| !["fork", "resume", "start"].contains(&k.as_str()))
    {
        return Err(format!(
            "`{key}` means nothing to `{tool}`: it takes `fork`, `resume` and `start`"
        ));
    }
    let mut members = Vec::new();
    for item in items(&value["fork"], "`fork`")? {
        let [name, mandate] = fields(item, "a fork", ["name", "mandate"], &[])?;
        members.push(Member {
            name,
            mandate,
            how: How::Fork,
        });
    }
    for item in items(&value["resume"], "`resume`")? {
        let [agent, mandate] = fields(item, "a resume", ["agent", "mandate"], &["quotes"])?;
        members.push(Member {
            name: agent.clone(),
            mandate,
            how: How::Resume {
                agent,
                quotes: labels(item)?,
            },
        });
    }
    let start = &value["start"];
    let (brief, quotes) = match start {
        Value::Null => (None, Vec::new()),
        Value::Object(roles) => {
            let fresh = members.len();
            for (role, list) in roles.iter().filter(|(k, _)| !SHARED.contains(&k.as_str())) {
                for item in items(list, &format!("`start.{role}`"))? {
                    let what = format!("a fresh `{role}`");
                    let [name, mandate] = fields(item, &what, ["name", "mandate"], &["dir"])?;
                    members.push(Member {
                        name,
                        mandate,
                        how: How::Fresh {
                            role: role.clone(),
                            dir: string(item, "dir")?,
                        },
                    });
                }
            }
            if members.len() == fresh {
                return Err(
                    "`start` starts no one: list the agents to start under their role".into(),
                );
            }
            (string(start, "brief")?, labels(start)?)
        }
        _ => return Err("`start` must be an object, keyed by role".into()),
    };
    if members.is_empty() {
        return Err(format!(
            "`{tool}` starts no one: give it a `fork`, a `resume` or a `start`"
        ));
    }
    Ok(Spawn {
        members,
        brief,
        quotes,
    })
}

/// The items of the list `value`, which `what` names; none if absent.
fn items<'a>(value: &'a Value, what: &str) -> Result<&'a [Value], String> {
    match value {
        Value::Null => Ok(&[]),
        Value::Array(items) => Ok(items),
        _ => Err(format!("{what} must be a list")),
    }
}

/// The required string fields of `item`, a member that is `what`, which
/// may also take `optional`.
pub(crate) fn fields<const N: usize>(
    item: &Value,
    what: &str,
    required: [&str; N],
    optional: &[&str],
) -> Result<[String; N], String> {
    let object = item
        .as_object()
        .ok_or(format!("each of {what}s must be an object"))?;
    if let Some(key) =
        (object.keys()).find(|k| !required.contains(&k.as_str()) && !optional.contains(&k.as_str()))
    {
        let mut takes: Vec<&str> = required.to_vec();
        takes.extend(optional);
        return Err(format!(
            "`{key}` means nothing to {what}: it takes `{}`",
            takes.join("`, `")
        ));
    }
    let mut out = Vec::new();
    for key in required {
        out.push(string(item, key)?.ok_or(format!("{what} needs a `{key}`"))?);
    }
    Ok(out.try_into().expect("one per key"))
}

/// The labels `value`'s `quotes` lists.
pub(crate) fn labels(value: &Value) -> Result<Vec<String>, String> {
    match &value["quotes"] {
        Value::Null => Ok(Vec::new()),
        Value::Array(labels) => labels
            .iter()
            .map(|l| l.as_str().map(str::to_owned))
            .collect::<Option<_>>()
            .ok_or("`quotes` must be a list of labels".into()),
        _ => Err("`quotes` must be a list of labels".into()),
    }
}

/// The string `key` of `value`, if it has one.
pub(crate) fn string(value: &Value, key: &str) -> Result<Option<String>, String> {
    match &value[key] {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        _ => Err(format!("`{key}` must be a string")),
    }
}

/// The member names a spawn's input gives, for the neutral answer a fork
/// sees: read from any input it was given, as the call was made, in
/// today's shape or the one list of `tasks` (`task`, for `add_sibling`)
/// that records from before it hold.
pub(crate) fn names(input: &[u8]) -> Vec<String> {
    let Ok(value) = serde_json::from_slice::<Value>(input) else {
        return Vec::new();
    };
    let field = |key: &'static str| move |t: &Value| t[key].as_str().map(str::to_owned);
    let list = |v: &Value| v.as_array().cloned().unwrap_or_default();
    let mut names: Vec<String> = (list(&value["tasks"]).iter())
        .chain([&value["task"]])
        .chain(&list(&value["fork"]))
        .filter_map(field("name"))
        .collect();
    names.extend(list(&value["resume"]).iter().filter_map(field("agent")));
    if let Some(start) = value["start"].as_object() {
        let fresh = (start.iter())
            .filter(|(k, _)| !SHARED.contains(&k.as_str()))
            .flat_map(|(_, l)| list(l));
        names.extend(fresh.filter_map(|t| field("name")(&t)));
    }
    names
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

/// What a call to `limb` expects: the versions the context knows of the
/// resources of the limb the call is sent to, which for a limb of before
/// is the one hosting it now.
pub fn expects(state: &State, context: &Context, limb: Option<LimbId>) -> Vec<Observation> {
    let limb = limb.map(|l| state.resolved(l));
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
/// bytes: routed by the caller's binding, never to the brain's own limb,
/// whose calls act on the harness at once.
pub fn background_call(
    state: &State,
    parent: &Task,
    from: CallId,
    brain: LimbId,
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
    if state.resolve(limb) == Some(brain) && tool != SPAWN {
        return Err(format!("`{tool}` cannot run in the background"));
    }
    let call = Call {
        caller: Caller::Background {
            task: parent.id,
            from,
            name,
        },
        expects: expects(state, context, Some(limb)),
        tool,
        input: input.id(),
    };
    Ok((call, input))
}

/// Max's words a label names, among those `parent` has heard: his messages,
/// the mandates he gave, his answers, and what briefs and messages quoted.
pub fn quoted(state: &State, parent: &Task, label: &str) -> Result<ValueId, String> {
    let words = state.deliveries().flat_map(|d| match &d.delivery.content {
        Content::Words(words) => vec![*words],
        Content::Message(message) => message.quotes.clone(),
        Content::Prompt(_) => Vec::new(),
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
