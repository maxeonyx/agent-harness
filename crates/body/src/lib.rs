//! The one parser of a page body ([`harness_core::body`]): GFM markdown read
//! into the design language's small vocabulary. The brain checks every edit
//! with it and the face draws what it parsed, so what was checked is what is
//! drawn. Nothing is refused: whatever is outside the vocabulary is drawn as
//! well as it can be and listed as a [`Found`] problem for the agent.

mod html;
mod numeric;

use std::collections::VecDeque;

use harness_core::body::{Block, Body, Cell, Figure, Fold, Image, Inline, Item, Problem, Table};
use harness_core::id::{TaskId, ValueId};
use harness_core::page::Ref;
use harness_core::source::ItemKey;
use pulldown_cmark::{CodeBlockKind, Event, LinkType, Options, Parser, Tag};

use html::Tok;

/// How deep quotes, lists, folds and inline spans nest before the rest of
/// one is read as its plain text: the parser recurses per level, and an
/// agent's body must not be able to overflow the brain's stack.
const DEEPEST: usize = 16;

/// A problem in a body, at its 1-based line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub line: Option<u32>,
    pub problem: Problem,
}

/// A body's blocks, and its problems. `kinds` are the item kinds a live
/// name may have (`github.pr`).
pub fn body(markdown: &str, kinds: &[&str]) -> (Body, Vec<Found>) {
    let (body, mut found) = read(markdown, kinds);
    // A body escaped twice: its line breaks came as the two characters,
    // read in its prose, since `\n` inside code is literal text.
    if !markdown.contains('\n') && body.blocks.iter().any(|b| prose_has(b, "\\n")) {
        found.push(Found {
            line: None,
            problem: Problem::Escaped,
        });
    }
    (body, found)
}

/// Whether `block`'s prose, outside code and Max's quoted words, contains
/// `text`.
fn prose_has(block: &Block, text: &str) -> bool {
    let inlines = |list: &[Inline]| {
        list.iter().any(|i| match i {
            Inline::Text(t) => t.contains(text),
            Inline::Link { words, .. } => words.contains(text),
            _ => false,
        })
    };
    match block {
        Block::Para(list) => inlines(list),
        Block::List { items, .. } => items
            .iter()
            .flat_map(|i| &i.blocks)
            .any(|b| prose_has(b, text)),
        Block::Quote(blocks) => blocks.iter().any(|b| prose_has(b, text)),
        Block::Table(t) => t
            .head
            .iter()
            .chain(t.rows.iter().flatten())
            .any(|c| inlines(c)),
        Block::Fold(f) => f.summary.contains(text) || f.body.iter().any(|b| prose_has(b, text)),
        Block::Said { .. } | Block::Code { .. } | Block::Figure(_) => false,
    }
}

fn read(markdown: &str, kinds: &[&str]) -> (Body, Vec<Found>) {
    let mut reader = Reader::new(markdown, kinds);
    let blocks = reader.blocks(false);
    (Body { blocks }, reader.found)
}

/// `markdown` with each image's destination that `cite` knows replaced by
/// what it gives: how the brain keeps a picture cited by the call that
/// showed it (`![alt](call-3fa2c1)`) as that result's value
/// (`value:<id>`), the one form [`body`] draws as a picture. Any other
/// destination stays as written.
pub fn cited(markdown: &str, cite: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut from = 0;
    for (event, range) in Parser::new_ext(markdown, options()).into_offset_iter() {
        let Event::Start(Tag::Image { dest_url, .. }) = event else {
            continue;
        };
        let Some(to) = cite(&dest_url) else {
            continue;
        };
        // The destination comes after the alt, so its last occurrence in
        // the image is it.
        let Some(at) = markdown[range.clone()].rfind(&*dest_url) else {
            continue;
        };
        let at = range.start + at;
        out.push_str(&markdown[from..at]);
        out.push_str(&to);
        from = at + dest_url.len();
    }
    out.push_str(&markdown[from..]);
    out
}

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH
}

