//! Where the browser is: one the limb launches, with its profile in the
//! limb's state, or one it attaches to at an endpoint (a dedicated Chrome
//! on another machine, through a tunnel). Never Max's own browser profile:
//! a launched browser's profile is the limb's, and an attached one is
//! whatever Max chose to expose.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::{Duration, Instant};

use harness_core::id::LimbId;
use harness_limb::scope::{self, Scope, Scoped};

use crate::Settings;
use crate::cdp;

/// The browser the limb drives: its websocket URL, and the process when
/// the limb launched it.
pub struct Browser {
    pub url: String,
    pub process: Option<Child>,
}

/// Find the browser the settings name, launching it if they name none to
/// attach to. A browser an earlier instance of this limb launched on the
/// same profile, still running, is reused. One it launches runs in a
/// systemd scope of its own, `harness-browser-<limb>.scope`, so it counts
/// as the browser's and not the limb's.
pub fn open(settings: &Settings, state: &Path, limb: LimbId) -> Result<Browser, String> {
    if let Some(endpoint) = &settings.endpoint {
        let url = if endpoint.starts_with("ws://") {
            endpoint.clone()
        } else {
            cdp::discover(endpoint)?
        };
        return Ok(Browser { url, process: None });
    }
    let profile = state.join("profile");
    let port_file = profile.join("DevToolsActivePort");
    if let Some(url) = running(&port_file) {
        return Ok(Browser { url, process: None });
    }
    let program = match &settings.chrome {
        Some(path) => path.clone(),
        None => playwright().ok_or(
            "no browser: set `chrome` or `endpoint` in the limb's settings \
             (Playwright's chrome-headless-shell was not found in ~/.cache/ms-playwright)",
        )?,
    };
    let _ = std::fs::remove_file(&port_file);
    let mut args: Vec<OsString> = vec![
        "--remote-debugging-port=0".into(),
        format!("--user-data-dir={}", profile.display()).into(),
        "--no-first-run".into(),
        "--no-default-browser-check".into(),
        "--window-size=1280,900".into(),
    ];
    if settings.headless {
        args.push("--headless=new".into());
    }
    let launch = |scope: Option<&Scope>| -> Result<(Child, Scoped), String> {
        let mut spawn = scope::command(program.as_os_str(), &args, scope, None)
            .map_err(|e| format!("{}: {e}", program.display()))?;
        let child = spawn
            .command
            .spawn()
            .map_err(|e| format!("{}: {e}", program.display()))?;
        Ok((child, spawn.scoped()))
    };
    let scope = Scope {
        unit: format!("harness-browser-{limb}"),
        slice: None,
        description: format!("the browser of the harness's limb {limb}"),
    };
    let mut process = match launch(Some(&scope))? {
        (child, Scoped::In) => child,
        (child, Scoped::Without(why)) => {
            eprintln!("the browser runs without a scope of its own: {why}");
            child
        }
        (mut failed, Scoped::Failed(why)) => {
            eprintln!("the browser runs without a scope of its own: {why}");
            let _ = failed.wait();
            launch(None)?.0
        }
    };
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(url) = running(&port_file) {
            return Ok(Browser {
                url,
                process: Some(process),
            });
        }
        if let Ok(Some(status)) = process.try_wait() {
            return Err(format!("{} exited at start: {status}", program.display()));
        }
        if Instant::now() >= deadline {
            let _ = process.kill();
            return Err(format!(
                "{} did not open its debugging port within 20s",
                program.display()
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The websocket URL a browser wrote to its profile, if one is listening
/// there.
fn running(port_file: &Path) -> Option<String> {
    let text = std::fs::read_to_string(port_file).ok()?;
    let mut lines = text.lines();
    let port = lines.next()?.trim();
    let path = lines.next()?.trim();
    std::net::TcpStream::connect(("127.0.0.1", port.parse().ok()?)).ok()?;
    Some(format!("ws://127.0.0.1:{port}{path}"))
}

/// The newest chrome-headless-shell Playwright installed.
fn playwright() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let mut found: Vec<PathBuf> = std::fs::read_dir(home.join(".cache/ms-playwright"))
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("chromium_headless_shell-")
        })
        .map(|e| {
            e.path()
                .join("chrome-headless-shell-linux64/chrome-headless-shell")
        })
        .filter(|p| p.exists())
        .collect();
    found.sort();
    found.pop()
}
