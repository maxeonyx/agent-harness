//! Running each tool.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use harness_core::call::{Call, Outcome, Output};
use harness_core::id::{CallId, Digest, ValueId};
use harness_core::live::Live;
use harness_core::resource::{Observation, Version};
use regex::Regex;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use harness_limb::{Report, Running, lock};
use harness_runtime::Cancel;

use crate::record::replace;
use crate::shell::{self, Ended};
use crate::trunc::{Truncated, Truncator};
use crate::{Inner, canonical};

const DEFAULT_TIMEOUT: u64 = 120_000;
const MAX_TIMEOUT: u64 = 600_000;

#[derive(Deserialize)]
struct Bash {
    command: String,
    grep: Option<String>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
struct Read {
    path: String,
    offset: Option<usize>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct Write {
    path: String,
    content: String,
}

#[derive(Deserialize)]
struct Edit {
    path: String,
    edits: Vec<Replace>,
}

/// One replacement of an edit.
#[derive(Deserialize)]
struct Replace {
    old: String,
    new: String,
    #[serde(default)]
    all: bool,
}

#[derive(Deserialize)]
struct Grep {
    pattern: String,
    path: Option<String>,
}

/// The most text one `read` returns: whole lines, each numbered, until the
/// next would pass it. The tool's description states it.
pub(crate) const READ_BUDGET: usize = 64 * 1024;

/// Lines a refused edit's look starts before its `old`, in a file too
/// large to show whole.
const LEAD: usize = 20;

/// The largest image `read` shows: the provider's limit on an image, as
/// base64, is 5 MB.
const IMAGE_BUDGET: usize = 3 * 1024 * 1024;

/// A path as the tools see it: where it is, canonical, and its resource
/// name, which is that path.
struct Place {
    path: PathBuf,
    name: String,
}

impl Inner {
    /// Run `call` (the running call as its caller wrote it) in `root`.
    pub(crate) fn execute(
        &self,
        root: &Path,
        running: &Running<'_>,
        call: &Call,
        report: &mut Report,
    ) -> Outcome {
        let (id, input) = (running.id, running.input);
        let result = match call.tool.as_str() {
            "bash" => parse(input).and_then(|i| self.bash(root, running, i, report)),
            "read" => parse(input).and_then(|i| self.read(root, i, report)),
            "write" => parse(input).and_then(|i| self.write(root, id, i, call, report)),
            "edit" => parse(input).and_then(|i| self.edit(root, id, i, call, report)),
            "grep" => parse(input).and_then(|i| self.grep(root, id, i, running.cancel, report)),
            other => Err(format!("the workspace has no tool `{other}`")),
        };
        result.unwrap_or_else(|message| report.failed(&message))
    }

    fn bash(
        &self,
        root: &Path,
        running: &Running<'_>,
        input: Bash,
        report: &mut Report,
    ) -> Result<Outcome, String> {
        let call = running.id;
        let timeout =
            Duration::from_millis(input.timeout_ms.unwrap_or(DEFAULT_TIMEOUT).min(MAX_TIMEOUT));
        let truncator = Truncator::new(pattern(input.grep.as_deref())?);
        // A character split across two reads is held until it is whole.
        let mut tail = Vec::new();
        let progress = |bytes: &[u8]| {
            tail.extend_from_slice(bytes);
            let whole = match std::str::from_utf8(&tail) {
                Err(e) if e.error_len().is_none() => e.valid_up_to(),
                _ => tail.len(),
            };
            let chunk = String::from_utf8_lossy(&tail[..whole]).into_owned();
            tail.drain(..whole);
            self.voice.live(Live::Call { call, chunk });
        };
        // Without this record, an instance killed now would leave the tree
        // running with nobody to end it.
        self.marks
            .running(call)
            .map_err(|e| format!("cannot record the command: {e}"))?;
        let scope = self.scope(call, &running.call.caller, &input.command);
        let ran = shell::run(
            call,
            &input.command,
            root,
            &scope,
            timeout,
            running.limit,
            running.cancel,
            truncator,
            progress,
        )
        .map_err(|e| format!("cannot run the command: {e}"))?;
        // What outlived SIGKILL stays recorded, for the next instance.
        if ran.tree.survivors.is_empty() {
            self.marks.ran(call);
        }
        let ended = shell::describe(&ran, timeout, running.shutting_down());
        let (view, full) = self.shown(call, ran.output, report);
        let output = Output {
            model: report.text(&format!("{view}{ended}")),
            full,
            observes: Vec::new(),
        };
        Ok(match ran.ended {
            Ended::Exited(status) if status.success() => Outcome::Completed(output),
            Ended::Exited(_) | Ended::TimedOut => Outcome::Failed(output),
            Ended::Stopped => Outcome::Interrupted {
                partial: Some(output),
            },
        })
    }

