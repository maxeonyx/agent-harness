//! The gate: whether this build works on a database, shown without writing
//! to it or acting on anything ([`Pathway::GATE`], in order).
//!
//! - **Fold**: every record folds, and none is skipped.
//! - **Decide**: what its launch would say first ([`Opening`]) is computed
//!   and accepted by the state, and on the state it would leave, the to-do
//!   list and the face's view are derived, none of them panicking.
//! - **Serve**: the face's certificate loads, its port is bound, and the
//!   face serves what it folded, read-only, while the rest is checked.
//! - **Requests**: each live session's next request is rendered as [`next`]
//!   would for the live brain, and the provider counts its tokens, which
//!   generates nothing ([`Client::count`]). A session still waiting on a
//!   call has the call answered as done elsewhere, so what is checked is the
//!   shape of everything the build renders around it.
//! - **Response**: one short response is streamed ([`Client::ping`]), made
//!   to use one tool, and that tool use is parsed as a session's would be.
//!
//! A refusal the build is to blame for ([`blames_build`]) fails at once;
//! provider trouble that is not the build's is tried again for a minute.
//! Signed out of claude.ai, it asks the provider nothing: each session is
//! only rendered, and the response waits. The
//! [scripted](harness_provider::SCRIPTED) model is counted as the newest
//! Sonnet, which takes the same request parameters, so a test brain's
//! sessions are checked as a real model would read them.
//!
//! The rehearsal runs it on a copy of the live database
//! (`harness-brain --check`); a [`Start::Candidate`] runs it on the live
//! database, and launches on what it folded and bound once it passes.
//!
//! [`Start::Candidate`]: harness_core::build::Start::Candidate

use std::cell::RefCell;
use std::fmt;
use std::net::TcpListener;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use harness_core::build::{Pathway, blames_build};
use harness_core::id::ValueId;
use harness_core::model_call::{Ending, Stop};
use harness_core::schema;
use harness_core::state::State;
use harness_core::task::Task;
use harness_provider::{Client, Counted, Error, PING, SCRIPTED, tool_uses};
use harness_runtime::Cancel;
use harness_store::Store;

use crate::brain::{Next, Opening, next, now};

/// How long provider trouble that is not the build's is tried again for.
const RETRY: Duration = Duration::from_secs(60);
/// How long it waits between tries.
const PAUSE: Duration = Duration::from_secs(5);

/// What a passed gate leaves to launch on: the fold and what it says first.
pub struct Passed {
    pub decider: schema::Brain<Store>,
    pub opening: Opening,
}

/// A pathway that failed, and why.
pub struct Failure {
    pub pathway: Pathway,
    pub why: String,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "gate: {}: {}", self.pathway, self.why)
    }
}

/// Run the gate on the database at `db`, reading the role files in `roles`
/// and binding `port` (0 for any), with `client`. Once it has decided, the
/// fold, the port and its certificate go to `face`, which serves them
/// read-only. Each pathway that passes is handed to `passed` with how long
/// it took; every one that fails is returned. A pathway that cannot run
/// without one before it is not run.
pub fn gate(
    db: &Path,
    client: &Client,
    roles: Option<&Path>,
    port: u16,
    mut passed: impl FnMut(Pathway, Duration),
    face: impl FnOnce(&schema::Brain<Store>, TcpListener, Arc<rustls::ServerConfig>),
) -> Result<Passed, Vec<Failure>> {
    let mut failures = Vec::new();
    let mut check = |pathway: Pathway, run: Result<(), Vec<String>>, started: Instant| match run {
        Ok(()) => passed(pathway, started.elapsed()),
        Err(whys) => failures.extend(whys.into_iter().map(|why| Failure { pathway, why })),
    };

    let started = Instant::now();
    let (decider, skipped) = match fold(db) {
        Ok(folded) => folded,
        Err(whys) => {
            check(Pathway::Fold, Err(whys), started);
            return Err(failures);
        }
    };
    check(Pathway::Fold, skipped, started);

    let started = Instant::now();
    let opening = decide(&decider, roles);
    let decided = opening
        .as_ref()
        .map(|_| ())
        .map_err(|why| vec![why.clone()]);
    check(Pathway::Decide, decided, started);

    // The face's view is derived as Decide derived it: if that panicked,
    // so would the face.
    if opening.is_ok() {
        let started = Instant::now();
        let served = serve(port).map(|(listener, tls)| face(&decider, listener, tls));
        check(Pathway::Serve, served.map_err(|why| vec![why]), started);
    }

    let started = Instant::now();
    check(Pathway::Requests, requests(&decider, client), started);

    let started = Instant::now();
    check(
        Pathway::Response,
        response(client).map_err(|why| vec![why]),
        started,
    );

    match opening {
        Ok(opening) if failures.is_empty() => Ok(Passed { decider, opening }),
        _ => Err(failures),
    }
}

