//! The shared design system pages are written in.
//!
//! Not yet folded: see [`crate::page`].

use serde::{Deserialize, Serialize};

use crate::id::ValueId;
use crate::resource::Observation;

/// The shared design system: themes, components, and how to write with them.
///
/// **Concept.** What every page has in common, owned in one place (the
/// Harness UI work), read from files in the repo and versioned as a role
/// is. The system provides the look: type, spacing, how each component
/// draws, and the themes. The agent provides content, in the order Max
/// needs it: the lead, then prose, tables and figures one at a time. So
/// every page is calm and readable on the phone without each agent
/// learning CSS, and a design change reaches every page, old versions
/// included.
///
/// Serves: "use nice stylings that are unique and different in a structured
/// way compared to all other agents"; "encourages the agent to do really
/// high quality stuff, diagrams, tables, visual hierarchies and colors";
/// the vision's area colour with an agent variant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Design {
    pub themes: Vec<Theme>,
    pub components: Vec<Component>,
    /// The stylesheet the face draws pages with.
    pub css: ValueId,
    /// What an agent holding the `page` tool set is told: open on the
    /// lead (the question, or the point, and the scene) and nothing else;
    /// then one thing at a time, in the order he needs it, each ask after
    /// its why; name live things where the prose mentions them instead of
    /// retyping their state; no dashboard, no repeating the title, the state
    /// or the chat; detail last, folded. With the components and an example
    /// page.
    pub guide: ValueId,
    /// The files this version was read from.
    pub source: Option<Observation>,
}

/// A look an agent can take.
///
/// **Concept.** An area's colour and an agent's variant of it. An agent
/// picks its theme when it starts ([`crate::page::Description::theme`]); a
/// task Max starts picks a family, and what it starts picks variants within
/// that family, so where Max is shows at a glance. It is an accent, not a
/// frame: the chat's top edge, the mark above the lead, a figure's colours.
/// The state tones
/// ([`Tone`]) are never a theme's, so "your move" looks the same on every
/// page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    /// `harbour`, `harbour-dusk`.
    pub name: String,
    /// `harbour`.
    pub family: String,
}

/// An element a body may use beyond markdown.
///
/// **Concept.** The vocabulary, as data: the brain's check and the agent's
/// guide both read it, so a new component is a design system change, not a
/// parser change. It is small on purpose: live names and asks bind to a
/// [`crate::page::Ref`]; figures draw (mermaid, inline SVG, images from
/// values); a fold holds detail at the end. There are no cards, grids or
/// stat tiles, since a page is read one thing at a time and the map is the
/// overview.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    pub tag: String,
    pub attributes: Vec<String>,
    /// One line on when to use it, for the guide.
    pub about: String,
}

/// The state colours: the same on every page and the map, never a theme's.
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
