//! The agent's page: how an agent facing Max talks to him.
//!
//! **Concept.** A task's page is its standing report to Max, and the thing
//! he answers. A task has one when it holds the `max` tool set: the tool
//! review's model, in which a task facing Max holds that set, and holding
//! it decides that the task's turns end on his move, that its result goes
//! to him, and which tools reach him. Those tools are the page tools
//! (`page_read`, `page_write`, `page_edit`, `page_look`, and `present`,
//! which ends a turn), the ask tools (`ask`, `withdraw`, `cite`),
//! `pass_on` and `open_chat`. A task
//! without the set has no page and no way to ask him anything, so no call
//! is ever refused for not having him. The page is what opens when he
//! taps the task on the map, read top to bottom: the vision's "an agent
//! answers with a page", and "a question is a page plus a node, answered
//! by talking".
//!
//! **Always the same order.** A page brings Max's picture of the work level
//! with the agent's. Three things can differ, each with its own authority:
//! where the work is going (his to agree with), where it stands (the
//! agent's to report, and his to believe), and how it gets from one to the
//! other (the agent's to propose, his to redirect). So every page says, in
//! this order, set out by the harness and not the agent:
//!
//! ```mermaid
//! flowchart TB
//!   lead["The lead: the question, problem or news"] --> e["Where this ends: a claim, and what shows it"]
//!   e --> n["Where we are: a claim, and what shows it"]
//!   n --> p["The plan: the task's steps, each open ask inside the step it unblocks"]
//!   p --> w["Why this route"]
//! ```
//!
//! 1. The lead ([`Lead`]): why he is reading this now. A lead new to him
//!    fills his first screen alone, on the task's painted ground.
//! 2. Where this ends ([`Sheet::end`]): the eventual state, concretely
//!    enough to point at a sentence and say "no". If he doesn't agree,
//!    nothing below matters.
//! 3. Where we are ([`Sheet::now`]), as fact, the concrete thing first.
//! 4. The plan ([`Step`]), which is most of a page: the task's own steps
//!    ([`Description::plan`]), with each of its open asks
//!    ([`crate::ask::Ask`]) drawn whole inside the step it unblocks. Steps and asks are values
//!    of their own, drawn as they stand now, never retold on the page.
//! 5. Why this route ([`Sheet::why`]), after the route it argues for.
//!
//! Nothing names a part: no headings over them, no small labels above
//! them, no cues to scroll. Each claim frames its part in its own words
//! ("When this is done, …", "Today …"). A part Max is done with shrinks:
//! a lead he has read takes a short band over the end, an end he has agreed
//! (his go, or a reply that leaves it standing) folds to its claim and opens
//! again when the agent changes it, and what changed since he last looked
//! carries a bar in the left gutter.
//!
//! **The agent writes content; the design system draws it.** The agent
//! gives typed parts, and short markdown inside them ([`crate::body`]). It
//! never sets type, colour or layout. The design system
//! ([`crate::design::Design`]) draws every page in one language, so what
//! marks a question for Max, a step or a live name is the same on every
//! page, and the task's look ([`crate::design::Look`]) gives the page its
//! theme's ground, palette and display face, painted for this agent. How
//! to fill each part is the design system's guide, given with the `max`
//! set. What Max says about how pages look or read, the agent passes on to
//! the design system's keeper ([`crate::design::Feedback`]), since it has
//! no say over either.
//!
//! **Current by construction.** Three rules keep the page Max reads
//! current:
//! - What a machine can know is bound, never written. A [`Ref`] draws its
//!   thing's state now, whoever changed it; a plan step bound to one takes
//!   its state from it; and the asks are drawn from the asks themselves.
//! - A turn that ends on Max's move ends with `present`, which shows him
//!   the page as it is now ([`Presented`]).
//! - While the task works, its chat opens at its progress (the summaries
//!   of its calls, which the harness already holds), and the newest
//!   version is one line above it, marked as from before.
//!
//! **A shape, not a refusal.** A page tool says no only when the world did:
//! a write over a part another task changed since this one saw it, or an
//! edit whose text is not in the body. Everything else is made impossible
//! to get wrong, or drawn as well as it can be and listed for the agent as
//! a [`crate::body::Problem`]. Every `page_write` states the lead, the end
//! and where we are, so a page can't lack them.
//!
//! **Folded so far.** A page is [`crate::fact::Fact::Page`], held by
//! [`crate::state::State::page`]; a description is
//! [`crate::task::TaskEvent::Described`], on [`crate::task::Task`]; feedback
//! is [`crate::fact::Fact::Feedback`]; an ask is a call. A round that
//! presents ends the turn on Max's move once its calls have settled.
//! Every page tool, `describe` included, is the `max` set's.
//!
//! **Not yet folded.** `Fact::Kind(SourceKind)`, `Fact::Design(Design)`,
//! `TaskEvent::Looked(Look)`, `Phase::AwaitingMax` carrying an
//! `Option<Presented>` and `Prompt::Present`, Max's words carrying his
//! [`crate::ask::Press`]es and the [`crate::design::Screen`] he was
//! reading, a `says` on `resource::Change`, and `attention::Ranked`
//! naming an [`crate::ask::Owed`].

