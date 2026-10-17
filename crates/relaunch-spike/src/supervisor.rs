//! The supervisor: the one long-lived process. It owns the listening
//! socket, the brain processes and the installed builds; it starts a
//! candidate beside the running brain, tells the old one to hand over only
//! once the candidate has opened the database, and goes back to the build
//! before when the candidate fails its probation.

use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpListener;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio, exit};
use std::sync::mpsc::{self, Sender};

use harness_core::build::{BuildId, Failure};
use harness_core::id::CallId;

use crate::brain::{Mode, ToBrain, ToSupervisor};
use crate::schema::failure;

struct Brain {
    serial: u64,
    build: BuildId,
    child: Child,
    stdin: ChildStdin,
}

enum Event {
    Said(ToSupervisor),
    Ended,
}

struct Host<'a> {
    domain: &'a Path,
    socket: RawFd,
    events: Sender<(u64, Event)>,
    serial: u64,
}

pub fn run(domain: &Path, port: u16, first: BuildId) -> ! {
    let listener = TcpListener::bind(("127.0.0.1", port)).unwrap_or_else(|e| die(&e.to_string()));
    let good = || {
        std::fs::read_to_string(domain.join("builds/good"))
            .ok()
            .and_then(|text| text.trim().parse().ok())
            .unwrap_or(first)
    };
    let (events, rx) = mpsc::channel();
    let mut host = Host {
        domain,
        socket: listener.as_raw_fd(),
        events,
        serial: 0,
    };
    eprintln!(
        "[supervisor] listening on {port}; starting build {}",
        good().short()
    );
    let mut active = host.spawn(good(), Mode::Started);
    let mut candidate: Option<(Brain, CallId)> = None;
    // While a new build is on probation: the build to go back to.
    let mut previous: Option<BuildId> = None;
    let mut failed: Option<Failure> = None;

    while let Ok((from, event)) = rx.recv() {
        let from_active = from == active.serial;
        let from_candidate = candidate.as_ref().is_some_and(|(c, _)| c.serial == from);
        match event {
            Event::Said(ToSupervisor::Relaunch { call, relaunch }) if from_active => {
                let refusal = if relaunch.replaces != active.build {
                    Some(Failure::Stale {
                        running: active.build,
                    })
                } else if !host.installed(relaunch.to) {
                    Some(Failure::Missing)
                } else {
                    None
                };
                match refusal {
                    Some(why) => send(&mut active, &ToBrain::NotRelaunched { call, why }),
                    None => {
                        eprintln!(
                            "[supervisor] starting candidate {} beside {}",
                            relaunch.to.short(),
                            active.build.short()
                        );
                        let mode = Mode::Candidate {
                            from: active.build,
                            by: call,
                        };
                        candidate = Some((host.spawn(relaunch.to, mode), call));
                    }
                }
            }
            Event::Said(ToSupervisor::Ready) if from_candidate => {
                let to = candidate
                    .as_ref()
                    .map(|(c, _)| c.build)
                    .unwrap_or(active.build);
                send(&mut active, &ToBrain::Handover { to });
            }
            Event::Said(ToSupervisor::Handed) if from_active => {
                let Some((mut next, _)) = candidate.take() else {
                    continue;
                };
                let _ = active.child.wait();
                send(&mut next, &ToBrain::Take);
                previous = Some(active.build);
                active = next;
            }
            Event::Said(ToSupervisor::Proven) if from_active => {
                let _ = std::fs::write(domain.join("builds/good"), active.build.to_string());
                eprintln!("[supervisor] build {} is proven", active.build.short());
                previous = None;
            }
            Event::Said(ToSupervisor::Failed(why)) if from_active => failed = Some(why),
            Event::Ended if from_candidate => {
                let Some((mut c, call)) = candidate.take() else {
                    continue;
                };
                let code = c.child.wait().ok().and_then(|s| s.code());
                eprintln!(
                    "[supervisor] candidate {} exited ({code:?}); {} carries on",
                    c.build.short(),
                    active.build.short()
                );
                let why = Failure::Exited { code, stderr: None };
                send(&mut active, &ToBrain::NotRelaunched { call, why });
            }
            Event::Ended if from_active => {
                let code = active.child.wait().ok().and_then(|s| s.code());
                if let Some((mut c, _)) = candidate.take() {
                    let _ = c.child.kill();
                }
                let why = failed
                    .take()
                    .unwrap_or(Failure::Exited { code, stderr: None });
                active = match previous.take() {
                    Some(back) => {
                        eprintln!(
                            "[supervisor] build {} {}; going back to {}",
                            active.build.short(),
                            failure(&why),
                            back.short()
                        );
                        let from = active.build;
                        host.spawn(back, Mode::RolledBack { from, why })
                    }
                    None => {
                        eprintln!(
                            "[supervisor] build {} exited ({code:?}); starting the proven build {}",
                            active.build.short(),
                            good().short()
                        );
                        host.spawn(good(), Mode::Started)
                    }
                };
            }
            // From a brain that has already been replaced.
            Event::Said(_) | Event::Ended => {}
        }
    }
    exit(0)
}

impl Host<'_> {
    fn installed(&self, build: BuildId) -> bool {
        self.domain
            .join("builds")
            .join(build.to_string())
            .join("harness")
            .exists()
    }

    /// Runs `build`'s brain, with the listening socket as its fd 3.
    fn spawn(&mut self, build: BuildId, mode: Mode) -> Brain {
        self.serial += 1;
        let serial = self.serial;
        let exe = self
            .domain
            .join("builds")
            .join(build.to_string())
            .join("harness");
        let mode = serde_json::to_string(&mode).unwrap_or_default();
        let socket = self.socket;
        let mut command = Command::new(exe);
        command
            .arg("brain")
            .arg(self.domain)
            .arg(mode)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        // SAFETY: only dup, dup2 and close run between fork and exec.
        unsafe {
            command.pre_exec(move || {
                let copy = libc::dup(socket);
                if copy < 0 {
                    return Err(io::Error::last_os_error());
                }
                if copy != 3 {
                    if libc::dup2(copy, 3) < 0 {
                        return Err(io::Error::last_os_error());
                    }
                    libc::close(copy);
                }
                Ok(())
            });
        }
        let mut child = command.spawn().unwrap_or_else(|e| die(&e.to_string()));
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            die("a brain without pipes")
        };
        let events = self.events.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(said) = serde_json::from_str(&line) {
                    let _ = events.send((serial, Event::Said(said)));
                }
            }
            let _ = events.send((serial, Event::Ended));
        });
        Brain {
            serial,
            build,
            child,
            stdin,
        }
    }
}

fn send(brain: &mut Brain, said: &ToBrain) {
    let line = serde_json::to_string(said).unwrap_or_default();
    let _ = writeln!(brain.stdin, "{line}");
    let _ = brain.stdin.flush();
}

fn die(why: &str) -> ! {
    eprintln!("[supervisor] {why}");
    exit(1)
}
