//! The shared design system pages are drawn in, the themes they wear, the
//! agent that keeps them, and how what Max says about the look reaches it.
//!
//! ```mermaid
//! flowchart LR
//!   max["Max, reading X's page"] -->|"his words, with the Screen"| x["Agent X"]
//!   x -->|pass_on_style| f[("Feedback, open")]
//!   tab["His tab"] -->|"a ground failed"| f
//!   f -->|wakes| k["Keeper"]
//!   k -->|"the art"| a["Art agent"]
//!   a --> m["Artists"]
//!   m --> c["Critic, then max-advocate"]
//!   k -->|"merged, Answered"| d[("Design")]
//!   d --> pages["Every page wearing it, from its next open"]
//!   k -->|"before and after, proposals"| kp["Keeper's page"]
//! ```
//!
//! Not yet folded: `Fact::Feedback(FeedbackId, FeedbackEvent)`, the open
//! feedback a keeper wakes on, Max's words carrying a [`Screen`], and the
//! rest listed in [`crate::page`].

use serde::{Deserialize, Serialize};

use crate::id::{AskId, DeliveryId, RoleId, TaskId, ValueId};
use crate::page::Spot;
use crate::resource::Observation;

/// The shared design system: one design language, the themes, the
/// components, and the agent that keeps them.
///
/// **Concept.** Everything about how a page looks, owned in one place and
/// by one agent ([`Design::keeper`]), read from files in the repo and
/// versioned as a role is. Agents never style: they give typed parts and
/// short markdown ([`crate::page::Sheet`], [`crate::ask::Ask`]), and the
/// face draws them in the language, dressed in the task's theme. A change
/// here reaches every page, old versions included, from its next open: a
/// page open on Max's screen is never redrawn under him.
///
/// **The language is the same on every page.** It is what Max learns once:
/// - The order: where this ends, where we are, the asks, then the detail,
///   folded.
/// - Slant and weight carry meaning, not decoration. Italic is where this
///   ends, and nothing else. Bold is a question for Max, and nothing else.
///   Everything else is regular. Agents' markdown has no bold or headings.
/// - Four sizes on one scale: the cover, a heading, reading, a label.
/// - One reading face and one measure, so body text reads the same
///   everywhere.
/// - No cards, and no boxes around prose. An ask is set off by space and a
///   hairline, and its frame by small labels: end, now, why.
/// - A pill is something Max can press: only answers are pills. A live
///   thing's state is a dot in its state's tone, never a pill.
/// - The state tones ([`Tone`]): orange is always his move.
///
/// **A theme varies only what makes an agent its own:** the ground, the
/// palette and the display face ([`Theme`]).
///
/// Serves: "we should have a design language for the text", "the use of
/// plain font versus bold versus italics, sizing of fonts, cards or not,
/// pills or not. These kind of things need to be consistent across things";
/// "the agent shouldn't be in charge of the styling"; the vision's area
/// colour with an agent variant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Design {
    /// The language: the stylesheet every theme shares, written against the
    /// tokens a theme's style sets, and nothing else.
    pub language: ValueId,
    /// The themes offered to agents, and those proposed to Max.
    pub themes: Vec<Theme>,
    pub components: Vec<Component>,
    /// What a task holding the user plugin is told: choose your theme first;
    /// the end is the world when the work is done, a state and not a plan; the point is the update in one sentence; then
    /// show it, a figure first; ask with `ask`, each ask with its own end,
    /// now and why, and never retell it on the page; name live things
    /// instead of retyping their state; write no colours, typefaces, bold or
    /// headings; detail goes in `more`; Max's words about how your page
    /// looks, not what it says, go to the keeper whole with
    /// `pass_on_style`. With the components and
    /// an example page.
    pub guide: ValueId,
    /// The role of the standing agent that owns the look: the one address
    /// for what Max says about it, from any page ([`Feedback`]), which is
    /// kept for this role while no keeper runs. The keeper keeps the
    /// language, the components and the guide, and hands the art (grounds,
    /// palettes, new themes) to a standing art agent of its own, which holds
    /// the themes as a set, so they stay distinct from each other.
    ///
    /// Nobody judges their own work. A fix goes to the artist that made the
    /// theme, resumed while it lives (it knows the code), or to a fresh one
    /// that starts from the theme's file. Taste work (a new theme, "not
    /// beautiful enough", "these look alike") goes to two or three artists
    /// on materially different directions. Then a critic that made none of
    /// it judges the result against Max's words, the whole set side by side
    /// and the paintings he loved, and `max-advocate` judges which findings
    /// he would act on. What he has said about the look is kept in the
    /// design system's files beside the art, never only in an agent's
    /// context. He sees a change on the keeper's own page: before and after
    /// at the size he saw it, painted live by the ground figure.
    pub keeper: RoleId,
    /// The longest a ground may take to paint on a phone. The face stops a
    /// slower paint and draws the paper; the check holds every ground to
    /// it at a quarter of desktop speed. 150 ms: he opens pages on his phone
    /// between other things.
    pub paint_ms: u32,
    /// The files this version was read from.
    pub source: Option<Observation>,
}