    fn read(&self, root: &Path, input: Read, report: &mut Report) -> Result<Outcome, String> {
        let place = place(root, &input.path);
        let shown = &input.path;
        if place.path.is_dir() {
            let mut names: Vec<String> = fs::read_dir(&place.path)
                .map_err(|e| format!("cannot list {shown}: {e}"))?
                .filter_map(|e| e.ok())
                .map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    match e.file_type().is_ok_and(|t| t.is_dir()) {
                        true => name + "/",
                        false => name,
                    }
                })
                .collect();
            names.sort();
            return Ok(report.completed(
                &budgeted(names.iter().map(String::as_str), 1, false),
                Vec::new(),
            ));
        }
        let bytes = fs::read(&place.path).map_err(|e| format!("cannot read {shown}: {e}"))?;
        let observes = self.observations(&place.name, Version::Is(Digest::of(&bytes)));
        if let Some(media) = image(&bytes) {
            if bytes.len() > IMAGE_BUDGET {
                return Err(format!(
                    "{shown} is a {} KiB image; an image is shown only up to {} KiB. Scale it down first.",
                    bytes.len() / 1024,
                    IMAGE_BUDGET / 1024
                ));
            }
            let blocks = serde_json::json!([{
                "type": "image",
                "source": { "type": "base64", "media_type": media, "data": base64(&bytes) },
            }]);
            return Ok(Outcome::Completed(Output {
                model: report.value(blocks.to_string().into_bytes()),
                full: None,
                observes,
            }));
        }
        if bytes.is_empty() {
            return Ok(report.completed("(empty file)", observes));
        }
        if bytes[..bytes.len().min(8192)].contains(&0) {
            let said = format!("(a binary file of {} bytes, not shown)", bytes.len());
            return Ok(report.completed(&said, observes));
        }
        let content = String::from_utf8_lossy(&bytes);
        let offset = input.offset.unwrap_or(1).max(1);
        let lines = content
            .lines()
            .skip(offset - 1)
            .take(input.limit.unwrap_or(usize::MAX));
        Ok(report.completed(&budgeted(lines, offset, true), observes))
    }

    fn write(
        &self,
        root: &Path,
        id: CallId,
        input: Write,
        call: &Call,
        report: &mut Report,
    ) -> Result<Outcome, String> {
        let place = place(root, &input.path);
        let _files = lock(&self.files);
        let current =
            version(&place.path).map_err(|e| format!("cannot read {}: {e}", input.path))?;
        let target = Version::Is(Digest::of(input.content.as_bytes()));
        if current == target {
            // Writing what is already there changes nothing, so a rerun of a
            // write that landed completes rather than being refused.
            let said = format!("{} already has this content.", input.path);
            return Ok(report.completed(&said, self.observations(&place.name, target)));
        }
        if let Some(refused) = self.refuse(root, id, &place, current, call, None, report) {
            return Ok(refused);
        }
        if let Some(parent) = place.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        replace(&place.path, input.content.as_bytes())
            .map_err(|e| format!("cannot write {}: {e}", input.path))?;
        self.changed(report, &place.name, target);
        let said = format!(
            "Wrote {} ({} lines).",
            input.path,
            input.content.lines().count()
        );
        Ok(report.completed(&said, self.observations(&place.name, target)))
    }

