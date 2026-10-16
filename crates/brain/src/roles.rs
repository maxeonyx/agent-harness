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
//! runs or defines itself (`meta`, `user`); `places` name the limbs each task
//! is given its own of, each a place of the same plugin kind (`workspace`: a
//! clone); `starts` names the roles it can start fresh tasks in. Claude Code
//! ignores the keys it does not know.

use std::collections::BTreeMap;
use std::path::Path;

use harness_core::environment::{Environment, Place};
use harness_core::id::{EnvironmentId, LimbId};
use harness_core::value::Bytes;

/// One role file, parsed.
pub struct Role {
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
            parse(&text).map_err(|e| format!("{p:?}: {e}"))
        })
        .collect()
}

fn parse(text: &str) -> Result<Role, String> {
    let rest = text.strip_prefix("---\n").ok_or("no frontmatter")?;
    let (front, prompt) = rest
        .split_once("\n---\n")
        .ok_or("unterminated frontmatter")?;
    let keys: BTreeMap<&str, &str> = front
        .lines()
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim(), v.trim()))
        .collect();
    let one = |k: &str| keys.get(k).map(|v| v.to_string());
    let list = |k: &str| -> Vec<String> {
        keys.get(k)
            .map(|v| v.trim_start_matches('[').trim_end_matches(']'))
            .into_iter()
            .flat_map(|v| v.split(','))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };
    let model = one("model").ok_or("no `model`")?;
    let effort = one("effort").unwrap_or_else(|| "medium".into());
    Ok(Role {
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
/// resolves a shared limb's name.
pub fn environments(
    roles: &[Role],
    current: &[Environment],
    limb: &dyn Fn(&str) -> Option<LimbId>,
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
            places: r
                .places
                .iter()
                .map(|p| Place {
                    name: p.clone(),
                    kind: p.clone(),
                })
                .collect(),
            starts: r
                .starts
                .iter()
                .map(|s| {
                    ids.get(s.as_str())
                        .copied()
                        .ok_or(format!("{}: no role `{s}` to start", r.name))
                })
                .collect::<Result<_, _>>()?,
            source: None,
        };
        values.extend([role, params]);
        out.push(env);
    }
    Ok((out, values))
}
