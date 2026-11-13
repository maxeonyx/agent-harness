//! The `max` set's page and ask tools ([`harness_core::page`],
//! [`harness_core::ask`]): what each call does to the state, as the facts
//! and values the brain states in the batch that settles it, and what the
//! call says back.
//!
//! A page tool never refuses what it can draw: a body breaking the
//! language's rules is taken and its problems are listed in the result, so
//! the agent iterates rather than retries. It says no only when the world
//! did: an edit whose text is not in the page, a present with nothing
//! staged, an ask that names nothing open.
//!
//! An ask is a background call of the asking task, as `open_chat` is: the
//! `ask` call returns at once, and the background call it makes stays open
//! until it settles, its result reaching the agent as a message.

use harness_core::ask::{Ask, Choice, Link, Otherwise, Want};
use harness_core::body::Problem;
use harness_core::call::{Call, CallEvent, Caller, Outcome, Output};
use harness_core::design::{Feedback, FeedbackEvent};
use harness_core::fact::Fact;
use harness_core::id::{CallId, DeliveryId, EventId, FeedbackId, TaskId, ValueId};
use harness_core::limb::{Audience, Tool};
use harness_core::page::{Claim, Description, Lead, PageEvent, Ref, Said, Sheet, Spot, Step};
use harness_core::source::ItemKey;
use harness_core::state::State;
use harness_core::task::{Origin, Task, TaskEvent};
use harness_core::value::Bytes;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::meta;
use crate::render::{Values, blocks, text};

pub use harness_core::task::PRESENT;
pub const PAGE_WRITE: &str = "page_write";
pub const PAGE_EDIT: &str = "page_edit";
pub const DESCRIBE: &str = "describe";
pub const ASK: &str = "ask";
pub const WITHDRAW: &str = "withdraw";
pub const CITE: &str = "cite";
pub const PASS_ON: &str = "pass_on";

/// Every tool this module runs.
pub const TOOLS: [&str; 8] = [
    PAGE_WRITE, PAGE_EDIT, PRESENT, DESCRIBE, ASK, WITHDRAW, CITE, PASS_ON,
];

/// Item kinds a live name may name: none has a source yet, so every item
/// link is drawn as its words.
pub const KINDS: &[&str] = &[];

/// What a call did: the values and facts to state with its settlement, and
/// what it says back.
pub struct Done {
    pub values: Vec<Bytes>,
    pub facts: Vec<Fact>,
    pub said: String,
}

/// The task whose page `task`'s page tools act on: its nearest ancestor that
/// is not a fork, since a fork is its source doing a step.
pub fn owner(state: &State, task: &Task) -> TaskId {
    let mut at = task;
    while let Origin::Fork { from } = at.origin {
        match state.task(from) {
            Some(source) => at = source,
            None => break,
        }
    }
    at.id
}

/// Run `tool` for `task`'s call `call`. `mint(i)` is the id of the `i`th
/// fact of [`Done::facts`], for a fact that creates something.
pub fn run(
    state: &State,
    values: Values,
    task: &Task,
    call: CallId,
    tool: &str,
    input: &[u8],
    mint: &dyn Fn(usize) -> EventId,
) -> Result<Done, String> {
    let input = input_of(input)?;
    let owner = owner(state, task);
    let text_of = |v: ValueId| Some(String::from_utf8_lossy(&values(v)).into_owned());
    match tool {
        PAGE_WRITE => write(state, owner, call, input),
        PAGE_EDIT => edit(state, owner, call, input, &text_of),
        PRESENT => present(state, owner, call),
        DESCRIBE => describe(owner, input),
        ASK => ask(state, task, call, input, mint),
        WITHDRAW | CITE => settle(state, task, tool, input),
        PASS_ON => pass_on(state, task, input, mint),
        _ => Err(format!("the harness has no `{tool}`")),
    }
}

/// A call's input as a value, without the `summary` every input may carry.
fn input_of(input: &[u8]) -> Result<Value, String> {
    let mut value: Value =
        serde_json::from_slice(input).map_err(|e| format!("invalid input: {e}"))?;
    let object = value.as_object_mut().ok_or("the input must be an object")?;
    object.remove("summary");
    Ok(value)
}

