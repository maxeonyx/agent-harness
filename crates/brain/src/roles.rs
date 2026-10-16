//! Role files: the definitions of the kinds of agent, read into
//! [`Environment`] versions.
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
//! limbs: [meta, user]
//! places: [workspace]
//! starts: []
//! ---
//! The role prompt.
//! ```
//!
//! `limbs` name the limbs every task of the kind shares, which the brain
//! runs or defines itself (`meta`, `user`); `places` name the plugin kinds
//! each task is given a limb of its own of (`workspace`: a clone); `starts` names the roles it can start fresh tasks in. Claude Code
//! ignores the keys it does not know.
//!
//! A role file is a resource of the meta limb, the brain's own: each
//! version of an environment records the file's version it was read from
//! ([`Environment::source`]), and the brain states each new version as a
//! change, so a task whose root embodies the old one is told.

use std::collections::BTreeMap;
use std::path::Path;

use harness_core::environment::Environment;
use harness_core::id::{Digest, EnvironmentId, LimbId};
use harness_core::resource::{Observation, ResourceKey, Version};
use harness_core::value::Bytes;

/// One role file, parsed.
pub struct Role {
    /// The file it was read from, and its version.
    pub path: String,
    pub version: Version,
    pub name: String,
    pub model: String,
    /// The request parameters, as the exact JSON object every turn sends.
    pub params: String,
    pub limbs: Vec<String>,
    pub places: Vec<String>,
    pub starts: Vec<String>,
    pub prompt: String,
}

/// Every `*.md` role file in `dir`.
pub fn read(dir: &Path) -> Result<Vec<Role>, String> {
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
const KEYS: [&str; 7] = [
    "name",
    "description",
    "model",
    "effort",
    "limbs",
    "places",
    "starts",
];
const LISTS: [&str; 3] = ["limbs", "places", "starts"];

/// A role file: frontmatter of one `key: value` per line, then the prompt.
/// The frontmatter is a subset of YAML, and anything outside it (a quoted,
/// folded or multi-line value, an unknown or repeated key) is refused
/// rather than misread.
fn parse(text: &str) -> Result<Role, String> {
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
    let effort = one("effort").unwrap_or_else(|| "medium".into());
    Ok(Role {
        path: String::new(),
        version: Version::Gone,
        name: one("name").ok_or("no `name`")?,
        model: model_id(&model),
        params: serde_json::json!({
            "max_tokens": 64000,
            "thinking": {"type": "adaptive"},
            "output_config": {"effort": effort},
        })
        .to_string(),
        limbs: list("limbs"),
        places: list("places"),
        starts: list("starts"),
        prompt: prompt.trim().to_string(),
    })
}

/// Claude Code's model aliases, as the API names them.
fn model_id(alias: &str) -> String {
    match alias {
        "opus" => "claude-opus-5-5",
        "sonnet" => "claude-sonnet-5",
        other => other,
    }
    .into()
}

/// Each role as an environment version, with the bytes it names: ids reused
/// by name from `current`, so a role's id outlives edits to it. `limb`
/// resolves a shared limb's name; `meta` owns the role files.
pub fn environments(
    roles: &[Role],
    current: &[Environment],
    limb: &dyn Fn(&str) -> Option<LimbId>,
    meta: LimbId,
) -> Result<(Vec<Environment>, Vec<Bytes>), String> {
    let ids: BTreeMap<&str, EnvironmentId> = roles
        .iter()
        .map(|r| {
            let id = current
                .iter()
                .find(|e| e.name == r.name)
                .map_or_else(EnvironmentId::generate, |e| e.id);
            (r.name.as_str(), id)
        })
        .collect();
    let mut values = Vec::new();
    let mut out = Vec::new();
    for r in roles {
        let role = Bytes::from(r.prompt.as_str());
        let params = Bytes::from(r.params.as_str());
        let env = Environment {
            id: ids[r.name.as_str()],
            name: r.name.clone(),
            role: role.id(),
            model: r.model.clone(),
            params: params.id(),
            limbs: r
                .limbs
                .iter()
                .map(|l| limb(l).ok_or(format!("{}: no limb `{l}`", r.name)))
                .collect::<Result<_, _>>()?,
            places: r.places.clone(),
            starts: r
                .starts
                .iter()
                .map(|s| {
                    ids.get(s.as_str())
                        .copied()
                        .ok_or(format!("{}: no role `{s}` to start", r.name))
                })
                .collect::<Result<_, _>>()?,
            source: Some(Observation {
                resource: ResourceKey {
                    limb: meta,
                    kind: "role".into(),
                    name: r.path.clone(),
                },
                version: r.version,
            }),
        };
        values.extend([role, params]);
        out.push(env);
    }
    Ok((out, values))
}
