//! Head + tail + grep truncation, after Max's `trunc`
//! (github.com/maxeonyx/trunc), with the whole output kept beside it.
//!
//! Output is fed in as it arrives and memory stays bounded however much
//! arrives. The view keeps the first lines, the last lines, and the first
//! and last lines matching an optional pattern with a few lines of context
//! around each; a pattern only adds lines, it never hides them. Each gap is
//! marked with the line numbers it hides and how many of them match. Lines
//! are cut only to fit the view in [`BUDGET`], the longest first, each to
//! its two ends or, when it matches, to the part around its match.
//!
//! Whatever the view leaves out stays in [`Truncated::whole`], so the
//! caller can keep it where the agent can `read` or `grep` it: nothing is
//! lost, only one call away.

use std::collections::VecDeque;

use harness_core::value::Bytes;
use regex::Regex;

/// Lines kept from the start.
const FIRST: usize = 30;
/// Lines kept from the end.
const LAST: usize = 30;
/// Matching lines kept, with their context, from each end of the middle.
const MATCHES: usize = 3;
/// Lines of context before and after each kept match.
const CONTEXT: usize = 3;
/// Characters the kept lines may have between them, about 4k tokens:
/// a few paragraphs whole, or a build's errors.
pub const BUDGET: usize = 16 * 1024;
/// Characters no line is cut below, however many share the budget.
const NARROWEST: usize = 120;
/// Characters a search's line keeps around its match: a pointer into the
/// file, which says the rest.
const SEARCH: usize = 1000;
/// Bytes of one line held; the rest of a longer line is only counted, so
/// a single endless line cannot grow without bound.
const PIECE: usize = 64 * 1024;

/// The view of an output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Truncated {
    pub text: String,
    /// The whole output, when the view left any of it out.
    pub whole: Option<Vec<u8>>,
}

/// Truncates one output as it arrives.
pub struct Truncator {
    /// Picks the lines worth seeing from the middle.
    select: Option<Regex>,
    /// Where a cut line keeps its part around.
    focus: Option<Regex>,
    /// Characters a line is cut to, however much budget is left.
    widest: usize,
    partial: Vec<u8>,
    /// Bytes of the current line past [`PIECE`].
    overflow: usize,
    /// Characters at the start of the current line never cut.
    prefix: usize,
    /// Bytes fed in all, and whether any was NUL: output that is binary
    /// is not shown, only kept.
    bytes: usize,
    binary: bool,
    lines: usize,
    matches: usize,
    head: Vec<Kept>,
    tail: VecDeque<Kept>,
    /// The last `CONTEXT` lines, the leading context of a match.
    recent: VecDeque<Kept>,
    /// The first matches of the middle, with context.
    early: Vec<Kept>,
    early_matches: usize,
    /// The last matches of the middle, each with context.
    late: VecDeque<Vec<Kept>>,
    /// Lines of trailing context still owed to the latest match.
    after: usize,
    whole: Whole,
}

#[derive(Clone, Debug)]
struct Kept {
    line: usize,
    text: String,
    /// Bytes of the line past what `text` holds.
    overflow: usize,
    /// Characters at its start never cut: a search's `path:line:`.
    prefix: usize,
    is_match: bool,
    /// Matches on earlier lines, so a gap can say how many it hides.
    matches_before: usize,
}

impl Truncator {
    /// A view of a command's output: `pattern` picks lines from the middle.
    pub fn new(pattern: Option<Regex>) -> Truncator {
        Truncator::with(pattern.clone(), pattern, usize::MAX)
    }

    /// A view of lines that each match `pattern`, such as a search's: a
    /// line keeps at most [`SEARCH`] characters, around its match.
    pub fn matching(pattern: Regex) -> Truncator {
        Truncator::with(None, Some(pattern), SEARCH)
    }

    fn with(select: Option<Regex>, focus: Option<Regex>, widest: usize) -> Truncator {
        Truncator {
            select,
            focus,
            widest,
            partial: Vec::new(),
            overflow: 0,
            prefix: 0,
            bytes: 0,
            binary: false,
            lines: 0,
            matches: 0,
            head: Vec::new(),
            tail: VecDeque::new(),
            recent: VecDeque::new(),
            early: Vec::new(),
            early_matches: 0,
            late: VecDeque::new(),
            after: 0,
            whole: Whole::default(),
        }
    }

