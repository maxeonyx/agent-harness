//! The shared design system pages are drawn in, the themes they wear, and
//! the agent that keeps them.
//!
//! Not yet folded: see [`crate::page`].

use serde::{Deserialize, Serialize};

use crate::id::{DeliveryId, RoleId, TaskId, ValueId};
use crate::resource::Observation;

/// The shared design system: one visual language, the themes, the guide,
/// and the agent that keeps them.
///
/// **Concept.** Everything about how a page looks, owned in one place and
/// by one agent ([`Design::keeper`]), read from files in the repo and
/// versioned as a role is. Agents never style: they give typed parts and
/// short markdown ([`crate::page::Sheet`], [`crate::body`],
/// [`crate::ask::Ask`]), and the face draws them in the language, dressed
/// in the task's theme. A change here reaches every page, old versions
/// included.
///
/// **The language is the same on every page.** It is what Max learns once,
/// so that on any agent's page he knows where to look:
/// - *Order.* The lead, where this ends, where we are, the plan with the
///   asks in its steps, then the detail, folded. Nothing names a part: no
///   headings over parts, no small labels above them, no cues to scroll.
///   A part's own first words frame it.
/// - *Faces.* The theme's display face for what is said once (the lead,
///   the two claims, a step's title, a question); one reading face for
///   prose; one UI face for small text and data; one mono face for literal
///   text. Only the display face varies by theme.
/// - *Sizes.* Four, on one scale: the lead (34 px desktop, 28 phone), a
///   claim or a step in play (22, 20), reading (17.5, 17), small (14.5,
///   14), which is for labels and data alike.
/// - *Weights.* Regular, and bold for what Max must read and answer now:
///   the lead, and a question that is his move. A question loses its bold
///   once it is settled or someone else's move, so bold on a page is
///   always a count of what is his. Nothing is italic, since italic is
///   harder for him to read. Agents' markdown has no headings, bold or
///   italic.
/// - *Measure and space.* One column, 640 px; figures and tables that need
///   it break out to 820 px on a desktop, and scroll in their own frame on
///   a phone. Space separates: 72 px between parts (56 on a phone), 44
///   between the end and the now, which are a pair, 28 between blocks.
/// - *No cards.* Nothing is boxed but literal text (code, a prepared call),
///   on the theme's tint. Hairlines separate rows of a table and nothing
///   else.
/// - *A pill is an answer, and nothing else is a pill.* Links, folds and
///   live names are pressable too, but each looks like what it is. The
///   pill the agent proposes is filled. "Wrong question" is always last,
///   and as plain as the others.
/// - *Marks.* Filled is reached or true; hollow is not yet. Each state has
///   one colour and one shape ([`Tone`]), the same on the page, the plan
///   rail, the map and in figures, so they read in greyscale too. Lines
///   say the same: solid exists, dashed will exist, dotted and struck goes
///   away.
/// - *Change.* What changed since he last looked carries a bar in the left
///   gutter, beside each block that changed. Nothing else uses the gutter.
/// - *His words.* Max's own words, wherever they appear, hang from a 2 px
///   rule: orange while he drafts, the line colour once sent.
/// - *One floating surface.* A live name's popover, only while pointed at.
///
/// **A theme varies only what makes an agent its own** ([`Theme`]): its
/// palette, its display face and its ground. The state tones are never a
/// theme's. Because a theme is typed, not a stylesheet, it cannot reach
/// the rest.
///
/// Serves: "we should have a design language for the text"; "the use of
/// plain font versus bold versus italics, sizing of fonts, cards or not,
/// pills or not. These kind of things need to be consistent across things,
/// but there's also things that shouldn't be consistent across things, and
/// we need to get that right"; "italics is harder to read"; "less is more,
/// okay? But more content, less noise"; "the agent shouldn't be in charge
/// of the styling".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Design {
    /// The language: the one stylesheet every theme shares, written against
    /// the tokens a [`Theme`] sets, and nothing else.
    pub language: ValueId,
    pub themes: Vec<Theme>,
    /// What a task holding the user plugin is told about writing a page:
    /// the parts and what each is for, the body's vocabulary
    /// ([`crate::body`]), and an example page.
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
/// three parts, and only these, so a theme can't change what the language
/// means: a palette, a display face, and a ground.
///
/// Themes come in families. A family is an area's art direction (the
/// harbour's depth contours, the kauri forest, tussock country), and each
/// agent in the area wears a variant of it, so where Max is shows at a
/// glance and each agent is still its own. Each family differs from the
/// others in its technique, its light and its hue, not only its colour. An
/// agent chooses its theme once, from the free ones, and keeps it.
///
/// Serves: Max's "really elegant and artistic [...] beautiful backgrounds,
/// UI themes, design systems, etc. that can be swapped between to make an
/// agent and its page unique"; "agents don't design their own themes";
/// "easy for me to see whereabouts I'm at".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    /// `tussock-noon`.
    pub name: String,
    /// `tussock`.
    pub family: String,
    /// What an agent chooses it by, in `choose_theme`'s list: "Rolling
    /// tussock hills under a high sun."
    pub about: String,
    pub palette: Palette,
    pub display: Face,
    pub ground: Ground,
}

/// A theme's colours: its paper, its inks, its accent, and its figures'
/// fills.
///
/// **Concept.** The tokens the language's stylesheet is written against.
/// The design system's check lists a palette whose text is under 4.5:1 on
/// its paper, whose figure fills are too close to tell apart at 18%, or
/// whose accent or fills read as a state tone (above all as orange, which
/// is always Max's move), so a theme never makes a mark lie.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Palette {
    pub paper: Rgb,
    /// Headings, the first column of a table, what he must read.
    pub ink: Rgb,
    /// Reading text.
    pub body: Rgb,
    /// Small text that can wait: table headers, past steps, dates.
    pub muted: Rgb,
    /// Hairlines.
    pub line: Rgb,
    /// The ground under literal text, and subgraphs in figures.
    pub tint: Rgb,
    /// Links, and the theme's own touch: never a state.
    pub accent: Rgb,
    /// Fills for up to four kinds of thing in a figure (`f1`..`f4`).
    pub figure: [Rgb; 4],
}

/// A colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rgb(pub [u8; 3]);

/// A theme's display face.
///
/// **Concept.** The one typeface a theme chooses: for the lead, the claims,
/// step titles and questions. Its bold is set per face so that bold looks
/// equally bold in every theme. Reading, UI and code faces are the
/// language's.
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

/// A theme's ground: the art behind the lead.
///
/// **Concept.** Background artwork, a drawing program the face runs for
/// each task, seeded by the task's id, so the same agent looks the same in
/// every face and no two agents share one, even in the same theme. It
/// fills the lead's cover and fades into the paper, so reading text always
/// sits on calm ground.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ground {
    pub draw: ValueId,
    /// How much of the cover, from the bottom, the art's busy part takes,
    /// in percent: the lead is set in the calm part above it. 0 centres the
    /// lead like a title page.
    pub band: u8,
}

/// What Max said about how a page looks, passed on to the agent that owns
/// the look.
///
/// **Concept.** Max gives his feedback to whichever agent he is reading,
/// and that agent can't act on it, since it has no say over its look. Its
/// `pass_on_style` call delivers this to the keeper's standing task, as
/// `Content::Feedback`, waking it: his own words, exactly what he was
/// looking at, and the agent's note. The keeper changes the design system,
/// and every page shows the change.
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