use serde::{Deserialize, Serialize};

use crate::id::{CallId, TaskId, Timestamp, ValueId};
use crate::source::{ItemKey, Whose};

/// A task's page: the stage its agent edits, and the versions Max was shown.
///
/// **Concept.** One per task that holds the `max` set. A fork's page tools
/// act on the page of its nearest ancestor that is not a fork, since a
/// fork is its source doing a step, and can build its source's page with
/// that whole context. Not a file: the stage and every version are facts,
/// in every face's view like the rest of the brain's state, the same at
/// work and at home, and versioned without git.
///
/// **Where it sits.** In the task's chat, at the point each version was
/// presented, so scrolling up from a page shows the chat that led to it.
/// When the task waits on Max, its chat opens at the top of the first
/// thing it said since he last looked, not at the bottom, with dictation
/// on. While it works, its progress comes first, and the newest version is
/// one line, marked as from before. A version a newer one replaced shrinks
/// to one line.
///
/// Serves: the deliverable is a page, never markdown in a chat; "the agent
/// should get tools to edit, stage, test, iterate on these pages before it
/// ends its turn"; "it should start me at the top of the report and not the
/// bottom"; "if I scrolled above the page, I would get the chat".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    pub task: TaskId,
    /// What the agent is editing, unseen by Max. `None` before its first
    /// write. `page_look` draws it as Max would see it, at a phone's width
    /// and a desktop's, and returns the screens and the problems, so the
    /// agent looks at its page before he does.
    pub stage: Option<Stage>,
    /// What Max was shown, oldest first, kept for the task's life.
    pub versions: Vec<Version>,
}

/// The page the agent is working on.
///
/// **Concept.** The page's working copy. `page_write` states the lead, the
/// end and where we are every time, and the why when it is given. A claim's
/// body, or the why, is replaced only when given. `page_edit` applies each
/// change to the body its old text occurs in once, as file edits do, so two
/// forks building one page merge by text. A write says no only over a part
/// another task changed after this one last saw it, since that work would
/// be lost. A body with problems is still taken: the call's result lists
/// them, each saying how to write it instead, so the agent iterates rather
/// than retries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stage {
    pub sheet: Sheet,
    /// The call that made the newest edit: whose work a write would replace.
    pub by: CallId,
}

/// A page Max was shown.
///
/// **Concept.** Immutable once presented. Its bodies are markdown in the
/// language's small vocabulary ([`crate::body`]), never CSS or colours, so
/// an old version still draws after the design system changes. The plan
/// and the asks are not in it: they are drawn live, as they stand now, so a
/// version never shows a step as waiting that has finished.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub n: u32,
    pub sheet: Sheet,
    /// When it was presented: where it sits among the chat's entries, and
    /// what `Fact::Seen` is compared with for what changed since he looked.
    pub at: Timestamp,
    /// When a `present` last found it still holding, unchanged: drawn as
    /// "still holds, checked 10:42".
    pub held: Timestamp,
    /// The `present` call that made it.
    pub by: CallId,
}

