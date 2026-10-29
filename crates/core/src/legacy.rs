//! Limbs as records from before limbs were per machine (29 Oct 2026)
//! define them, read and never stated.
//!
//! Then, each tool set was a limb of its own: the brain's meta actions, the
//! provider's web tools, Max's queue, the roles reader, Max's config, the
//! browser, the builds, and a workspace limb, with a process of its own,
//! for every directory an agent worked in. A brain's database keeps those
//! records for good, in tasks' places, contexts' tool bindings, calls and
//! resource keys, so a brain reads each as what it is now ([`now`]): a set
//! of the brain, a set of a machine's limb, or a working directory on one.
//! The state resolves a limb id from such a record to the limb that hosts
//! it now (`State::resolve`) before sending anything; a resource key from
//! one stays as it was, so a context from before has never seen any file
//! under its new name, and its first write to each is refused once, showing
//! the file, and goes through on the retry.

use std::path::PathBuf;

use crate::limb::Host;

use serde::{Deserialize, Serialize};

/// A limb's definition as one of those records states it. A new version
/// never changed its kind.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limb {
    pub id: crate::id::LimbId,
    /// Unique among the limbs from before: a workspace was named by its
    /// directory.
    pub name: String,
    pub kind: LimbKind,
}

/// What a limb from before was.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LimbKind {
    /// The brain's lifecycle tools: spawning, handing over, background
    /// calls. Run by the brain once, with its effects.
    Meta,
    /// Max, when he was a limb. It offered no tool.
    Max,
    /// Max's queue, run by the brain like `Meta`.
    Attention,
    /// The model provider: Max's reading of the plan's usage, run off the
    /// brain's thread, and again by a later brain, since it only reads.
    Provider,
    /// A server of its own, by plugin kind: `workspace`, `user`,
    /// `browser`, `builds`, or the `roles` thread of the brain.
    Plugin {
        kind: String,
        config: String,
        #[serde(default)]
        host: Host,
    },
}

impl LimbKind {
    /// Whether the brain ran it itself.
    pub fn is_brains(&self) -> bool {
        !matches!(self, LimbKind::Plugin { .. })
    }

    /// Whether `other` is the same kind on the same host, which every
    /// version of a limb was.
    pub(crate) fn same(&self, other: &LimbKind) -> bool {
        match (self, other) {
            (
                LimbKind::Plugin {
                    kind: a, host: h, ..
                },
                LimbKind::Plugin {
                    kind: b, host: i, ..
                },
            ) => a == b && h == i,
            (a, b) => std::mem::discriminant(a) == std::mem::discriminant(b),
        }
    }
}

/// What a limb from before is now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Now {
    /// A tool set the brain hosts: `meta`, `attention`, `web`, `roles`.
    Brain { set: &'static str },
    /// A tool set of the limb on `host`: `user`, `browser`, `builds`, or a
    /// plugin kind no build here knows, which routes to nothing it offers.
    Set { host: Host, set: String },
    /// A working directory on `host`: the old limb's name, which was its
    /// directory. `config` is its old settings as JSON, which name the
    /// branch it was cloned for, if any: the brain reads that, since the
    /// core reads no JSON.
    Workdir {
        host: Host,
        dir: PathBuf,
        config: String,
    },
    /// Max, who is not a limb: it routes nothing.
    Nothing,
}

/// What `limb` is now.
pub fn now(limb: &Limb) -> Now {
    match &limb.kind {
        LimbKind::Meta => Now::Brain { set: "meta" },
        LimbKind::Attention => Now::Brain { set: "attention" },
        LimbKind::Provider => Now::Brain { set: "web" },
        LimbKind::Max => Now::Nothing,
        LimbKind::Plugin { kind, config, host } => match (kind.as_str(), host.clone()) {
            ("roles", Host::Brain) => Now::Brain { set: "roles" },
            ("workspace", host) => Now::Workdir {
                host,
                dir: crate::limb::normal(std::path::Path::new(&limb.name)),
                config: config.clone(),
            },
            (set, host) => Now::Set {
                host,
                set: set.to_owned(),
            },
        },
    }
}
