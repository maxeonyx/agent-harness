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

use harness_core::ask::{Ask, Choice, Link, Otherwise, Verdict, Want};
use harness_core::body::{Block, Body, Inline, Problem, Spotted};
use harness_core::call::{Call, CallEvent, Caller, Outcome, Output};
use harness_core::design::{Feedback, FeedbackEvent};
use harness_core::fact::Fact;
use harness_core::id::{CallId, DeliveryId, EventId, FeedbackId, TaskId, ValueId};
use harness_core::limb::{Audience, Tool};
use harness_core::page::{Description, Lead, PageEvent, Ref, Said, Sheet, Spot, Step};
use harness_core::source::ItemKey;
use harness_core::state::State;
use harness_core::task::{Task, TaskEvent};
use harness_core::value::Bytes;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::meta;
use crate::render::{Values, blocks, text};

pub use harness_core::task::PRESENT;
pub const PAGE_WRITE: &str = "page_write";
pub const PAGE_EDIT: &str = "page_edit";
pub const DESCRIBE: &str = "describe";
pub use harness_core::ask::ASK;
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
    state.page_of(task.id)
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

/// A call's input as a value.
fn input_of(input: &[u8]) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(input).map_err(|e| format!("invalid input: {e}"))?;
    value.as_object().ok_or("the input must be an object")?;
    Ok(value)
}

/// Keep `markdown` as a value of the batch, and name it.
fn stash(values: &mut Vec<Bytes>, markdown: String) -> ValueId {
    let bytes = Bytes::from(markdown);
    let id = bytes.id();
    values.push(bytes);
    id
}