/// What the agent writes on its page: the lead, where this ends, where we
/// are, and why this route.
///
/// **Concept.** The written half of a page, in parts the harness sets out
/// in a fixed order, so every page reads the same way and the agent spends
/// its words on content, never on arrangement, labels or emphasis. The
/// other half is live and never written here: the plan's steps
/// ([`Description::plan`]), the asks, and the state of everything a [`Ref`]
/// names. There is no section of detail: detail folds beside the sentence
/// it backs ([`crate::body::Fold`]), so the why behind anything he reads is
/// one press from it.
///
/// Serves: "I want to see that we agree on the eventual state. Then I want
/// to see where we are. And then finally, I'm ready for my asks"; "most of
/// the pages should be about the third part, the plan, but the plan is
/// meaningless without the first two"; "the agent should be given quite a
/// structured way to put its information in"; "more content, less noise".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sheet {
    pub lead: Lead,
    /// Where this ends: the world as it will be when the work is done, in
    /// Max's nouns. "When this is done, every Claude Code session on the
    /// work laptop runs under the harness, and stopping one takes under a
    /// second." A state, not a list of work: what will be true, concretely
    /// enough that he could point at it and say "no, that's wrong". Its
    /// body shows what will be true as a task list (`- [ ]` not yet,
    /// `- [x]` true already), his words where they set the destination, the
    /// agent's own additions said as its own, since invented requirements
    /// hide there, and what is deliberately left out. A figure of its own
    /// when the end is a thing to look at, such as a screen.
    pub end: Claim,
    /// Where we are, as fact: "Today, a session runs only in a terminal,
    /// and stopping one takes 8.2 s." Its body shows it: the real log
    /// line, the count and where it was counted, one figure of the system
    /// with what is to come drawn dashed, so one picture shows the end and
    /// the now together; then the facts, naming live things ([`Ref`])
    /// rather than retyping their state. What the agent couldn't find out
    /// is said as a fact. A parent's plan step bound to this task shows
    /// this claim. When the work is done, it is the end reached, with the
    /// proof.
    pub now: Claim,
    /// Why this route, drawn after the steps it argues for: the routes Max
    /// would think of, each killed in a sentence with its reason; the
    /// calls the agent made for him, each with the question it answers, so
    /// he can overturn one by saying so; and where the plan stops short of
    /// the end, and what that costs. Body markdown. `None`: the steps say
    /// it all.
    pub why: Option<ValueId>,
}

/// The lead: this moment's question, problem or news, the first thing Max
/// reads.
///
/// **Concept.** The hook: what he needs to be on the same page, and why he
/// should read on. Not the task's title, which the window's header
/// already shows: the question the work now turns on ("Where should the
/// work brain live?"), the problem ("Stopping a session takes 8
/// seconds"), or the news since he last looked ("Every session at work now
/// runs under the harness"), bad news first. If his last words asked
/// something, it answers them. Never a label, a count of what is below, or
/// "nothing needs you": the harness says that, from the asks.
///
/// Drawn in the theme's display face at the lead size, upright and
/// readable, on the ground: alone on his first screen when it is new to
/// him, and as a short band over the end once he has seen it. Bold while
/// something on the page is his move; regular once nothing is, as on a
/// finished task's page. It is also what the map's node and his queue say
/// for the task, so a tap opens on what he tapped.
///
/// Serves: "I need the core framing, the core navigational aid. I need the
/// question. I need the problem. Whatever hooks me"; "I like the very
/// clear, bold title for the question"; "the hero text is too big,
/// unclearly worded, and italics is harder to read".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lead {
    /// Plain text, one line on a desktop, two on a phone. Over 70
    /// characters, or the task's title again, is drawn and listed as a
    /// problem.
    pub title: String,
    /// One or two plain sentences under it, in reading type, when the title
    /// alone would not say why it matters now.
    pub hook: Option<String>,
}

