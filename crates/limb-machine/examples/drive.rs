//! Plays a brain for one limb process, `harness-limb attach`, built beside
//! this example: defines a fresh limb offering the workspace, asks for two
//! working directories, and has two agents run a command in each at once.
//! While they run, it shows the systemd units they run in and the limb's
//! own memory; then each settlement; then it stops the limb's scope.
//!
//!   cargo build -q -p harness-limb-machine --bins --examples
//!   target/debug/examples/drive
//!
//! Its scratch directory is `limb-drive.ignore` in the current directory.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{env, fs, thread};

use harness_core::call::{Call, CallEvent, Caller, Outcome};
use harness_core::fact::{Fact, Refusal};
use harness_core::id::{CallId, EventId, LimbId, ReplicaId, TaskId, Time, ToolUseId};
use harness_core::limb::{
    Defined, Hello, Host, Limb, LimbDown, LimbKey, LimbRow, LimbUp, Offer, Prepared, WORKSPACE,
};
use harness_core::value::Bytes;
use harness_substrate::{Standing, Up, View, What};
use serde_json::{Value, json};

fn main() {
    let scratch = env::current_dir().unwrap().join("limb-drive.ignore");
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch).unwrap();
    // A brain of its own: a fresh replica, so a fresh limb and state.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let brain = ReplicaId(nanos as u64 & 0xffff_ffff_ffff);
    let mut n = 0;
    let mut mint = || {
        n += 1;
        EventId {
            at: Time { replica: brain, n },
            i: 0,
        }
    };
    let limb = LimbId(mint());
    let exe = env::current_exe().unwrap();
    let program = exe.parent().unwrap().parent().unwrap().join("harness-limb");
    println!("$ {} attach {limb}", program.display());
    let mut attach = Command::new(&program)
        .arg("attach")
        .arg(limb.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut down = attach.stdin.take().unwrap();
    let mut up = BufReader::new(attach.stdout.take().unwrap()).lines();
    let hello: Hello = serde_json::from_str(&up.next().unwrap().unwrap()).unwrap();
    println!("← Hello: limb {}, replica {}\n", hello.limb, hello.replica);

    let mut rows = BTreeMap::new();
    rows.insert(
        LimbKey::Definition,
        LimbRow::Definition(Defined::Limb(Limb {
            id: limb,
            name: "this machine".into(),
            host: Host::Local,
            offers: vec![Offer {
                set: WORKSPACE.into(),
                config: String::new(),
            }],
        })),
    );
    let agents = [
        ("alpha", scratch.join("a")),
        ("beta", scratch.join("b/nested/deeper")),
    ];
    let mut calls = Vec::new();
    for (name, dir) in &agents {
        let task = TaskId(mint());
        rows.insert(
            LimbKey::Workdir(dir.clone()),
            LimbRow::Workdir {
                branch: None,
                agents: BTreeMap::from([(task, name.to_string())]),
            },
        );
        let command = "echo in $(pwd); sed 's/^0:://' /proc/self/cgroup; sleep 3; echo done";
        let input = Bytes::from(json!({ "command": command }).to_string());
        let id = CallId(mint());
        let call = Call {
            caller: Caller::Agent {
                task,
                tool_use: ToolUseId(format!("toolu_{name}")),
            },
            tool: "bash".into(),
            input: input.id(),
            expects: Vec::new(),
        };
        rows.insert(LimbKey::Value(input.id()), LimbRow::Value(input));
        rows.insert(
            LimbKey::Call(id),
            LimbRow::Call {
                call,
                stopping: None,
                first_sent_to: hello.replica,
                owner: None,
                dir: Some(dir.clone()),
            },
        );
        calls.push((*name, id));
    }
    let view = LimbDown::View(View {
        as_of: Time {
            replica: brain,
            n: 1000,
        },
        standing: Standing::<Refusal> {
            upto: 0,
            refused: BTreeMap::new(),
        },
        rows,
    });
    println!("→ a whole view: the definition, 2 working directories, a command in each\n");
    writeln!(down, "{}", serde_json::to_string(&view).unwrap()).unwrap();

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for line in up {
            let Ok(line) = line else { return };
            if let Ok(up) = serde_json::from_str::<LimbUp>(&line) {
                let _ = tx.send(up);
            }
        }
    });
    let mut settled = BTreeMap::new();
    let mut shown = false;
    let start = Instant::now();
    while settled.len() < calls.len() {
        if !shown && start.elapsed() > Duration::from_millis(1500) {
            shown = true;
            show_units(limb);
        }
        let Ok(up) = rx.recv_timeout(Duration::from_millis(100)) else {
            continue;
        };
        let Up::Record(record) = up else { continue };
        let What::Said(batch) = record.what else {
            continue;
        };
        for fact in batch.facts {
            match fact {
                Fact::Workdir { dir, prepared, .. } => match prepared {
                    Prepared::Ready { instructions } => println!(
                        "← Workdir {}: ready; it says: {:?}",
                        dir.display(),
                        instructions[0].text.split(". ").next().unwrap_or_default()
                    ),
                    Prepared::Failed { why } => {
                        println!("← Workdir {}: failed: {why}", dir.display())
                    }
                },
                Fact::Described { description, .. } => {
                    for set in description.sets {
                        let tools: Vec<String> = set.tools.into_iter().map(|t| t.name).collect();
                        println!("← Described: the set `{}`: {}", set.name, tools.join(", "));
                    }
                }
                Fact::Call(id, CallEvent::Settled(outcome)) => {
                    let text = outcome_text(&outcome, &batch.values);
                    settled.insert(id, text);
                }
                _ => {}
            }
        }
    }
    for (name, id) in &calls {
        println!("\n← {name}'s command settled:");
        for line in settled[id].lines() {
            println!("    │ {line}");
        }
    }
    drop(down);
    let _ = attach.wait();
    println!("\nthe brain closed its link; the limb runs on, so stop it as systemd would:");
    let unit = format!("harness-limb-{limb}.scope");
    println!("$ systemctl --user stop {unit}");
    let _ = Command::new("systemctl")
        .args(["--user", "stop", &unit])
        .status();
    let state = state(limb);
    let log = fs::read_to_string(state.join("log")).unwrap_or_default();
    println!("its log ({}):", state.join("log").display());
    for line in log.lines() {
        println!("    │ {line}");
    }
}

