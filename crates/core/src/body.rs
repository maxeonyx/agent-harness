//! A body: the markdown an agent writes inside a page's parts, as the
//! design language reads it.
//!
//! **Concept.** The agent's whole vocabulary for content. A body is GFM
//! markdown, stored as a value, and read by one parser, `harness-body`: the
//! brain checks every edit with it, and parses each body for the view the
//! face draws, so what was checked is what is drawn. The vocabulary is small and fixed:
//! paragraphs, lists, quotes, code, and four components with rendering of
//! their own (a figure, a table, a fold, a live name). There are no cards,
//! grids, stat tiles, headings, bold, italic, colours or script, since the
//! language owns emphasis and the theme owns the look. Whatever the agent
//! writes outside the vocabulary is drawn as plain text and listed for it
//! as a [`Problem`], never refused.
//!
//! Serves: "agents never style"; "the agent should be given quite a
//! structured way to put its information in"; "diagrams, tables, visual
//! hierarchies and colors, to rapidly communicate information without lots
//! of words"; "less is more [...] more content, less noise".
//!
//! Not yet folded: see [`crate::page`].

use serde::{Deserialize, Serialize};

use crate::id::{DeliveryId, ValueId};
use crate::page::{Ref, Spot};

/// A parsed body: what the face draws.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Body {
    pub blocks: Vec<Block>,
}

/// One block of a body, in reading order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Block {
    /// Reading text, in the one reading face and measure every theme shares.
    Para(Vec<Inline>),
    List {
        ordered: bool,
        items: Vec<Item>,
    },
    /// Someone else's words, behind a hairline rule.
    Quote(Vec<Block>),
    /// Max's own words, quoted from the delivery they came in: drawn behind
    /// the rule that marks his words everywhere. Checked against that
    /// delivery, so an agent can quote him but never misquote him: words
    /// not in it are drawn as a plain quote and listed as
    /// [`Problem::Misquoted`].
    Said {
        of: DeliveryId,
        words: String,
    },
    /// Literal text, in the mono face on the theme's tint: the only text the
    /// language ever boxes. On a phone it runs to the screen's edge and
    /// scrolls in its own frame.
    Code {
        lang: Option<String>,
        text: String,
    },
    Figure(Figure),
    Table(Table),
    Fold(Fold),
}

/// One item of a list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// A task list's box: `Some(false)` for `- [ ]`, `Some(true)` for
    /// `- [x]`: a hollow ring for not yet, a filled dot for already true.
    pub holds: Option<bool>,
    pub blocks: Vec<Block>,
}

/// A run of text inside a block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Inline {
    Text(String),
    Code(String),
    /// A link out: drawn underlined in the theme's accent, never as a pill.
    Link {
        url: String,
        words: String,
    },
    /// A live name ([`Ref`]): drawn with its state's mark, and its state
    /// now when pointed at.
    ///
    /// **Concept.** Two forms. Worded, `[the stream-json driver](github.pr:maxeonyx/agent-harness#241)`,
    /// draws the agent's words and the mark: for prose. Bare,
    /// `<github.pr:maxeonyx/agent-harness#241>`, draws the source's short
    /// name, the mark and the state's word ("#241 ● approved, yours to
    /// merge"): for tables and lists. A state changed since the page was
    /// presented adds a small note in the new state's colour ("now
    /// approved"), so prose written against the old state is never read
    /// as current. On a desktop a click goes to the thing and pointing
    /// shows its popover; on a phone a tap opens the popover, which links
    /// out. The popover is the language's only floating surface.
    ///
    /// Serves: "it gives me links"; "GitHub PR statuses, agent statuses at
    /// work, the ticketing system statuses" got by the harness.
    Live {
        of: Ref,
        words: Option<String>,
    },
    Break,
}

/// A figure: the agent's picture, drawn in the theme's palette.
///
/// **Concept.** Where a picture can replace a paragraph. The agent writes
/// what to draw, never how it looks: a figure's own colours, styles, fonts
/// and scripts are dropped, and the language's figure grammar draws it.
/// Outline is state: solid exists, dashed will exist, dotted and struck
/// goes away, orange is Max's move, red is failing, blue is being built.
/// Fill is kind: up to four kinds, `f1` to `f4`, in the theme's figure
/// colours. There is no box, caption, number or title: the sentence after
/// the figure says what it shows. One that is too wide for its column
/// breaks out to the wide measure on a desktop, takes its narrow form on a
/// phone (left to right becomes top to bottom), then shrinks until its
/// labels are 12px, then scrolls in its own frame, and pressing it shows it whole.
///
/// Serves: "diagrams, tables, visual hierarchies and colors"; "concrete
/// diagrams"; "visual first".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Figure {
    /// A fenced ```` ```mermaid ```` block: graphs, sequences, charts. Its
    /// nodes take the grammar's classes (`planned`, `gone`, `yours`,
    /// `failed`, `working`, `f1`..`f4`: the plan's and the tones' own
    /// words) with `:::`.
    Mermaid { source: String },
    /// A fenced ```` ```svg ```` block, for what mermaid can't lay out,
    /// using the same classes.
    Svg { source: String },
    /// Pictures alone in a paragraph: screenshots or renders. One is drawn
    /// across the column with no caption, like any figure. Two or more
    /// share rows at one height, each with its alt under it in small type,
    /// since there the alt is what tells them apart: two screens to choose
    /// between, a phone beside a desktop, a set of thumbnails. Max taps one
    /// to see it at full size.
    Images(Vec<Image>),
}

