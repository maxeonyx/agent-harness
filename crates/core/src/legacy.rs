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

pub use crate::environment::{Limb, LimbKind};

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
        LimbKind::Plugin { kind, config, host } => {
            let host = match host {
                crate::environment::Host::Brain => Host::Brain,
                crate::environment::Host::Local => Host::Local,
                crate::environment::Host::Ssh { destination } => Host::Ssh {
                    destination: destination.clone(),
                },
            };
            match (kind.as_str(), host) {
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
            }
        }
    }
}
