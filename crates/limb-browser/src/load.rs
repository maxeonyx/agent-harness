//! A page loaded once in a headless browser of its own, for the exceptions
//! it throws: how a relaunch's rehearsal shows that the new build's page
//! runs on real state ([`harness_core::build::Pathway::Page`]).

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value as Json, json};

use crate::{cdp, chrome};

/// How long the browser gets to start, and the page to load.
const WITHIN: Duration = Duration::from_secs(30);

/// Load `url` in Playwright's chrome-headless-shell, launched for this
/// alone with a fresh profile at `profile` and trusting any certificate,
/// and return each uncaught exception the page throws by `settle` after
/// its load event, as `{description} at {url}:{line}`. Ends the browser
/// before it returns.
pub fn uncaught(url: &str, profile: &Path, settle: Duration) -> Result<Vec<String>, String> {
    let program = chrome::playwright()
        .ok_or("Playwright's chrome-headless-shell was not found in ~/.cache/ms-playwright")?;
    let _ = std::fs::remove_dir_all(profile);
    std::fs::create_dir_all(profile).map_err(|e| format!("{}: {e}", profile.display()))?;
    let browser = Browser(
        Command::new(&program)
            .arg("--remote-debugging-port=0")
            .arg(format!("--user-data-dir={}", profile.display()))
            .args(["--ignore-certificate-errors", "--no-first-run"])
            // Without it the browser waits on the system keyring, and the
            // page never loads.
            .arg("--password-store=basic")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("{}: {e}", program.display()))?,
    );
    let port_file = profile.join("DevToolsActivePort");
    let started = Instant::now();
    let endpoint = loop {
        if let Some(endpoint) = chrome::running(&port_file) {
            break endpoint;
        }
        if started.elapsed() > WITHIN {
            return Err(format!("{} did not start", program.display()));
        }
        thread::sleep(Duration::from_millis(100));
    };
    let thrown = Arc::new(Mutex::new(Vec::new()));
    let (loaded, load) = mpsc::channel();
    let cdp = cdp::Cdp::connect(&endpoint, {
        let thrown = Arc::clone(&thrown);
        move |event| match event.method.as_str() {
            "Runtime.exceptionThrown" => {
                crate::lock(&thrown).push(describe(&event.params["exceptionDetails"]))
            }
            "Page.loadEventFired" => {
                let _ = loaded.send(());
            }
            _ => {}
        }
    })?;
    let target = cdp.call(
        None,
        "Target.createTarget",
        json!({"url": "about:blank"}),
        WITHIN,
    )?;
    let attached = cdp.call(
        None,
        "Target.attachToTarget",
        json!({"targetId": target["targetId"], "flatten": true}),
        WITHIN,
    )?;
    let session = attached["sessionId"].as_str();
    cdp.call(session, "Runtime.enable", json!({}), WITHIN)?;
    cdp.call(session, "Page.enable", json!({}), WITHIN)?;
    let navigated = cdp.call(session, "Page.navigate", json!({"url": url}), WITHIN)?;
    if let Some(error) = navigated["errorText"].as_str() {
        return Err(format!("{url} did not load: {error}"));
    }
    load.recv_timeout(WITHIN)
        .map_err(|_| format!("{url} did not finish loading within {}s", WITHIN.as_secs()))?;
    thread::sleep(settle);
    // Closed, it ends its own helper processes; the kill is the fallback.
    let _ = cdp.call(None, "Browser.close", json!({}), WITHIN);
    drop(cdp);
    drop(browser);
    Ok(std::mem::take(&mut *crate::lock(&thrown)))
}

/// An exception as the browser describes it: its message and stack, or its
/// text, and where it was thrown.
fn describe(details: &Json) -> String {
    let what = (details["exception"]["description"].as_str())
        .or(details["text"].as_str())
        .unwrap_or("an exception");
    let what = what.lines().next().unwrap_or(what);
    match details["url"].as_str() {
        Some(at) if !at.is_empty() => {
            let line = details["lineNumber"].as_u64().unwrap_or(0) + 1;
            format!("{what} at {at}:{line}")
        }
        _ => what.to_owned(),
    }
}

/// The browser's process, ended when dropped.
struct Browser(Child);

impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
