//! The little HTML a body may hold: a fold's tags, and an SVG figure with
//! its styling taken out.

/// A piece of a body's HTML.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tok {
    /// `<details>`.
    Open,
    /// `<summary>…</summary>`, as its text.
    Summary(String),
    /// `</details>`.
    Close,
    /// Text between tags.
    Text(String),
    /// Any other tag: dropped.
    Other,
}

/// The pieces of `text`, and whether a summary held tags of its own.
pub fn tokens(text: &str) -> (Vec<Tok>, bool) {
    let mut toks = Vec::new();
    let mut styled = false;
    let mut rest = text;
    while !rest.is_empty() {
        let Some(open) = rest.find('<') else {
            push_text(&mut toks, rest);
            break;
        };
        push_text(&mut toks, &rest[..open]);
        let Some(close) = rest[open..].find('>') else {
            push_text(&mut toks, &rest[open..]);
            break;
        };
        let tag = &rest[open + 1..open + close];
        rest = &rest[open + close + 1..];
        match name(tag).as_str() {
            "details" => toks.push(Tok::Open),
            "/details" => toks.push(Tok::Close),
            "summary" => {
                let end = find_ci(rest, "</summary>").unwrap_or(rest.len());
                let (words, tagged) = strip_tags(&rest[..end]);
                styled |= tagged;
                toks.push(Tok::Summary(words.trim().to_owned()));
                rest = &rest[(end + "</summary>".len()).min(rest.len())..];
            }
            _ => toks.push(Tok::Other),
        }
    }
    (toks, styled)
}

fn push_text(toks: &mut Vec<Tok>, text: &str) {
    let text = text.trim();
    if !text.is_empty() {
        toks.push(Tok::Text(text.to_owned()));
    }
}

/// A tag's name, lowercased, with `/` for a closing one.
fn name(tag: &str) -> String {
    let tag = tag.trim_start();
    let (slash, tag) = match tag.strip_prefix('/') {
        Some(t) => ("/", t),
        None => ("", tag),
    };
    let word: String = tag
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == ':')
        .collect();
    format!("{slash}{}", word.to_ascii_lowercase())
}

fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
    haystack.to_ascii_lowercase().find(needle)
}

/// The text of `html` without its tags, and whether it had any.
fn strip_tags(html: &str) -> (String, bool) {
    let mut out = String::new();
    let mut tagged = false;
    let mut inside = false;
    for c in html.chars() {
        match c {
            '<' => {
                inside = true;
                tagged = true;
            }
            '>' if inside => inside = false,
            c if !inside => out.push(c),
            _ => {}
        }
    }
    (out, tagged)
}

/// An SVG with its styling dropped, so the language's figure grammar
/// draws it, and whether anything was dropped.
pub fn svg(source: &str) -> (String, bool) {
    let mut out = String::new();
    let mut dropped = false;
    let mut rest = source;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('>') else {
            rest = &rest[open..];
            break;
        };
        let tag = &rest[open + 1..open + close];
        rest = &rest[open + close + 1..];
        let tag_name = name(tag);
        if tag_name == "script" || tag_name == "style" {
            dropped = true;
            let end = format!("</{tag_name}");
            rest = match find_ci(rest, &end) {
                Some(at) => rest[at..].find('>').map_or("", |gt| &rest[at + gt + 1..]),
                None => "",
            };
            continue;
        }
        if tag_name.starts_with('/') || tag.starts_with('!') || tag.starts_with('?') {
            out.push('<');
            out.push_str(tag);
            out.push('>');
            continue;
        }
        let (kept, lost) = attributes(tag);
        dropped |= lost;
        out.push('<');
        out.push_str(&kept);
        out.push('>');
    }
    out.push_str(rest);
    (out, dropped)
}

/// An opening tag rebuilt without its styling attributes, and whether it
/// had any.
fn attributes(tag: &str) -> (String, bool) {
    let self_closing = tag.trim_end().ends_with('/');
    let body = tag.trim_end().trim_end_matches('/');
    let name_end = body.find(|c: char| c.is_whitespace()).unwrap_or(body.len());
    let mut out = body[..name_end].to_owned();
    let mut dropped = false;
    let mut rest = &body[name_end..];
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }
        let key_end = rest
            .find(|c: char| c.is_whitespace() || c == '=')
            .unwrap_or(rest.len());
        let key = &rest[..key_end];
        let mut end = key_end;
        if rest[key_end..].trim_start().starts_with('=') {
            let eq = key_end + rest[key_end..].find('=').expect("found");
            let after = rest[eq + 1..].trim_start();
            let start = rest.len() - after.len();
            end = match after.chars().next() {
                Some(q @ ('"' | '\'')) => {
                    after[1..].find(q).map_or(rest.len(), |at| start + at + 2)
                }
                _ => start + after.find(char::is_whitespace).unwrap_or(after.len()),
            };
        }
        let lower = key.to_ascii_lowercase();
        let styling = ["style", "fill", "color"].contains(&lower.as_str())
            || lower.starts_with("font")
            || lower.starts_with("stroke")
            || lower.starts_with("on");
        if styling {
            dropped = true;
        } else {
            out.push(' ');
            out.push_str(&rest[..end]);
        }
        rest = &rest[end..];
    }
    if self_closing {
        out.push('/');
    }
    (out, dropped)
}
