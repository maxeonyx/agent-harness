//! The workspace as a kind of [`harness_limb`] server: opened on the
//! definition the brain ships, from its settings.

use std::path::Path;
use std::time::Duration;

use harness_core::environment::{Limb, LimbDown, LimbKind};
use harness_core::schema::{Harness, Record};
use harness_limb::Out;
use harness_store::Store;
use harness_substrate::{Log, Restored};

use crate::{Config, Kind, Settings, Workspace};

impl harness_limb::Kind for Workspace {
    fn open(
        definition: &Limb,
        log: Log<Harness, Store>,
        state: &Path,
        out: Out,
    ) -> Result<Workspace, String> {
        let kind = match &definition.kind {
            LimbKind::Plugin { kind, .. } => Kind::of(kind),
            _ => None,
        };
        let kind = kind.ok_or("the definition is not of a workspace or user limb")?;
        let settings = Settings::of(definition).ok_or("the definition's settings do not parse")?;
        let config = Config {
            limb: definition.id,
            kind,
            settings,
            state: state.to_path_buf(),
        };
        Workspace::open(config, log, out).map_err(|e| e.to_string())
    }

    fn apply(&self, down: LimbDown) -> Result<Vec<Record>, Restored> {
        Workspace::apply(self, down)
    }

    fn stop_all(&self, within: Duration) {
        Workspace::stop_all(self, within)
    }

    fn join(&self) {
        Workspace::join(self)
    }
}
