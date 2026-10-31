//! The builds tool set: relaunches its brain onto the commit of a clone,
//! as a call of the harness's core model, through a rehearsal and a gated
//! opening, and goes back if the new build dies just after it opens
//! ([`harness_core::build`] holds the model, [`relaunch`] the procedure).
//!
//! **Concept.** Offered by the limb on the brain's machine, its resource is
//! the brain's own service, the systemd user unit its settings name
//! ([`Settings`]). Its `relaunch` tool refuses in band a clone with
//! uncommitted changes, a commit that does not descend from the running one
//! unless the call says `back`, a second relaunch while one is under way,
//! a build that fails its rehearsal, and a one-way relaunch unless the call
//! says `one_way`: each before anything touches the live database, since
//! the call's result arrives only after the relaunch. Otherwise the running
//! brain drains and stops, the new build starts as a candidate, and the
//! call settles once, after a window in which the limb watches the opened
//! build: with the stage it reached, in words, and whether it went back.
//!
//! The limb is a process of the build that was running, and outlives the
//! brain it relaunches, so the judge of the new build, and the way back,
//! never run the new build's code.
//!
//! The limb's executable, `harness-limb`, also relaunches a unit by hand
//! (`relaunch`), and installs and starts a build for a brain's first launch
//! (`install`, `start`): what `dev/serve` runs.

pub mod relaunch;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use harness_core::call::Outcome;
use harness_core::id::Digest;
use harness_core::limb::{self, Audience, Instruction, Offer, Tool};
use harness_core::resource::{Observation, ResourceKey, Version};
use harness_limb::{Opening, Report, Running, ToolSet, Voice, lock};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::relaunch::{Asked, Ended};

/// The tool set's name.
pub const SET: &str = "builds";
/// Its one tool.
pub const RELAUNCH: &str = "relaunch";
/// The kind of its resource: the brain's service.
const SERVICE: &str = "service";
/// The environment variable that tells a brain the `relaunch` call that
/// started it: `Launch::Relaunched`'s `by`. In the unit's environment, not
/// its command line, which a way back starts again as it was.
pub const RELAUNCHED_BY: &str = "HARNESS_RELAUNCHED_BY";

/// The builds set's settings: its offer's `config`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// The brain's systemd user unit, `harness-NAME.service`. `None` for a
    /// brain no unit runs, such as a test brain: it cannot be relaunched.
    pub unit: Option<String>,
}

impl Settings {
    /// The settings for the brain this process is: the unit whose cgroup
    /// it is in, if that is a brain's.
    pub fn here() -> Settings {
        let cgroup = std::fs::read_to_string("/proc/self/cgroup").unwrap_or_default();
        let unit = cgroup
            .lines()
            .filter_map(|l| l.rsplit('/').next())
            .find(|u| u.starts_with("harness-") && u.ends_with(".service"))
            .map(String::from);
        Settings { unit }
    }

    /// The offer of the builds set with these settings.
    pub fn offer(&self) -> Result<Offer, String> {
        harness_limb::offer(SET, self)
    }
}

/// Open the builds set on its offer's settings.
pub fn open(offer: &Offer, opening: &Opening) -> Result<Arc<dyn ToolSet>, String> {
    Ok(Arc::new(Builds {
        voice: opening.voice.clone(),
        settings: Mutex::new(harness_limb::settings(&offer.config)?),
    }))
}

/// The builds set of one limb.
struct Builds {
    voice: Arc<Voice>,
    settings: Mutex<Settings>,
}