    /// Apply the replacements in order, each to the text the ones before it
    /// left, and write the result once: all of them or nothing. One that
    /// does not match exactly once (or at all, with `all`) fails the edit,
    /// naming it and showing the file, which the caller has then seen.
    fn edit(
        &self,
        root: &Path,
        id: CallId,
        input: Edit,
        call: &Call,
        report: &mut Report,
    ) -> Result<Outcome, String> {
        let place = place(root, &input.path);
        let path = &input.path;
        let first = input.edits.first().map(|r| r.old.as_str());
        let _files = lock(&self.files);
        let current = version(&place.path).map_err(|e| format!("cannot read {path}: {e}"))?;
        if let Some(refused) = self.refuse(root, id, &place, current, call, first, report) {
            return Ok(refused);
        }
        if input.edits.is_empty() {
            return Err("`edits` is empty: give at least one replacement".into());
        }
        let bytes = fs::read(&place.path).map_err(|e| format!("cannot read {path}: {e}"))?;
        let content = String::from_utf8(bytes).map_err(|_| format!("{path} is not UTF-8 text"))?;
        let mut edited = content.clone();
        let mut count = 0;
        for (n, r) in input.edits.iter().enumerate() {
            let found = edited.matches(&r.old).count();
            let wrong = match found {
                _ if r.old.is_empty() => {
                    Some("is empty; use `write` to replace a whole file".into())
                }
                0 => Some("does not occur".into()),
                1 => None,
                _ if r.all => None,
                n => Some(format!(
                    "occurs {n} times; include more context to pick one, or set `all`"
                )),
            };
            if let Some(wrong) = wrong {
                let wrong = match input.edits.len() {
                    1 => format!("`old` {wrong}"),
                    total => format!(
                        "the `old` of replacement {} of {total} {wrong}, in the text the ones before it left",
                        n + 1
                    ),
                };
                let from = start(&content, Some(&r.old)).unwrap_or(1);
                let shown = budgeted(content.lines().skip(from - 1), from, true);
                let said = format!(
                    "Nothing written to {path}: {wrong}. It is as it was, which counts as having read it:\n{shown}"
                );
                return Ok(Outcome::Failed(
                    report.output(&said, self.observations(&place.name, current)),
                ));
            }
            edited = match r.all {
                true => edited.replace(&r.old, &r.new),
                false => edited.replacen(&r.old, &r.new, 1),
            };
            count += found;
        }
        replace(&place.path, edited.as_bytes()).map_err(|e| format!("cannot write {path}: {e}"))?;
        let version = Version::Is(Digest::of(edited.as_bytes()));
        self.changed(report, &place.name, version);
        let said = format!(
            "Replaced {count} occurrence{} in {path}.",
            if count == 1 { "" } else { "s" },
        );
        Ok(report.completed(&said, self.observations(&place.name, version)))
    }

