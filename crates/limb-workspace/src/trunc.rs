//! Head + tail + grep truncation, after Max's `trunc`
//! (github.com/maxeonyx/trunc).
//!
//! Output is fed in as it arrives and memory stays bounded however much
//! arrives: the first lines, the last lines, and the first and last matches
//! of an optional pattern with a few lines of context around each. A
//! pattern only adds lines to the head and tail; it never hides them. Each
//! gap is marked with how many lines, and how many matches, it hides. Long
//! lines keep their two ends.

use std::collections::VecDeque;

use regex::Regex;

/// How much of an output is kept. The defaults are `trunc`'s.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Lines kept from the start.
    pub first: usize,
    /// Lines kept from the end.
    pub last: usize,
    /// Matches kept, with their context, from the start of the middle.
    pub matches_first: usize,
    /// Matches kept, with their context, from the end of the middle.
    pub matches_last: usize,
    /// Lines of context before and after each kept match.
    pub context: usize,
    /// Characters a line may have before its middle is cut; 0 never cuts.
    pub width: usize,
    /// Bytes after which a line without a newline is broken, so a single
    /// endless line cannot grow without bound.
    pub piece: usize,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            first: 30,
            last: 30,
            matches_first: 3,
            matches_last: 3,
            context: 3,
            width: 200,
            piece: 64 * 1024,
        }
    }
}

/// The kept view of an output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Truncated {
    pub text: String,
    /// Whether anything was left out, so the full output is worth keeping.
    pub cut: bool,
}

/// Truncates one output as it arrives.
pub struct Truncator {
    limits: Limits,
    pattern: Option<Regex>,
    /// The number given to the first line, when lines are shown numbered.
    numbered: Option<usize>,
    partial: Vec<u8>,
    lines: usize,
    matches: usize,
    widened: bool,
    head: Vec<Kept>,
    tail: VecDeque<Kept>,
    /// The last `context` lines, the leading context of a match.
    recent: VecDeque<Kept>,
    /// The first `matches_first` matches of the middle, with context.
    early: Vec<Kept>,
    early_matches: usize,
    /// The last `matches_last` matches of the middle, each with context.
    late: VecDeque<Vec<Kept>>,
    /// Lines of trailing context still owed to the latest match.
    after: usize,
}

#[derive(Clone, Debug)]
struct Kept {
    line: usize,
    text: String,
    is_match: bool,
    /// Matches on earlier lines, so a gap can say how many it hides.
    matches_before: usize,
}

impl Truncator {
    pub fn new(limits: Limits, pattern: Option<Regex>) -> Truncator {
        Truncator {
            limits,
            pattern,
            numbered: None,
            partial: Vec::new(),
            lines: 0,
            matches: 0,
            widened: false,
            head: Vec::new(),
            tail: VecDeque::new(),
            recent: VecDeque::new(),
            early: Vec::new(),
            early_matches: 0,
            late: VecDeque::new(),
            after: 0,
        }
    }

    /// Show each line with its number, the first being `first`.
    pub fn numbered(mut self, first: usize) -> Truncator {
        self.numbered = Some(first);
        self
    }

    /// Feed bytes as they arrive. Invalid UTF-8 is replaced, not refused.
    pub fn feed(&mut self, mut bytes: &[u8]) {
        while let Some(at) = bytes.iter().position(|&b| b == b'\n') {
            self.partial.extend_from_slice(&bytes[..at]);
            self.end_line();
            bytes = &bytes[at + 1..];
        }
        self.partial.extend_from_slice(bytes);
        while self.partial.len() >= self.limits.piece.max(1) {
            let rest = self.partial.split_off(self.limits.piece.max(1));
            self.end_line();
            self.partial = rest;
        }
    }

    /// Feed one whole line.
    pub fn line(&mut self, line: &str) {
        self.feed(line.as_bytes());
        self.end_line();
    }

    fn end_line(&mut self) {
        let raw = String::from_utf8_lossy(&self.partial).into_owned();
        self.partial.clear();
        self.lines += 1;
        let is_match = self.pattern.as_ref().is_some_and(|p| p.is_match(&raw));
        let text = self.cut_width(raw);
        let kept = Kept {
            line: self.lines,
            text,
            is_match,
            matches_before: self.matches,
        };
        if is_match {
            self.matches += 1;
        }
        let limits = self.limits;
        if self.lines <= limits.first {
            self.head.push(kept);
            return;
        }
        self.tail.push_back(kept.clone());
        if self.tail.len() > limits.last {
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
            if self.early_matches < limits.matches_first {
                self.early_matches += 1;
                self.early.extend(group);
                self.after = limits.context;
            } else if limits.matches_last > 0 {
                self.late.push_back(group);
                if self.late.len() > limits.matches_last {
                    self.late.pop_front();
                }
                self.after = limits.context;
            }
        }
        self.recent.push_back(kept);
        if self.recent.len() > limits.context {
            self.recent.pop_front();
        }
    }

    fn cut_width(&mut self, line: String) -> String {
        let width = self.limits.width;
        let count = line.chars().count();
        if width == 0 || count <= width {
            return line;
        }
        self.widened = true;
        let half = width / 2;
        let start: String = line.chars().take(half).collect();
        let end: String = line.chars().skip(count - half).collect();
        format!("{start}[... {} chars ...]{end}", count - 2 * half)
    }

    /// The kept view of everything fed.
    pub fn finish(mut self) -> Truncated {
        if !self.partial.is_empty() {
            self.end_line();
        }
        let total = self.lines;
        let (first, last) = (self.limits.first, self.limits.last);
        let in_body = |k: &Kept| k.line > first && k.line + last <= total;
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

        let mut text = String::new();
        let mut cut = self.widened;
        let mut previous: Option<&Kept> = None;
        for k in &kept {
            let (line, matches) =
                previous.map_or((0, 0), |p| (p.line, p.matches_before + p.is_match as usize));
            let hidden = k.line - line - 1;
            if hidden > 0 {
                cut = true;
                text.push_str(&marker(hidden, k.matches_before - matches));
            }
            if let Some(first) = self.numbered {
                text.push_str(&format!("{:>6}\t", first + k.line - 1));
            }
            text.push_str(&k.text);
            text.push('\n');
            previous = Some(k);
        }
        Truncated { text, cut }
    }
}

fn marker(lines: usize, matches: usize) -> String {
    match matches {
        0 => format!("[... {lines} lines truncated ...]\n"),
        _ => format!("[... {lines} lines and {matches} matches truncated ...]\n"),
    }
}