/// One paragraph's inlines, for a field that holds a line rather than a
/// body: blocks past the first are flattened into it after a break.
pub fn inline(markdown: &str, kinds: &[&str]) -> (Vec<Inline>, Vec<Found>) {
    let (body, found) = read(markdown, kinds);
    let mut out = Vec::new();
    for block in &body.blocks {
        if !out.is_empty() {
            out.push(Inline::Break);
        }
        flatten(block, &mut out);
    }
    (merged(out), found)
}

/// An inline, or an image, which is a figure only alone in its paragraph.
enum Piece {
    Inline(Inline),
    Image { src: String, alt: String, line: u32 },
}

struct Reader<'m, 'k> {
    events: Vec<(Event<'m>, u32)>,
    at: usize,
    /// Tags and text of the HTML read so far and not yet placed.
    html: VecDeque<(Tok, u32)>,
    /// How many folds are open where the reader is.
    folds: usize,
    /// How many containers are open where the reader is.
    depth: usize,
    /// The task box of the list item being read.
    marker: Option<bool>,
    kinds: &'k [&'k str],
    found: Vec<Found>,
}

impl<'m, 'k> Reader<'m, 'k> {
    fn new(markdown: &'m str, kinds: &'k [&'k str]) -> Reader<'m, 'k> {
        let breaks: Vec<usize> = markdown.match_indices('\n').map(|(i, _)| i).collect();
        let events = Parser::new_ext(markdown, options())
            .into_offset_iter()
            .map(|(event, range)| {
                (
                    event,
                    breaks.partition_point(|&b| b < range.start) as u32 + 1,
                )
            })
            .collect();
        Reader {
            events,
            at: 0,
            html: VecDeque::new(),
            folds: 0,
            depth: 0,
            marker: None,
            kinds,
            found: Vec::new(),
        }
    }

    fn spot(&mut self, line: u32, problem: Problem) {
        self.found.push(Found {
            line: Some(line),
            problem,
        });
    }

    fn next(&mut self) -> Option<(Event<'m>, u32)> {
        let event = self.events.get(self.at).cloned();
        self.at += 1;
        event
    }

    fn peek(&self) -> Option<&Event<'m>> {
        self.events.get(self.at).map(|(e, _)| e)
    }

    /// The blocks up to the end of the container being read, which it
    /// consumes, or, in a fold, up to its `</details>`. Past [`DEEPEST`], a
    /// container is one paragraph of its text, and a fold is left open.
    fn blocks(&mut self, in_fold: bool) -> Vec<Block> {
        if self.depth >= DEEPEST {
            if in_fold {
                return Vec::new();
            }
            let pieces = self.flat();
            return self.para(pieces).into_iter().collect();
        }
        self.depth += 1;
        let out = self.blocks_within(in_fold);
        self.depth -= 1;
        out
    }

    fn blocks_within(&mut self, in_fold: bool) -> Vec<Block> {
        let mut out = Vec::new();
        loop {
            if let Some((tok, line)) = self.html.pop_front() {
                match tok {
                    Tok::Open => self.fold(line, &mut out),
                    Tok::Close if in_fold => return out,
                    Tok::Close | Tok::Summary(_) | Tok::Other => self.spot(line, Problem::Styled),
                    Tok::Text(text) => out.push(Block::Para(vec![Inline::Text(text)])),
                }
                continue;
            }
            let Some((event, line)) = self.events.get(self.at).cloned() else {
                return out;
            };
            if inline_event(&event) {
                let mut pieces = Vec::new();
                while self.peek().is_some_and(inline_event) {
                    let (event, line) = self.next().expect("peeked");
                    self.inline(event, line, &mut pieces);
                }
                out.extend(self.para(pieces));
                continue;
            }
            self.at += 1;
            match event {
                // A fold's end is its container's too, so the container reads it.
                Event::End(_) if in_fold => {
                    self.at -= 1;
                    return out;
                }
                Event::End(_) => return out,
                Event::Start(Tag::Paragraph) => {
                    let pieces = self.inlines();
                    out.extend(self.para(pieces));
                }
                Event::Start(Tag::Heading { .. }) => {
                    self.spot(line, Problem::Heading);
                    let pieces = self.inlines();
                    out.extend(self.para(pieces));
                }
                Event::Start(Tag::BlockQuote(_)) => out.push(Block::Quote(self.blocks(false))),
                Event::Start(Tag::CodeBlock(kind)) => out.push(self.code(kind, line)),
                Event::Start(Tag::List(start)) => out.push(self.list(start.is_some())),
                Event::Start(Tag::Table(_)) => out.push(self.table(line)),
                Event::Start(Tag::HtmlBlock) => {
                    while let Some((Event::Html(text), line)) = self.next() {
                        self.read_html(&text, line);
                    }
                }
                Event::Html(text) => self.read_html(&text, line),
                Event::Rule => self.spot(line, Problem::Styled),
                Event::Start(_) => {
                    // Outside the parser's options: drawn as its text.
                    self.spot(line, Problem::Styled);
                    let pieces = self.inlines();
                    out.extend(self.para(pieces));
                }
                _ => {}
            }
        }
    }

