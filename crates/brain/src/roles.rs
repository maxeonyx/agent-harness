//! The roles limb: the definitions of the kinds of agent, read from role
//! files into [`Environment`] versions, which it states as facts of its
//! own. Co-hosted: it runs on a thread of the brain's process, on a link of
//! values over a channel, so the brain itself reads nothing but its
//! database.
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
//! runs or defines itself (`meta`, `max`, `user`); `places` name the plugin
//! kinds each task is given a limb of its own of (`workspace`: a clone);
//! `starts` names the roles it can start fresh tasks in. Claude Code
//! ignores the keys it does not know.
//!
//! A role file is a resource of this limb: each version of an environment
//! records the file's version it was read from ([`Environment::source`]),
//! and the limb states each new version with a change to the file, so a
//! task whose root embodies the old one is told. The brain's rows for it
//! are the environments it stated ([`LimbRow::Environment`]): it restates
//! a role only when its file says something else.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use harness_core::environment::{Environment, Limb, LimbDown, LimbKey, LimbKind, LimbRow, LimbUp};
use harness_core::fact::{Batch, Fact, Refusal};
use harness_core::id::{Digest, EnvironmentId, LimbId, PlaceId, Time};
use harness_core::resource::{Change, Observation, ResourceKey, Version};
use harness_core::schema::Harness;
use harness_core::value::Bytes;
use harness_substrate::{Log, Memory, Replica, Up};
use serde::{Deserialize, Serialize};

/// The plugin kind it is.
pub const KIND: &str = "roles";

/// How often the role files are read again.
const EVERY: Duration = Duration::from_secs(2);

/// The roles limb's settings: the `config` of its definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub dir: PathBuf,
    /// The shared limbs a role may name, by the name it uses.
    pub limbs: BTreeMap<String, LimbId>,
}

impl Settings {
    pub fn definition(&self, id: LimbId) -> Limb {
        Limb {
            id,
            name: "roles".into(),
            kind: LimbKind::Plugin {
                kind: KIND.into(),
                config: serde_json::to_string(self).expect("settings are JSON"),
            },
        }
    }

    pub fn of(limb: &Limb) -> Option<Settings> {
        match &limb.kind {
            LimbKind::Plugin { kind, config } if kind == KIND => serde_json::from_str(config).ok(),
            _ => None,
        }
    }
}

/// Run the roles limb `limb` as the place `here`, on a link: what the brain
/// ships arrives on `down`, and what it says goes to `up`. Its log is in
/// memory, so each run is a new place: everything it says, it derives
/// again from its files. Returns once the brain shuts down or lets go of
/// the link.
pub fn run(limb: &Limb, here: PlaceId, down: Receiver<LimbDown>, up: impl Fn(LimbUp)) {
    let Some(settings) = Settings::of(limb) else {
        return eprintln!("the roles limb's settings do not parse");
    };
    let mut log: Log<Harness, Memory<Harness>> =
        Log::open(here, Memory::default()).expect("memory opens");
    let mut rows: Replica<LimbKey, LimbRow, Refusal> = Replica::default();
    let mut error: Option<String> = None;
    loop {
        match down.recv_timeout(EVERY) {
            Ok(shipped) => {
                let connected = matches!(shipped, LimbDown::View(_));
                rows.apply(shipped);
                let Some(view) = rows.view() else { continue };
                if connected {
                    for record in log.unsent(&view.standing).unwrap_or_default() {
                        up(Up::Record(record));
                    }
                }
                for (n, why) in &view.standing.refused {
                    eprintln!("the brain refused the roles limb's record {n}: {why}");
                }
                let _ = log.cover(&view.standing);
                if view.rows.contains_key(&LimbKey::Shutdown) {
                    return;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let Some(view) = rows.view() else { continue };
        let current: Vec<&Environment> = view
            .rows
            .values()
            .filter_map(|r| match r {
                LimbRow::Environment(env) => Some(env),
                _ => None,
            })
            .collect();
        let said = read(&settings.dir).and_then(|roles| {
            let batch = environments(&roles, &current, &settings.limbs, limb.id, log.next())?;
            if batch.facts.is_empty() {
                return Ok(());
            }
            let record = log
                .say(crate::brain::now(), |_| batch)
                .map_err(|e| e.to_string())?;
            up(Up::Record(record));
            Ok(())
        });
        let now = said.err();
        if now.is_some() && now != error {
            eprintln!("the roles are unchanged: {}", now.as_deref().unwrap_or(""));
        }
        error = now;
    }
}

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

/// The batch that brings the environments up to the role files: each role
/// that is new or says something else, as a new version, with the change to
/// its file. Ids are kept by name from `current`; a new role's is its
/// fact's event at `at`, the record the batch is said in. `limbs` resolves
/// a shared limb's name; `owner` is this limb, whose resources the files
/// are.
fn environments(
    roles: &[Role],
    current: &[&Environment],
    limbs: &BTreeMap<String, LimbId>,
    owner: LimbId,
    at: Time,
) -> Result<Batch, String> {
    let existing = |name: &str| current.iter().find(|e| e.name == name);
    // New roles first, so each one's fact is at the index its id names.
    let mut order: Vec<&Role> = roles
        .iter()
        .filter(|r| existing(&r.name).is_none())
        .collect();
    let fresh = order.len();
    order.extend(roles.iter().filter(|r| existing(&r.name).is_some()));
    let ids: BTreeMap<&str, EnvironmentId> = order
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let id = existing(&r.name).map_or_else(|| EnvironmentId(at.event(i as u32)), |e| e.id);
            (r.name.as_str(), id)
        })
        .collect();
    let mut batch = Batch::default();
    let mut changes = Vec::new();
    for (i, r) in order.into_iter().enumerate() {
        let role = Bytes::from(r.prompt.as_str());
        let params = Bytes::from(r.params.as_str());
        let resource = ResourceKey {
            limb: owner,
            kind: "role".into(),
            name: r.path.clone(),
        };
        let env = Environment {
            id: ids[r.name.as_str()],
            name: r.name.clone(),
            role: role.id(),
            model: r.model.clone(),
            params: params.id(),
            limbs: r
                .limbs
                .iter()
                .map(|l| {
                    limbs
                        .get(l)
                        .copied()
                        .ok_or(format!("{}: no limb `{l}`", r.name))
                })
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
                resource: resource.clone(),
                version: r.version,
            }),
        };
        let old = existing(&r.name);
        if i >= fresh && old == Some(&&env) {
            continue;
        }
        if old.and_then(|e| e.source.as_ref()).map(|s| s.version) != Some(r.version) {
            changes.push(Fact::Change(Change {
                resource,
                version: r.version,
                by: None,
                content: None,
            }));
        }
        batch.values.extend([role, params]);
        batch.facts.push(Fact::Environment(env));
    }
    batch.facts.extend(changes);
    Ok(batch)
}