    fn grep(
        &self,
        root: &Path,
        call: CallId,
        input: Grep,
        cancel: &Cancel,
        report: &mut Report,
    ) -> Result<Outcome, String> {
        let regex = Regex::new(&input.pattern).map_err(|e| format!("invalid pattern: {e}"))?;
        let place = place(root, input.path.as_deref().unwrap_or("."));
        let mut truncator = Truncator::matching(regex.clone());
        let mut found = 0;
        let mut pending = vec![place.path];
        while let Some(path) = pending.pop() {
            if cancel.is_requested() {
                return Ok(Outcome::Interrupted { partial: None });
            }
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.is_dir() {
                let Ok(entries) = fs::read_dir(&path) else {
                    continue;
                };
                let mut children: Vec<PathBuf> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_name() != ".git")
                    .map(|e| e.path())
                    .collect();
                // Popped from the end, so reverse order reads in name order.
                children.sort_by(|a, b| b.cmp(a));
                pending.extend(children);
                continue;
            }
            if !meta.is_file() {
                continue;
            }
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };
            if bytes[..bytes.len().min(8192)].contains(&0) {
                continue;
            }
            let name = shown_as(root, &path);
            for (n, line) in String::from_utf8_lossy(&bytes).lines().enumerate() {
                if regex.is_match(line) {
                    found += 1;
                    truncator.line(&format!("{name}:{}:", n + 1), line);
                }
            }
        }
        if found == 0 {
            return Ok(report.completed("No matches.", Vec::new()));
        }
        let (view, full) = self.shown(call, truncator.finish(), report);
        Ok(Outcome::Completed(Output {
            model: report.text(&view),
            full,
            observes: Vec::new(),
        }))
    }

    /// The view of an output, and the whole of it when the view left some
    /// out: kept as a file the agent can `read` or `grep`, which the view
    /// names, and as a value for Max.
    fn shown(
        &self,
        call: CallId,
        output: Truncated,
        report: &mut Report,
    ) -> (String, Option<ValueId>) {
        let Some(whole) = output.whole else {
            return (output.text, None);
        };
        let kept = self.outputs.keep(call, &whole);
        let said = match kept {
            Ok(path) => format!(
                "[cut to fit; the whole output is in {}: `read` or `grep` it]\n",
                path.display()
            ),
            Err(e) => format!("[cut to fit; the whole output could not be kept: {e}]\n"),
        };
        (output.text + &said, Some(report.value(whole)))
    }

    /// The refusal of a write or edit whose caller has not seen `current`,
    /// unless its own command made it. It shows the file as `read` would,
    /// from a little before `near` (an edit's `old`) when the whole would
    /// not fit, so the caller has seen it once the refusal reaches it.
    #[allow(clippy::too_many_arguments)]
    fn refuse(
        &self,
        root: &Path,
        id: CallId,
        place: &Place,
        current: Version,
        call: &Call,
        near: Option<&str>,
        report: &mut Report,
    ) -> Option<Outcome> {
        let key = self.key(crate::FILE, &place.name);
        // Named as the caller's context saw it, which may have been by
        // another path to the same file.
        let expected = call
            .expects
            .iter()
            .find(|o| {
                o.resource.kind == key.kind
                    && o.resource.limb == key.limb
                    && o.resource.name.starts_with('/')
                    && canonical(Path::new(&o.resource.name)) == place.path
            })
            .map_or(Version::Gone, |o| o.version);
        if expected == current || self.made_by(&call.caller, &place.path) {
            return None;
        }
        let shown = match current {
            Version::Gone => Some(report.output(
                "(it does not exist now)",
                self.observations(&place.name, Version::Gone),
            )),
            Version::Is(_) => self.look(root, place, near, report),
        };
        let shown = shown.map(
            |shown| match self.shown_before(id, &call.caller, &shown.observes) {
                true => report.output(
                    "(as shown with the earlier refusal of this response)",
                    shown.observes,
                ),
                false => shown,
            },
        );
        Some(Outcome::Refused {
            current: vec![Observation {
                resource: key,
                version: current,
            }],
            shown,
        })
    }

    /// What `read` returns of a file, from a little before the first line
    /// holding `near` when the file is over the budget.
    fn look(
        &self,
        root: &Path,
        place: &Place,
        near: Option<&str>,
        report: &mut Report,
    ) -> Option<Output> {
        let offset = fs::read(&place.path)
            .ok()
            .and_then(|bytes| start(&String::from_utf8_lossy(&bytes), near));
        let read = Read {
            path: place.path.to_string_lossy().into_owned(),
            offset,
            limit: None,
        };
        match self.read(root, read, report) {
            Ok(Outcome::Completed(output)) => Some(output),
            _ => None,
        }
    }

    pub(crate) fn observations(&self, name: &str, version: Version) -> Vec<Observation> {
        self.keys(name)
            .into_iter()
            .map(|resource| Observation { resource, version })
            .collect()
    }
}

