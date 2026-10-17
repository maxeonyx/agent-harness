//! The spine, end to end: real brains on one database, and two of Max's
//! tabs (his PC and his phone) as replicas on the face's link, played by the
//! same `Tab` the page runs as wasm. See `demos/spine`.
//!
//! 1. The phone dictates a draft; the PC finishes it and sends it. Both
//!    tabs show it sent, from their own replicas.
//! 6. A request interrupted before its first byte.
//! 5. Ctrl-C mid-command: the brain drains and exits, and the next launch
//!    carries the agent on, reading the prompt cache. Ctrl-C twice
//!    mid-request: it exits at once, and the next launch sends the lost
//!    request again, reading the cache.
//! 2. Done on a chat the brain already finished: the phone, which had not
//!    seen it finish, is refused, in its own standing.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};
use std::{env, fs, thread};

use harness_core::delivery::Read;
use harness_core::draft::DraftEvent;
use harness_core::fact::{Batch, Fact};
use harness_core::id::{DeliveryId, ModelCallId};
use harness_core::model_call::ModelCallEvent;
use harness_core::schema::Harness;
use harness_core::task::{Drain, TaskEvent};
use harness_store::Store;
use harness_substrate::{Status, Storage, What};
use serde_json::{Value, json};

mod device;

use device::{Device, blocks, status};

fn main() {
    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(4398);
    let dir = PathBuf::from("target/spine-demo");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let made = Command::new("demos/brain/scratch.sh")
        .arg(dir.join("repo"))
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert!(made.success(), "the scratch repo");
    let db = dir.join("brain.sqlite");

    title("launch 1: a new database, and a chat with Max");
    let mut brain = Brain::launch(&db, port, 1);
    let mut pc = Device::connect("pc", port);
    let mut phone = Device::connect("phone", port);
    pc.pump_until(|d| d.environment("engineer").is_some());
    let engineer = pc.environment("engineer").unwrap();
    let repo = fs::canonicalize(dir.join("repo")).unwrap();
    pc.chat(
        engineer,
        repo.to_str(),
        "This chat is a demonstration of the harness. Max sends short messages: do exactly what each asks, \
in as few words as you can. Now reply only `ready`, and end your turn.",
    );
    brain.wait("responded, EndTurn");
    pc.pump_until(|d| d.session().is_some());
    phone.pump_until(|d| d.session().is_some());
    let session = pc.session().unwrap();

    title("1. The phone dictates; the PC finishes the words and sends them");
    let opened = phone.tab.next();
    let draft = DeliveryId(opened.event(0));
    let dictated = blocks("Reply with one word:");
    phone.say(Batch {
        values: vec![dictated.clone()],
        facts: vec![
            Fact::Draft(draft, DraftEvent::Opened { session, at: None }),
            Fact::Draft(draft, DraftEvent::Text(dictated.id())),
        ],
    });
    pc.pump_until(|d| d.draft(draft).is_some());
    println!(
        "  the PC's replica holds the phone's draft {draft}: {:?}",
        pc.draft(draft).unwrap()
    );
    let finished = blocks("Reply with one word: pineapple.");
    let sent = pc.say(Batch {
        values: vec![finished.clone()],
        facts: vec![
            Fact::Draft(draft, DraftEvent::Text(finished.id())),
            Fact::Draft(
                draft,
                DraftEvent::Send {
                    text: finished.id(),
                    read: Read::Next,
                },
            ),
        ],
    });
    pc.pump_until(|d| matches!(d.tab.status(sent), Status::Included));
    phone.pump_until(|d| d.draft(draft).is_none());
    println!(
        "  PC: its Send {}; its replica has no draft row: sent",
        status(&pc, sent)
    );
    println!(
        "  phone: the draft row is gone from its replica too (as of {}): sent",
        phone.tab.view().unwrap().as_of
    );
    println!("  agent: {}", brain.wait("responded, EndTurn"));

    title("6. A request interrupted before its first byte");
    let asked = pc.message(session, "Count from 1 to 40, one number per line.");
    pc.pump_until(|d| matches!(d.tab.status(asked), Status::Included));
    let turn = pc
        .turn(session)
        .expect("the message's request is the session's newest");
    let interrupt = pc.say(Batch {
        values: Vec::new(),
        facts: vec![Fact::Task(session, TaskEvent::Drain(Drain::ToMax { turn }))],
    });
    pc.pump_until(|d| !matches!(d.tab.status(interrupt), Status::Pending));
    println!("  the interrupt naming {turn}: {}", status(&pc, interrupt));
    let seen = brain.until(": response cut");
    let first = seen.iter().any(|l| l.contains("first byte"));
    println!("  brain: {}", seen.last().unwrap());
    println!("  it had a first byte before it ended: {first}");
    brain.settle();

    title("5. Ctrl-C mid-command: the brain drains, exits, and the next launch carries on");
    pc.message(
        session,
        "Use bash to run `sleep 8 && echo slept`, then reply with what it printed, or how it ended.",
    );
    brain.wait("bash dispatched");
    thread::sleep(Duration::from_secs(1));
    brain.signal("INT");
    brain.wait_exit();
    let mut brain = Brain::launch(&db, port, 2);
    pc.reconnect(port);
    phone.reconnect(port);
    let after = brain.wait("responded");
    println!("  agent, after the restart: {after}");
    brain.usage(1);
    brain.settle();

    title(
        "5. Ctrl-C twice mid-request: the brain exits at once, and the next launch sends it again",
    );
    pc.message(session, "Name three fruits, one per line.");
    brain.wait("request sent");
    brain.signal("INT");
    thread::sleep(Duration::from_millis(100));
    brain.signal("INT");
    brain.wait_exit();
    let mut brain = Brain::launch(&db, port, 3);
    pc.reconnect(port);
    phone.reconnect(port);
    println!("  agent, after the restart: {}", brain.wait("responded"));
    brain.usage(1);
    brain.settle();

    title("2. Done on a chat the brain already finished");
    // The phone looks away: what it holds stays as it was.
    let stale = phone.tab.view().unwrap().as_of;
    let done = pc.done(session);
    pc.pump_until(|d| matches!(d.tab.status(done), Status::Included));
    println!("  PC: its Done {}", status(&pc, done));
    brain.wait("the task is done");
    let late = phone.done(session);
    phone.pump_until(|d| !matches!(d.tab.status(late), Status::Pending));
    println!(
        "  phone, which had seen the brain as of {stale}: its Done {}",
        status(&phone, late)
    );
    brain.signal("INT");
    brain.wait_exit();
    println!("\nThe database: {}", db.display());
}

