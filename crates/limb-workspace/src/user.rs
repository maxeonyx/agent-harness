//! The `user` tool set: Max's own agent configuration, which every role
//! holds.
//!
//! Its instructions are his `CLAUDE.md` and the index of his skills, from
//! his config directory (`~/.claude`); `skill` loads one skill and observes
//! the version it returned, so when Max edits a skill or his `CLAUDE.md`,
//! every context that read it is told at its next turn.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use harness_core::call::Outcome;
use harness_core::fact::{Batch, Fact};
use harness_core::id::Digest;
use harness_core::limb::{self, Audience, Instruction, LimbKey, LimbRow, Offer, Tool};
use harness_core::resource::{Change, Observation, ResourceKey, Version};
use harness_limb::{Opening, Report, Rows, Running, ToolSet, Voice, lock};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::absolute;

/// The tool set's name.
pub const SET: &str = "user";

/// The file of Max's own instructions, relative to the directory, and the
/// name of the resource it is among the set's instructions.
const INSTRUCTIONS: &str = "CLAUDE.md";

/// The resource name of the skills index among the set's instructions.
const INDEX: &str = "skills";

/// The kind of a skill as a resource, named by the skill's name.
const SKILL: &str = "skill";

/// The `user` set's settings: its offer's `config`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Max's config directory: absolute, or `~/…`.
    pub dir: PathBuf,
    /// A file naming the set of Max's skills it offers, a name a line
    /// (`#` for comments): the index lists only these and `skill` loads
    /// only these. Every skill when absent, or when the file is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            dir: PathBuf::from("~/.claude"),
            skills: None,
        }
    }
}

impl Settings {
    /// The offer of the `user` set with these settings.
    pub fn offer(&self) -> Result<Offer, String> {
        harness_limb::offer(SET, self)
    }

    fn parse(config: &str) -> Result<Settings, String> {
        let settings: Settings = harness_limb::settings(config)?;
        Ok(Settings {
            dir: absolute(&settings.dir).map_err(|e| e.to_string())?,
            skills: settings.skills,
        })
    }
}

/// Open the `user` set on its offer's settings.
pub fn open(offer: &Offer, opening: &Opening) -> Result<Arc<dyn ToolSet>, String> {
    Ok(Arc::new(User {
        voice: opening.voice.clone(),
        settings: Mutex::new(Settings::parse(&offer.config)?),
        watched: Mutex::default(),
    }))
}

struct User {
    voice: Arc<Voice>,
    settings: Mutex<Settings>,
    /// The resources the brain asked to hear about, at the version last
    /// reported.
    watched: Mutex<HashMap<ResourceKey, Version>>,
}

impl ToolSet for User {
    fn describe(&self) -> limb::ToolSet {
        let root = self.root();
        let mut instructions = Vec::new();
        if let Ok(own) = fs::read_to_string(root.join(INSTRUCTIONS)) {
            instructions.push(Instruction {
                observes: Observation {
                    resource: self.key(INSTRUCTIONS),
                    version: Version::Is(Digest::of(own.as_bytes())),
                },
                text: format!("Max's own instructions, for every agent:\n\n{own}"),
            });
        }
        let index = self.index();
        instructions.push(Instruction {
            observes: Observation {
                resource: self.key(INDEX),
                version: Version::Is(Digest::of(index.as_bytes())),
            },
            text: index,
        });
        limb::ToolSet {
            name: SET.into(),
            instructions,
            tools: vec![tool()],
        }
    }

    fn define(&self, config: &str) -> Result<(), String> {
        *lock(&self.settings) = Settings::parse(config)?;
        Ok(())
    }

    fn apply(&self, rows: &Rows, _connected: bool) {
        if let Some(LimbRow::Watch { known }) = rows.rows.get(&LimbKey::Watch) {
            self.watch(known);
        }
    }

    fn rerun(&self, _tool: &str) -> bool {
        true
    }

    fn run(&self, running: &Running<'_>, report: &mut Report) -> Outcome {
        let outcome = match running.call.tool.as_str() {
            "skill" => self.skill(running.input, report),
            other => report.failed(&format!("Max's config has no tool `{other}`")),
        };
        self.rescan(report);
        outcome
    }
}

impl User {
    fn root(&self) -> PathBuf {
        lock(&self.settings).dir.clone()
    }

    fn key(&self, name: &str) -> ResourceKey {
        ResourceKey {
            limb: self.voice.limb(),
            kind: "instructions".into(),
            name: name.into(),
        }
    }

    /// A resource of this set as it is now; `None` for one it does not
    /// keep.
    fn version_of(&self, resource: &ResourceKey) -> Option<Version> {
        if resource.limb != self.voice.limb() {
            return None;
        }
        let text = |path: &Path| match fs::read(path) {
            Ok(bytes) => Version::Is(Digest::of(&bytes)),
            Err(_) => Version::Gone,
        };
        match (resource.kind.as_str(), resource.name.as_str()) {
            ("instructions", INSTRUCTIONS) => Some(text(&self.root().join(INSTRUCTIONS))),
            ("instructions", INDEX) => Some(Version::Is(Digest::of(self.index().as_bytes()))),
            (SKILL, name) => Some(text(&self.skill_file(name))),
            _ => None,
        }
    }

