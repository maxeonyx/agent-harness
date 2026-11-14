//! The design system pages are drawn in, the themes they wear, the look
//! each agent is given, and how what Max says about the look reaches the
//! agent that keeps it.
//!
//! ```mermaid
//! flowchart LR
//!   max["Max, reading X's page"] -->|"his words, with the Screen"| x["Agent X"]
//!   x -->|pass_on| f[("Feedback, open")]
//!   tab["His tab"] -->|"a ground failed"| f
//!   f -->|wakes| k["Design keeper"]
//!   k --> art["Art agent: themes, grounds"]
//!   k --> lang["Language agent: type, marks, components"]
//!   k --> rep["Report agent: the guide"]
//!   art & lang & rep --> adv["Critic, then max-advocate"]
//!   k -->|"merged, Answered"| d[("Design")]
//!   d --> pages["Every page, from its next open"]
//! ```
//!
//! Not yet folded: see [`crate::page`].

use serde::{Deserialize, Serialize};

use crate::id::{CallId, DeliveryId, RoleId, TaskId, ValueId};
use crate::page::Spot;
use crate::resource::Observation;

/// The shared design system: one visual language, the themes, the guide,
/// and the agent that keeps them.
///
/// **Concept.** Everything about how a page looks and reads, owned in one
/// place and by one agent ([`Design::keeper`]), read from files in the
/// repo and versioned as a role is. Agents never style: they give typed
/// parts and short markdown ([`crate::page::Sheet`], [`crate::body`],
/// [`crate::ask::Ask`]), and the face draws them in the language, in the
/// task's look ([`Look`]). A change here reaches every page, old versions
/// included, from its next open: a page open on Max's screen is never
/// redrawn under him.
///
/// **The language is the same on every page.** It is what Max learns once,
/// so that on any agent's page he knows where to look:
/// - Order. The lead, the end, the now, the plan with the asks in its
///   steps, why this route ([`crate::page`]). Nothing names a part: no headings over
///   parts, no small labels above them, no cues to scroll, and no line
///   about the page itself, such as whether it still holds.
/// - Faces. The theme's display face for what is said once (the lead, the
///   end's goal, a step's title, a question); one reading face for prose;
///   one UI face for small text and data; one mono face for literal text.
///   Only the display face varies by theme.
/// - Sizes. Four, on one scale: the lead (34 px on a desktop, 28 on a
///   phone), a step in play (22, 20), reading (17.5, 17), small
///   (14.5, 14), for labels and data alike.
/// - Weights. Regular, and bold only for what Max must read and answer
///   now: the lead while something is his move, and a question that is
///   his move. Nothing is italic, since italic is harder for him to read.
///   Agents' markdown has no headings, bold or italic.
/// - Measure and space. One column, 640 px; figures and tables that need
///   it break out to 820 px on a desktop, and scroll in their own frame on
///   a phone. Space separates: 72 px between parts (56 on a phone), 28
///   between blocks.
/// - No cards. Nothing is boxed but literal text (code, a prepared call),
///   on the theme's tint. Hairlines separate the rows of a table and
///   nothing else.
/// - A pill is an answer, and nothing else is a pill. Links, folds and live
///   names are pressable too, but each looks like what it is. The pill the
///   agent proposes is filled. "Wrong question" is always last, and as
///   plain as the others.
/// - Marks. Filled is reached or true; hollow is not yet. Each state has
///   one colour and one shape ([`Tone`]), the same on the page, the plan
///   rail, the map and in figures. Lines say the same: solid exists, dashed
///   will exist, dotted and struck goes away.
/// - Change. What changed since he last looked carries a bar in the left
///   gutter. Nothing else uses the gutter.
/// - His words. Max's own words, wherever they appear, hang from a 2 px
///   rule: orange while he drafts, the line colour once sent.
/// - One floating surface: a live name's popover, only while pointed at.
///
/// **A theme varies only what makes an agent its own** ([`Theme`]): its
/// ground, its palette and its display face. Because a theme is typed, not
/// a stylesheet, it cannot reach the rest.
///
/// Serves: "we should have a design language for the text"; "the use of
/// plain font versus bold versus italics, sizing of fonts, cards or not,
/// pills or not. These kind of things need to be consistent across things,
/// but there's also things that shouldn't be consistent across things, and
/// we need to get that right"; "the agent shouldn't be in charge of the
/// styling"; "less is more, okay? But more content, less noise".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Design {
    /// The language: the one stylesheet every theme shares, written against
    /// the tokens a [`Theme`] sets, and nothing else.
    pub language: ValueId,
    /// The themes offered, and those proposed to Max.
    pub themes: Vec<Theme>,
    /// What a task holding the `max` set is told about its page: what each
    /// part is for and how to fill it, what to cut, the body's vocabulary
    /// ([`crate::body`]), how to ask, and an example page. One guide, so
    /// how pages read has one owner and one address, as how they look does.
    pub guide: ValueId,
    /// The role of the standing agent that owns the design: the one address
    /// for what Max says about how pages look or read, from any page
    /// ([`Feedback`]), kept for this role while no keeper runs. It hands
    /// the work to standing agents of its own: the art (grounds, palettes,
    /// new themes, held as a set so they stay distinct), the language
    /// (type, marks, components), and the guide (what a really good report
    /// holds).
    ///
    /// Nobody judges their own work. A fix goes to the agent that made the
    /// thing, resumed while it lives, or to a fresh one that starts from its
    /// file. Taste work (a new theme, "not beautiful enough", "these look
    /// alike") goes to two or three makers on materially different
    /// directions; then a critic that made none of it judges the result
    /// against Max's words, and `max-advocate` judges which findings he
    /// would act on. What he has said about the design is kept in the
    /// design system's files beside what it is about, never only in an
    /// agent's context.
    pub keeper: RoleId,
    /// How long the face waits for a ground, on Max's own device, before it
    /// keeps the paper and reports a [`Fault`]. The page never waits for
    /// its ground: dictation starts as a page opens, so the face paints off
    /// the page's thread, shows the paper until the painting arrives, and
    /// keeps each painting per task and window size.
    pub paint_ms: u32,
    /// The files this version was read from.
    pub source: Option<Observation>,
}