/// `path` as the tools see it, relative to `root` unless absolute.
fn place(root: &Path, path: &str) -> Place {
    let path = canonical(&root.join(path));
    Place {
        name: path.to_string_lossy().into_owned(),
        path,
    }
}

/// How a path is shown in a listing: relative to `root` when inside it.
fn shown_as(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(relative) => relative.to_string_lossy().into_owned(),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

/// A file's version on disk.
pub(crate) fn version(path: &Path) -> io::Result<Version> {
    match fs::read(path) {
        Ok(bytes) => Ok(Version::Is(Digest::of(&bytes))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Version::Gone),
        Err(e) => Err(e),
    }
}

fn parse<T: DeserializeOwned>(input: &[u8]) -> Result<T, String> {
    serde_json::from_slice(input).map_err(|e| format!("invalid input: {e}"))
}

fn pattern(grep: Option<&str>) -> Result<Option<Regex>, String> {
    match grep {
        None | Some("") => Ok(None),
        Some(p) => Regex::new(p)
            .map(Some)
            .map_err(|e| format!("invalid grep pattern: {e}")),
    }
}

/// The line to show `content` from so that `near`, or failing that its
/// first line, is in view: from a little before it, when the whole of
/// `content` would not fit in [`READ_BUDGET`].
fn start(content: &str, near: Option<&str>) -> Option<usize> {
    if content.len() <= READ_BUDGET {
        return None;
    }
    let near = near?;
    let at = content
        .find(near)
        .or_else(|| content.find(near.lines().find(|l| !l.trim().is_empty())?.trim()))?;
    Some(content[..at].lines().count().saturating_sub(LEAD).max(1))
}

/// `lines` from line `first` on, whole, until [`READ_BUDGET`]; then where
/// to read on. A single line longer than the budget is shown up to it, and
/// says how much of it is left out.
fn budgeted<'a>(lines: impl Iterator<Item = &'a str>, first: usize, numbered: bool) -> String {
    let mut text = String::new();
    for (n, line) in (first..).zip(lines) {
        let number = if numbered {
            format!("{n:>6}\t")
        } else {
            String::new()
        };
        if text.len() + number.len() + line.len() + 1 > READ_BUDGET {
            if text.is_empty() {
                let end = (0..=READ_BUDGET - number.len())
                    .rev()
                    .find(|&i| line.is_char_boundary(i))
                    .unwrap_or(0);
                return format!(
                    "{number}{}\n[line {n} goes on for {} more bytes than a read returns ({} KiB)]",
                    &line[..end],
                    line.len() - end,
                    READ_BUDGET / 1024
                );
            }
            let on = if numbered {
                format!("; read on with offset {n}")
            } else {
                String::new()
            };
            return format!(
                "{text}[stopped before line {n}: a read returns at most {} KiB{on}]",
                READ_BUDGET / 1024
            );
        }
        text.push_str(&number);
        text.push_str(line);
        text.push('\n');
    }
    text
}

/// The media type of an image the provider can be shown, by its first bytes.
fn image(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => Some("image/png"),
        [0xFF, 0xD8, 0xFF, ..] => Some("image/jpeg"),
        [b'G', b'I', b'F', b'8', ..] => Some("image/gif"),
        _ if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") => Some("image/webp"),
        _ => None,
    }
}

/// Standard base64, padded.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | (*b as u32) << (16 - 8 * i));
        for i in 0..4 {
            out.push(match i <= chunk.len() {
                true => ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char,
                false => '=',
            });
        }
    }
    out
}
