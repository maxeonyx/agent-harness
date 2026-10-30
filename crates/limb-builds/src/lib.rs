//! The builds tool set: relaunches its brain onto the commit of a clone,
//! as a call of the harness's core model, and goes back when the new build
//! fails ([`harness_core::build`] holds the model).
//!
//! **Concept.** Offered by the limb on the brain's machine, its resource is
//! the brain's own service, the systemd user unit its settings name
//! ([`Settings`]). Its `relaunch` tool refuses in band a clone with
//! uncommitted changes, a commit that does not descend from the running one
//! unless the call says `back`, and a second relaunch while one is under
//! way. Otherwise it builds and installs the commit, stops the brain (it
//! drains), starts the new build on the same database, and settles the
//! call: the settlement wakes the caller, whose next request is the new
//! build's first. Its result observes the service, so the limb watching the
//! new build ([`relaunch::Takeover::watch`]) and going back is a change of
//! that resource, saying why, at the caller's next turn.
//!
//! The limb is a process of the build that was running, and outlives the
//! brain it relaunches, so the way back never runs the new build's code.
//! The limb's executable, `harness-limb`, also relaunches a unit by hand
//! (`relaunch`), and installs and starts a build for a brain's first launch
//! (`install`, `start`): what `dev/serve` runs.

pub mod relaunch;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use harness_core::call::Outcome;
use harness_core::fact::{Batch, Fact};
use harness_core::id::Digest;
use harness_core::limb::{self, Audience, Instruction, Offer, Tool};
use harness_core::resource::{Change, Observation, ResourceKey, Version};
use harness_core::value::Bytes;
use harness_limb::{Opening, Report, Running, ToolSet, Voice, lock};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::relaunch::Ended;

/// The tool set's name.
pub const SET: &str = "builds";
/// Its one tool.
pub const RELAUNCH: &str = "relaunch";
/// The kind of its resource: the brain's service.
const SERVICE: &str = "service";
/// The environment variable that tells a brain the `relaunch` call that
/// started it: `Launch::Relaunched`'s `by`. In the unit's environment, not
/// its command line, which the way back starts again as it was.
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
        watching: Mutex::default(),
    }))
}

/// The builds set of one limb.
struct Builds {
    voice: Arc<Voice>,
    settings: Mutex<Settings>,
    /// The relaunches it still watches on probation, after their calls
    /// settled.
    watching: Mutex<Vec<JoinHandle<()>>>,
}

impl ToolSet for Builds {
    fn describe(&self) -> limb::ToolSet {
        let unit = lock(&self.settings).unit.clone();
        let text = format!(
            "## Relaunching the harness\n\n\
`relaunch` moves the brain you run on ({}) onto the commit a clone has checked out: it builds and installs \
it, the brain stops (its requests in flight finish first), and the new build carries on every session, \
yours included, from the same records. Merge your work first: it refuses a clone with uncommitted changes, \
and a commit that does not descend from the one running unless you say `back`. Relaunch only at a moment \
no other agent's work would be lost. A broken build bricks nothing: until it is proven the harness watches it \
from the build before, and goes back on its own.\n",
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

    /// Builds, stops the brain, and starts the new build: the call settles
    /// once the new build has taken over, and the set watches it on
    /// probation after, going back if it fails. Never stopped part way:
    /// stopping the brain mid-way is the one thing it must not leave half
    /// done.
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
        let takeover = match relaunch::take_over(
            &unit,
            &request.clone,
            request.back,
            Some(running.id),
            &say,
        ) {
            Ok(takeover) => takeover,
            Err(why) => return report.failed(&why),
        };
        let (old, new) = (takeover.old.commit.clone(), takeover.new.commit.clone());
        let said = format!(
            "{unit} runs {new} now, which took over from {old}: this session carries on, on it. \
             It is on probation until it has had a response and a call settled by a limb; until then, \
             a refused request, its process ending, or a request unanswered for 5 minutes makes the harness \
             go back to {old}, and you will be told why as a change of `{unit}`."
        );
        let outcome = report.completed(&said, vec![observe(&self.voice, &unit, &new)]);
        let voice = self.voice.clone();
        let watch = thread::spawn(move || match takeover.watch() {
            Ended::Proven => say(&format!("{new} is proven")),
            Ended::Back(why) => {
                say(&why);
                let content = Bytes(why.into_bytes());
                let observed = observe(&voice, &unit, &old);
                voice.say(Batch {
                    facts: vec![Fact::Change(Change {
                        resource: observed.resource,
                        version: observed.version,
                        by: None,
                        content: Some(content.id()),
                        wakes: true,
                    })],
                    values: vec![content],
                });
            }
        });
        let mut watching = lock(&self.watching);
        watching.retain(|w| !w.is_finished());
        watching.push(watch);
        outcome
    }

    fn holds(&self) -> bool {
        lock(&self.watching).iter().any(|w| !w.is_finished())
    }

    fn close(&self) {
        let watching: Vec<JoinHandle<()>> = lock(&self.watching).drain(..).collect();
        for watch in watching {
            let _ = watch.join();
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
}

fn tool() -> Tool {
    let definition = json!({
        "name": RELAUNCH,
        "description": "Relaunch the harness onto the commit a clone has checked out. It returns once the new build \
    has taken over; your next turn is its first request.",
        "input_schema": { "type": "object", "properties": {
            "clone": { "type": "string", "description": "The clone's directory: its HEAD is built." },
            "back": { "type": "boolean", "description": "Allow a commit that does not descend from the running one." },
        }, "required": ["clone"] },
    });
    Tool {
        name: RELAUNCH.into(),
        definition: definition.to_string(),
        audience: Audience::Agent,
    }
}
