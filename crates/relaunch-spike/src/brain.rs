//! The brain: the one decider, run from an installed build. It opens the
//! database whatever wrote it, takes over only when told, continues the
//! session that asked for it, and serves a line protocol on the socket the
//! supervisor owns (fd 3).

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::ops::ControlFlow;
use std::os::fd::FromRawFd;
use std::path::Path;
use std::process::exit;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use harness_core::build::{BuildId, Failure, Handover, Launch, Launched, Proven, Relaunch};
use harness_core::context::{Message, Role, Root};
use harness_core::id::{CallId, ModelCallId, PlaceId, Time, Timestamp, ValueId};
use harness_core::model_call::{Ending, ModelCall, Usage};
use harness_provider::{Client, Finished, IDENTITY, Login};
use harness_substrate::Decider;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::schema::{Said, Spike, failure};
use crate::store::Lines;

/// The brain's place: every build continues the one timeline.
const BRAIN: PlaceId = PlaceId(1);

type Brain = Decider<Spike, Lines>;

/// How this brain came to run, as the supervisor started it.
#[derive(Serialize, Deserialize)]
pub enum Mode {
    Started,
    Candidate { from: BuildId, by: CallId },
    RolledBack { from: BuildId, why: Failure },
}

/// What a brain tells its supervisor: one JSON line each, on stdout.
#[derive(Serialize, Deserialize)]
pub enum ToSupervisor {
    /// A candidate has opened the database.
    Ready,
    /// The running brain has written its handover and stops.
    Handed,
    Proven,
    /// A build on probation failed; it exits next.
    Failed(Failure),
    Relaunch {
        call: CallId,
        relaunch: Relaunch,
    },
}

/// What the supervisor tells a brain, on stdin.
#[derive(Serialize, Deserialize)]
pub enum ToBrain {
    /// The old brain has handed over: decide from here.
    Take,
    Handover {
        to: BuildId,
    },
    NotRelaunched {
        call: CallId,
        why: Failure,
    },
}

#[cfg(not(feature = "bad-requests"))]
const PARAMS: &str = r#"{"max_tokens":200}"#;
#[cfg(feature = "bad-requests")]
const PARAMS: &str = r#"{"max_tokens":200,"temperature":7}"#;

const PROBE: &str = "One short line: which brig is listed first in the ledger, and what did the harness tell you just now?";

struct Me {
    build: BuildId,
    tag: String,
    probation: bool,
    client: Client,
}

pub fn run(domain: &Path, mode: Mode) -> ! {
    let exe = std::env::current_exe().unwrap_or_else(|e| die(&e.to_string()));
    let build = BuildId::of(&std::fs::read(exe).unwrap_or_else(|e| die(&e.to_string())));
    let tag = format!("[brain {}]", build.short());
    let open = || {
        let store = Lines {
            path: domain.join("records.jsonl"),
        };
        let (brain, divergent) = Decider::open(BRAIN, store).unwrap_or_else(|e| die(&e));
        let state = brain.state();
        eprintln!(
            "{tag} opened the database: {} records, {} sessions, {} unread, {} divergent",
            brain.head().n,
            state.sessions.len(),
            state.unread.len(),
            divergent.len(),
        );
        for (at, why) in &state.unread {
            eprintln!("{tag}   holding unread record {at}: {why}");
        }
        brain
    };
    let mut brain = open();
    if cfg!(feature = "crash-on-open") {
        panic!("this build's fold has a bug");
    }

    let (tx, supervisor) = mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if let Ok(said) = serde_json::from_str::<ToBrain>(&line) {
                let _ = tx.send(said);
            }
        }
    });
    let how = match mode {
        Mode::Started => Launch::Started,
        Mode::Candidate { from, by } => {
            tell(&ToSupervisor::Ready);
            eprintln!(
                "{tag} ready; build {} serves until it hands over",
                from.short()
            );
            if !matches!(supervisor.recv(), Ok(ToBrain::Take)) {
                exit(0);
            }
            // Fold what the old brain wrote while this one was opening.
            brain = open();
            Launch::Relaunched { from, by: Some(by) }
        }
        Mode::RolledBack { from, why } => Launch::RolledBack { from, why },
    };
    let probation = matches!(how, Launch::Relaunched { .. });
    let asked = brain
        .state()
        .relaunching
        .as_ref()
        .map(|(_, s, _)| s.clone());
    say(&mut brain, Said::Launched(Launched { build, how })).unwrap_or_else(|e| die(&e));
    eprintln!("{tag} deciding");
    #[cfg(feature = "new-fact")]
    say(
        &mut brain,
        Said::Note {
            text: format!("build {} was here", build.short()),
        },
    )
    .unwrap_or_else(|e| die(&e));

    let login = Login::claude_code().unwrap_or_else(|e| die(&e.0));
    let me = Me {
        build,
        tag,
        probation,
        client: Client::new(login),
    };
    // The session that asked for this build carries on, and its request is
    // the probe that proves the build.
    if let Some(session) = asked.filter(|_| probation) {
        eprintln!("{} continuing {session}", me.tag);
        let reply = turn(&mut brain, &me, &session, PROBE);
        eprintln!("{} {session}: {reply}", me.tag);
    }
    serve(brain, me, supervisor)
}