/// The themes the harness gives areas, by [`Theme::name`], in the order a
/// new area takes them when none is worn more recently than another.
pub const THEMES: [&str; 5] = ["tussock", "harbour", "kauri", "paua", "marama"];

/// An art direction an agent's page wears.
///
/// **Concept.** A finished piece of design, made with care in the design
/// system's files by the keeper's art agent, and never by an agent at run
/// time. It has three parts, and only these, so a theme can't change what
/// the language means: a ground, a palette and a display face.
///
/// **The theme says where Max is; the identity says who.** Any number of
/// agents wear one theme at once, each with its own painting of it, told
/// apart by colour as well as composition ([`Identity`]): the same high
/// country, one at a rose dusk and one in silver morning light. So a theme
/// has no variants. Themes differ from each other in what they look at and
/// how they are made, not only in colour: Tussock looks across a
/// landscape, Harbour down on a chart, Kauri up through a forest, Pāua
/// into shell, Marama at the moon over the sea.
///
/// Serves: Max's "really elegant and artistic [...] beautiful backgrounds,
/// UI themes, design systems, etc. that can be swapped between to make an
/// agent and its page unique"; "agents don't design their own themes";
/// "easy for me to see whereabouts I'm at"; backgrounds "5 times better",
/// not "10% better".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    /// `harbour`.
    pub name: String,
    /// What it is, as Max sees it when he changes an area's theme: "A
    /// hand-tinted chart of a harbour: depth lines, soundings and a quiet
    /// sea."
    pub about: String,
    pub palette: Palette,
    pub display: Face,
    /// Its ground: an ES module whose `paint(ctx, scene)` paints into a 2D
    /// canvas, given the canvas's size, the task's [`Identity`], the box the
    /// page's lead sits in, which it keeps calm, and on a page the text's
    /// column, which it keeps open. Deterministic in its identity and size, so
    /// the same agent looks the same in every face and the keeper can paint
    /// again exactly what Max saw ([`Screen`]). Each part of the painting (the
    /// sun, the ridges, the coast) draws from its own stream of the seed, so
    /// revising one part leaves the rest of every agent's painting where it
    /// was. Painted once for the chat window a page is read in, never animated:
    /// it fills the window behind the whole page and holds still while the text
    /// scrolls over it, each block of text on a press of the painting that
    /// keeps its ink at 7:1. The map paints each area with its ground, faint
    /// behind the graph, each live agent in a pool of its identity's colour,
    /// repainted only when the zoom passes a step.
    ///
    /// The face runs only grounds a version of the design system declared,
    /// since a ground is a program and a page body may hold none. Before a
    /// version is merged, its check paints every ground at phone,
    /// desktop and thumbnail sizes for a set of identities, and holds it to
    /// its bars: the lead's ink on the ground in the lead's box at 7:1 for
    /// 95% of it and never under 4.5:1; the body's ink at 7:1 over the box
    /// the text covers, with its press; a thumbnail that reads as its
    /// theme; identities planned one after another told apart at a glance;
    /// the same pixels painted twice; and a phone's paint time.
    pub ground: ValueId,
    /// How many identities its ground paints told apart at a glance, in
    /// the order the art agent set and checked by eye: slot 0 the area's
    /// main agent, then each next agent as far from those before as the
    /// art allows ([`Identity::slot`]).
    pub slots: u8,
    /// Shown to Max on the keeper's page and never offered to an agent: a
    /// new theme waits for his choice, so no agent can lock in one he then
    /// rejects.
    pub proposed: bool,
}