    fn read_html(&mut self, text: &str, line: u32) {
        let (toks, styled) = html::tokens(text);
        if styled {
            self.spot(line, Problem::Styled);
        }
        self.html.extend(toks.into_iter().map(|t| (t, line)));
    }

    fn fold(&mut self, line: u32, out: &mut Vec<Block>) {
        let summary = match self.html.pop_front() {
            Some((Tok::Summary(s), _)) => s,
            other => {
                if let Some(tok) = other {
                    self.html.push_front(tok);
                }
                String::new()
            }
        };
        if is_label(&summary) {
            self.spot(line, Problem::Summary);
        }
        self.folds += 1;
        let nested = self.folds > 1;
        let body = self.blocks(true);
        self.folds -= 1;
        if nested {
            self.spot(line, Problem::Nested);
            out.extend(body);
        } else {
            out.push(Block::Fold(Fold { summary, body }));
        }
    }

    fn code(&mut self, kind: CodeBlockKind, line: u32) -> Block {
        let mut text = String::new();
        while let Some((event, _)) = self.next() {
            match event {
                Event::Text(t) => text.push_str(&t),
                Event::End(_) => break,
                _ => {}
            }
        }
        let lang = match kind {
            CodeBlockKind::Fenced(info) => info.split_whitespace().next().map(str::to_owned),
            CodeBlockKind::Indented => None,
        };
        match lang.as_deref() {
            Some("mermaid") => {
                // The face drops a figure's own styling: the theme owns the look.
                let styled = text.trim_start().starts_with("---")
                    || text.contains("%%{")
                    || text.lines().any(|l| {
                        let l = l.trim_start();
                        ["style ", "classDef ", "linkStyle "]
                            .iter()
                            .any(|k| l.starts_with(k))
                    });
                if styled {
                    self.spot(line, Problem::Styled);
                }
                Block::Figure(Figure::Mermaid { source: text })
            }
            Some("svg") => {
                let (source, dropped) = html::svg(&text);
                if dropped {
                    self.spot(line, Problem::Styled);
                }
                Block::Figure(Figure::Svg { source })
            }
            _ => Block::Code { lang, text },
        }
    }

    fn list(&mut self, ordered: bool) -> Block {
        let mut items = Vec::new();
        while let Some((event, _)) = self.next() {
            match event {
                Event::Start(Tag::Item) => {
                    // An outer item's box was read before this one began.
                    let outer = self.marker.take();
                    let blocks = self.blocks(false);
                    items.push(Item {
                        holds: self.marker.take(),
                        blocks,
                    });
                    self.marker = outer;
                }
                Event::End(_) => break,
                _ => {}
            }
        }
        Block::List { ordered, items }
    }

