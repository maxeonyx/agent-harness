//! The shared design system pages are drawn in, the themes they wear, and
//! the agent that keeps them.
//!
//! Not yet folded: see [`crate::page`].

use serde::{Deserialize, Serialize};

use crate::id::{DeliveryId, RoleId, TaskId, ValueId};
use crate::resource::Observation;

/// The shared design system: one design language, the themes, the
/// components, and the agent that keeps them.
///
/// **Concept.** Everything about how a page looks, owned in one place and
/// by one agent ([`Design::keeper`]), read from files in the repo and
/// versioned as a role is. Agents never style: they give typed parts and
/// short markdown ([`crate::page::Sheet`], [`crate::ask::Ask`]), and the
/// face draws them in the language, dressed in the task's theme. A change
/// here reaches every page, old versions included.
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
    pub themes: Vec<Theme>,
    pub components: Vec<Component>,
    /// What a task holding the user plugin is told: choose your theme first;
    /// the end is the world when the work is done, a state and not a plan; the point is the update in one sentence; then
    /// show it, a figure first; ask with `ask`, each ask with its own end,
    /// now and why, and never retell it on the page; name live things
    /// instead of retyping their state; write no colours, typefaces, bold or
    /// headings; detail goes in `more`; Max's words about how your page
    /// looks go to the keeper with `pass_on_style`. With the components and
    /// an example page.
    pub guide: ValueId,
    /// The role of the standing agent that owns the look, and the one place
    /// Max's words about it arrive, from any page ([`Feedback`]). It keeps
    /// the language and the guide itself, and hands everything about the art
    /// (grounds, palettes, new themes) to a standing art agent of its own.
    /// The art agent keeps one artist per theme and resumes that artist when
    /// its theme is spoken about, so the agent that knows a ground's code is
    /// the one that changes it; a new theme gets a new artist. A change
    /// reaches Max on the keeper's own page, before and after on the page he
    /// spoke from, as an ask he can reject.
    pub keeper: RoleId,
    /// The files this version was read from.
    pub source: Option<Observation>,
}

/// A look an agent wears: what makes its page and chat window its own.
///
/// **Concept.** A finished piece of design, made with care in the design
/// system's files by the art agent ([`Design::keeper`]), and never by an
/// agent at run time. It has three parts:
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
/// theme's. An agent chooses its theme once and keeps it
/// ([`crate::page::Description`]).
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
    /// box the page's lead sits in, which it keeps calm. The same agent looks
    /// the same in every face, and no other agent looks quite like it. The
    /// face paints it once when the window opens, at up to twice the CSS
    /// pixels, and never animates it; scrolling slides and fades it into the
    /// paper. Before a design version is published, the art agent's check
    /// paints every ground at phone, desktop and thumbnail sizes for a set of
    /// seeds and holds it to its bars: the lead's ink against the ground in
    /// the lead's box at 7:1 for 95% of it and never under 4.5:1; a phone
    /// paint within 150 ms at a quarter of desktop speed; a thumbnail that
    /// reads as its theme. A ground that throws leaves plain paper, and its
    /// error reaches the keeper.
    pub ground: ValueId,
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

/// What Max said about how a page looks, passed on to the agent that owns
/// the look.
///
/// **Concept.** Max gives his feedback to whichever agent he is reading,
/// and that agent can't act on it, since it has no say over its look. Its
/// `pass_on_style` call delivers this to the keeper's standing task, as
/// `Content::Feedback`, waking it: his own words, exactly what he was
/// looking at, and the agent's note. The keeper changes the design system,
/// and every page shows the change. The tool is declared only while a
/// keeper task is live, and the map draws the delivery as a line from the
/// page's agent to the keeper.
///
/// Serves: "if I give feedback on the styling, I'm going to give it to that
/// agent, so it needs to be able to pass it on, even though it's not able
/// to action it itself".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feedback {
    /// The task whose page he was reading.
    pub page: TaskId,
    /// The version he was reading, if it had been published.
    pub version: Option<u32>,
    /// The theme it wore.
    pub theme: String,
    /// His words, as delivered to the agent: quoted, never retold.
    pub words: DeliveryId,
    /// The agent's note: which part of the page he meant.
    pub note: String,
    /// The window he was reading in, so the art agent paints exactly what he
    /// saw: a ground composes itself around the lead, which sits differently
    /// on a phone. `None` when he spoke without the page open.
    pub seen: Option<Viewport>,
}

/// A window's size, in CSS pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
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