/// A theme's colours: its paper, its inks, its accent, its figures' fills,
/// and the inks for text set on its ground.
///
/// **Concept.** The tokens the language's stylesheet is written against.
/// The design system's check lists a palette whose text is under 4.5:1 on
/// its paper, whose figure fills are too close to tell apart, or whose
/// accent or fills read as a state tone (above all as orange, which is
/// always Max's move), so a theme never makes a mark lie.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Palette {
    pub paper: Rgb,
    /// Step titles, questions, the first column of a table.
    pub ink: Rgb,
    /// Reading text.
    pub body: Rgb,
    /// Small text that can wait: table headers, past steps, dates.
    pub muted: Rgb,
    /// Hairlines.
    pub line: Rgb,
    /// The ground under literal text, and groups in figures.
    pub tint: Rgb,
    /// Links, and the theme's own touch: never a state.
    pub accent: Rgb,
    /// Fills for up to four kinds of thing in a figure (`f1`..`f4`).
    pub figure: [Rgb; 4],
    /// The lead's ink, on the ground.
    pub cover_ink: Rgb,
    /// Reading text's ink, on the ground.
    pub cover_body: Rgb,
}

/// A colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rgb(pub [u8; 3]);

/// A theme's display face.
///
/// **Concept.** The one typeface a theme chooses, for the lead, step
/// titles and questions, upright only. Its bold is set per face so
/// that bold looks equally bold in every theme. The reading, UI and mono
/// faces are the language's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Face {
    /// `Fraunces`.
    pub family: String,
    /// Its font files.
    pub files: Vec<ValueId>,
    /// The weight bold is drawn at: 640 for Fraunces.
    pub bold: u16,
    /// Variable axes, as CSS `font-variation-settings`: `"SOFT" 100,
    /// "opsz" 72`.
    pub axes: Option<String>,
}

/// What a task wears: its theme, and who it is among the agents wearing it.
///
/// **Concept.** Given by the harness when the task starts, never chosen by
/// an agent, and fixed for the task's life, since an agent's look is how
/// Max finds it again. A new area (a task Max starts) is given the theme
/// no live area wears, or else the one worn least recently, so areas look
/// as unlike each other as five themes allow. Every task under it wears
/// the area's theme, with its own [`Identity`], so where Max is shows at a
/// glance and who is still its own. Only Max changes an area's theme, from
/// its page or the map, and its tasks redraw. A fork or a resume carries
/// its source's look. Folded from `TaskEvent::Looked`.
///
/// Serves: "every area can have some kind of like color or pattern, and
/// each agent within that can again have variant color and patterns, so
/// that it's easily visually distinguishable for me"; "agents don't
/// design their own themes. They only inform their own themes".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Look {
    /// A [`Theme::name`].
    pub theme: String,
    pub identity: Identity,
}

/// Who an agent is among the agents wearing its theme, as its painting
/// shows it.
///
/// **Concept.** Planned by the harness, not left to chance: with six
/// colours and four agents, a random pick gives two of them the same
/// colour 72% of the time. When a task is given its look, it takes the
/// lowest slot no live task wearing its theme holds, so the agents on the
/// map at once wear the slots the art agent made most distinct. Slots, not
/// a continuous range, because equal steps of hue don't look equally far
/// apart: the art agent checks each slot by eye against the others.
/// Past the theme's slots, slots repeat and the seed's composition tells
/// them apart. `root` and `line` place the task among its ancestors
/// wearing the same theme, for a ground whose picture nests: a Harbour
/// child's chart is a closer look at a region of its parent's, so who
/// started whom shows in the paintings.
///
/// Serves: "inner variation in colour and identity, not just layout, so
/// that individual agents within an area are distinguishable at a glance";
/// "the harness needs to plan those identities to separate them"; "making
/// successive subagents zoom in on ever more detailed regions"; "nod to
/// identity somehow incorporated too".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    /// Which of the theme's identities ([`Theme::slots`]) it wears.
    pub slot: u8,
    /// Whose id seeds the composition: the task's own, or for a resume its
    /// source's.
    pub seed: TaskId,
    /// The furthest ancestor reached through parents wearing the same
    /// theme: the task itself when it has none.
    pub root: TaskId,
    /// The child numbers from `root` down to the task, each its place among
    /// its siblings by creation; empty for `root` itself.
    pub line: Vec<u32>,
}