    fn table(&mut self, line: u32) -> Block {
        let mut head = Vec::new();
        let mut rows = Vec::new();
        while let Some((event, _)) = self.next() {
            match event {
                Event::Start(Tag::TableHead) => head = self.row(),
                Event::Start(Tag::TableRow) => rows.push(self.row()),
                Event::End(_) => break,
                _ => {}
            }
        }
        let mut whys = Vec::new();
        if rows.is_empty() {
            whys.push("it has no rows".to_owned());
        }
        if head.len() < 2 {
            whys.push("it has one column: write a list".to_owned());
        }
        let long = (head.iter().chain(rows.iter().flatten()))
            .any(|cell| text(cell).split_whitespace().count() > 15);
        if long {
            whys.push("a cell is over fifteen words: say it in a sentence after the table".into());
        }
        for why in whys {
            self.spot(line, Problem::Table { why });
        }
        let numeric = (0..head.len())
            .map(|c| {
                let cells: Vec<String> = (rows.iter())
                    .filter_map(|r| r.get(c))
                    .map(|cell| text(cell).trim().to_owned())
                    .filter(|t| !t.is_empty())
                    .collect();
                !cells.is_empty() && cells.iter().all(|t| numeric::is(t))
            })
            .collect();
        Block::Table(Table {
            head,
            rows,
            numeric,
        })
    }

    fn row(&mut self) -> Vec<Cell> {
        let mut cells = Vec::new();
        while let Some((event, _)) = self.next() {
            match event {
                Event::Start(Tag::TableCell) => {
                    let pieces = self.inlines();
                    for piece in &pieces {
                        if let Piece::Image { line, .. } = piece {
                            let why = "it sits in a table".to_owned();
                            self.spot(*line, Problem::Picture { why });
                        }
                    }
                    let cell = pieces.into_iter().map(as_inline).collect();
                    cells.push(merged(cell));
                }
                Event::End(_) => break,
                _ => {}
            }
        }
        cells
    }

    /// The pieces up to the end of the inline container being read, which
    /// it consumes.
    fn inlines(&mut self) -> Vec<Piece> {
        if self.depth >= DEEPEST {
            return self.flat();
        }
        self.depth += 1;
        let mut out = Vec::new();
        while let Some((event, line)) = self.next() {
            if let Event::End(_) = event {
                break;
            }
            self.inline(event, line, &mut out);
        }
        self.depth -= 1;
        out
    }

    /// The text up to the end of the container being read, which it
    /// consumes without recursing: a container nested past [`DEEPEST`].
    fn flat(&mut self) -> Vec<Piece> {
        if let Some((_, line)) = self.events.get(self.at) {
            self.spot(*line, Problem::Nested);
        }
        let mut out = Vec::new();
        let mut open = 0;
        while let Some((event, _)) = self.next() {
            match event {
                Event::Start(_) => open += 1,
                Event::End(_) if open == 0 => break,
                Event::End(_) => open -= 1,
                Event::Text(t) | Event::Code(t) => out.push(Piece::Inline(Inline::Text(t.into()))),
                Event::SoftBreak | Event::HardBreak => {
                    out.push(Piece::Inline(Inline::Text(" ".into())))
                }
                _ => {}
            }
        }
        out
    }