/// The fold of the database at `db`, opened without writing, and whether it
/// skipped any record: of those, the first, in fold order, since the rest
/// may follow from it.
#[allow(clippy::type_complexity)]
fn fold(db: &Path) -> Result<(schema::Brain<Store>, Result<(), Vec<String>>), Vec<String>> {
    let store = Store::read(db).map_err(|e| vec![format!("the database opens: {e}")])?;
    let (decider, divergences) = schema::Brain::open(store.replica(), store)
        .map_err(|e| vec![format!("the database reads: {e}")])?;
    let skipped = match divergences.as_slice() {
        [] => Ok(()),
        [first, rest @ ..] => {
            let why = format!("it skips the record at {}: {}", first.at, first.why);
            Err(vec![match rest.len() {
                0 => why,
                more => format!("{why}, and {more} after it"),
            }])
        }
    };
    Ok((decider, skipped))
}

/// What a launch would say first, said only to a copy of the state, and on
/// that state the to-do list and the face's view.
fn decide(decider: &schema::Brain<Store>, roles: Option<&Path>) -> Result<Opening, String> {
    caught(|| {
        let at = decider.next();
        let opening = Opening::of(decider.state(), at, roles)?;
        let state = (opening.said(decider.state(), at))
            .map_err(|why| format!("the state refuses its launch: {why:?}"))?;
        let now = now();
        state.todo(now);
        let value = |id| decider.storage().value(id).ok().flatten();
        harness_face_web::rows(&state, &value, &[], crate::SOURCE, now);
        Ok(opening)
    })
    .and_then(|decided| decided)
}

/// Each live session's next request, rendered and, signed in, counted.
fn requests(decider: &schema::Brain<Store>, client: &Client) -> Result<(), Vec<String>> {
    let signed_in = (client.login().signed_in()).map_err(|e| vec![format!("the login: {e}")])?;
    let cancel = Cancel::new();
    let stand_in = match signed_in {
        true => Some(
            retried(|| match client.models(&cancel) {
                Ok(offered) => Tried::Done(newest("sonnet", &offered)),
                Err(e) => Tried::Trouble(format!("the provider's models: {e}")),
            })
            .and_then(|newest| newest)
            .map_err(|why| vec![why])?,
        ),
        false => None,
    };
    let state = decider.state();
    let stored = |id: ValueId| {
        (decider.storage().value(id))
            .unwrap_or_else(|e| panic!("the store reads: {e}"))
            .unwrap_or_else(|| panic!("the value {id} is stored"))
    };
    let provider = stand_in.as_deref().map(|s| (client, s, &cancel));
    let failed: Vec<String> = state
        .tasks()
        .filter(|t| t.live().is_some() && t.head().is_some())
        .filter_map(|task| {
            let why = session(state, &stored, task, provider).err()?;
            Some(format!("the session {}: {why}", task.id))
        })
        .collect();
    match failed.is_empty() {
        true => Ok(()),
        false => Err(failed),
    }
}

/// One session's next request, rendered and, given a provider and the
/// model that stands in for the scripted one, accepted by its token count.
fn session(
    state: &State,
    stored: &dyn Fn(ValueId) -> Vec<u8>,
    task: &Task,
    provider: Option<(&Client, &str, &Cancel)>,
) -> Result<(), String> {
    let Next {
        mut call,
        values,
        model,
    } = caught(|| next(state, stored, task, true)).map_err(|why| format!("rendering {why}"))?;
    let Some((client, stand_in, cancel)) = provider else {
        return Ok(());
    };
    if model == SCRIPTED {
        call.model = Some(stand_in.to_owned());
    }
    let value = |id: ValueId| {
        (values.iter().find(|v| v.id() == id))
            .map(|v| v.0.clone())
            .or_else(|| caught(|| stored(id)).ok())
    };
    retried(
        || match client.count(&call, &state.context(call.base), value, cancel) {
            Ok(Counted::Tokens(_)) => Tried::Done(()),
            Ok(Counted::Refused { status, message }) => {
                let why = format!("the provider refuses it, {status}: {message}");
                match blames_build(status) {
                    true => Tried::Failed(why),
                    false => Tried::Trouble(why),
                }
            }
            Err(e) => trouble(e),
        },
    )
}