/// One of the page's two states: where this ends, or where we are.
///
/// **Concept.** A claim Max can agree with or dispute, and what shows it.
/// The claim is one sentence, drawn at the claim size in the display face,
/// framed by its own words rather than a label. The body under it shows
/// the claim, visual first: a figure, a table, a task list, then a few
/// sentences. Because the claim is all he needs once he has agreed, an
/// agreed end folds to its claim, and opens to its body when pressed or
/// when the agent changes it.
///
/// Serves: "the agent can communicate to me its idea of the eventual state,
/// can communicate to me the, in fact, current state"; "demonstrate
/// through detailed and concrete diagrams, text [...] that it is on the
/// same page as me"; "visual first".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    /// One plain sentence. More than one, or over 160 characters, is drawn
    /// and listed as a problem.
    pub says: String,
    /// Body markdown ([`crate::body::Body`]). `None`: the claim says it all,
    /// as for a small fix.
    pub shows: Option<ValueId>,
}

/// What happens to a page, stated by the brain in the batch that settles
/// the call `by`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageEvent {
    /// The stage after a `page_write` or `page_edit`.
    Edited { sheet: Box<Sheet>, by: CallId },
    /// `present`: Max is shown the stage, and the turn ends on his move. A
    /// stage changed since the newest version makes a new version; an
    /// unchanged one says that version still holds ([`Version::held`]),
    /// which is the cheapest way to end a turn. A quick reply while he
    /// works turn by turn is the text of the response that presents: a
    /// line in the chat above the page.
    Presented { by: CallId },
}

impl Page {
    /// The page after `event`, at `at`; `None` when it makes no sense: a
    /// present with nothing staged.
    pub fn apply(&mut self, event: &PageEvent, at: Timestamp) -> Option<()> {
        match event {
            PageEvent::Edited { sheet, by } => {
                self.stage = Some(Stage {
                    sheet: (**sheet).clone(),
                    by: *by,
                });
            }
            PageEvent::Presented { by } => {
                let sheet = &self.stage.as_ref()?.sheet;
                match self.versions.last_mut() {
                    Some(newest) if newest.sheet == *sheet => newest.held = at,
                    _ => {
                        let n = self.versions.len() as u32 + 1;
                        self.versions.push(Version {
                            n,
                            sheet: sheet.clone(),
                            at,
                            held: at,
                            by: *by,
                        });
                    }
                }
            }
        }
        Some(())
    }
}

impl PageEvent {
    /// The values it names.
    pub fn values(&self, out: &mut Vec<ValueId>) {
        if let PageEvent::Edited { sheet, by: _ } = self {
            let Sheet {
                lead: _,
                end,
                now,
                why,
            } = &**sheet;
            out.extend(end.shows.into_iter().chain(now.shows).chain(*why));
        }
    }
}

impl Description {
    /// The values it names.
    pub fn values(&self, out: &mut Vec<ValueId>) {
        out.extend(self.plan.iter().flatten().filter_map(|s| s.body));
    }

    /// This description with `newer`'s fields over it: each field given
    /// replaces only itself.
    pub fn and(&self, newer: &Description) -> Description {
        Description {
            title: newer.title.clone().or_else(|| self.title.clone()),
            plan: newer.plan.clone().or_else(|| self.plan.clone()),
        }
    }
}

