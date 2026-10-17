//! Spike: one brain, many builds.
//!
//! `spike install <domain>` installs this executable under its digest.
//! `spike supervise <domain> <port> <build>` runs the supervisor, which
//! runs `spike brain` from an installed build. `spike call <port> <line>`
//! and `spike await <port>` are the client the demo drives it with.

mod brain;
mod schema;
mod store;
mod supervisor;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::exit;
use std::time::{Duration, Instant};

use harness_core::build::BuildId;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args[..] {
        ["install", domain] => install(Path::new(domain)),
        ["supervise", domain, port, build] => {
            let build = build.parse().unwrap_or_else(|_| die("unreadable build id"));
            supervisor::run(Path::new(domain), port_of(port), build)
        }
        ["brain", domain, mode] => {
            let mode = serde_json::from_str(mode).unwrap_or_else(|e| die(&e.to_string()));
            brain::run(Path::new(domain), mode)
        }
        ["call", port, ref line @ ..] => println!("{}", call(port_of(port), &line.join(" "))),
        ["await", port] => settle(port_of(port)),
        _ => die("usage: spike install|supervise|brain|call|await ..."),
    }
}

/// Installs this executable under its digest, where the supervisor runs it
/// from. Building again overwrites `target/`, never an installed build.
fn install(domain: &Path) {
    let exe = std::env::current_exe().unwrap_or_else(|e| die(&e.to_string()));
    let bytes = std::fs::read(exe).unwrap_or_else(|e| die(&e.to_string()));
    let build = BuildId::of(&bytes);
    let dir = domain.join("builds").join(build.to_string());
    let path = dir.join("harness");
    if !path.exists() {
        let partial = dir.join("harness.partial");
        let written = std::fs::create_dir_all(&dir)
            .and_then(|()| std::fs::write(&partial, &bytes))
            .and_then(|()| {
                std::fs::set_permissions(&partial, std::fs::Permissions::from_mode(0o555))
            })
            .and_then(|()| std::fs::rename(&partial, &path));
        written.unwrap_or_else(|e| die(&e.to_string()));
    }
    println!("{build}");
}

/// One line to the brain, and its reply. Connections queue on the
/// supervisor's socket while no brain is accepting, so a relaunch is a
/// pause here, never a refusal.
fn call(port: u16, line: &str) -> String {
    let start = Instant::now();
    let mut stream = loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => break stream,
            Err(_) if start.elapsed() < Duration::from_secs(30) => {
                std::thread::sleep(Duration::from_millis(200))
            }
            Err(e) => die(&e.to_string()),
        }
    };
    let _ = writeln!(stream, "{line}");
    let mut reply = String::new();
    let _ = stream.read_to_string(&mut reply);
    reply.trim_end().to_owned()
}

/// Waits until no relaunch is under way and the running build is proven,
/// and prints the status.
fn settle(port: u16) {
    let start = Instant::now();
    loop {
        let status = call(port, "status");
        if status.contains("proven=true relaunching=false") {
            println!("{status}");
            return;
        }
        if start.elapsed() > Duration::from_secs(180) {
            die(&format!("not settled: {status}"));
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

fn port_of(port: &str) -> u16 {
    port.parse().unwrap_or_else(|_| die("unreadable port"))
}

fn die(why: &str) -> ! {
    eprintln!("{why}");
    exit(1)
}