    fn inline(&mut self, event: Event<'m>, line: u32, out: &mut Vec<Piece>) {
        let text = |t: &str| Piece::Inline(Inline::Text(t.to_owned()));
        match event {
            Event::Text(t)
            | Event::InlineMath(t)
            | Event::DisplayMath(t)
            | Event::FootnoteReference(t) => out.push(text(&t)),
            Event::Code(c) => out.push(Piece::Inline(Inline::Code(c.into_string()))),
            Event::SoftBreak => out.push(text(" ")),
            Event::HardBreak => out.push(Piece::Inline(Inline::Break)),
            Event::InlineHtml(_) | Event::Html(_) => self.spot(line, Problem::Styled),
            Event::TaskListMarker(done) => self.marker = Some(done),
            Event::Start(Tag::Emphasis) => {
                self.spot(line, Problem::Italic);
                out.extend(self.inlines());
            }
            Event::Start(Tag::Strong) => {
                self.spot(line, Problem::Bold);
                out.extend(self.inlines());
            }
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                ..
            }) => {
                let inner = self.inlines();
                for piece in &inner {
                    if let Piece::Image { line, .. } = piece {
                        let why = "it sits inside a link, and a picture is already its own link to full size";
                        self.spot(*line, Problem::Picture { why: why.into() });
                    }
                }
                let words = pieces_text(&inner);
                let link = self.link(link_type, &dest_url, words, line);
                out.push(Piece::Inline(link));
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                let alt = pieces_text(&self.inlines());
                out.push(Piece::Image {
                    src: dest_url.into_string(),
                    alt,
                    line,
                });
            }
            Event::Start(_) => out.extend(self.inlines()),
            _ => {}
        }
    }

    fn link(&mut self, link_type: LinkType, dest: &str, words: String, line: u32) -> Inline {
        let bare = matches!(link_type, LinkType::Autolink | LinkType::Email);
        if link_type == LinkType::Email {
            return Inline::Link {
                url: format!("mailto:{dest}"),
                words,
            };
        }
        let live = |of| Inline::Live {
            of,
            words: (!bare).then(|| words.clone()),
        };
        if let Some(id) = dest.strip_prefix("task:") {
            return match id.parse::<TaskId>() {
                Ok(id) => live(Ref::Task(id)),
                Err(_) => Inline::Text(words),
            };
        }
        if let Some((kind, name)) = dest.split_once(':')
            && kind.contains('.')
        {
            if self.kinds.contains(&kind) {
                return live(Ref::Item(ItemKey {
                    kind: kind.to_owned(),
                    name: name.to_owned(),
                }));
            }
            let kinds = self.kinds.iter().map(|k| (*k).to_owned()).collect();
            let kind = kind.to_owned();
            self.spot(line, Problem::Kind { kind, kinds });
            return Inline::Text(words);
        }
        if ["http://", "https://", "mailto:"]
            .iter()
            .any(|s| dest.starts_with(s))
        {
            return Inline::Link {
                url: dest.to_owned(),
                words,
            };
        }
        Inline::Text(words)
    }

    /// A paragraph, or a figure when it holds only images; `None` when it
    /// holds nothing drawn.
    fn para(&mut self, pieces: Vec<Piece>) -> Option<Block> {
        let images: Vec<(&str, &str, u32)> = (pieces.iter())
            .filter_map(|p| match p {
                Piece::Image { src, alt, line } => Some((src.as_str(), alt.as_str(), *line)),
                Piece::Inline(_) => None,
            })
            .collect();
        let alone = pieces.iter().all(|p| match p {
            Piece::Image { .. } | Piece::Inline(Inline::Break) => true,
            Piece::Inline(Inline::Text(t)) => t.trim().is_empty(),
            Piece::Inline(_) => false,
        });
        let value = |src: &str| src.strip_prefix("value:")?.parse::<ValueId>().ok();
        let values: Option<Vec<ValueId>> = images.iter().map(|(src, _, _)| value(src)).collect();
        for (src, _, line) in &images {
            let why = match value(src) {
                None if src.starts_with("call-") => format!(
                    "`{src}` names no call of yours or your agents' whose result showed you one picture"
                ),
                None => format!(
                    "`{src}` is a file or an address, and a page draws only a picture you have seen"
                ),
                Some(_) if !alone => "it sits amid text".to_owned(),
                Some(_) => continue,
            };
            self.spot(*line, Problem::Picture { why });
        }
        if alone
            && !images.is_empty()
            && let Some(values) = values
        {
            let mut figure = Vec::new();
            for ((_, alt, line), value) in images.iter().zip(values) {
                if alt.trim().is_empty() {
                    self.spot(*line, Problem::NoAlt);
                }
                figure.push(Image {
                    value,
                    alt: (*alt).to_owned(),
                });
            }
            return Some(Block::Figure(Figure::Images(figure)));
        }
        let inlines = merged(pieces.into_iter().map(as_inline).collect());
        (!inlines.is_empty()).then_some(Block::Para(inlines))
    }
}

/// Whether an event belongs inside a paragraph: a tight list item's text
/// comes without one.
fn inline_event(event: &Event) -> bool {
    matches!(
        event,
        Event::Text(_)
            | Event::Code(_)
            | Event::InlineMath(_)
            | Event::FootnoteReference(_)
            | Event::SoftBreak
            | Event::HardBreak
            | Event::InlineHtml(_)
            | Event::TaskListMarker(_)
            | Event::Start(
                Tag::Emphasis
                    | Tag::Strong
                    | Tag::Strikethrough
                    | Tag::Superscript
                    | Tag::Subscript
                    | Tag::Link { .. }
                    | Tag::Image { .. }
            )
    )
}

