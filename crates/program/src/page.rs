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
use harness_core::page::{
    Description, End, Lead, PageEvent, Ref, Said, Sheet, Spot, Stands, Step, Use,
};
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
        DESCRIBE => describe(state, owner, input),
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
    now: Option<String>,
    why: Option<String>,
}

fn write(state: &State, owner: TaskId, call: CallId, input: Value) -> Result<Done, String> {
    let Write { lead, now, why } = parse(input)?;
    let mut values = Vec::new();
    let mut body = |markdown: Option<String>| markdown.map(|m| stash(&mut values, m));
    let sheet = Sheet {
        lead: Lead { title: lead },
        now: body(now),
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
        body_text(sheet.now, text_of),
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
    let [lead, now, why] = <[String; 3]>::try_from(parts).expect("three parts");
    let lift = |t: String| Some(t).filter(|t| !t.is_empty());
    let written = json!({ "lead": lead, "now": lift(now), "why": lift(why) });
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
    let task = state.task(owner);
    let title = task.map(|t| t.description.title.clone().unwrap_or(t.name.clone()));
    let end = task.and_then(|t| t.description.end.as_ref());
    let lead = &sheet.lead.title;
    if lead.chars().count() > 70
        || Some(lead) == title.as_ref()
        || end.is_some_and(|e| &e.goal == lead)
    {
        found.push(Spotted {
            at: Spot::Lead,
            line: None,
            problem: Problem::Lead,
        });
    }
    // The end is there so Max can check the aim before he reads a plan: a
    // page with no plan, such as a one-off answer, needs none.
    let planned = (task.and_then(|t| t.description.plan.as_ref())).is_some_and(|p| !p.is_empty());
    if planned && end.is_none() {
        found.push(Spotted {
            at: Spot::End,
            line: None,
            problem: Problem::Unseen,
        });
    }
    for (spot, body) in [(Spot::Now, sheet.now), (Spot::Why, sheet.why)] {
        if let Some(markdown) = body.and_then(text_of) {
            let (body, problems) = harness_body::body(&markdown, KINDS);
            if told(&body) {
                found.push(Spotted {
                    at: spot.clone(),
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
                None => spot_name(&at),
            };
            said.push_str(&format!("\n- {at}: {}", explain(&problem)));
        }
    }
    said
}

/// Whether a body is told rather than shown: over about 80 words of
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

/// Whether `blocks` hold a figure anywhere: a list's items and a fold's
/// body count, since the figure is still drawn there.
fn pictured(blocks: &[Block]) -> bool {
    blocks.iter().any(|b| match b {
        Block::Figure(_) => true,
        Block::List { items, .. } => items.iter().any(|i| pictured(&i.blocks)),
        Block::Fold(fold) => pictured(&fold.body),
        Block::Quote(quoted) => pictured(quoted),
        _ => false,
    })
}

fn words(text: &str) -> usize {
    text.split_whitespace().count()
}

/// A now, as a use or a step writes it: over about fifteen words, and the
/// inline problems in it.
fn check_now(now: Option<&str>, at: &Spot, found: &mut Vec<Spotted>) {
    let Some(now) = now else { return };
    if words(now) > 15 {
        found.push(Spotted {
            at: at.clone(),
            line: None,
            problem: Problem::Wordy,
        });
    }
    let (_, problems) = harness_body::inline(now, KINDS);
    found.extend(problems.into_iter().map(|f| Spotted {
        at: at.clone(),
        line: None,
        problem: f.problem,
    }));
}

fn spot_name(spot: &Spot) -> String {
    match spot {
        Spot::Lead => "The lead".into(),
        Spot::End => "The end".into(),
        Spot::Use(key) => format!("The use `{key}`"),
        Spot::Now => "The now".into(),
        Spot::Step(key) => format!("The step `{key}`"),
        Spot::Ask(_) => "An ask".into(),
        Spot::Why => "Why this route".into(),
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
        Problem::Lead => "over 70 characters, or the task's title or the end's goal again: the lead is the question of an ask on your page, or else the one fact Max most needs, in a line.".into(),
        Problem::Told => "told rather than shown: over 80 words of paragraphs with no table, figure, code or quote. Put what lines up in a table, the system in a figure, the log line as code, and keep only the sentences a picture can't carry.".into(),
        Problem::Unseen => "no end, or an end with no figure: `describe` the `end`: the goal in a line, the system as it will be as a ```mermaid figure (what doesn't exist yet `planned`) or the mockup, and the uses, so Max can check you're aiming at the right thing before he reads where it stands.".into(),
        Problem::Wordy => "too long to read at a glance: a goal is a line of about twenty words, and a use or a now a few words (\"You pan the map on your phone and it keeps up\", \"#284, live since 10:14\"); put the detail in a body.".into(),
        Problem::NoNow => "no `now`: say where it stands in a few words (\"in review\", \"only in the mockups\", \"#284, live since 10:14\"); a done step's now is its proof, not its body.".into(),
        Problem::Escaped => "line breaks that arrived as the two characters `\\n`, so it draws as one paragraph: send real line breaks.".into(),
        Problem::Repeated => "the same words as a use's now: the use already says where Max stands, so say where this step's own work stands (\"in review\", \"1 of 4 clones points at it\").".into(),
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
    end: Option<EndIn>,
    plan: Option<Vec<StepIn>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EndIn {
    goal: String,
    shows: Option<String>,
    #[serde(default)]
    uses: Vec<UseIn>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UseIn {
    key: String,
    does: String,
    stands: StandsIn,
    now: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum StandsIn {
    Works,
    Partly,
    NotYet,
    Broken,
    Unknown,
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

/// The end as `describe` gives it, its problems listed in `found`.
fn end_of(given: EndIn, values: &mut Vec<Bytes>, found: &mut Vec<Spotted>) -> Result<End, String> {
    let EndIn { goal, shows, uses } = given;
    let spot = |found: &mut Vec<Spotted>, at: &Spot, problem: Problem, line: Option<u32>| {
        found.push(Spotted {
            at: at.clone(),
            line,
            problem,
        })
    };
    if words(&goal) > 20 {
        spot(found, &Spot::End, Problem::Wordy, None);
    }
    let shows = match shows {
        Some(markdown) => {
            let (body, problems) = harness_body::body(&markdown, KINDS);
            if !pictured(&body.blocks) {
                spot(found, &Spot::End, Problem::Unseen, None);
            }
            for f in problems {
                spot(found, &Spot::End, f.problem, f.line);
            }
            Some(stash(values, markdown))
        }
        None => {
            spot(found, &Spot::End, Problem::Unseen, None);
            None
        }
    };
    let mut out: Vec<Use> = Vec::new();
    for u in uses {
        if out.iter().any(|o| o.key == u.key) {
            return Err(format!("two uses have the key `{}`", u.key));
        }
        let at = Spot::Use(u.key.clone());
        if words(&u.does) > 15 {
            spot(found, &at, Problem::Wordy, None);
        }
        let stands = match u.stands {
            StandsIn::Works => Stands::Works,
            StandsIn::Partly => Stands::Partly,
            StandsIn::NotYet => Stands::NotYet,
            StandsIn::Broken => Stands::Broken,
            StandsIn::Unknown => Stands::Unknown,
        };
        let now = u.now.filter(|n| !n.trim().is_empty());
        if stands != Stands::Works && now.is_none() {
            spot(found, &at, Problem::NoNow, None);
        }
        check_now(now.as_deref(), &at, found);
        out.push(Use {
            key: u.key,
            does: u.does,
            stands,
            now,
        });
    }
    Ok(End {
        goal,
        shows,
        uses: out,
    })
}

fn describe(state: &State, owner: TaskId, input: Value) -> Result<Done, String> {
    let DescribeIn { title, end, plan } = parse(input)?;
    let mut values = Vec::new();
    let mut found: Vec<Spotted> = Vec::new();
    let end = end
        .map(|e| end_of(e, &mut values, &mut found))
        .transpose()?;
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
                let now = s.now.filter(|n| !n.trim().is_empty());
                if matches!(said, Said::Done | Said::Doing) && s.of.is_none() && now.is_none() {
                    found.push(Spotted {
                        at: at.clone(),
                        line: None,
                        problem: Problem::NoNow,
                    });
                }
                check_now(now.as_deref(), &at, &mut found);
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
                    now,
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
    // A step's now that says again what a use's now says, in the end and
    // plan given now or before.
    let had = state.task(owner).and_then(|t| t.description.end.as_ref());
    let uses = end.as_ref().or(had).map_or(&[][..], |e| &e.uses[..]);
    let steps = plan
        .as_ref()
        .or(state.task(owner).and_then(|t| t.description.plan.as_ref()));
    for step in steps.into_iter().flatten() {
        let Some(now) = step.now.as_deref().map(str::trim) else {
            continue;
        };
        if uses.iter().any(|u| {
            u.now
                .as_deref()
                .is_some_and(|n| n.trim().eq_ignore_ascii_case(now))
        }) {
            found.push(Spotted {
                at: Spot::Step(step.key.clone()),
                line: None,
                problem: Problem::Repeated,
            });
        }
    }
    // A plan with no end to go with it, given now or before.
    if plan.as_ref().is_some_and(|p| !p.is_empty()) && end.is_none() && had.is_none() {
        found.push(Spotted {
            at: Spot::End,
            line: None,
            problem: Problem::Unseen,
        });
    }
    let mut parts = Vec::new();
    if title.is_some() {
        parts.push("the title".to_string());
    }
    if let Some(end) = &end {
        parts.push(format!("an end of {} uses", end.uses.len()));
    }
    if let Some(plan) = &plan {
        parts.push(format!("a plan of {} steps", plan.len()));
    }
    let said = match parts.is_empty() {
        true => {
            "Nothing to describe: give a `title`, an `end`, a `plan`, or any of them.".to_string()
        }
        false => listed(&format!("Described: {}.", parts.join(", ")), found),
    };
    Ok(Done {
        values,
        facts: vec![Fact::Task(
            owner,
            TaskEvent::Described(Description { title, end, plan }),
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
            "description": "Stage your page's written parts: the lead, the now and why this route. The harness draws the end from `describe` under the lead, then the now, then your plan with its asks, then why. Max sees it when you `present` it.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "lead": { "type": "string", "description": "The question of an ask on this page, or else the fact Max most needs: the problem or the news, bad news first. Under 70 characters; never a request, and never the end's goal again." },
                    "now": markdown("How things are today, shown: a screenshot, the output, the log line, the table of what you checked and what each showed. What backs the lead and the uses' marks; not the end or the plan again. Body markdown."),
                    "why": markdown("What he might overturn: the routes he'd think of and why not, the calls you made for him, where the plan stops short of the end. Body markdown; a table where they line up.")
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
            "description": "Say your title, where your work ends, your plan, or any of them: each replaces only itself. Max reads your end right under your page's lead, before anything else, and your plan last, with your asks inside their steps.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "What the work does, in Max's nouns." },
                    "end": {
                        "type": "object",
                        "description": "Where the work is going, stepped back far enough that Max can tell you're aiming at the right thing: his goal, the system as it will be, and what he does with it. Replaces the end whole.",
                        "properties": {
                            "goal": { "type": "string", "description": "What he wants it for, in a line: the reason the system exists, as you understand it: \"Every agent's page puts you level with it in a minute's reading\"." },
                            "shows": markdown("The system as it will be, as a figure: a ```mermaid block drawn top to bottom (`flowchart TB`, so it reads on a phone) of its parts, how they connect and where he is in it, with what doesn't exist yet `planned`, or the mockup. Then where your end departs from his words: quote them and say why, in a table. Body markdown."),
                            "uses": {
                                "type": "array",
                                "description": "What he does with it once it's done, most important first.",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "key": { "type": "string", "description": "Your name for it, unique in the end; never shown." },
                                        "does": { "type": "string", "description": "Who does what, and what happens, in a line: \"You pan the map on your phone and it keeps up\"." },
                                        "stands": { "type": "string", "enum": ["works", "partly", "not_yet", "broken", "unknown"], "description": "Whether it works today." },
                                        "now": { "type": "string", "description": "Unless it works, where it stands, in a few words: \"only in the mockups\". Inline markdown." }
                                    },
                                    "required": ["key", "does", "stands"],
                                    "additionalProperties": false
                                }
                            }
                        },
                        "required": ["goal", "uses"],
                        "additionalProperties": false
                    },
                    "plan": {
                        "type": "array",
                        "description": "The work from the now to the end, drawn last: every step, done ones included, in order. Replaces the plan whole.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "key": { "type": "string", "description": "Your name for it, unique in the plan; never shown." },
                                "title": { "type": "string", "description": "What it makes true once done, in Max's nouns, short enough for a node." },
                                "after": { "type": "array", "items": { "type": "string" }, "description": "The keys of the steps it waits on. Default: the step before it." },
                                "of": { "type": "string", "description": "The live thing it is, once there is one: `task:<id>`." },
                                "said": { "type": "string", "enum": ["planned", "doing", "done", "failed"] },
                                "failed": { "type": "string", "description": "Why, when it failed." },
                                "now": { "type": "string", "description": "Where its own work stands today, in a few words: \"in review\", \"#284, live since 10:14\"; not its use's now again. A done step's is its proof. Needed on a step done or doing. Inline markdown." },
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
                    "end": { "type": "string", "description": "What is true once it is answered and acted on, in a few words: a part of your end." },
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