/// Whether the page Max is about to read is current: carried by a turn that
/// ends on his move.
///
/// **Concept.** The gate that makes "current by construction" a state, not
/// a hope. A task holding the `max` set ends a turn on Max's move with
/// `present`, as an autonomous task ends with `compact_and_report`: the
/// page is the turn's ending, and [`Presented::Current`]. A turn that ends
/// in plain text instead gets one more turn (`Prompt::Present`: "your page
/// is version 4, from before Max's message at 10:02; update it, or present
/// it as it is if it still holds"). A second that ends the same way waits
/// on Max, [`Presented::Stale`], drawn as such. It never loops, and a stale
/// page is never shown as current. The same rule serves a Claude Code
/// session the harness drives, whose `present` is a tool of its MCP server
/// and whose extra turn is a user message on its stream-json input, so it
/// needs no hook an enterprise policy could block.
///
/// What it can't prove is that the page is true, only that the agent
/// looked: the guide makes "still holds" a judgement, and the change bar
/// shows Max what moved.
///
/// Serves: "when it ends its turn, it will be presented to me"; "it's up
/// to date because the harness is built with that just as a foundational
/// assumption".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Presented {
    Current {
        version: u32,
    },
    /// Not presented after a reminder: the newest version, if any, is
    /// drawn marked as from before the turn.
    Stale {
        version: Option<u32>,
    },
}

/// Something a page or an ask points at, whose state the harness knows
/// without the agent.
///
/// **Concept.** One pointer for every live thing: a name in a page's prose
/// drawn with its state now, a plan step's subject, an ask's subject, an
/// item node on the map, and what wakes an agent when it changes. In
/// markdown it is a link's target: `task:<id>`, or `<kind>:<name>` for an
/// item, such as `github.pr:maxeonyx/agent-harness#241`
/// ([`crate::body::Inline::Live`]). The brain resolves each when the stage
/// is edited. One it cannot resolve is drawn as the link's plain words and
/// listed as a problem, naming the kinds there are. A live name is drawn
/// with its state's mark ([`crate::design::Tone`]), never as a pill: in the
/// design language, a pill is an answer.
///
/// Serves: "the harness should be in charge of getting anything that can be
/// gotten automatically"; "it's up to date because the harness is built
/// with that just as a foundational assumption"; Underview's "what the
/// source knows, the source says".
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Ref {
    /// A task: its title (Max's rename wins), its now-claim, and whose move
    /// it is. An orchestrator's page names its children by these.
    Task(TaskId),
    /// A thing outside the harness that a source reads: a PR, a ticket.
    Item(ItemKey),
}

/// Where on a page Max was reading when he spoke.
///
/// **Concept.** His words carry what he was looking at (Underview's "know
/// what Max was looking at"), set by the face from the scroll as he sends:
/// the ask he last pressed in, else the part in the reading band. So "no,
/// that's not where this is going", said while the end fills the screen,
/// reaches the agent as a rejection of the end, with no button and no
/// form. It is what he was reading, for the agent to interpret, and never
/// routes anything: his words go to the agent whose page it is. Also where
/// a [`crate::body::Problem`] is, for the agent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Spot {
    Lead,
    End,
    Now,
    /// A plan step, by its key.
    Step(String),
    /// An ask, by its call.
    Ask(CallId),
    Why,
}

/// What a task says about itself: its name and its plan.
///
/// **Concept.** The task's metadata, chosen by its own agent (Max: "the
/// metadata of a chat and a stream and stuff should be chosen by the
/// agent"). Stated by the agent's `describe` call at any time, each field
/// replacing only itself (`None` leaves it as it was), and folded as
/// `TaskEvent::Described`. Every task has one, with or without a page, so
/// every node on the map has a face and every plan is drawn the same on
/// the map and on the page. Its look is not here: it is given, not chosen
/// per call ([`crate::design::Look`]).
///
/// Serves: chats that start alike no longer look alike; "easy for me to see
/// whereabouts I'm at"; the vision's plans as provisional nodes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Description {
    /// What the work does: "Claude Code under the harness". A name Max gave
    /// (`TaskEvent::Renamed`) is shown over it, and `describe`'s result says
    /// so. The window's header, not the page's lead.
    pub title: Option<String>,
    /// How the work gets from where it started to where it ends, done steps
    /// included, so the route reads whole. Replaced whole: a step left out
    /// is gone, and its node on the map with it. An open ask that named it
    /// moves to the first step not behind him, and `describe`'s result says
    /// so, for the agent to revise or withdraw it: never refused, since the
    /// ask still has a place.
    pub plan: Option<Vec<Step>>,
}