/// A look an agent wears: what makes its page and chat window its own.
///
/// **Concept.** A finished piece of design, made with care in the design
/// system's files by the keeper's art agent ([`Design::keeper`]), and never
/// by an agent at run time. It has three parts:
/// - a ground: a generative painting, drawn for each task from its id;
/// - a palette: paper, ink, an accent and the figure colours, with the inks
///   for text set on the ground where they differ;
/// - a display face, for the end, where we are and the questions. It must
///   have a true italic and a true bold, since the language needs both.
///
/// **The theme says where Max is; the seed says who.** A theme is one art
/// direction, and each agent wearing it gets its own painting of it: the
/// same kind of harbour chart with a different coast, the same high
/// country under a different sun. So a theme has no variants, and any
/// number of agents may wear one at once, each still its own. Themes
/// differ from each other in what they look at and how they are made, not
/// only in colour: Tussock looks across a landscape, Harbour down on a
/// chart, Kauri up through a forest.
///
/// The ground fills the cover behind the lead, composed around the lead's
/// box so text sits on calm ground, then fades into the theme's paper as
/// Max scrolls. An agent's figures use the palette's tokens, never colours,
/// so a figure redraws in any theme. The state tones ([`Tone`]) are never a
/// theme's. An agent chooses its theme's name once and keeps it
/// ([`crate::page::Description`]); a revised theme reaches it from its
/// page's next open, so a fix shows on the page Max complained on.
///
/// Serves: Max's "really elegant and artistic [...] beautiful backgrounds,
/// UI themes, design systems, etc. that can be swapped between to make an
/// agent and its page unique"; "easy for me to see whereabouts I'm at";
/// "agents don't design their own themes"; Harbour and Kauri "need to be a
/// little bit visually more distinguished"; backgrounds "5 times better",
/// not "10% better".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    /// `harbour`.
    pub name: String,
    /// What an agent chooses it by, in `choose_theme`'s list: "A hand-engraved
    /// chart of a harbour: soundings, depth contours, shallows tinted blue."
    pub about: String,
    /// Its palette and display face: a stylesheet setting the language's
    /// tokens, with the font files it names.
    pub style: ValueId,
    /// Its ground: an ES module whose `paint(ctx, scene)` paints into a 2D
    /// canvas, given the canvas's size, the task's id as its seed, and the
    /// box the page's lead sits in, which it keeps calm. It is deterministic
    /// in its seed and size, so the same agent looks the same in every face,
    /// no other agent looks quite like it, and the keeper can paint again
    /// exactly what Max saw ([`Screen`]). The face paints it once when the
    /// window opens, at up to twice the CSS pixels, off the page's thread,
    /// and never animates it; scrolling slides and fades it into the paper.
    /// A ground that throws or passes [`Design::paint_ms`] leaves plain
    /// paper and reports itself ([`Fault`]). The face runs only grounds some
    /// version of the design system declared, since a ground is a program
    /// and a page body may hold none.
    ///
    /// Before a design version is published, the check paints every ground
    /// at phone, desktop and thumbnail sizes, for a set of seeds and the
    /// paintings Max loved, and holds it to its bars: the lead's ink against
    /// the ground in the lead's box at 7:1 for 95% of it and never under
    /// 4.5:1, since he reads it every day; [`Design::paint_ms`] at a quarter
    /// of desktop speed; a thumbnail that reads as its theme, since that is
    /// how he finds where he is; the same pixels when painted twice.
    pub ground: ValueId,
    /// Shown to Max on the keeper's page and never offered to
    /// `choose_theme`: a new theme waits for his choice, so no agent can
    /// lock in one he then rejects. One he rejects is deleted, and his words
    /// go in the design system's files.
    pub proposed: bool,
}

