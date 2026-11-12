//! The shared design system pages are written in, and the themes they wear.
//!
//! Not yet folded: see [`crate::page`].

use serde::{Deserialize, Serialize};

use crate::id::ValueId;
use crate::resource::Observation;

/// The shared design system: themes, components, and how to write with them.
///
/// **Concept.** What every page has in common, owned in one place (the
/// Harness UI work), read from files in the repo and versioned as a role
/// is. The system provides the look: the themes, spacing, and how each
/// component draws. The agent provides content, in the order Max needs it:
/// the lead, then prose, tables and figures one at a time. So every page is
/// calm and readable on the phone without each agent learning CSS, and a
/// design change reaches every page, old versions included.
///
/// Serves: "use nice stylings that are unique and different in a structured
/// way compared to all other agents"; "encourages the agent to do really
/// high quality stuff, diagrams, tables, visual hierarchies and colors";
/// the vision's area colour with an agent variant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Design {
    pub themes: Vec<Theme>,
    pub components: Vec<Component>,
    /// The stylesheet every theme shares: layout, spacing, how components
    /// draw, written against the tokens each theme's style sets.
    pub css: ValueId,
    /// What an agent holding the `page` tool set is told: describe yourself
    /// first, a title for the work and a theme from the free ones; the lead
    /// is a point (the question, or the point) and a scene of a few lines,
    /// and nothing else is seen before Max scrolls; then one thing at a
    /// time, in the order he needs it, each ask after its why; name live
    /// things where the prose mentions them instead of retyping their
    /// state; write no colours or typefaces, since figures take the theme's
    /// palette tokens; no dashboard, no repeating the title, the state or
    /// the chat; detail last, folded. With the components and an example
    /// page.
    pub guide: ValueId,
    /// The files this version was read from.
    pub source: Option<Observation>,
}

/// A look an agent takes: everything about how its page and chat window
/// look.
///
/// **Concept.** A finished piece of design, made with care in the design
/// system's files and reviewed there, never by an agent at run time. It has
/// three parts:
/// - a ground: the background artwork, drawn for each task from its id, so
///   no two agents share one even in the same theme;
/// - a palette: paper, ink, an accent and the figure colours;
/// - two typefaces, self-hosted: one for the lead, one for reading.
///
/// It dresses the whole chat window: the ground behind the lead, the paper
/// the body and the chat sit on, the header and the composer. The page's
/// content stays sparse; the beauty is the theme's. An agent's figures use
/// the palette's tokens, never colours, so a figure redraws in any theme.
///
/// Themes come in families. A family is an area's art direction (sea mist
/// and depth contours; tree rings; a night sky), and each agent in the area
/// takes a variant of it (dawn, deep, moss), so where Max is shows at a
/// glance and each agent is still its own. An agent picks its theme from
/// the free ones when it starts ([`crate::page::Description::theme`]). The
/// state tones ([`Tone`]) are never a theme's.
///
/// Serves: Max's "really elegant and artistic [...] beautiful backgrounds,
/// UI themes, design systems, etc. that can be swapped between to make an
/// agent and its page unique"; themes "only from a list that's provided by
/// the design system"; the vision's area colour or pattern, with a variant
/// per agent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    /// `harbour-dawn`.
    pub name: String,
    /// `harbour`.
    pub family: String,
    /// What an agent picks it by, in `describe`'s list: "Sea mist and depth
    /// contours, set in Newsreader. Calm, for careful technical work."
    pub about: String,
    /// Its palette and typefaces: a stylesheet setting the design system's
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
/// parser change. It is small on purpose: live names and asks bind to a
/// [`crate::page::Ref`]; figures draw (mermaid, inline SVG, images from
/// values) in the theme's palette; a fold holds detail at the end. There are
/// no cards, grids or stat tiles, since a page is read one thing at a time
/// and the map is the overview.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    pub tag: String,
    pub attributes: Vec<String>,
    /// One line on when to use it, for the guide.
    pub about: String,
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
