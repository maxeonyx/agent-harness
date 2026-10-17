//! Running each tool.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use harness_core::call::{Call, Outcome, Output};
use harness_core::id::{CallId, Digest};
use harness_core::live::Live;
use harness_core::resource::{Observation, Version};
use regex::Regex;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use harness_runtime::{Cancel, Ended as TreeEnded};

use crate::record::replace;
use crate::shell::{self, Ended};
use crate::trunc::{Limits, Truncator};
use crate::{Inner, Report, lock};

const DEFAULT_TIMEOUT: u64 = 120_000;
const MAX_TIMEOUT: u64 = 600_000;

#[derive(Deserialize)]
struct Bash {
    command: String,
    grep: String,
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

/// The largest image `read` shows: the provider's limit on an image, as
/// base64, is 5 MB.
const IMAGE_BUDGET: usize = 3 * 1024 * 1024;

/// A path as the tools see it: where it is, and its resource name (relative
/// to the root when inside it).
struct Place {
    path: PathBuf,
    name: String,
}

impl Inner {
    pub(crate) fn execute(
        &self,
        id: CallId,
        call: &Call,
        input: &[u8],
        cancel: &Cancel,
        report: &mut Report,
    ) -> Outcome {
        let expects = &call.expects;
        if self.kind == crate::Kind::User {
            return match call.tool.as_str() {
                "skill" => self.skill(input, report),
                other => report.failed(&format!("Max's config has no tool `{other}`")),
            };
        }
        let result = match call.tool.as_str() {
            "bash" => parse(input).and_then(|i| self.bash(id, i, cancel, report)),
            "read" => parse(input).and_then(|i| self.read(i, report)),
            "write" => parse(input).and_then(|i| self.write(i, expects, report)),
            "edit" => parse(input).and_then(|i| self.edit(i, expects, report)),
            "grep" => parse(input).and_then(|i| self.grep(i, cancel, report)),
            other => Err(format!("this workspace has no tool `{other}`")),
        };
        result.unwrap_or_else(|message| report.failed(&message))
    }

    fn bash(
        &self,
        call: CallId,
        input: Bash,
        cancel: &Cancel,
        report: &mut Report,
    ) -> Result<Outcome, String> {
        let timeout =
            Duration::from_millis(input.timeout_ms.unwrap_or(DEFAULT_TIMEOUT).min(MAX_TIMEOUT));
        let truncator = Truncator::new(Limits::default(), pattern(Some(&input.grep))?);
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
            self.live(Live::Call { call, chunk });
        };
        // Without this record, an instance killed now would leave the tree
        // running with nobody to end it.
        self.marks
            .running(call)
            .map_err(|e| format!("cannot record the command: {e}"))?;
        let ran = shell::run(
            call,
            &input.command,
            &self.root,
            timeout,
            cancel,
            truncator,
            progress,
        )
        .map_err(|e| format!("cannot run the command: {e}"))?;
        // What outlived SIGKILL stays recorded, for the next instance.
        if !matches!(&ran.tree, TreeEnded::Killed { survivors } if !survivors.is_empty()) {
            self.marks.ran(call);
        }
        let shutdown = self.shutting_down.load(std::sync::atomic::Ordering::SeqCst);
        let output = Output {
            model: report.text(&format!(
                "{}{}",
                ran.output.text,
                shell::describe(&ran, timeout, shutdown)
            )),
            full: ran.full.map(|full| report.value(full)),
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

    fn read(&self, input: Read, report: &mut Report) -> Result<Outcome, String> {
        let place = self.place(&input.path);
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
        input: Write,
        expects: &[Observation],
        report: &mut Report,
    ) -> Result<Outcome, String> {
        let place = self.place(&input.path);
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
        if let Some(refused) = self.refuse(&place, current, expects) {
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

    fn edit(
        &self,
        input: Edit,
        expects: &[Observation],
        report: &mut Report,
    ) -> Result<Outcome, String> {
        let place = self.place(&input.path);
        let _files = lock(&self.files);
        let current =
            version(&place.path).map_err(|e| format!("cannot read {}: {e}", input.path))?;
        if let Some(refused) = self.refuse(&place, current, expects) {
            return Ok(refused);
        }
        let bytes =
            fs::read(&place.path).map_err(|e| format!("cannot read {}: {e}", input.path))?;
        let content =
            String::from_utf8(bytes).map_err(|_| format!("{} is not UTF-8 text", input.path))?;
        if input.old.is_empty() {
            return Err("`old` is empty; use `write` to replace a whole file".into());
        }
        let count = content.matches(&input.old).count();
        let edited = match count {
            0 => return Err(format!("`old` does not occur in {}", input.path)),
            1 => content.replacen(&input.old, &input.new, 1),
            n if !input.all => {
                return Err(format!(
                    "`old` occurs {n} times in {}; include more context to pick one, or set `all`",
                    input.path
                ));
            }
            _ => content.replace(&input.old, &input.new),
        };
        replace(&place.path, edited.as_bytes())
            .map_err(|e| format!("cannot write {}: {e}", input.path))?;
        let version = Version::Is(Digest::of(edited.as_bytes()));
        self.changed(report, &place.name, version);
        let said = format!(
            "Replaced {count} occurrence{} in {}.",
            if count == 1 { "" } else { "s" },
            input.path
        );
        Ok(report.completed(&said, self.observations(&place.name, version)))
    }

    fn grep(&self, input: Grep, cancel: &Cancel, report: &mut Report) -> Result<Outcome, String> {
        let regex = Regex::new(&input.pattern).map_err(|e| format!("invalid pattern: {e}"))?;
        let place = self.place(input.path.as_deref().unwrap_or("."));
        let mut truncator = Truncator::new(Limits::default(), None);
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
            let name = self.name(&path);
            for (n, line) in String::from_utf8_lossy(&bytes).lines().enumerate() {
                if regex.is_match(line) {
                    found += 1;
                    truncator.line(&format!("{name}:{}:{line}", n + 1));
                }
            }
        }
        let said = match found {
            0 => "No matches.".into(),
            _ => truncator.finish().text,
        };
        Ok(report.completed(&said, Vec::new()))
    }

    /// The refusal of a write or edit whose caller has not seen `current`.
    fn refuse(&self, place: &Place, current: Version, expects: &[Observation]) -> Option<Outcome> {
        let key = &self.keys(&place.name)[0];
        let expected = expects
            .iter()
            .find(|o| &o.resource == key)
            .map_or(Version::Gone, |o| o.version);
        (expected != current).then(|| Outcome::Refused {
            current: vec![Observation {
                resource: key.clone(),
                version: current,
            }],
        })
    }

    pub(crate) fn observations(&self, name: &str, version: Version) -> Vec<Observation> {
        self.keys(name)
            .into_iter()
            .map(|resource| Observation { resource, version })
            .collect()
    }

    fn place(&self, path: &str) -> Place {
        let path = normalize(&self.root.join(path));
        Place {
            name: self.name(&path),
            path,
        }
    }

    fn name(&self, path: &Path) -> String {
        match path.strip_prefix(&self.root) {
            Ok(relative) => relative.to_string_lossy().into_owned(),
            Err(_) => path.to_string_lossy().into_owned(),
        }
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

/// Resolve `.` and `..` without touching the disk, since the path may not
/// exist yet.
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
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => Some("image/webp"),
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

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}