fn outcome_text(outcome: &Outcome, values: &[Bytes]) -> String {
    let Some(output) = outcome.output() else {
        return format!("{outcome:?}");
    };
    let Some(bytes) = values.iter().find(|v| v.id() == output.model) else {
        return "(no output)".into();
    };
    let content: Value = serde_json::from_slice(&bytes.0).unwrap();
    content[0]["text"].as_str().unwrap_or_default().to_string()
}

/// What runs where, while both commands run: the units, and the limb's own
/// memory.
fn show_units(limb: LimbId) {
    println!("\n$ systemctl --user list-units --no-legend 'harness-*'");
    let out = Command::new("systemctl")
        .args([
            "--user",
            "list-units",
            "--no-legend",
            "--plain",
            "harness-*",
        ])
        .output()
        .unwrap();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if line.contains(&limb.0.at.replica.to_string()) {
            println!(
                "    {}",
                line.split_whitespace().collect::<Vec<_>>().join(" ")
            );
        }
    }
    let serve = Command::new("pgrep")
        .args(["-f", &format!("harness-limb serve {limb}")])
        .output()
        .unwrap();
    for pid in String::from_utf8_lossy(&serve.stdout).split_whitespace() {
        let status = fs::read_to_string(format!("/proc/{pid}/status")).unwrap_or_default();
        let rss = status
            .lines()
            .find(|l| l.starts_with("VmRSS"))
            .unwrap_or("");
        println!(
            "the limb, `harness-limb serve`, pid {pid}: {}",
            rss.split_whitespace().collect::<Vec<_>>().join(" ")
        );
    }
    println!();
}

fn state(limb: LimbId) -> PathBuf {
    let base = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(&env::var_os("HOME").unwrap()).join(".local/state"));
    base.join("harness/limbs").join(limb.to_string())
}