/// An image that isn't drawn as a picture: a link to it in its alt's words
/// when it is on the web, or else its alt's words, since a value or a
/// call's label is nothing to open.
fn as_inline(piece: Piece) -> Inline {
    match piece {
        Piece::Inline(inline) => inline,
        Piece::Image { src, alt, .. }
            if src.starts_with("https://") || src.starts_with("http://") =>
        {
            Inline::Link {
                url: src,
                words: alt,
            }
        }
        Piece::Image { alt, .. } => Inline::Text(alt),
    }
}

/// A fold summary that names no content: "Details", "Show more:".
fn is_label(summary: &str) -> bool {
    let s = summary.trim().to_lowercase();
    const LABELS: [&str; 7] = [
        "",
        "details",
        "more",
        "show more",
        "show details",
        "expand",
        "click to expand",
    ];
    LABELS.contains(&s.as_str()) || s.ends_with(':') || s.chars().count() > 100
}

fn merged(inlines: Vec<Inline>) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::new();
    for inline in inlines {
        match (out.last_mut(), inline) {
            (Some(Inline::Text(a)), Inline::Text(b)) => a.push_str(&b),
            (_, inline) => out.push(inline),
        }
    }
    if let Some(Inline::Text(t)) = out.first_mut() {
        *t = t.trim_start().to_owned();
    }
    if let Some(Inline::Text(t)) = out.last_mut() {
        *t = t.trim_end().to_owned();
    }
    out.retain(|i| !matches!(i, Inline::Text(t) if t.is_empty()));
    out
}

fn inline_text(inline: &Inline) -> String {
    match inline {
        Inline::Text(t) | Inline::Code(t) => t.clone(),
        Inline::Link { words, .. } => words.clone(),
        Inline::Live { words, .. } => words.clone().unwrap_or_default(),
        Inline::Break => " ".into(),
    }
}

fn text(inlines: &[Inline]) -> String {
    inlines.iter().map(inline_text).collect()
}

fn pieces_text(pieces: &[Piece]) -> String {
    (pieces.iter())
        .map(|p| match p {
            Piece::Inline(i) => inline_text(i),
            Piece::Image { alt, .. } => alt.clone(),
        })
        .collect()
}

fn flatten(block: &Block, out: &mut Vec<Inline>) {
    let blocks = |blocks: &[Block], out: &mut Vec<Inline>| {
        for (n, b) in blocks.iter().enumerate() {
            if n > 0 {
                out.push(Inline::Break);
            }
            flatten(b, out);
        }
    };
    match block {
        Block::Para(inlines) => out.extend(inlines.iter().cloned()),
        Block::List { items, .. } => {
            for (n, item) in items.iter().enumerate() {
                if n > 0 {
                    out.push(Inline::Break);
                }
                blocks(&item.blocks, out);
            }
        }
        Block::Quote(inner) => blocks(inner, out),
        Block::Said { words, .. } => out.push(Inline::Text(words.clone())),
        Block::Code { text, .. } => out.push(Inline::Code(text.clone())),
        Block::Figure(Figure::Images(images)) => {
            let alts: Vec<&str> = images.iter().map(|i| i.alt.as_str()).collect();
            out.push(Inline::Text(alts.join(", ")));
        }
        Block::Figure(_) => {}
        Block::Table(table) => {
            for (n, row) in std::iter::once(&table.head).chain(&table.rows).enumerate() {
                if n > 0 {
                    out.push(Inline::Break);
                }
                let cells: Vec<String> = row.iter().map(|c| text(c)).collect();
                out.push(Inline::Text(cells.join(", ")));
            }
        }
        Block::Fold(fold) => {
            out.push(Inline::Text(fold.summary.clone()));
            out.push(Inline::Break);
            blocks(&fold.body, out);
        }
    }
}