/// One short response, made to use [`PING`], and its tool use parsed. Signed
/// out, it waits: the build's first real request after the sign-in is its
/// first test of them.
fn response(client: &Client) -> Result<(), String> {
    let signed_in = (client.login().signed_in()).map_err(|e| format!("the login: {e}"))?;
    if !signed_in {
        return Ok(());
    }
    let cancel = Cancel::new();
    let haiku = retried(|| match client.models(&cancel) {
        Ok(offered) => Tried::Done(newest("haiku", &offered)),
        Err(e) => Tried::Trouble(format!("the provider's models: {e}")),
    })??;
    retried(|| match client.ping(&haiku, &cancel) {
        Ok(finished) => match finished.ending {
            Ending::Responded {
                stop: Stop::ToolUse,
                ..
            } => {
                let content = finished.value.unwrap_or_default();
                let used = tool_uses(&content);
                match used.as_slice() {
                    [used]
                        if used.name == PING
                            && serde_json::from_str::<serde_json::Value>(&used.input)
                                .is_ok_and(|input| input["word"].is_string()) =>
                    {
                        Tried::Done(())
                    }
                    _ => Tried::Failed(format!(
                        "it reads no use of `{PING}` in the response: {}",
                        String::from_utf8_lossy(&content)
                    )),
                }
            }
            Ending::Responded { stop, .. } => Tried::Failed(format!(
                "the response stopped {stop:?}, not to use `{PING}`"
            )),
            Ending::Failed {
                status: Some(status),
                message,
                ..
            } if blames_build(status) => {
                Tried::Failed(format!("the provider refuses it, {status}: {message}"))
            }
            ending => Tried::Trouble(format!("the response ended {ending:?}")),
        },
        Err(e) => trouble(e),
    })
}

/// The face's certificate, and its port bound.
pub fn serve(port: u16) -> Result<(TcpListener, Arc<rustls::ServerConfig>), String> {
    let certs = PathBuf::from(std::env::var("HOME").map_err(|e| format!("$HOME: {e}"))?)
        .join(".config/dev-certs/shared-local");
    let tls = harness_face_web::tls(
        &certs.join("wildcard-local.crt"),
        &certs.join("wildcard-local.key"),
    )
    .map_err(|e| format!("the certificate: {e}"))?;
    let listener = TcpListener::bind(("0.0.0.0", port)).map_err(|e| format!("port {port}: {e}"))?;
    Ok((listener, tls))
}

/// The newest model of `family` among those `offered`.
fn newest(family: &str, offered: &[harness_provider::Model]) -> Result<String, String> {
    harness_provider::resolve(family, offered.iter().map(|m| m.id.as_str()))
        .map(str::to_owned)
        .ok_or(format!("the provider offers no {family} model"))
}

/// How one try went.
enum Tried<T> {
    Done(T),
    /// The build is to blame: no other try would go otherwise.
    Failed(String),
    /// Trouble that is not the build's, which another try may not meet.
    Trouble(String),
}

/// A provider error as a try: a request this build cannot render is its
/// failure; the login and the exchange are trouble.
fn trouble<T>(e: Error) -> Tried<T> {
    match e {
        Error::Request(_) => Tried::Failed(e.to_string()),
        _ => Tried::Trouble(e.to_string()),
    }
}

/// `attempt`, tried again on trouble until [`RETRY`] has passed.
fn retried<T>(mut attempt: impl FnMut() -> Tried<T>) -> Result<T, String> {
    let until = Instant::now() + RETRY;
    loop {
        match attempt() {
            Tried::Done(done) => return Ok(done),
            Tried::Failed(why) => return Err(why),
            Tried::Trouble(why) if Instant::now() >= until => {
                return Err(format!("{why} (tried for {}s)", RETRY.as_secs()));
            }
            Tried::Trouble(_) => thread::sleep(PAUSE),
        }
    }
}

thread_local! {
    /// Where the last panic on this thread happened, while [`caught`] runs.
    static PANICKED: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// `run`, or what it panicked with and where: a panic in what every launch
/// computes is a pathway's failure, not the gate's end.
fn caught<T>(run: impl FnOnce() -> T) -> Result<T, String> {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|info| {
        let at = info.location().map(ToString::to_string).unwrap_or_default();
        PANICKED.with(|p| *p.borrow_mut() = Some(at));
    }));
    let ran = panic::catch_unwind(AssertUnwindSafe(run));
    panic::set_hook(hook);
    ran.map_err(|panicked| {
        let why = (panicked.downcast_ref::<String>().cloned())
            .or_else(|| panicked.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default();
        let at = PANICKED.with(|p| p.borrow_mut().take()).unwrap_or_default();
        format!("panicked at {at}: {why}")
    })
}