    /// Report each resource of this set the brain watches whose version is
    /// none it knows, and from then on each change.
    fn watch(&self, known: &[Observation]) {
        let mut changes = Vec::new();
        let mut watched = lock(&self.watched);
        watched.retain(|resource, _| known.iter().any(|o| o.resource == *resource));
        for o in known {
            if watched.contains_key(&o.resource) {
                continue;
            }
            let Some(version) = self.version_of(&o.resource) else {
                continue;
            };
            watched.insert(o.resource.clone(), version);
            if known
                .iter()
                .all(|k| k.resource != o.resource || k.version != version)
            {
                changes.push(change(o.resource.clone(), version));
            }
        }
        drop(watched);
        if !changes.is_empty() {
            self.voice.say(Batch {
                values: Vec::new(),
                facts: changes,
            });
        }
    }

    /// After a call: add every watched resource that moved.
    fn rescan(&self, report: &mut Report) {
        let mut watched = lock(&self.watched);
        for (resource, known) in watched.iter_mut() {
            if let Some(version) = self.version_of(resource)
                && version != *known
            {
                *known = version;
                report.changed(resource.clone(), version, false);
            }
        }
    }

    /// The skills offered, by name, as the skills file says now; every one
    /// when there is no file.
    fn offered(&self) -> Option<Vec<String>> {
        let skills = lock(&self.settings).skills.clone()?;
        let text = fs::read_to_string(skills).ok()?;
        let names = text
            .lines()
            .map(|l| l.split('#').next().unwrap_or("").trim())
            .filter(|l| !l.is_empty())
            .map(String::from);
        Some(names.collect())
    }

    fn skill_file(&self, name: &str) -> PathBuf {
        self.root().join("skills").join(name).join("SKILL.md")
    }

    /// The skills index: each skill's name and when to load it, from its
    /// `SKILL.md` frontmatter; only those offered.
    fn index(&self) -> String {
        let only = self.offered();
        let mut skills: Vec<(String, String)> = fs::read_dir(self.root().join("skills"))
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let text = fs::read_to_string(e.path().join("SKILL.md")).ok()?;
                let front = text.strip_prefix("---\n")?.split_once("\n---")?.0;
                let field = |k: &str| {
                    front
                        .lines()
                        .find_map(|l| l.strip_prefix(k)?.strip_prefix(':'))
                        .map(|v| v.trim().trim_matches('"').to_string())
                };
                Some((field("name")?, field("description").unwrap_or_default()))
            })
            .filter(|(name, _)| only.as_ref().is_none_or(|only| only.contains(name)))
            .collect();
        skills.sort();
        let lines: Vec<String> = skills
            .iter()
            .map(|(name, when)| format!("- {name}: {when}"))
            .collect();
        format!(
            "Max's skills: load one with `skill` when its description says to, before doing what it covers.\n\n{}",
            lines.join("\n")
        )
    }

    fn skill(&self, input: &[u8], report: &mut Report) -> Outcome {
        #[derive(Deserialize)]
        struct Skill {
            name: String,
        }
        let loaded = serde_json::from_slice::<Skill>(input)
            .map_err(|e| format!("invalid input: {e}"))
            .and_then(|Skill { name }| {
                let offered = self.offered().is_none_or(|s| s.contains(&name));
                let missing = || format!("no skill `{name}`; the index lists them all");
                if name.contains('/') || name.starts_with('.') || !offered {
                    return Err(missing());
                }
                let bytes = fs::read(self.skill_file(&name)).map_err(|_| missing())?;
                let version = Version::Is(Digest::of(&bytes));
                Ok((String::from_utf8_lossy(&bytes).into_owned(), name, version))
            });
        match loaded {
            Ok((text, name, version)) => {
                let resource = ResourceKey {
                    limb: self.voice.limb(),
                    kind: SKILL.into(),
                    name,
                };
                report.completed(&text, vec![Observation { resource, version }])
            }
            Err(message) => report.failed(&message),
        }
    }
}

fn change(resource: ResourceKey, version: Version) -> Fact {
    Fact::Change(Change {
        resource,
        version,
        by: None,
        content: None,
        wakes: false,
    })
}

fn tool() -> Tool {
    let definition = json!({
        "name": "skill",
        "description": "Load one of Max's skills by name: its full instructions.",
        "input_schema": {
            "type": "object",
            "properties": { "name": { "type": "string", "description": "The skill's name, as the index lists it." } },
            "required": ["name"]
        }
    });
    Tool {
        name: "skill".into(),
        definition: definition.to_string(),
        audience: Audience::Both,
    }
}