fn serve(mut brain: Brain, me: Me, supervisor: Receiver<ToBrain>) -> ! {
    // SAFETY: the supervisor hands every brain its listening socket as fd 3.
    let listener = unsafe { TcpListener::from_raw_fd(3) };
    listener
        .set_nonblocking(true)
        .unwrap_or_else(|e| die(&e.to_string()));
    loop {
        loop {
            match supervisor.try_recv() {
                Ok(ToBrain::Handover { to }) => {
                    let handover = Handover {
                        to,
                        runner: None,
                        runs: Vec::new(),
                    };
                    say(&mut brain, Said::Handover(handover)).unwrap_or_else(|e| die(&e));
                    tell(&ToSupervisor::Handed);
                    eprintln!("{} handed over to {}", me.tag, to.short());
                    exit(0)
                }
                Ok(ToBrain::NotRelaunched { call, why }) => {
                    eprintln!("{} relaunch {call} failed: {}", me.tag, failure(&why));
                    say(&mut brain, Said::NotRelaunched { call, why }).unwrap_or_else(|e| die(&e));
                }
                Ok(ToBrain::Take) => {}
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    eprintln!("{} the supervisor is gone; stopping", me.tag);
                    exit(0)
                }
            }
        }
        match listener.accept() {
            Ok((stream, _)) => handle(&mut brain, &me, stream),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20))
            }
            Err(e) => die(&e.to_string()),
        }
    }
}

fn handle(brain: &mut Brain, me: &Me, mut stream: TcpStream) {
    let _ = stream.set_nonblocking(false);
    let mut line = String::new();
    if BufReader::new(&stream).read_line(&mut line).is_err() {
        return;
    }
    let words: Vec<&str> = line.trim().splitn(3, ' ').collect();
    let reply = match words[..] {
        ["open", session] => {
            let (root, values) = root(brain.state().sessions.len());
            let session = session.to_owned();
            match say(
                brain,
                Said::Opened {
                    session: session.clone(),
                    root,
                    values,
                },
            ) {
                Ok(_) => format!("opened {session}"),
                Err(why) => format!("refused: {why}"),
            }
        }
        ["ask", session, text] if brain.state().sessions.contains_key(session) => {
            turn(brain, me, session, text)
        }
        ["relaunch", session, rest] => relaunch(brain, me, session, rest),
        ["status"] => status(brain, me),
        _ => format!("unknown: {}", line.trim()),
    };
    let _ = writeln!(stream, "{reply}");
}

fn relaunch(brain: &mut Brain, me: &Me, session: &str, rest: &str) -> String {
    let mut ids = rest.split(' ').map(str::parse::<BuildId>);
    let (Some(Ok(to)), replaces) = (ids.next(), ids.next()) else {
        return "usage: relaunch <session> <build> [<replaces>]".into();
    };
    let replaces = match replaces {
        Some(Ok(id)) => id,
        Some(Err(_)) => return "unreadable build id".into(),
        None => me.build,
    };
    let relaunch = Relaunch { to, replaces };
    let said = Said::Relaunching {
        session: session.into(),
        relaunch: relaunch.clone(),
    };
    match say(brain, said) {
        Ok(at) => {
            let call = CallId(at.event(0));
            tell(&ToSupervisor::Relaunch { call, relaunch });
            format!("relaunch to {} asked: call {call}", to.short())
        }
        Err(why) => format!("refused: {why}"),
    }
}

/// One request in `session`, recorded with what it cost; what to show.
fn turn(brain: &mut Brain, me: &Me, session: &str, text: &str) -> String {
    let state = brain.state();
    let s = &state.sessions[session];
    let content = json!([{ "type": "text", "text": render(&s.notices, text) }])
        .to_string()
        .into_bytes();
    let params = PARAMS.as_bytes().to_vec();
    let message = Message {
        role: Role::User,
        content: ValueId::of(&content),
    };
    let head = message.id_after(s.head);
    let call = ModelCall {
        task: s.task,
        base: s.head,
        appends: vec![message],
        renders: Vec::new(),
        params: ValueId::of(&params),
        breakpoints: vec![s.root, head],
    };
    let id = ModelCallId(
        Time {
            place: BRAIN,
            n: brain.head().n + 1,
        }
        .event(0),
    );
    let new = [content.clone(), params.clone()];
    let lookup = |v: ValueId| {
        new.iter()
            .find(|bytes| ValueId::of(bytes) == v)
            .or_else(|| state.values.get(&v))
            .cloned()
    };
    let sent = me
        .client
        .send(id, &call, &state.context(s.head), lookup, |_| {
            ControlFlow::Continue(())
        });
    let session = session.to_owned();
    let refused = match sent {
        Ok(Finished {
            ending: Ending::Responded {
                content: response, ..
            },
            value: Some(bytes),
            usage,
            ..
        }) => {
            let usage = usage.unwrap_or_default();
            let answer = text_of(&bytes);
            let turn = Said::Turn {
                session,
                call,
                response,
                values: vec![content, params, bytes],
                usage,
            };
            say(brain, turn).unwrap_or_else(|e| die(&e));
            if !brain.state().proven.contains(&me.build) {
                let proven = Proven {
                    build: me.build,
                    by: id,
                };
                say(brain, Said::Proven(proven)).unwrap_or_else(|e| die(&e));
                tell(&ToSupervisor::Proven);
                eprintln!("{} proven", me.tag);
            }
            return format!("{answer}\n  {}", cost(me.build, usage));
        }
        Ok(Finished { ending, .. }) => match ending {
            Ending::Failed {
                status, message, ..
            } => Failure::Refused {
                call: id,
                status,
                message,
            },
            other => Failure::Refused {
                call: id,
                status: None,
                message: format!("{other:?}"),
            },
        },
        Err(e) => Failure::Refused {
            call: id,
            status: None,
            message: e.to_string(),
        },
    };
    let why = failure(&refused);
    say(
        brain,
        Said::Failed {
            session,
            why: why.clone(),
        },
    )
    .unwrap_or_else(|e| die(&e));
    if me.probation && !brain.state().proven.contains(&me.build) {
        eprintln!("{} {why}; failing probation", me.tag);
        tell(&ToSupervisor::Failed(refused));
        exit(1)
    }
    format!("failed: {why}")
}