fn parse<T: for<'de> Deserialize<'de>>(input: Value) -> Result<T, String> {
    serde_json::from_value(input).map_err(|e| e.to_string())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Write {
    lead: String,
    facts: Option<String>,
    why: Option<String>,
}

fn write(state: &State, owner: TaskId, call: CallId, input: Value) -> Result<Done, String> {
    let Write { lead, facts, why } = parse(input)?;
    let mut values = Vec::new();
    let mut body = |markdown: Option<String>| markdown.map(|m| stash(&mut values, m));
    let sheet = Sheet {
        lead: Lead { title: lead },
        facts: body(facts),
        why: body(why),
    };
    let read = |v: ValueId| {
        (values.iter())
            .find(|b| b.id() == v)
            .map(|b| String::from_utf8_lossy(b).into_owned())
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
    // The parts an edit can reach, as text: the lead and the bodies.
    let mut parts: Vec<String> = vec![
        sheet.lead.title.clone(),
        body_text(sheet.facts, text_of),
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
    let [lead, facts, why] = <[String; 3]>::try_from(parts).expect("three parts");
    let lift = |t: String| Some(t).filter(|t| !t.is_empty());
    let written = json!({ "lead": lead, "facts": lift(facts), "why": lift(why) });
    write(state, owner, call, written)
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
    let mut found: Vec<Spotted> = Vec::new();
    let title = state
        .task(owner)
        .map(|t| t.description.title.clone().unwrap_or(t.name.clone()));
    if sheet.lead.title.chars().count() > 70 || Some(&sheet.lead.title) == title.as_ref() {
        found.push(Spotted {
            at: Spot::Lead,
            line: None,
            problem: Problem::Lead,
        });
    }
    for (spot, body) in [(Spot::Facts, sheet.facts), (Spot::Why, sheet.why)] {
        if let Some(markdown) = body.and_then(text_of) {
            let (body, problems) = harness_body::body(&markdown, KINDS);
            if spot == Spot::Facts && told(&body) {
                found.push(Spotted {
                    at: Spot::Facts,
                    line: None,
                    problem: Problem::Told,
                });
            }
            found.extend(problems.into_iter().map(|f| Spotted {
                at: spot.clone(),
                line: f.line,
                problem: f.problem,
            }));
        }
    }
    listed(
        "Staged. Max sees it when you `present` it; until then he sees the version he was last shown.",
        found,
    )
}

/// `said`, then each problem found with how to write it instead.
fn listed(said: &str, found: Vec<Spotted>) -> String {
    let mut said = said.to_string();
    if !found.is_empty() {
        said.push_str(" It is drawn as well as it can be, but these differ from what you wrote:");
        for Spotted { at, line, problem } in found {
            let at = match line {
                Some(line) => format!("{}, line {line}", spot_name(&at)),
                None => spot_name(&at).to_string(),
            };
            said.push_str(&format!("\n- {at}: {}", explain(&problem)));
        }
    }
    said
}

/// Whether facts are told rather than shown: over about 80 words of
/// paragraphs, with nothing visual (a table, figure, code, quote or Max's
/// own words) anywhere in them.
fn told(body: &Body) -> bool {
    fn shown(block: &Block) -> bool {
        match block {
            Block::Table(_) | Block::Figure(_) | Block::Code { .. } | Block::Quote(_) => true,
            Block::Said { .. } => true,
            Block::List { items, .. } => items.iter().any(|i| i.blocks.iter().any(shown)),
            Block::Fold(fold) => fold.body.iter().any(shown),
            Block::Para(_) => false,
        }
    }
    fn words(block: &Block) -> usize {
        match block {
            Block::Para(inlines) => (inlines.iter())
                .map(|i| match i {
                    Inline::Text(t) => t.split_whitespace().count(),
                    Inline::Link { words, .. } => words.split_whitespace().count(),
                    _ => 0,
                })
                .sum(),
            Block::List { items, .. } => items.iter().flat_map(|i| &i.blocks).map(words).sum(),
            _ => 0,
        }
    }
    !body.blocks.iter().any(shown) && body.blocks.iter().map(words).sum::<usize>() > 80
}

fn spot_name(spot: &Spot) -> &'static str {
    match spot {
        Spot::Lead => "The lead",
        Spot::Facts => "The facts",
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
        Problem::Nested => "nested too deep, drawn as its text: folds do not nest, and nothing nests past 16 levels.".into(),
        Problem::Summary => "a fold's summary that is a label: say what is inside, in under 100 characters.".into(),
        Problem::Lead => "over 70 characters, or the task's title again: the lead is the question of an ask on your page, or else the one fact Max most needs, in a line.".into(),
        Problem::Told => "told rather than shown: over 80 words of paragraphs with no table, figure, code or quote. Put the facts that line up in a table, the system in a figure, the log line as code, and keep only the sentences a picture can't carry.".into(),
        Problem::Now => "over about fifteen words: a step's now is where its thing stands, in a few words (\"#284, live since 10:14\"); put the detail in its body.".into(),
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
    now: Option<String>,
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
    let mut found: Vec<Spotted> = Vec::new();
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
                let at = Spot::Step(s.key.clone());
                if (s.now.as_deref()).is_some_and(|n| n.split_whitespace().count() > 15) {
                    found.push(Spotted {
                        at: at.clone(),
                        line: None,
                        problem: Problem::Now,
                    });
                }
                for text in s.now.iter() {
                    let (_, problems) = harness_body::inline(text, KINDS);
                    found.extend(problems.into_iter().map(|f| Spotted {
                        at: at.clone(),
                        line: None,
                        problem: f.problem,
                    }));
                }
                for markdown in s.body.iter() {
                    let (_, problems) = harness_body::body(markdown, KINDS);
                    found.extend(problems.into_iter().map(|f| Spotted {
                        at: at.clone(),
                        line: f.line,
                        problem: f.problem,
                    }));
                }
                let body = s.body.map(|m| stash(&mut values, m));
                out.push(Step {
                    key: s.key,
                    title: s.title,
                    after: s.after,
                    of: s.of.as_deref().map(reference).transpose()?,
                    said,
                    now: s.now,
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
        (_, Some(plan)) => listed(
            &format!("Described: a plan of {} steps.", plan.len()),
            found,
        ),
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
    let shows = a.shows.map(|m| stash(&mut values, m));
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
    facts.push(Fact::Page(
        owner(state, task),
        PageEvent::Asked {
            call: made,
            revises: revised,
        },
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

/// Settle the open ask `call` on the answer Max pressed for it
/// ([`State::pressed`]): what the asker reads as the call's result.
pub fn pressed(state: &State, values: Values, call: CallId) -> Result<Done, String> {
    let (delivery, verdict) = state.pressed(call).ok_or("no answer was pressed")?;
    let record = state.call(call).ok_or("no such ask")?;
    let ask: Ask = serde_json::from_slice(&values(record.call.input))
        .map_err(|e| format!("the ask's input does not read: {e}"))?;
    let answer = match (verdict, &ask.want) {
        (Verdict::Yes, Want::Go) => "Go".to_string(),
        (Verdict::Yes, Want::Do { .. }) => "Done".to_string(),
        (Verdict::Yes, _) => "Yes".to_string(),
        (Verdict::No, _) => "No".to_string(),
        (Verdict::Chose(key), Want::Choose { options, .. }) => {
            match options.iter().find(|o| &o.key == key) {
                Some(o) => format!("{} (`{key}`)", o.label),
                None => format!("`{key}`"),
            }
        }
        (Verdict::Chose(key), _) => format!("`{key}`"),
        (Verdict::Reject, _) => {
            "Wrong question: he rejects its frame, so ask again, revised, or withdraw it"
                .to_string()
        }
    };
    let words = match state.delivery(*delivery).map(|d| &d.delivery.content) {
        Some(harness_core::delivery::Content::Words(w)) => *w,
        _ => return Err("the press came with no words".into()),
    };
    let said = match crate::render::words(&values(words)).is_empty() {
        true => format!("Max answered by pressing: {answer}. He sent no words with it."),
        false => format!(
            "Max answered by pressing: {answer}, with his words `{}`.",
            crate::render::label(words)
        ),
    };
    let (model, outcome) = completed(&said);
    Ok(Done {
        values: vec![model],
        facts: vec![Fact::Call(call, CallEvent::Settled(outcome))],
        said,
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
    let definitions = [
        json!({
            "name": PAGE_WRITE,
            "description": "Stage your whole page: the lead, the facts and why this route. Your plan, from `describe`, is drawn between the lead and the facts. Max sees it when you `present` it.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "lead": { "type": "string", "description": "The question of an ask on this page, or else the fact Max most needs: the problem or the news, bad news first. Under 70 characters; never a request." },
                    "facts": markdown("What backs the lead and the steps' nows, shown: the log line, the table of what you checked, the before and after, a figure. Body markdown."),
                    "why": markdown("What he might overturn: the routes he'd think of and why not, the calls you made for him, where the plan stops short. Body markdown; a table where they line up.")
                },
                "required": ["lead"],
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
            "description": "Say your title, your plan, or both: each replaces only itself. The plan is where your work ends and where each part of it stands, drawn on your page with your asks inside their steps.",
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
                                "title": { "type": "string", "description": "What will be true once it's done, in Max's nouns, short enough for a node." },
                                "after": { "type": "array", "items": { "type": "string" }, "description": "The keys of the steps it waits on. Default: the step before it." },
                                "of": { "type": "string", "description": "The live thing it is, once there is one: `task:<id>`." },
                                "said": { "type": "string", "enum": ["planned", "doing", "done", "failed"] },
                                "failed": { "type": "string", "description": "Why, when it failed." },
                                "now": { "type": "string", "description": "Where what the title names stands today, in a few words; a done step's is its proof. Inline markdown." },
                                "body": markdown("Body markdown: what the step changes and how, and why it is there if it is your own addition.")
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
                    "end": { "type": "string", "description": "What is true once it is answered and acted on, in a few words." },
                    "now": { "type": "string", "description": "Where that stands now, in a few words: inline markdown." },
                    "why": { "type": "string", "description": "Why the answer gets from now to the end, and why it is his to give: inline markdown." },
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
                    "otherwise": { "type": "string", "description": "What you do meanwhile, as a sentence that stands alone: \"Until you answer, I build on the defaults.\" Leave it out to wait for him." }
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