/// One step of a task's plan.
///
/// **Concept.** A thing the task does or produces on the way to its end,
/// the size Max would recognise: a PR, a child task, a piece of the
/// agent's own work that changes something he would notice. One value,
/// drawn twice: as a node on the map (planned ones dashed, as everything
/// that will exist is), and as a stop on the page's plan rail, with the
/// asks it waits on inside it. A step bound to a live thing ([`Step::of`])
/// takes its state from it ([`StepState`]), so a merged PR shows its step
/// done without the agent saying so. A step bound to a child task shows the
/// child's now-claim, if it has a page, else its title and phase, and each
/// of the child's open asks as one line, its question, which opens the
/// child's page at that ask: the orchestrator never retells a child's
/// question, and sends Max to each agent for its part.
///
/// What an answer unblocks is its step and every step after it
/// ([`Step::after`]): how the queue ranks an ask, since some unblock hours
/// of agent work and others unblock nothing.
///
/// Drawn by relevance: steps behind him are one small line each, and more
/// than three fold into one line of their titles; the steps in play are
/// full, with their body and asks; the steps ahead are their title and a
/// short body.
///
/// Serves: "then the plan, the current plan on how to get from A to B, and
/// including then asks"; the vision's "planned pull requests, planned
/// design agents, planned user questions" as provisional nodes; "dependency
/// diagrams, essentially, on actions for me, on chats, on agents to each
/// other".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    /// The agent's name for it, unique in the plan: how an ask names the
    /// step it sits in ([`crate::ask::Ask::step`]), how `after` names it,
    /// and how a new plan keeps a step's node on the map. Never drawn.
    pub key: String,
    /// What it makes true, in Max's nouns, short enough for a node: "The
    /// advocate can search your words", not "T6". `None`: the name of what
    /// it is bound to, so a child task's step shows the child's own title,
    /// renamed or not.
    pub title: Option<String>,
    /// The steps that must be done before it starts, by key. `None`: the
    /// step before it in the plan, so a plain list is a route with no more
    /// said; `Some` with none: it can start at once, beside the others.
    pub after: Option<Vec<String>>,
    /// The PR, child task or other live thing this step is, once there is
    /// one.
    pub of: Option<Ref>,
    pub said: Said,
    /// Body markdown: why this step, what it changes. Drawn in full while
    /// the step is in play, shortened ahead, folded once behind him.
    pub body: Option<ValueId>,
}

/// Where the agent says a step stands.
///
/// **Concept.** The agent's word, for what no machine knows. A step bound
/// to a live thing is drawn by that thing's state when it has one, and the
/// agent's word only fills the gaps.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Said {
    Planned,
    Doing,
    Done,
    /// It failed or was abandoned, and stays on the page with why, so the
    /// plan shows what was tried.
    Failed {
        why: String,
    },
}

/// Where a step stands as drawn: derived, never stored.
///
/// **Concept.** The step's mark on the rail and on the map. Derived in
/// order: a bound thing that ended done makes it [`StepState::Done`]; a
/// step holding an open ask that is Max's move is his move; a bound
/// thing's reading gives whose move it is; else the agent's [`Said`]
/// (`Doing` is the asker's own move). Filled marks are reached, hollow ones
/// are not, and the rail into a step is solid once it is reached and dashed
/// before.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepState {
    Done,
    Failed {
        why: String,
    },
    /// Someone is moving it: drawn in that party's tone.
    Moving(Whose),
    /// Not started; `until` when it can't start before a date.
    Planned {
        until: Option<Timestamp>,
    },
}
