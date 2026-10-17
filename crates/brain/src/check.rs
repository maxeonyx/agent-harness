//! `harness-brain --check DB`: whether this build can take over the brain
//! whose database `DB` is a copy of, shown without changing anything that
//! brain owns.
//!
//! It folds every record (a record this build folds differently from the
//! build that included it is a failure), renders each live session's next
//! request as [`next`] would for the live brain, and has the provider check
//! each by counting its tokens, which generates nothing
//! ([`Client::count`]); then it streams one response a few tokens long
//! ([`Client::ping`]). It starts no limb and serves no face. A session still
//! waiting on a call has the call answered as done elsewhere, so what is
//! checked is the shape of everything the build renders around it.
//!
//! The [scripted](harness_provider::SCRIPTED) model is counted as the newest
//! Sonnet, which takes the same request parameters: a test brain's sessions
//! are checked as a real model would read them.
//!
//! It prints its report as one JSON object and fails if anything did.

use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use harness_core::id::ValueId;
use harness_core::model_call::Ending;
use harness_core::schema;
use harness_core::task::Task;
use harness_provider::{Client, Counted, SCRIPTED};
use harness_runtime::Cancel;
use harness_store::Store;
use serde_json::{Value, json};

use crate::brain::{Next, next};

/// Check the database at `db` with `client`; the report, and whether
/// everything in it passed.
pub fn check(db: &Path, client: &Client, source: &str) -> Result<(Value, bool), String> {
    let store = Store::open(db).map_err(|e| format!("the database opens: {e}"))?;
    let (decider, divergences) = schema::Brain::open(store.replica(), store)
        .map_err(|e| format!("the database folds: {e}"))?;
    let mut ok = divergences.is_empty();
    let skipped: Vec<Value> = divergences
        .iter()
        .map(|d| json!({ "at": d.at.to_string(), "why": format!("{:?}", d.why) }))
        .collect();

    let cancel = Cancel::new();
    let offered = client.models(&cancel)?;
    let newest = |family: &str| {
        harness_provider::resolve(family, offered.iter().map(|m| m.id.as_str()))
            .map(str::to_owned)
            .ok_or(format!("the provider offers no {family} model"))
    };
    let stand_in = newest("sonnet")?;

    let state = decider.state();
    let stored = |id: ValueId| {
        decider
            .storage()
            .value(id)
            .unwrap_or_else(|e| panic!("the store reads: {e}"))
            .unwrap_or_else(|| panic!("the value {id} is stored"))
    };
    let sessions: Vec<Value> = state
        .tasks()
        .filter(|t| t.live().is_some() && t.head().is_some())
        .map(|task| {
            let checked = session(state, &stored, task, client, &stand_in, &cancel);
            ok &= checked.get("tokens").is_some();
            checked
        })
        .collect();

    let ping = match client.ping(&newest("haiku")?, &cancel) {
        Ok(finished) => match finished.ending {
            Ending::Responded { stop, .. } => json!({ "stop": format!("{stop:?}") }),
            ending => json!({ "failed": format!("{ending:?}") }),
        },
        Err(e) => json!({ "failed": e.to_string() }),
    };
    ok &= ping.get("stop").is_some();

    let report = json!({
        "build": source,
        "ok": ok,
        "skipped": skipped,
        "sessions": sessions,
        "ping": ping,
    });
    Ok((report, ok))
}

/// One session's next request, rendered and counted: `tokens` if the
/// provider accepts it, else `refused` in its words or `failed`.
fn session(
    state: &harness_core::state::State,
    stored: &dyn Fn(ValueId) -> Vec<u8>,
    task: &Task,
    client: &Client,
    stand_in: &str,
    cancel: &Cancel,
) -> Value {
    let mut said = json!({ "task": task.id.to_string() });
    let rendered = panic::catch_unwind(AssertUnwindSafe(|| {
        let Next {
            mut call,
            values,
            model,
        } = next(state, stored, task, true);
        said["model"] = model.clone().into();
        if model == SCRIPTED {
            call.model = Some(stand_in.to_owned());
            said["counted as"] = stand_in.into();
        }
        let value = |id: ValueId| {
            (values.iter().find(|v| v.id() == id))
                .map(|v| v.0.clone())
                .or_else(|| Some(stored(id)))
        };
        client.count(&call, &state.context(call.base), value, cancel)
    }));
    match rendered {
        Ok(Ok(Counted::Tokens(tokens))) => said["tokens"] = tokens.into(),
        Ok(Ok(Counted::Refused { status, message })) => {
            said["refused"] = json!({ "status": status, "message": message });
        }
        Ok(Err(e)) => said["failed"] = e.to_string().into(),
        Err(panicked) => {
            let why = (panicked.downcast_ref::<String>().cloned())
                .or_else(|| panicked.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            said["failed"] = format!("rendering panicked: {why}").into();
        }
    }
    said
}
