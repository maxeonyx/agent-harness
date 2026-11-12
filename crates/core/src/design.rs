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
/// - The order: the lead, where this ends, where we are, the plan, then
///   the asks ([`crate::page`]). No part is labelled: order and type say
///   which part is which.
/// - Slant and weight carry meaning, not decoration. Italic is where this
///   ends, and nothing else. Bold is a question for Max, and nothing else.
///   Everything else is regular. Agents' markdown has no bold or headings.
/// - Four sizes on one scale: the cover, a heading, reading, a label.
/// - One reading face and one measure, so body text reads the same
///   everywhere.
/// - No cards, and no boxes around prose. An ask is set off by space and a
///   hairline, and its frame reads as sentences, with no labels.
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
    /// What a task holding the user plugin is told about the look: choose
    /// your theme first, from the free ones; the components, each with when
    /// to use it; figures use the palette's tokens; write no colours,
    /// typefaces, bold, headings or labels; Max's words about how your page
    /// looks go to the keeper with `pass_on_style`. What a report says, part
    /// by part, is not the look's: it is the user plugin's report guide.
    pub guide: ValueId,
    /// The role of the standing agent that owns the look: the art, the
    /// themes, the language and the guide. It makes new themes on request,
    /// as high-effort programmatic art, and Max's words about the look reach
    /// it from any page ([`Feedback`]).
    pub keeper: RoleId,
    /// The files this version was read from.
    pub source: Option<Observation>,
}

/// A look an agent wears: what makes its page and chat window its own.
///
/// **Concept.** A finished piece of design, made with care in the design
/// system's files by its keeper, and never by an agent at run time. It has
/// three parts:
/// - a ground: background artwork, drawn for each task from its id, so no
///   two agents share one, even in the same theme;
/// - a palette: paper, ink, an accent and the figure colours;
/// - a display face, for the end, where we are and the questions. It must
///   have a true italic and a true bold, since the language needs both.
///
/// The ground fills the cover around the end, then gives way to the
/// theme's paper, so text always sits on calm ground. An agent's figures
/// use the palette's tokens, never colours, so a figure redraws in any
/// theme. The state tones ([`Tone`]) are never a theme's.
///
/// Themes come in families. A family is an area's art direction (the
/// harbour's depth contours, the kauri forest, tussock country), and each
/// agent in the area wears a variant of it, so where Max is shows at a
/// glance and each agent is still its own. Each family differs from the
/// others in its technique, its light and its hue, not only its colour. An
/// agent chooses its theme once, from the free ones, and keeps it
/// ([`crate::page::Description`]).
///
/// Serves: Max's "really elegant and artistic [...] beautiful backgrounds,
/// UI themes, design systems, etc. that can be swapped between to make an
/// agent and its page unique"; "agents don't design their own themes";
/// themes "only from a list that's provided by the design system".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    /// `harbour-dawn`.
    pub name: String,
    /// `harbour`.
    pub family: String,
    /// What an agent chooses it by, in `choose_theme`'s list: "Depth
    /// contours on sea mist, at first light."
    pub about: String,
    /// Its palette and display face: a stylesheet setting the language's
    /// tokens, with the font files it names.
    pub style: ValueId,
    /// Its ground: a drawing program the face runs for each task, seeded by
    /// the task's id, so the same agent looks the same in every face and no
    /// other agent looks quite like it.
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