    /// Feed bytes as they arrive. Invalid UTF-8 is replaced, not refused.
    pub fn feed(&mut self, mut bytes: &[u8]) {
        self.whole.feed(bytes);
        self.bytes += bytes.len();
        self.binary |= bytes.contains(&0);
        while let Some(at) = bytes.iter().position(|&b| b == b'\n') {
            self.hold(&bytes[..at]);
            self.end_line();
            bytes = &bytes[at + 1..];
        }
        self.hold(bytes);
    }

    /// Feed one whole line, whose first `prefix` characters are never cut.
    pub fn line(&mut self, prefix: &str, line: &str) {
        self.prefix = prefix.chars().count();
        self.feed(prefix.as_bytes());
        self.feed(line.as_bytes());
        self.feed(b"\n");
    }

    fn hold(&mut self, bytes: &[u8]) {
        let room = PIECE.saturating_sub(self.partial.len()).min(bytes.len());
        self.partial.extend_from_slice(&bytes[..room]);
        self.overflow += bytes.len() - room;
    }

    fn end_line(&mut self) {
        let text = String::from_utf8_lossy(&self.partial).into_owned();
        self.partial.clear();
        self.lines += 1;
        let is_match = self.select.as_ref().is_some_and(|p| p.is_match(&text));
        let kept = Kept {
            line: self.lines,
            text,
            overflow: std::mem::take(&mut self.overflow),
            prefix: std::mem::take(&mut self.prefix),
            is_match,
            matches_before: self.matches,
        };
        if is_match {
            self.matches += 1;
        }
        if self.lines <= FIRST {
            self.head.push(kept);
            return;
        }
        self.tail.push_back(kept.clone());
        if self.tail.len() > LAST {
            self.tail.pop_front();
        }
        if self.after > 0 {
            self.after -= 1;
            match self.late.back_mut() {
                Some(group) => group.push(kept.clone()),
                None => self.early.push(kept.clone()),
            }
        }
        if is_match {
            let group: Vec<Kept> = self.recent.iter().cloned().chain([kept.clone()]).collect();
            if self.early_matches < MATCHES {
                self.early_matches += 1;
                self.early.extend(group);
            } else {
                self.late.push_back(group);
                if self.late.len() > MATCHES {
                    self.late.pop_front();
                }
            }
            self.after = CONTEXT;
        }
        self.recent.push_back(kept);
        if self.recent.len() > CONTEXT {
            self.recent.pop_front();
        }
    }

    /// The view of everything fed.
    pub fn finish(mut self) -> Truncated {
        if !self.partial.is_empty() || self.overflow > 0 {
            self.end_line();
        }
        let total = self.lines;
        if self.binary {
            return Truncated {
                text: format!("[{} bytes of binary output, not shown]\n", self.bytes),
                whole: Some(self.whole.finish()),
            };
        }
        let in_body = |k: &Kept| k.line > FIRST && k.line + LAST <= total;
        let mut kept: Vec<Kept> = self
            .head
            .into_iter()
            .chain(
                self.early
                    .into_iter()
                    .chain(self.late.into_iter().flatten())
                    .filter(in_body),
            )
            .chain(self.tail)
            .collect();
        kept.sort_by_key(|k| k.line);
        kept.dedup_by_key(|k| k.line);

        let width = width(kept.iter().map(|k| k.text.chars().count()).collect()).min(self.widest);
        let mut text = String::new();
        let mut cut = false;
        let mut previous: Option<&Kept> = None;
        for k in &kept {
            let (line, matches) =
                previous.map_or((0, 0), |p| (p.line, p.matches_before + p.is_match as usize));
            if k.line > line + 1 {
                cut = true;
                let matching = self.select.is_some().then_some(k.matches_before - matches);
                text.push_str(&gap(line + 1, k.line - 1, matching));
            }
            let (prefix, rest) = k.text.split_at(
                k.text
                    .char_indices()
                    .nth(k.prefix)
                    .map_or(k.text.len(), |(at, _)| at),
            );
            let shown = shorten(rest, width, self.focus.as_ref(), k.overflow);
            cut |= shown.len() != rest.len();
            text.push_str(prefix);
            text.push_str(&shown);
            text.push('\n');
            previous = Some(k);
        }
        Truncated {
            text,
            whole: cut.then(|| self.whole.finish()),
        }
    }
}