fn parse<T: for<'de> Deserialize<'de>>(input: Value) -> Result<T, String> {
    serde_json::from_value(input).map_err(|e| e.to_string())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Write {
    lead: LeadIn,
    end: ClaimIn,
    now: ClaimIn,
    why: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LeadIn {
    title: String,
    hook: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimIn {
    says: String,
    shows: Option<String>,
}

fn write(state: &State, owner: TaskId, call: CallId, input: Value) -> Result<Done, String> {
    let Write {
        lead,
        end,
        now,
        why,
    } = parse(input)?;
    let mut values = Vec::new();
    let mut body = |markdown: Option<String>| {
        markdown.map(|m| {
            let bytes = Bytes::from(m);
            let id = bytes.id();
            values.push(bytes);
            id
        })
    };
    let sheet = Sheet {
        lead: Lead {
            title: lead.title,
            hook: lead.hook,
        },
        end: Claim {
            says: end.says,
            shows: body(end.shows),
        },
        now: Claim {
            says: now.says,
            shows: body(now.shows),
        },
        why: body(why),
    };
    let texts: Vec<(ValueId, String)> = values
        .iter()
        .map(|b| (b.id(), String::from_utf8_lossy(b).into_owned()))
        .collect();
    let read = |v: ValueId| {
        texts
            .iter()
            .find(|(id, _)| *id == v)
            .map(|(_, t)| t.clone())
    };
    let said = staged(state, owner, &sheet, &read);
    Ok(Done {
        values,
        facts: vec![Fact::Page(
            owner,
            PageEvent::Edited {
                sheet: Box::new(sheet),
                by: call,
            },
        )],
        said,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Edits {
    edits: Vec<Replace>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Replace {
    old: String,
    new: String,
}

/// Apply each edit to the one part of the stage its old text occurs in
/// once, as file edits do.
fn edit(
    state: &State,
    owner: TaskId,
    call: CallId,
    input: Value,
    text_of: &dyn Fn(ValueId) -> Option<String>,
) -> Result<Done, String> {
    let Edits { edits } = parse(input)?;
    let stage = state
        .page(owner)
        .and_then(|p| p.stage.as_ref())
        .ok_or("nothing is staged yet: write the page with `page_write` first")?;
    let sheet = &stage.sheet;
    // The parts an edit can reach, as text: the plain fields and the bodies.
    let mut parts: Vec<String> = vec![
        sheet.lead.title.clone(),
        sheet.lead.hook.clone().unwrap_or_default(),
        sheet.end.says.clone(),
        body_text(sheet.end.shows, text_of),
        sheet.now.says.clone(),
        body_text(sheet.now.shows, text_of),
        body_text(sheet.why, text_of),
    ];
    for Replace { old, new } in &edits {
        let at: Vec<usize> = (parts.iter().enumerate())
            .filter(|(_, p)| !old.is_empty() && p.contains(old.as_str()))
            .map(|(i, _)| i)
            .collect();
        match (
            at.as_slice(),
            at.first().map(|i| parts[*i].matches(old.as_str()).count()),
        ) {
            ([i], Some(1)) => parts[*i] = parts[*i].replacen(old.as_str(), new, 1),
            ([], _) => return Err(format!("nothing was edited: `{old}` is not on the page")),
            _ => {
                return Err(format!(
                    "nothing was edited: `{old}` occurs more than once; give more of the text around it"
                ));
            }
        }
    }
    let [title, hook, end, end_shows, now, now_shows, why] =
        <[String; 7]>::try_from(parts).expect("seven parts");
    let lift = |t: String| Some(t).filter(|t| !t.is_empty());
    let mut written = json!({
        "lead": { "title": title, "hook": lift(hook) },
        "end": { "says": end, "shows": lift(end_shows) },
        "now": { "says": now, "shows": lift(now_shows) },
        "why": lift(why),
    });
    // Absent rather than null, as a writer would give it.
    strip_nulls(&mut written);
    write(state, owner, call, written)
}

fn strip_nulls(value: &mut Value) {
    if let Value::Object(map) = value {
        map.retain(|_, v| !v.is_null());
        map.values_mut().for_each(strip_nulls);
    }
}

fn body_text(body: Option<ValueId>, text_of: &dyn Fn(ValueId) -> Option<String>) -> String {
    body.and_then(text_of).unwrap_or_default()
}

/// What a `page_write` or `page_edit` says: staged, and each problem with
/// how to write it instead.
fn staged(
    state: &State,
    owner: TaskId,
    sheet: &Sheet,
    text_of: &dyn Fn(ValueId) -> Option<String>,
) -> String {
    let mut found: Vec<(Spot, Option<u32>, Problem)> = Vec::new();
    let title = state
        .task(owner)
        .map(|t| t.description.title.clone().unwrap_or(t.name.clone()));
    if sheet.lead.title.chars().count() > 70 || Some(&sheet.lead.title) == title.as_ref() {
        found.push((Spot::Lead, None, Problem::Lead));
    }
    for (spot, claim) in [(Spot::End, &sheet.end), (Spot::Now, &sheet.now)] {
        if claim.says.chars().count() > 160 || sentences(&claim.says) > 1 {
            found.push((spot.clone(), None, Problem::Claim));
        }
    }
    for (spot, body) in [
        (Spot::End, sheet.end.shows),
        (Spot::Now, sheet.now.shows),
        (Spot::Why, sheet.why),
    ] {
        if let Some(markdown) = body.and_then(text_of) {
            let (_, problems) = harness_body::body(&markdown, KINDS);
            found.extend(
                problems
                    .into_iter()
                    .map(|f| (spot.clone(), f.line, f.problem)),
            );
        }
    }
    let mut said = String::from(
        "Staged. Max sees it when you `present` it; until then he sees the version he was last shown.",
    );
    if !found.is_empty() {
        said.push_str(" It is drawn as well as it can be, but these differ from what you wrote:");
        for (spot, line, problem) in found {
            let at = match line {
                Some(line) => format!("{}, line {line}", spot_name(&spot)),
                None => spot_name(&spot).to_string(),
            };
            said.push_str(&format!("\n- {at}: {}", explain(&problem)));
        }
    }
    said
}

/// How many sentences `text` has, roughly: full stops, question and
/// exclamation marks followed by a space and more text.
fn sentences(text: &str) -> usize {
    let text = text.trim();
    1 + text
        .match_indices(['.', '?', '!'])
        .filter(|(i, _)| text[i + 1..].starts_with(' ') && i + 2 < text.len())
        .count()
}

fn spot_name(spot: &Spot) -> &'static str {
    match spot {
        Spot::Lead => "The lead",
        Spot::End => "Where this ends",
        Spot::Now => "Where we are",
        Spot::Step(_) => "A step",
        Spot::Ask(_) => "An ask",
        Spot::Why => "Why this route",
    }
}

/// What a problem means to the agent, and how to write it instead.
pub fn explain(problem: &Problem) -> String {
    match problem {
        Problem::Heading => "a heading, drawn as plain text: the page's parts are its only structure, so say it in a sentence.".into(),
        Problem::Bold => "bold, drawn regular: bold is the harness's, for what Max must answer.".into(),
        Problem::Italic => "italic, drawn upright: nothing is italic, since it is harder to read.".into(),
        Problem::Styled => "HTML or styling, drawn as text or dropped: the theme owns the look.".into(),
        Problem::Kind { kind, kinds } if kinds.is_empty() => format!("`{kind}:` names no kind the harness reads, drawn as its words: there are no kinds yet, so link to its URL."),
        Problem::Kind { kind, kinds } => format!("`{kind}:` is not a kind the harness reads ({}), drawn as its words.", kinds.join(", ")),
        Problem::NotFound(of) => format!("{of:?} is not known, drawn as its words."),
        Problem::NoAlt => "an image with no alt text: say what it shows.".into(),
        Problem::Misquoted => "words given as Max's that he did not say, drawn as a plain quote.".into(),
        Problem::Figure { error } => format!("a figure that would not draw: {error}"),
        Problem::Table { why } => format!("a table that reads badly: {why}."),
        Problem::Nested => "a fold inside a fold, drawn open: folds do not nest.".into(),
        Problem::Summary => "a fold's summary that is a label: say what is inside, in under 100 characters.".into(),
        Problem::Lead => "over 70 characters, or the task's title again: the lead is the question, problem or news this moment turns on.".into(),
        Problem::Claim => "more than one sentence, or over 160 characters: the claim is one sentence, and its body shows it.".into(),
    }
}

fn present(state: &State, owner: TaskId, call: CallId) -> Result<Done, String> {
    let page = state.page(owner);
    let stage = page
        .and_then(|p| p.stage.as_ref())
        .ok_or("there is no page to present: write it with `page_write` first")?;
    let said = match page.and_then(|p| p.versions.last()) {
        Some(newest) if newest.sheet == stage.sheet => format!(
            "Presented: version {} still holds. Your turn ends once this round's calls have settled.",
            newest.n
        ),
        newest => format!(
            "Presented as version {}. Your turn ends once this round's calls have settled.",
            newest.map_or(1, |v| v.n + 1)
        ),
    };
    Ok(Done {
        values: Vec::new(),
        facts: vec![Fact::Page(owner, PageEvent::Presented { by: call })],
        said,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DescribeIn {
    title: Option<String>,
    plan: Option<Vec<StepIn>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StepIn {
    key: String,
    title: Option<String>,
    after: Option<Vec<String>>,
    of: Option<String>,
    #[serde(default)]
    said: SaidIn,
    failed: Option<String>,
    body: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum SaidIn {
    #[default]
    Planned,
    Doing,
    Done,
    Failed,
}

fn describe(owner: TaskId, input: Value) -> Result<Done, String> {
    let DescribeIn { title, plan } = parse(input)?;
    let mut values = Vec::new();
    let plan = match plan {
        None => None,
        Some(steps) => {
            let mut out: Vec<Step> = Vec::new();
            for s in steps {
                if out.iter().any(|o| o.key == s.key) {
                    return Err(format!("two steps have the key `{}`", s.key));
                }
                let said = match s.said {
                    SaidIn::Planned => Said::Planned,
                    SaidIn::Doing => Said::Doing,
                    SaidIn::Done => Said::Done,
                    SaidIn::Failed => Said::Failed {
                        why: s.failed.unwrap_or_default(),
                    },
                };
                let body = s.body.map(|m| {
                    let bytes = Bytes::from(m);
                    let id = bytes.id();
                    values.push(bytes);
                    id
                });
                out.push(Step {
                    key: s.key,
                    title: s.title,
                    after: s.after,
                    of: s.of.as_deref().map(reference).transpose()?,
                    said,
                    body,
                });
            }
            for step in &out {
                for key in step.after.iter().flatten() {
                    if !out.iter().any(|o| &o.key == key) {
                        return Err(format!(
                            "step `{}` is after `{key}`, which is not in the plan",
                            step.key
                        ));
                    }
                }
            }
            Some(out)
        }
    };
    let said = match (&title, &plan) {
        (_, Some(plan)) => format!("Described: a plan of {} steps.", plan.len()),
        (Some(_), None) => "Described: the title is set.".to_string(),
        (None, None) => "Nothing to describe: give a `title`, a `plan`, or both.".to_string(),
    };
    Ok(Done {
        values,
        facts: vec![Fact::Task(
            owner,
            TaskEvent::Described(Description { title, plan }),
        )],
        said,
    })
}

/// A live name as a page or ask writes it: `task:<id>` or `<kind>:<name>`.
fn reference(text: &str) -> Result<Ref, String> {
    if let Some(id) = text.strip_prefix("task:") {
        return id
            .parse()
            .map(Ref::Task)
            .map_err(|_| format!("`{text}` names no task"));
    }
    match text.split_once(':') {
        Some((kind, name)) if kind.contains('.') => Ok(Ref::Item(ItemKey {
            kind: kind.into(),
            name: name.into(),
        })),
        _ => Err(format!("`{text}` is not `task:<id>` or `<kind>:<name>`")),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AskIn {
    key: String,
    question: String,
    end: String,
    now: String,
    why: String,
    shows: Option<String>,
    want: WantIn,
    options: Option<Vec<ChoiceIn>>,
    recommended: Option<String>,
    link: Option<Link>,
    paste: Option<String>,
    step: Option<String>,
    about: Option<String>,
    otherwise: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum WantIn {
    Go,
    Answer,
    Choose,
    Do,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChoiceIn {
    key: String,
    label: String,
    means: Option<String>,
}

/// An `ask`: the background call that is the ask, made in this batch and
/// open until it settles. Asking again under an open ask's key revises it.
fn ask(
    state: &State,
    task: &Task,
    call: CallId,
    input: Value,
    mint: &dyn Fn(usize) -> EventId,
) -> Result<Done, String> {
    let a: AskIn = parse(input)?;
    let want = match a.want {
        WantIn::Go => Want::Go,
        WantIn::Answer => Want::Answer,
        WantIn::Choose => {
            let options: Vec<Choice> = (a.options.unwrap_or_default().into_iter())
                .map(|c| Choice {
                    key: c.key,
                    label: c.label,
                    means: c.means,
                })
                .collect();
            if options.len() < 2 {
                return Err("`choose` needs at least two `options`".into());
            }
            if let Some(r) = &a.recommended
                && !options.iter().any(|o| &o.key == r)
            {
                return Err(format!("`recommended` names `{r}`, which is not an option"));
            }
            Want::Choose {
                options,
                recommended: a.recommended,
            }
        }
        WantIn::Do => Want::Do {
            link: a.link,
            paste: a.paste,
        },
    };
    let mut values = Vec::new();
    let shows = a.shows.map(|m| {
        let bytes = Bytes::from(m);
        let id = bytes.id();
        values.push(bytes);
        id
    });
    let ask = Ask {
        key: a.key.clone(),
        question: a.question,
        end: a.end,
        now: a.now,
        why: a.why,
        shows,
        want,
        step: a.step,
        about: a.about.as_deref().map(reference).transpose()?,
        otherwise: a.otherwise.map(|then| Otherwise { then, at: None }),
    };
    let input = Bytes::from(serde_json::to_vec(&ask).expect("an ask serialises"));
    let mut facts = Vec::new();
    let revised = open_ask(state, task, &a.key);
    if let Some(old) = revised {
        let (model, outcome) = completed(&format!(
            "Revised: you asked again under `{}`, and the new ask replaces this one.",
            a.key
        ));
        values.push(model);
        facts.push(Fact::Call(old, CallEvent::Settled(outcome)));
    } else if meta::named(state, task, &a.key).is_some() {
        return Err(format!(
            "you already have background work named `{}` running",
            a.key
        ));
    }
    let made = CallId(mint(facts.len()));
    facts.push(Fact::Call(
        made,
        CallEvent::Made(Call {
            caller: Caller::Background {
                task: task.id,
                from: call,
                name: a.key.clone(),
            },
            expects: Vec::new(),
            tool: ASK.into(),
            input: input.id(),
        }),
    ));
    values.push(input);
    let step = match &ask.step {
        Some(key) => format!(" It is drawn in the step `{key}`."),
        None => String::new(),
    };
    let then = match &ask.otherwise {
        Some(_) => "Carry on with what you said you would do meanwhile.",
        None => "End your turn by presenting your page if you can't go on without it.",
    };
    let verb = if revised.is_some() {
        "Revised"
    } else {
        "Asked"
    };
    Ok(Done {
        values,
        facts,
        said: format!(
            "{verb} `{}` (`{}`): it is on your page from your next `present`, and Max's answer comes to you as a message when it settles. {then}{step}",
            a.key,
            made.label()
        ),
    })
}

/// `task`'s open ask under `key`.
fn open_ask(state: &State, task: &Task, key: &str) -> Option<CallId> {
    task.background.iter().copied().find(|c| {
        state.call(*c).is_some_and(|r| {
            r.call.tool == ASK
                && r.settled.is_none()
                && matches!(&r.call.caller, Caller::Background { name, .. } if name == key)
        })
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Settle {
    key: String,
    why: Option<String>,
    words: Option<String>,
}

/// A `withdraw` or a `cite`: the asker closes one of its open asks, with
/// why, or on Max's words.
fn settle(state: &State, task: &Task, tool: &str, input: Value) -> Result<Done, String> {
    let Settle { key, why, words } = parse(input)?;
    let ask = open_ask(state, task, &key).ok_or(format!("you have no open ask `{key}`"))?;
    let said = match tool {
        WITHDRAW => format!(
            "Withdrawn: {}",
            why.ok_or("say `why` you withdraw it: Max sees it where the ask was")?
        ),
        _ => {
            let label = words.ok_or("name the label of Max's words that answer it (`max-…`)")?;
            meta::quoted(state, task, &label)?;
            format!("Answered by Max's words `{label}`.")
        }
    };
    let (model, outcome) = completed(&said);
    Ok(Done {
        values: vec![model],
        facts: vec![Fact::Call(ask, CallEvent::Settled(outcome))],
        said: format!("Closed `{key}`."),
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PassOn {
    words: String,
    note: String,
}

/// A `pass_on`: Max's words about how pages look or read, kept for the
/// design keeper.
fn pass_on(
    state: &State,
    task: &Task,
    input: Value,
    mint: &dyn Fn(usize) -> EventId,
) -> Result<Done, String> {
    let PassOn { words, note } = parse(input)?;
    let quoted = meta::quoted(state, task, &words)?;
    let delivery = said_in(state, task, quoted).ok_or(format!(
        "`{words}` is not words Max said to you in a message"
    ))?;
    let id = FeedbackId(mint(0));
    Ok(Done {
        values: Vec::new(),
        facts: vec![Fact::Feedback(
            id,
            FeedbackEvent::Given(Feedback::Words {
                words: delivery,
                by: task.id,
                note,
            }),
        )],
        said: "Passed on to the design keeper: it is kept, open until the keeper answers it."
            .into(),
    })
}

/// The delivery to `task` that carried Max's words `value`.
fn said_in(state: &State, task: &Task, value: ValueId) -> Option<DeliveryId> {
    state.deliveries().find_map(|d| match d.delivery.content {
        harness_core::delivery::Content::Words(w) if w == value && d.delivery.to == task.id => {
            Some(d.delivery.id)
        }
        _ => None,
    })
}

fn completed(said: &str) -> (Bytes, Outcome) {
    let model = blocks(&[text(said)]);
    let output = Output {
        model: model.id(),
        full: None,
        observes: Vec::new(),
    };
    (model, Outcome::Completed(output))
}

/// The tool definitions.
pub fn tools() -> Vec<Tool> {
    let markdown = |what: &str| json!({ "type": "string", "description": what });
    let claim = |what: &str| {
        json!({
            "type": "object",
            "properties": {
                "says": { "type": "string", "description": what },
                "shows": markdown("Body markdown that shows the claim, visual first.")
            },
            "required": ["says"],
            "additionalProperties": false
        })
    };
    let definitions = [
        json!({
            "name": PAGE_WRITE,
            "description": "Stage your whole page: the lead, where this ends, where we are, and why this route. Max sees it when you `present` it.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "lead": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string", "description": "The question, problem or news this moment turns on, in under 70 characters." },
                            "hook": { "type": "string", "description": "One or two sentences under it, when the title alone would not say why it matters now." }
                        },
                        "required": ["title"],
                        "additionalProperties": false
                    },
                    "end": claim("Where this ends, in one sentence: the world when the work is done."),
                    "now": claim("Where we are, in one sentence: the fact today."),
                    "why": markdown("Why this route, in body markdown: the routes not taken and the calls you made for Max.")
                },
                "required": ["lead", "end", "now"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": PAGE_EDIT,
            "description": "Edit your staged page: each `old` text, which must occur once on it, becomes `new`.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "edits": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": { "old": { "type": "string" }, "new": { "type": "string" } },
                            "required": ["old", "new"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["edits"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": PRESENT,
            "description": "Show Max your staged page and end your turn on his move, once this round's calls have settled.",
            "input_schema": { "type": "object", "properties": {}, "additionalProperties": false }
        }),
        json!({
            "name": DESCRIBE,
            "description": "Say your title, your plan, or both: each replaces only itself. The plan is drawn on your page with your asks inside their steps.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "What the work does, in Max's nouns." },
                    "plan": {
                        "type": "array",
                        "description": "Every step, done ones included, in order. Replaces the plan whole.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "key": { "type": "string", "description": "Your name for it, unique in the plan; never shown." },
                                "title": { "type": "string", "description": "What it makes true, short enough for a node." },
                                "after": { "type": "array", "items": { "type": "string" }, "description": "The keys of the steps it waits on. Default: the step before it." },
                                "of": { "type": "string", "description": "The live thing it is, once there is one: `task:<id>`." },
                                "said": { "type": "string", "enum": ["planned", "doing", "done", "failed"] },
                                "failed": { "type": "string", "description": "Why, when it failed." },
                                "body": markdown("Body markdown: why this step, what it changes.")
                            },
                            "required": ["key", "title"],
                            "additionalProperties": false
                        }
                    }
                },
                "additionalProperties": false
            }
        }),
        json!({
            "name": ASK,
            "description": "Ask Max one thing, drawn whole on your page inside the step it unblocks. Returns at once; his answer comes to you as a message.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "Your name for it. Asking again under an open ask's key revises it." },
                    "question": { "type": "string", "description": "What he is asked, in words he can answer by talking." },
                    "end": { "type": "string", "description": "What is true once it is answered and acted on." },
                    "now": { "type": "string", "description": "Where it is now: inline markdown." },
                    "why": { "type": "string", "description": "Why this ask gets from now to the end, and why it is his: inline markdown." },
                    "shows": markdown("Body markdown that shows the choice: the options as a table, a figure."),
                    "want": { "type": "string", "enum": ["go", "answer", "choose", "do"], "description": "go: his go before costly, outward or one-way work. answer: his words. choose: one of `options`. do: something only he can do, at `link`." },
                    "options": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "key": { "type": "string" },
                                "label": { "type": "string", "description": "The answer's words." },
                                "means": { "type": "string", "description": "What choosing it means and costs." }
                            },
                            "required": ["key", "label"],
                            "additionalProperties": false
                        }
                    },
                    "recommended": { "type": "string", "description": "The key of the option you would take." },
                    "link": {
                        "type": "object",
                        "properties": { "url": { "type": "string" }, "label": { "type": "string" } },
                        "required": ["url", "label"],
                        "additionalProperties": false
                    },
                    "paste": { "type": "string", "description": "Text he will need there, drawn with a copy button." },
                    "step": { "type": "string", "description": "The key of the plan step it unblocks." },
                    "about": { "type": "string", "description": "The live thing it is about: `task:<id>`." },
                    "otherwise": { "type": "string", "description": "What you do meanwhile without his answer. Leave it out to wait for him." }
                },
                "required": ["key", "question", "end", "now", "why", "want"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": WITHDRAW,
            "description": "Take back one of your open asks: no longer needed, or overtaken.",
            "input_schema": {
                "type": "object",
                "properties": { "key": { "type": "string" }, "why": { "type": "string", "description": "Shown to Max where the ask was." } },
                "required": ["key", "why"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": CITE,
            "description": "Close one of your open asks on what Max said: name the label of his words that answer it.",
            "input_schema": {
                "type": "object",
                "properties": { "key": { "type": "string" }, "words": { "type": "string", "description": "The label of his words, `max-…`." } },
                "required": ["key", "words"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": PASS_ON,
            "description": "Pass Max's words about how pages look or read to the design keeper, whole.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "words": { "type": "string", "description": "The label of his words, `max-…`." },
                    "note": { "type": "string", "description": "What on your page he seems to mean." }
                },
                "required": ["words", "note"],
                "additionalProperties": false
            }
        }),
    ];
    definitions
        .into_iter()
        .map(|definition| Tool {
            name: definition["name"].as_str().expect("a name").into(),
            definition: definition.to_string(),
            audience: Audience::Agent,
            held: false,
        })
        .collect()
}