/// **Concept.** A picture on a page: the image a call's result showed the
/// agent, kept as that result's value, so every version of the page shows
/// what it showed when written, whatever happens to the file. The agent
/// cites the call by its label (`![what it shows](call-3fa2c1)`), so it
/// shows Max only what it has looked at itself; the brain keeps the
/// citation as the value (`![…](value:<id>)`, which the body's parser
/// reads).
///
/// Serves: "Show me the underlying stuff, not your explanations of it";
/// "information is best presented visually, not verbally"; of a choice
/// between themes, "Literally the only information I need ... is a picture".
///
/// The alt is required, since it is what a reader hears, what an old face
/// shows, and what names it beside another.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    /// The result that showed it: content blocks holding one image.
    pub value: ValueId,
    pub alt: String,
}

/// A table: a GFM pipe table, drawn in the language's one table style.
///
/// **Concept.** For facts that line up: comparisons, results, before and
/// after ("Today | When done"). Small UI type, the first column in ink,
/// muted sentence-case headers over a stronger hairline, hairlines between
/// rows, and nothing else: no boxes, vertical rules, zebra stripes or
/// captions. The agent's alignment colons are ignored; a numeric column is
/// found from its cells and set right-aligned in tabular figures. A table
/// that fits its column fills it; on a desktop one that would wrap breaks
/// out to the wide measure; one that can't fit even wrapped scrolls
/// sideways, its first column held in place.
///
/// Serves: "tables [...] to rapidly communicate information without lots of
/// words".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Table {
    pub head: Vec<Cell>,
    pub rows: Vec<Vec<Cell>>,
    /// Per column: every non-empty cell is a number, with units, currency,
    /// signs or ratios ("8.2 s", "14/14", "+612 −40").
    pub numeric: Vec<bool>,
}

/// A table cell: inline content, without breaks.
pub type Cell = Vec<Inline>;

/// Detail Max opens if he wants it.
///
/// **Concept.** A line that says what is inside, and the inside. Closed, it
/// is a thin chevron and the summary in small UI type, with no box, rule or
/// label; open, the body hangs from a hairline under it. Written inline
/// (`<details><summary>`), right after the sentence it backs, so detail is
/// one press from its claim and never an appendix. Its inside is markdown
/// of the same body, parsed with it. Every new version of a page starts
/// with every fold closed; within one version a fold he opened stays open.
/// Folds do not nest.
///
/// Serves: "I can scroll to see more"; "stuff should only come once it's
/// relevant"; "if I want, I can expand".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fold {
    /// What is inside, as a line he can decide on: "The 8.2 s stop, call by
    /// call". "Details", "Show more", a trailing colon, or over 100
    /// characters is listed as a problem.
    pub summary: String,
    pub body: Vec<Block>,
}

/// Something in an agent's writing the language draws differently from how
/// it was written, with where it is.
///
/// **Concept.** The language's rules, as the agent meets them. Each comes
/// back in the result of the call that wrote it and of `page_look`, saying
/// how to write it instead. Max never sees a problem: he sees the page
/// drawn as well as it can be.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spotted {
    pub at: Spot,
    /// The body's line, for a problem inside one.
    pub line: Option<u32>,
    pub problem: Problem,
}

/// A rule of the language an agent's writing broke.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Problem {
    /// A heading: the page's parts are the only structure. Drawn as text.
    Heading,
    /// Bold: bold is the language's, for what Max must answer. Drawn
    /// regular.
    Bold,
    /// Italic: no text is italic, since it is harder to read. Drawn upright.
    Italic,
    /// HTML other than a fold, a colour, or a style. Drawn as text, or
    /// dropped from a figure.
    Styled,
    /// A link to a kind that does not exist: drawn as its words.
    Kind { kind: String, kinds: Vec<String> },
    /// A live name its source does not know: drawn as its words.
    NotFound(Ref),
    /// An image with no alt.
    NoAlt,
    /// An image that can't be drawn as a picture: it names no call whose
    /// result showed the agent one, or it sits amid text or in a table.
    /// Drawn as its alt's words, or a link in them to an image on the web.
    Picture { why: String },
    /// Words given as Max's that are not in the delivery named.
    Misquoted,
    /// A figure that would not draw, with the drawer's message.
    Figure { error: String },
    /// A table with no rows, rows of different lengths, one column, or a
    /// cell over about fifteen words.
    Table { why: String },
    /// A fold inside a fold, drawn open, or anything nested past
    /// `harness_body`'s deepest level, drawn as its text.
    Nested,
    /// A fold's summary that is a label, not what is inside.
    Summary,
    /// A lead over 70 characters, or the task's title or the end's goal
    /// again.
    Lead,
    /// A body told rather than shown: over about 80 words of paragraphs,
    /// with no table, figure, code or quote.
    Told,
    /// No end, or an end that shows no figure: Max can't see what the work
    /// is aimed at.
    Unseen,
    /// A goal over about twenty words, or a use or a now over about
    /// fifteen: the detail goes in a body.
    Wordy,
    /// A use that doesn't work yet, or a step done or in progress and bound
    /// to nothing, with no now: where it stands is what Max reads it for.
    NoNow,
    /// Line breaks that arrived as the two characters `\n`, so the body
    /// draws as one paragraph: it was escaped twice. Read in its prose
    /// only, since `\n` inside code is literal text.
    Escaped,
    /// A step's now in the same words as a use's now: the use already says
    /// where Max stands, so the step says where its own work stands.
    Repeated,
}