impl ToolSet for Builds {
    fn describe(&self) -> limb::ToolSet {
        let unit = lock(&self.settings).unit.clone();
        let text = format!(
            "## Relaunching the harness\n\n\
`relaunch` moves the brain you run on ({}) onto the commit a clone has checked out, and every session, \
yours included, carries on from the same records on the new build. It builds and installs the commit, and \
rehearses it beside the running brain: the new build's gate on a copy of the database, two scratch brains \
of it making a tool round trip through the running build's limb and its own, and the running build reading \
back what they wrote. Then the brain stops (its requests in flight finish first), and the new build opens \
only once its gate has passed on the live database, writing nothing before. For a few minutes after it \
opens, if its process ends, the harness goes back to the build before. The call returns once, after that, \
saying how it went; your next turn may be on either build.\n\n\
Merge your work first: it refuses a clone with uncommitted changes, and a commit that does not descend \
from the one running unless you say `back`. It refuses a one-way relaunch, one the running build could \
not read back, naming what it could not read, unless you say `one_way`: going back after a one-way \
relaunch skips what the older build cannot read and may run a call a second time. Relaunch only at a \
moment no other agent's work would be lost.\n",
            unit.as_deref()
                .unwrap_or("no systemd unit: it cannot be relaunched")
        );
        limb::ToolSet {
            name: SET.into(),
            instructions: vec![Instruction {
                observes: Observation {
                    resource: ResourceKey {
                        limb: self.voice.limb(),
                        kind: "instructions".into(),
                        name: SET.into(),
                    },
                    version: Version::Is(Digest::of(text.as_bytes())),
                },
                text,
            }],
            tools: vec![tool()],
        }
    }

    fn define(&self, config: &str) -> Result<(), String> {
        *lock(&self.settings) = harness_limb::settings(config)?;
        Ok(())
    }

    /// The relaunch, to its end ([`relaunch::relaunch`]). Never stopped
    /// part way: stopping the brain mid-way is the one thing it must not
    /// leave half done.
    fn run(&self, running: &Running<'_>, report: &mut Report) -> Outcome {
        if running.call.tool != RELAUNCH {
            return report.failed(&format!(
                "the builds set has no tool `{}`",
                running.call.tool
            ));
        }
        let request = match serde_json::from_slice::<Request>(running.input) {
            Ok(request) => request,
            Err(e) => return report.failed(&format!("the input: {e}")),
        };
        let Some(unit) = lock(&self.settings).unit.clone() else {
            return report.failed("no systemd unit runs this brain, so it cannot be relaunched");
        };
        let say = {
            let unit = unit.clone();
            move |line: &str| eprintln!("{unit}: {line}")
        };
        let asked = Asked {
            back: request.back,
            one_way: request.one_way,
        };
        let ended = relaunch::relaunch(&unit, &request.clone, asked, Some(running.id), &say);
        let said = format!("{}: {}", ended.stage(), ended.words());
        say(&said);
        match ended {
            Ended::Opened(_) => {
                let commit =
                    relaunch::Running::of(&unit).map_or_else(|_| String::new(), |r| r.commit);
                report.completed(&said, vec![observe(&self.voice, &unit, &commit)])
            }
            Ended::Refused(_) | Ended::Failed(_) | Ended::WentBack(_) => report.failed(&said),
        }
    }
}

/// The service as running `commit`.
fn observe(voice: &Voice, unit: &str, commit: &str) -> Observation {
    Observation {
        resource: ResourceKey {
            limb: voice.limb(),
            kind: SERVICE.into(),
            name: unit.into(),
        },
        version: Version::Is(Digest::of(commit.as_bytes())),
    }
}

/// A `relaunch` call's input.
#[derive(Deserialize)]
struct Request {
    clone: PathBuf,
    #[serde(default)]
    back: bool,
    #[serde(default)]
    one_way: bool,
}

fn tool() -> Tool {
    let definition = json!({
        "name": RELAUNCH,
        "description": "Relaunch the harness onto the commit a clone has checked out, through a rehearsal and a \
    gated opening. It returns a few minutes after the new build opens, or when the relaunch is refused or fails, \
    saying the stage it reached and whether the harness went back.",
        "input_schema": { "type": "object", "properties": {
            "clone": { "type": "string", "description": "The clone's directory: its HEAD is built." },
            "back": { "type": "boolean", "description": "Allow a commit that does not descend from the running one." },
            "one_way": { "type": "boolean", "description": "Allow a one-way relaunch: one whose records the running build \
    cannot all read, so going back after it skips those and may run a call a second time. A relaunch refused as \
    one-way says so; call again with this to go ahead." },
        }, "required": ["clone"] },
    });
    Tool {
        name: RELAUNCH.into(),
        definition: definition.to_string(),
        audience: Audience::Agent,
    }
}