fn status(brain: &Brain, me: &Me) -> String {
    let state = brain.state();
    let mut out = format!(
        "running {} proven={} relaunching={} pid={}\n",
        me.build.short(),
        state.proven.contains(&me.build),
        state.relaunching.is_some(),
        std::process::id(),
    );
    out += &format!(
        "  {} records; {} unread; notes: {:?}\n",
        brain.head().n,
        state.unread.len(),
        state.notes,
    );
    for (name, session) in &state.sessions {
        out += &format!("  session {name}: {} turns\n", session.turns);
    }
    out += "  history:\n";
    for line in &state.history {
        out += &format!("    {line}\n");
    }
    out.trim_end().to_owned()
}

#[cfg(not(feature = "render-v2"))]
fn render(notices: &[String], text: &str) -> String {
    if notices.is_empty() {
        return text.into();
    }
    format!("[{}]\n\n{text}", notices.join("; "))
}

/// The fix: each notice on its own line, said to be the harness's.
#[cfg(feature = "render-v2")]
fn render(notices: &[String], text: &str) -> String {
    if notices.is_empty() {
        return text.into();
    }
    let lines: Vec<String> = notices.iter().map(|n| format!("- {n}")).collect();
    format!(
        "From the harness, since your last turn:\n{}\n\n{text}",
        lines.join("\n")
    )
}

fn cost(build: BuildId, usage: Usage) -> String {
    format!(
        "(build {} | cache_read={} cache_write={} input={} output={})",
        build.short(),
        usage.cache_read,
        usage.cache_write_short + usage.cache_write_long,
        usage.input,
        usage.output,
    )
}

fn text_of(content: &[u8]) -> String {
    let blocks: Vec<serde_json::Value> = serde_json::from_slice(content).unwrap_or_default();
    let text: Vec<&str> = blocks.iter().filter_map(|b| b["text"].as_str()).collect();
    text.join(" ").replace('\n', " ")
}

/// A session's root: a system with a ledger of about 3k tokens, unique to
/// this run so no earlier run's cache is read; and the bytes it names.
fn root(n: usize) -> (Root, Vec<Vec<u8>>) {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let ports = ["Lyttelton", "Napier", "Nelson", "Timaru", "Bluff", "Picton"];
    let mut ledger = format!("Shipping ledger {nonce}-{n}. Answer questions about it briefly.\n");
    for i in 0..200 {
        ledger.push_str(&format!(
            "1887-{:02}-{:02}: the {} brig No. {i} left {} with {} tons of {}.\n",
            1 + i % 12,
            1 + i * 7 % 28,
            ["red", "grey", "swift", "old"][i % 4],
            ports[i * 5 % 6],
            100 + i * 37 % 900,
            ["wool", "flax", "timber", "coal", "grain"][i % 5],
        ));
    }
    let system = json!([
        { "type": "text", "text": IDENTITY },
        { "type": "text", "text": ledger },
    ])
    .to_string()
    .into_bytes();
    let tools = b"[]".to_vec();
    let model = std::env::var("SPIKE_MODEL").unwrap_or_else(|_| "claude-sonnet-5".into());
    let root = Root {
        model,
        system: ValueId::of(&system),
        tools: ValueId::of(&tools),
    };
    (root, vec![system, tools])
}

fn say(brain: &mut Brain, said: Said) -> Result<Time, String> {
    brain.say(now(), |_| said).map_err(|e| format!("{e:?}"))
}

fn now() -> Timestamp {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    Timestamp(ms as i64)
}

fn tell(said: &ToSupervisor) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", serde_json::to_string(said).unwrap_or_default());
    let _ = out.flush();
}

fn die(why: &str) -> ! {
    eprintln!("[brain] {why}");
    exit(1)
}
