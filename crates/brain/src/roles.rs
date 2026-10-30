//! The `roles` tool set the brain offers: the definitions of the kinds of
//! agent, read from role files into [`Role`] versions, which the brain
//! states. The files are read on a thread of the brain's process
//! ([`watch`]), and the brain turns what it read into facts ([`batch`]).
//!
//! A role file is Claude Code's agent file with the harness's keys added, so
//! one file defines a role for both while the work moves between them:
//!
//! ```text
//! ---
//! name: engineer
//! description: Builds one task …
//! model: opus
//! effort: medium
//! tools: [meta, web, user, workspace]
//! starts: []
//! ---
//! The role prompt.
//! ```
//!
//! `tools` names the tool sets its agents hold, in the order their tools
//! are declared: the brain's own (`meta`, `web`) and those of the limb on a
//! machine (`user`, `workspace`, `browser`, `builds`). A role holding
//! `workspace` gives each agent a working directory. `starts` names the
//! roles it can start fresh tasks in. Claude Code ignores the keys it does
//! not know.
//!
//! A role file is a resource of the brain's limb: each version of a role
//! records the file's version it was read from ([`Role::source`]), and the
//! brain states each new version with a change to the file, so a task whose
//! root embodies the old one is told. A role is restated only when its file
//! says something else.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use harness_core::fact::{Batch, Fact};
use harness_core::id::{Digest, LimbId, RoleId, Time};
use harness_core::limb::Offer;
use harness_core::resource::{Change, Observation, ResourceKey, Version};
use harness_core::role::{Effort, Role, UnknownEffort};
use harness_core::value::Bytes;
use harness_provider::Model;
use serde::{Deserialize, Serialize};

/// The tool set's name, as the brain's limb offers it.
pub const SET: &str = "roles";

/// How often the role files are read again.
const EVERY: Duration = Duration::from_secs(2);

/// The `roles` offer's settings: where the role files are.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub dir: PathBuf,
}

impl Settings {
    pub fn offer(&self) -> Offer {
        Offer {
            set: SET.into(),
            config: serde_json::to_string(self).unwrap_or_default(),
        }
    }

    /// The settings of `offer`, if they parse.
    pub fn of(offer: &Offer) -> Option<Settings> {
        serde_json::from_str(&offer.config).ok()
    }
}

/// What a role's `model` resolves against, for one run of the brain.
#[derive(Clone, Debug, Default)]
pub struct Models {
    /// The models on offer, newest first ([`harness_provider::Client::models`]).
    pub offered: Vec<Model>,
    /// The model every role runs on, over its file's: `--model scripted`
    /// for a brain that costs nothing.
    pub every: Option<String>,
    /// The effort every role runs at, over its file's: `--model sonnet/low`.
    pub effort: Option<Effort>,
}

impl Models {
    /// The provider's name for the model a role naming `named` runs on:
    /// [`resolved`](harness_provider::resolve) against the models on offer,
    /// then against the one its role had (`before`), so an alias
    /// keeps its model while the provider cannot be asked; else as named,
    /// for the provider to refuse.
    fn resolve(&self, named: &str, before: Option<&str>) -> String {
        let named = self.every.as_deref().unwrap_or(named);
        let ids = self.offered.iter().map(|m| m.id.as_str()).chain(before);
        harness_provider::resolve(named, ids)
            .unwrap_or(named)
            .to_owned()
    }
}

/// Read the role files in `dir` every few seconds, handing `read` each
/// reading that differs from the last: the files, or why they cannot be
/// read. Returns once `read` says no one is listening.
pub fn watch(dir: &Path, read: impl Fn(Result<Vec<File>, String>) -> bool) {
    let mut last: Option<Result<Vec<(String, Version)>, String>> = None;
    loop {
        let now = self::read(dir);
        let seen = (now.as_ref())
            .map(|files| (files.iter().map(|f| (f.path.clone(), f.version))).collect())
            .map_err(String::clone);
        if last.as_ref() != Some(&seen) {
            last = Some(seen);
            if !read(now) {
                return;
            }
        }
        std::thread::sleep(EVERY);
    }
}

/// One role file, parsed.
pub struct File {
    /// The file it was read from, and its version.
    pub path: String,
    pub version: Version,
    pub name: String,
    pub description: String,
    /// As the file names it: an alias, an id, or `scripted`.
    pub model: String,
    /// How hard it thinks: its request parameters' `output_config.effort`.
    pub effort: Effort,
    /// The tool sets it holds, by name.
    pub tools: Vec<String>,
    pub starts: Vec<String>,
    pub prompt: String,
}

/// Every `*.md` role file in `dir`.
pub fn read(dir: &Path) -> Result<Vec<File>, String> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("the roles {dir:?}: {e}"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|p| {
            let text = std::fs::read_to_string(p).map_err(|e| format!("{p:?}: {e}"))?;
            let mut role = parse(&text).map_err(|e| format!("{p:?}: {e}"))?;
            role.path = p.display().to_string();
            role.version = Version::Is(Digest::of(text.as_bytes()));
            Ok(role)
        })
        .collect()
}