/// The widest a line may be so that lines of `lengths` fit in [`BUDGET`],
/// cutting the longest first; `usize::MAX` when they fit as they are.
fn width(mut lengths: Vec<usize>) -> usize {
    lengths.sort_unstable();
    let mut left = BUDGET;
    for (i, &length) in lengths.iter().enumerate() {
        let share = left / (lengths.len() - i);
        if length > share {
            return share.max(NARROWEST);
        }
        left -= length;
    }
    usize::MAX
}

/// `line` in at most about `width` characters: around the first match of
/// `focus` if it has one, otherwise its two ends. `overflow` bytes of it
/// past `line` were not held, and are marked at its end.
fn shorten(line: &str, width: usize, focus: Option<&Regex>, overflow: usize) -> String {
    let count = line.chars().count();
    let more = match overflow {
        0 => String::new(),
        n => format!(" and {n} bytes"),
    };
    if count <= width {
        return match overflow {
            0 => line.into(),
            n => format!("{line}[... {n} bytes ...]"),
        };
    }
    let chars: Vec<char> = line.chars().collect();
    let take = |from: usize, to: usize| chars[from..to].iter().collect::<String>();
    let hidden = |n: usize| format!("[... {n} chars ...]");
    let last = |n: usize| format!("[... {n} chars{more} ...]");
    match focus.and_then(|f| f.find(line)) {
        Some(m) => {
            let at = line[..m.start()].chars().count();
            let start = at.saturating_sub(width / 4).min(count - width);
            let end = start + width;
            let mut out = String::new();
            if start > 0 {
                out.push_str(&hidden(start));
            }
            out.push_str(&take(start, end));
            if end < count || overflow > 0 {
                out.push_str(&last(count - end));
            }
            out
        }
        None => {
            let half = width / 2;
            let tail = match overflow {
                0 => String::new(),
                n => format!("[... {n} bytes ...]"),
            };
            format!(
                "{}{}{}{tail}",
                take(0, half),
                hidden(count - 2 * half),
                take(count - half, count)
            )
        }
    }
}

/// The marker of hidden lines `from` to `to`, saying how many match when
/// lines were picked by a pattern.
fn gap(from: usize, to: usize, matching: Option<usize>) -> String {
    let lines = match from == to {
        true => format!("line {from}"),
        false => format!("lines {from}-{to}"),
    };
    match matching {
        None => format!("[... {lines} hidden ...]\n"),
        Some(0) => format!("[... {lines} hidden, none matching ...]\n"),
        Some(n) => format!("[... {lines} hidden, {n} matching ...]\n"),
    }
}

/// The whole output, cut to fit [`Bytes::LIMIT`]: its head and its tail,
/// and how much was left out between them. Holds at most the limit, however
/// much arrives.
#[derive(Default)]
struct Whole {
    head: Vec<u8>,
    tail: VecDeque<u8>,
    omitted: u64,
}

/// Each of the head and the tail, leaving room for the marker between.
const HALF: usize = Bytes::LIMIT / 2 - 64;

impl Whole {
    fn feed(&mut self, mut bytes: &[u8]) {
        let room = HALF - self.head.len();
        let n = room.min(bytes.len());
        self.head.extend_from_slice(&bytes[..n]);
        bytes = &bytes[n..];
        self.tail.extend(bytes);
        let over = self.tail.len().saturating_sub(HALF);
        self.tail.drain(..over);
        self.omitted += over as u64;
    }

    fn finish(self) -> Vec<u8> {
        let mut out = self.head;
        if self.omitted > 0 {
            out.extend_from_slice(
                format!("\n[... {} bytes left out ...]\n", self.omitted).as_bytes(),
            );
        }
        out.extend(self.tail);
        out
    }
}
