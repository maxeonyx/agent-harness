//! The user limb: Max's own agent configuration, as a limb every role holds.
//!
//! It is a workspace over his config directory (`~/.claude`) with one tool
//! instead of the coding tools. Its instructions are his `CLAUDE.md` and the
//! index of his skills; `skill` loads one skill and observes the version it
//! returned, so when Max edits a skill or his `CLAUDE.md`, every context
//! that read it is told at its next turn, like any file it read.

use std::fs;
use std::path::Path;

use harness_core::call::Outcome;
use harness_core::environment::{Audience, Tool};
use harness_core::id::Digest;
use harness_core::resource::Version;
use serde::Deserialize;
use serde_json::json;

use crate::{Inner, Report};

/// The plugin kind of a user limb.
pub const KIND: &str = "user";

/// The file of Max's own instructions, relative to the directory.
pub const INSTRUCTIONS: &str = "CLAUDE.md";

/// The resource name of the skills index among the limb's instructions.
pub const INDEX: &str = "skills";

pub fn tools() -> Vec<Tool> {
    let definition = json!({
        "name": "skill",
        "description": "Load one of Max's skills by name: its full instructions.",
        "input_schema": {
            "type": "object",
            "properties": { "name": { "type": "string", "description": "The skill's name, as the index lists it." } },
            "required": ["name"]
        }
    });
    vec![Tool {
        name: "skill".into(),
        definition: definition.to_string(),
        audience: Audience::Both,
    }]
}

/// The skills index: each skill's name and when to load it, from its
/// `SKILL.md` frontmatter; only those of `only`, if it is given.
pub fn index(root: &Path, only: Option<&[String]>) -> String {
    let mut skills: Vec<(String, String)> = fs::read_dir(root.join("skills"))
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
        .filter(|(name, _)| only.is_none_or(|only| only.contains(name)))
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

#[derive(Deserialize)]
struct Skill {
    name: String,
}

impl Inner {
    /// The skills offered, by name, as the skills file says now; every one
    /// when there is no file.
    pub(crate) fn offered(&self) -> Option<Vec<String>> {
        let text = fs::read_to_string(self.skills.as_ref()?).ok()?;
        let names = text
            .lines()
            .map(|l| l.split('#').next().unwrap_or("").trim())
            .filter(|l| !l.is_empty())
            .map(String::from);
        Some(names.collect())
    }

    pub(crate) fn skill(&self, input: &[u8], report: &mut Report) -> Outcome {
        let loaded = serde_json::from_slice::<Skill>(input)
            .map_err(|e| format!("invalid input: {e}"))
            .and_then(|Skill { name }| {
                let offered = self.offered().is_none_or(|s| s.contains(&name));
                if name.contains('/') || name.starts_with('.') || !offered {
                    return Err(format!("no skill `{name}`; the index lists them all"));
                }
                let file = format!("skills/{name}/SKILL.md");
                let bytes = fs::read(self.root.join(&file))
                    .map_err(|_| format!("no skill `{name}`; the index lists them all"))?;
                let version = Version::Is(Digest::of(&bytes));
                Ok((String::from_utf8_lossy(&bytes).into_owned(), file, version))
            });
        match loaded {
            Ok((text, file, version)) => report.completed(&text, self.observations(&file, version)),
            Err(message) => report.failed(&message),
        }
    }
}