fn title(text: &str) {
    println!("\n== {text}");
}

/// One launch of the brain, its log followed line by line.
struct Brain {
    child: Child,
    lines: Receiver<String>,
    db: PathBuf,
    /// The model calls already reported, so `usage` shows the new ones.
    shown: usize,
}

impl Brain {
    fn launch(db: &Path, port: u16, n: u32) -> Brain {
        let mut child = Command::new("target/release/harness-brain")
            .args(["--roles", "roles", "--db"])
            .arg(db)
            .args(["--port", &port.to_string()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("target/release/harness-brain runs");
        let (send, lines) = mpsc::channel();
        let out = child.stdout.take().unwrap();
        let err = child.stderr.take().unwrap();
        let log = db.with_file_name(format!("launch{n}.log"));
        let mut file = fs::File::create(&log).unwrap();
        let send2 = send.clone();
        thread::spawn(move || {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                let _ = send2.send(format!("stderr: {line}"));
            }
        });
        thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let _ = send.send(line);
            }
        });
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            for line in lines {
                use std::io::Write;
                let _ = writeln!(file, "{line}");
                let _ = tx.send(line);
            }
        });
        println!(
            "  brain {} launched (pid {}), log {}",
            n,
            child.id(),
            log.display()
        );
        let mut brain = Brain {
            child,
            lines: rx,
            db: db.to_path_buf(),
            shown: 0,
        };
        brain.shown = brain.ended().len();
        brain
    }

    /// The next log line containing `pattern`, trimmed of its timing.
    fn wait(&mut self, pattern: &str) -> String {
        self.until(pattern).pop().unwrap()
    }

    /// The log lines up to the next containing `pattern`, each trimmed of
    /// its timing.
    fn until(&mut self, pattern: &str) -> Vec<String> {
        let until = Instant::now() + Duration::from_secs(180);
        let mut seen = Vec::new();
        loop {
            let left = until.saturating_duration_since(Instant::now());
            let line = match self.lines.recv_timeout(left) {
                Ok(line) => line,
                Err(_) => panic!("the brain never said {pattern:?}"),
            };
            if line.starts_with("stderr: harness-brain") {
                println!("  {}", line.trim_start_matches("stderr: "));
            }
            let trimmed = line.trim_start();
            let trimmed = trimmed.split_once("  ").map_or(trimmed, |(_, l)| l);
            seen.push(trimmed.to_string());
            if line.contains(pattern) {
                return seen;
            }
        }
    }

    /// Let the turn end: the agent's move is Max's again.
    fn settle(&mut self) {
        thread::sleep(Duration::from_millis(500));
        while self.lines.try_recv().is_ok() {}
    }

    fn signal(&self, name: &str) {
        println!("  ** kill -{name} the brain");
        let ok = Command::new("kill")
            .arg(format!("-{name}"))
            .arg(self.child.id().to_string())
            .status()
            .unwrap()
            .success();
        assert!(ok);
    }

    fn wait_exit(&mut self) {
        let until = Instant::now() + Duration::from_secs(60);
        while Instant::now() < until {
            match self.lines.recv_timeout(Duration::from_millis(200)) {
                Ok(line) if line.starts_with("stderr: harness-brain") => {
                    println!("  {}", line.trim_start_matches("stderr: "));
                }
                Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            if let Ok(Some(_)) = self.child.try_wait() {
                break;
            }
        }
        let status = self.child.wait().unwrap();
        println!("  brain exited: {status}");
    }

    /// Every model call's usage, in the order the database holds them.
    fn ended(&self) -> Vec<(ModelCallId, Value)> {
        let store = Store::open(&self.db).unwrap();
        let records = Storage::<Harness>::load(&store).unwrap();
        let mut ended = Vec::new();
        for record in records {
            let What::Said(batch) = &record.what else {
                continue;
            };
            for fact in &batch.facts {
                if let Fact::ModelCall(id, ModelCallEvent::Ended { usage, .. }) = fact {
                    ended.push((*id, json!(usage)));
                }
            }
        }
        ended
    }

    /// Print the usage of the first `n` requests this launch has ended.
    fn usage(&mut self, n: usize) {
        let ended = self.ended();
        for (id, usage) in ended.iter().skip(self.shown).take(n) {
            println!("  usage of {id}, the first request of this launch: {usage}");
        }
        self.shown = ended.len();
    }
}

/// A demo that stops part way leaves no brain behind. Its limbs see their
/// link gone, and exit once it stays gone.
impl Drop for Brain {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