/// An element a body may use beyond markdown.
///
/// **Concept.** The vocabulary, as data: the brain's check and the agent's
/// guide both read it, so a new component is a design system change, not a
/// parser change. It is small on purpose: live names bind to a
/// [`crate::page::Ref`]; figures draw (mermaid, inline SVG, images from
/// values) in the theme's palette; a fold holds detail. There are no cards,
/// grids or stat tiles, since a page is read one thing at a time and the
/// map is the overview.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    pub tag: String,
    pub attributes: Vec<String>,
    /// One line on when to use it, for the guide.
    pub about: String,
}

/// What reaches the keeper about how a page looks: Max's words, passed on
/// by the agent he said them to, or a ground that failed on his screen.
///
/// **Concept.** Max gives his feedback to whichever agent he is reading,
/// and that agent has no say over its look. So it passes his words on with
/// `pass_on_style`, declared with the user plugin whether or not a keeper
/// runs. They are kept as a fact, open until the keeper answers them
/// ([`FeedbackEvent`]), and addressed to the keeper's role rather than to a
/// task: nothing he says is lost while no keeper runs, a second comment
/// during a fix is never dropped, and a keeper that wakes fresh reads what
/// is open rather than remembering it. The page he spoke on shows his
/// words' state, from taken to answered, and the map draws an open one as
/// a line from the page's agent to the keeper.
///
/// Serves: "if I give feedback on the styling, I'm going to give it to that
/// agent, so it needs to be able to pass it on, even though it's not able
/// to action it itself".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Feedback {
    /// His words, passed on whole: the agent never splits or retells them,
    /// since it is the least able to say which part is the ground's and
    /// which the type's. They carry the [`Screen`] he was looking at. Words
    /// he says in the keeper's own chat are kept the same way, by the
    /// keeper.
    Words {
        words: DeliveryId,
        /// The agent that passed them on.
        by: TaskId,
        /// Its note: what on its page he seems to mean.
        note: String,
    },
    /// A ground that failed in his tab, reported by the face itself, once
    /// per tab, ground, seed and failure. It needs nothing from him.
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
    Asked(AskId),
    /// A merged commit answers it: a change to the look, live on every page
    /// from its next open, or only his words recorded in the design
    /// system's files ("I love this one"). His rejection of the answer is
    /// new feedback, and a revert answers that.
    Answered {
        /// It names this feedback in its message.
        commit: String,
        /// What changed, for Max, beside his words and the before and after.
        note: String,
    },
}

/// What was on Max's screen when he spoke, set by the face as he sends.
///
/// **Concept.** Exactly what he saw, as values rather than a picture: which
/// page, which part of it, drawn by which programs, at what size. Grounds
/// are deterministic, so any agent can paint it again, before and after a
/// change, with no screenshot and no work data leaving the page. A ground
/// composes itself around the lead, which sits differently on a phone, so
/// the size is half of what he saw. His words carry it in place of the bare
/// [`Spot`], and every agent can read it ("he was on his phone").
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Screen {
    /// Whose page: also the seed its ground was painted with.
    pub task: TaskId,
    /// The page version drawn; `None`: none was published.
    pub page: Option<u32>,
    /// What he was reading, from the scroll.
    pub spot: Option<Spot>,
    pub theme: String,
    /// What drew it, so a later version can't be mistaken for the one he
    /// saw.
    pub language: ValueId,
    pub style: ValueId,
    pub ground: ValueId,
    pub viewport: Viewport,
    /// The lead's box, which the ground keeps calm.
    pub lead: Option<Rect>,
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

/// A box on the screen, in CSS pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
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

/// The state colours: the same hues on every page and on the map, never a
/// theme's, lightened only to read on a dark ground. "Your move" is the
/// same orange whatever an agent wears.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tone {
    /// Max's move.
    Yours,
    Working,
    /// Someone else's move.
    Others,
    Done,
    Failed,
    Quiet,
}