/// What reaches the design keeper about how pages look or read: Max's
/// words, passed on by the agent he said them to, or a ground that failed
/// on his screen.
///
/// **Concept.** Max gives his feedback to whichever agent he is reading,
/// and that agent has no say over its look or its page's form. So it passes
/// his words on with `pass_on`, declared with the `max` set whether or not a
/// keeper runs: whole, never split or retold, since it is the least able
/// to say which part is the ground's, the type's or the guide's. What he
/// said about its own page's content, it acts on itself; a general rule
/// ("no eyebrows") it acts on and passes on. Feedback is kept as a fact,
/// open until the keeper answers it ([`FeedbackEvent`]), and addressed to
/// the keeper's role rather than to a task: nothing he says is lost while
/// no keeper runs, a second comment during a fix is never dropped, and a
/// keeper that wakes fresh reads what is open. The page he spoke on shows
/// his words' state, from taken to answered.
///
/// Serves: "if I give feedback on the styling, I'm going to give it to that
/// agent, so it needs to be able to pass it on, even though it's not able
/// to action it itself".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Feedback {
    /// His words, which carry the [`Screen`] he was looking at.
    Words {
        words: DeliveryId,
        /// The agent that passed them on.
        by: TaskId,
        /// Its note: what on its page he seems to mean.
        note: String,
    },
    /// A ground that failed in his tab, reported by the face itself, once
    /// per tab, ground, identity and failure. It needs nothing from him.
    Fault(Box<Fault>),
}

/// What happens to a piece of feedback, folded from
/// `Fact::Feedback(FeedbackId, FeedbackEvent)`: open from `Given` until
/// `Answered`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedbackEvent {
    Given(Feedback),
    /// It needs Max first: a choice between studies, or words that
    /// contradict what he said before. Still open.
    Asked(CallId),
    /// A merged commit answers it: a change live on every page from its
    /// next open, or only his words recorded in the design system's files.
    /// His rejection of the answer is new feedback.
    Answered {
        /// It names this feedback in its message.
        commit: String,
        /// What changed, for Max, beside his words.
        note: String,
    },
}

/// What was on Max's screen when he spoke, set by the face as he sends.
///
/// **Concept.** What he saw, as values rather than a picture, so any agent
/// can draw it again, before and after a change, with no screenshot and no
/// work data leaving the page. Only what can't be derived: the task's look
/// is fixed, and the design version is the one that stood at the time. His
/// words carry it, and every agent can read it ("he was on his phone").
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Screen {
    /// Whose page.
    pub task: TaskId,
    /// The page version drawn; `None`: none was presented.
    pub version: Option<u32>,
    /// What he was reading, from the scroll.
    pub spot: Option<Spot>,
    pub viewport: Viewport,
    /// CSS pixels from the top.
    pub scroll: u32,
    /// How long the ground took to paint; `None`: it didn't finish.
    pub paint_ms: Option<u32>,
}

/// A screen's size, in CSS pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    /// Device pixels per CSS pixel, in percent: 300 on a 3x phone.
    pub scale_percent: u16,
}

/// A ground that threw or passed [`Design::paint_ms`] on Max's screen.
///
/// **Concept.** The page never pays for a broken ground: the face paints
/// off the page's thread, stops at the deadline, and draws the theme's
/// paper instead.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fault {
    pub screen: Screen,
    /// The browser's user agent, since a phone's GPU is not the check's.
    pub device: String,
    pub failure: Failure,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Failure {
    /// Its message and stack.
    Threw(String),
    /// Stopped at the deadline.
    Slow,
}

/// The state tones: whose move a thing is, the same colour and shape on
/// every page, the plan rail and the map, never a theme's, lightened only
/// to read on a dark ground.
///
/// **Concept.** One mark per state, told apart by shape as well as colour,
/// so it reads at a glance, in greyscale and to a colour-blind eye. A mark
/// read from a source that may be out of date is drawn faded. "Your move"
/// is the same orange whatever an agent wears.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tone {
    /// Max's move: an orange dot with a halo.
    Yours,
    /// An agent is on it: a half-filled blue dot, with a halo while it runs.
    Working,
    /// A person's or an outside party's move: a violet ring.
    Others,
    /// Done: a filled green dot.
    Done,
    /// Failing: a red diamond.
    Failed,
    /// Nobody's move yet, or not started: a dotted grey ring.
    Quiet,
}