/// The keys a role's frontmatter may have: a plain value each, except the
/// lists, which are written `[a, b]`.
const KEYS: [&str; 6] = ["name", "description", "model", "effort", "tools", "starts"];
const LISTS: [&str; 2] = ["tools", "starts"];

/// A role file: frontmatter of one `key: value` per line, then the prompt.
/// The frontmatter is a subset of YAML, and anything outside it (a quoted,
/// folded or multi-line value, an unknown or repeated key) is refused
/// rather than misread.
fn parse(text: &str) -> Result<File, String> {
    let rest = text.strip_prefix("---\n").ok_or("no frontmatter")?;
    let (front, prompt) = rest
        .split_once("\n---\n")
        .ok_or("unterminated frontmatter")?;
    let mut keys: BTreeMap<&str, &str> = BTreeMap::new();
    for line in front.lines().filter(|l| !l.trim().is_empty()) {
        let outside = || format!("`{line}` is outside the frontmatter this harness reads");
        let (key, value) = line.split_once(':').ok_or_else(outside)?;
        let value = value.trim();
        let list = LISTS.contains(&key);
        let plain = !value.starts_with(['"', '\'', '|', '>', '[', '{', '&', '*', '!', '#']);
        let ok = KEYS.contains(&key)
            && if list {
                value.starts_with('[') && value.ends_with(']')
            } else {
                plain
            };
        if !ok || keys.insert(key, value).is_some() {
            return Err(outside());
        }
    }
    let one = |k: &str| keys.get(k).map(|v| v.to_string());
    let list = |k: &str| -> Vec<String> {
        keys.get(k)
            .map(|v| &v[1..v.len() - 1])
            .into_iter()
            .flat_map(|v| v.split(','))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };
    let model = one("model").ok_or("no `model`")?;
    let effort: Effort = match one("effort") {
        Some(named) => named.parse().map_err(|e: UnknownEffort| e.to_string())?,
        None => Effort::Medium,
    };
    Ok(File {
        path: String::new(),
        version: Version::Gone,
        name: one("name").ok_or("no `name`")?,
        description: one("description").unwrap_or_default(),
        model,
        effort,
        tools: list("tools"),
        starts: list("starts"),
        prompt: prompt.trim().to_string(),
    })
}

/// The batch that brings the roles up to the role files: each role that
/// is new or says something else, as a new version, with the change to its
/// file. Ids are kept by name from `current`; a new role's is its fact's
/// event at `at`, the record the batch is said in. `owner` is the brain's
/// limb, whose resources the files are.
pub fn batch(
    files: &[File],
    current: &[&Role],
    models: &Models,
    owner: LimbId,
    at: Time,
) -> Result<Batch, String> {
    let existing = |name: &str| current.iter().find(|r| r.name == name).copied();
    // New roles first, so each one's fact is at the index its id names.
    let mut order: Vec<&File> = files
        .iter()
        .filter(|f| existing(&f.name).is_none())
        .collect();
    let fresh = order.len();
    order.extend(files.iter().filter(|f| existing(&f.name).is_some()));
    let ids: BTreeMap<&str, RoleId> = order
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let id = existing(&f.name).map_or_else(|| RoleId(at.event(i as u32)), |r| r.id);
            (f.name.as_str(), id)
        })
        .collect();
    let mut batch = Batch::default();
    let mut changes = Vec::new();
    for (i, f) in order.into_iter().enumerate() {
        let prompt = Bytes::from(f.prompt.as_str());
        let effort = models.effort.unwrap_or(f.effort);
        let params = serde_json::json!({
            "max_tokens": 64000,
            "thinking": {"type": "adaptive"},
            "output_config": {"effort": effort},
        });
        let params = Bytes::from(params.to_string());
        let source = Observation {
            resource: ResourceKey {
                limb: owner,
                kind: "role".into(),
                name: f.path.clone(),
            },
            version: f.version,
        };
        let old = existing(&f.name);
        let role = Role {
            id: ids[f.name.as_str()],
            name: f.name.clone(),
            description: f.description.clone(),
            prompt: prompt.id(),
            model: models.resolve(&f.model, old.map(|r| r.model.as_str())),
            params: params.id(),
            tools: f.tools.clone(),
            starts: f
                .starts
                .iter()
                .map(|s| {
                    ids.get(s.as_str())
                        .copied()
                        .ok_or(format!("{}: no role `{s}` to start", f.name))
                })
                .collect::<Result<_, _>>()?,
            source: Some(source.clone()),
        };
        if i >= fresh && old == Some(&role) {
            continue;
        }
        if old.and_then(|r| r.source.as_ref()) != Some(&source) {
            changes.push(Fact::Change(Change {
                resource: source.resource,
                version: f.version,
                by: None,
                content: None,
                wakes: false,
            }));
        }
        batch.values.extend([prompt, params]);
        batch.facts.push(Fact::Role(role));
    }
    batch.facts.extend(changes);
    Ok(batch)
}
