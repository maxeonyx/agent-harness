//! Max's controls, his queue and live text, from a tab: a real brain run on
//! the brain demo's scratch repo and mandate, driven only by records said at
//! one of Max's tabs, the way the page drives it. See `demos/face-controls`.
//!
//! Every control is a batch of Max's facts at the tab; the brain's verdict
//! is in the tab's standing, and the run's progress in the tab's rows.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::str::FromStr;
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};
use std::{env, fs, thread};

use harness_core::attention::{Focus, Ranked, Ranking};
use harness_core::delivery::{Content, Delivery};
use harness_core::fact::{Batch, Fact};
use harness_core::id::{DeliveryId, ModelCallId, TaskId};
use harness_core::live::Live;
use harness_core::schema::Harness;
use harness_core::task::{Drain, Pause, TaskEvent};
use harness_face_tab::{FaceKey, FaceRow, Up};
use harness_store::Store;
use harness_substrate::{Status, Storage, What};
use serde_json::Value;

mod device;

use device::{Device, blocks, role_args, status};

fn main() {
    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(4400);
    let dir = PathBuf::from("target/face-controls-demo");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let made = Command::new("demos/brain/scratch.sh")
        .arg(dir.join("repo"))
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert!(made.success(), "the scratch repo");
    let db = dir.join("brain.sqlite");
    // Its children must be mid-request when parked and interrupted, which a
    // real model's work gives: Sonnet at low effort unless `MODEL` says.
    let roles = role_args(&device::model("sonnet/low"));
    let mut brain = Brain::launch(&db, &roles, port);
    brain.serving();
    println!("  watch it at https://mx-hv-mj.local:{port}");
    let mut max = Device::connect("tab", port);
    max.pump_until(|d| d.environment("orchestrator").is_some());
    let orchestrator = max.environment("orchestrator").unwrap();
    let repo = fs::canonicalize(dir.join("repo")).unwrap();
    let mandate = fs::read_to_string("demos/brain/mandate.md").unwrap();
    max.chat(orchestrator, repo.to_str(), &mandate);
    max.pump_until(|d| d.root().is_some());
    let root = max.root().unwrap();
    say(&format!("the root task is {}", name(&max, root)));

    title("pause the root: nothing under it sends a request until it is resumed");
    let held = say_facts(
        &mut max,
        vec![Fact::Task(root, TaskEvent::Pause(Pause::Now))],
    );
    verdict(&mut max, "pause", held);
    max.pump_until(|d| !with_waiting(d, "held").is_empty());
    brain.poll();
    let names: Vec<String> = with_waiting(&max, "held")
        .into_iter()
        .map(|t| name(&max, t))
        .collect();
    say(&format!("paused now: {}", names.join(", ")));
    max.listen(Duration::from_secs(5), |_| false);
    let sent = brain
        .poll()
        .iter()
        .filter(|l| l.contains("request sent"))
        .count();
    say(&format!(
        "5 s later: {sent} requests sent meanwhile; resume it"
    ));
    let released = say_facts(
        &mut max,
        vec![Fact::Task(root, TaskEvent::Pause(Pause::Off))],
    );
    verdict(&mut max, "resume", released);

    title(
        "park the root once both children are under way: each brings its work to a safe point and waits",
    );
    max.listen(Duration::from_secs(180), |d| children(d).len() == 2);
    let kids = children(&max);
    assert_eq!(kids.len(), 2, "the root spawned two children");
    for &kid in &kids {
        // A task whose limbs are still connecting has nothing to interrupt.
        max.listen(Duration::from_secs(120), |d| {
            d.turn(kid).is_some() && waiting(d, kid) != "starting"
        });
    }
    let parked = say_facts(
        &mut max,
        vec![Fact::Task(root, TaskEvent::Pause(Pause::Park))],
    );
    verdict(&mut max, "park", parked);
    max.listen(Duration::from_secs(300), |d| {
        kids.iter().all(|k| waiting(d, *k) == "parked")
    });
    for &kid in &kids {
        say(&format!("{}: {}", name(&max, kid), waiting(&max, kid)));
        assert_eq!(waiting(&max, kid), "parked", "{} parked", name(&max, kid));
    }
    // The turn that read the park prompt is over; resuming starts another.
    let last: Vec<_> = kids.iter().map(|k| max.turn(*k)).collect();
    let resumed = say_facts(
        &mut max,
        vec![Fact::Task(root, TaskEvent::Pause(Pause::Off))],
    );
    verdict(&mut max, "resume", resumed);
    max.listen(Duration::from_secs(120), |d| {
        kids.iter()
            .zip(&last)
            .all(|(k, l)| d.turn(*k).is_some() && d.turn(*k) != *l)
    });
    for &kid in &kids {
        say(&format!(
            "{}: carrying on, on '{}'",
            name(&max, kid),
            waiting(&max, kid)
        ));
    }

    title("interrupt both children in their next request");
    for &kid in &kids {
        let turn = max.turn(kid).expect("the child's request");
        say(&format!(
            "interrupt {} while it is on '{}', naming its request {turn}",
            name(&max, kid),
            waiting(&max, kid)
        ));
        let n = say_facts(
            &mut max,
            vec![Fact::Task(kid, TaskEvent::Drain(Drain::ToMax { turn }))],
        );
        verdict(&mut max, "interrupt", n);
    }
    for &kid in &kids {
        max.listen(Duration::from_secs(120), |d| waiting(d, kid) == "max");
        assert_eq!(
            waiting(&max, kid),
            "max",
            "{} waits on Max",
            name(&max, kid)
        );
    }
    for line in brain.poll() {
        if line.contains("interrupted") || line.contains("abandoned") {
            println!("          brain: {}", trim(&line));
        }
    }
    say("both wait on Max; his queue, oldest first:");
    print_queue(&max);

    title("reorder the queue: the second to the top");
    let order: Vec<Ranked> = queue(&max)
        .into_iter()
        .rev()
        .map(|(task, _)| Ranked {
            task,
            why: "placed by you".into(),
        })
        .collect();
    let ranked = say_facts(&mut max, vec![Fact::Ranking(Ranking { order })]);
    verdict(&mut max, "the ranking", ranked);
    print_queue(&max);
    let (top, other) = match queue(&max)[..] {
        [(top, _), (other, _), ..] => (top, other),
        _ => panic!("two tasks in the queue"),
    };

    title(&format!(
        "focus the top item, {}, and tell it to carry on",
        name(&max, top)
    ));
    let focus = say_facts(&mut max, vec![Fact::Focus(Focus { task: Some(top) })]);
    verdict(&mut max, "focus", focus);
    let words = blocks("Sorry for the interruption: carry on where you left off.");
    let id = DeliveryId(max.tab.next().event(0));
    let told = max.say(Batch {
        values: vec![words.clone()],
        facts: vec![Fact::Delivery(Delivery {
            id,
            to: top,
            content: Content::Words(words.id()),
        })],
    });
    verdict(&mut max, "words", told);
    say("the same record again, as a tab whose link dropped would resend it:");
    let record = max.tab.records().find(|r| r.at.n == told).cloned().unwrap();
    max.ship(Up::Record(record));
    max.listen(Duration::from_secs(1), |_| false);
    println!(
        "          the record {}; the database holds the delivery {} {} time(s): the resend was skipped",
        status(&max, told),
        id,
        deliveries(&db, id)
    );

    title(&format!("abandon the other, {}", name(&max, other)));
    let abandon = max.abandon(other, Duration::from_secs(120));
    verdict(&mut max, "abandon", abandon);
    max.listen(Duration::from_secs(180), |d| {
        !row(d, other)["outcome"].is_null()
    });
    let outcome = &row(&max, other)["outcome"];
    say(&format!(
        "{}: {} by {}",
        name(&max, other),
        outcome["kind"].as_str().unwrap_or("?"),
        outcome["by"].as_str().unwrap_or("?")
    ));

    title("what the brain refuses, in the tab's standing");
    let n = say_facts(
        &mut max,
        vec![Fact::Task(other, TaskEvent::Pause(Pause::Now))],
    );
    verdict(&mut max, "pause the abandoned task", n);
    let more = blocks("Are you still there?");
    let id = DeliveryId(max.tab.next().event(0));
    let n = max.say(Batch {
        values: vec![more.clone()],
        facts: vec![Fact::Delivery(Delivery {
            id,
            to: other,
            content: Content::Words(more.id()),
        })],
    });
    verdict(&mut max, "words to the abandoned task", n);
    let n = say_facts(
        &mut max,
        vec![Fact::Task(
            top,
            TaskEvent::Drain(Drain::Fail("because".into())),
        )],
    );
    verdict(&mut max, "failing a task, which is not Max's to say", n);

    title("live text: a streaming response, and a new link carrying on");
    // Press Done on the root once it waits on Max: its last turn streams.
    let mut pressed = false;
    let (call, first) = loop {
        if !pressed && waiting(&max, root) == "max" {
            max.done(root);
            pressed = true;
        }
        let Some(call) = streaming(&max) else {
            max.listen(Duration::from_millis(200), |_| false);
            continue;
        };
        let live = max.listen(Duration::from_secs(2), |_| false);
        let text = text_of(&live, call);
        // Try the next request if this one had no visible text yet, or
        // ended inside the 2 s.
        if !text.is_empty() && streaming(&max) == Some(call) {
            break (call, text);
        }
    };
    say(&format!(
        "the first link, for 2 s of {call}: {} characters",
        first.chars().count()
    ));
    max.reconnect(port);
    let live = max.listen(Duration::from_secs(300), |d| streaming(d) != Some(call));
    let again = text_of(&live, call);
    say(&format!(
        "a new link, until the response ended: {} characters, which start with the first link's text: {}",
        again.chars().count(),
        again.starts_with(&first)
    ));
    say("the streamed text:");
    for line in again.lines().take(6) {
        println!("          {line}");
    }

    title("wait for the root to end its turn, and mark it done");
    if !pressed {
        max.listen(Duration::from_secs(600), |d| waiting(d, root) == "max");
        max.done(root);
    }
    brain.until(&mut max, "the task is done");
    max.listen(Duration::from_secs(1), |_| false);
    brain.signal("INT");
    brain.wait_exit();
    say("the brain's log of the controls, and the root's result:");
    let end = brain
        .log
        .iter()
        .position(|l| l.contains("the task is done"));
    let result = brain.log[end.map_or(brain.log.len(), |e| e + 1)..]
        .iter()
        .take_while(|l| !l.starts_with("stderr: "));
    let controls = brain
        .log
        .iter()
        .filter(|l| CONTROLS.iter().any(|c| l.contains(c)));
    for line in controls.chain(result) {
        println!("{line}");
    }
    println!("\nThe database: {}", db.display());
}

/// What the brain logs of Max's controls.
const CONTROLS: &[&str] = &[
    " paused",
    " resumed",
    " interrupted",
    "response cut",
    "queue ranked",
    "changed focus",
    "delivered to",
    "abandoned by Max",
    "the task is done",
];

fn started() -> Instant {
    static START: OnceLock<Instant> = OnceLock::new();
    *START.get_or_init(Instant::now)
}

fn say(text: &str) {
    println!("{:6.1}s  {text}", started().elapsed().as_secs_f64());
}

fn title(text: &str) {
    say(&format!("== {text}"));
}

/// Say `facts` as Max at the tab: the record's time.
fn say_facts(max: &mut Device, facts: Vec<Fact>) -> u64 {
    max.say(Batch {
        values: Vec::new(),
        facts,
    })
}

/// Wait for the brain's verdict on the record at `n`, and print it.
fn verdict(max: &mut Device, what: &str, n: u64) {
    max.pump_until(|d| !matches!(d.tab.status(n), Status::Pending));
    println!("          {what}: {}", status(max, n));
}

fn row(d: &Device, task: TaskId) -> &Value {
    match d.rows().get(&FaceKey::Task(task)) {
        Some(FaceRow::Task(row)) => row,
        _ => &Value::Null,
    }
}

fn name(d: &Device, task: TaskId) -> String {
    row(d, task)["name"].as_str().unwrap_or("?").to_string()
}

fn waiting(d: &Device, task: TaskId) -> String {
    row(d, task)["waiting"]["kind"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

fn tasks(d: &Device) -> Vec<TaskId> {
    d.rows()
        .keys()
        .filter_map(|k| match k {
            FaceKey::Task(id) => Some(*id),
            _ => None,
        })
        .collect()
}

fn with_waiting(d: &Device, kind: &str) -> Vec<TaskId> {
    tasks(d)
        .into_iter()
        .filter(|t| waiting(d, *t) == kind)
        .collect()
}

/// The tasks something owns, by name.
fn children(d: &Device) -> Vec<TaskId> {
    let mut kids: Vec<TaskId> = tasks(d)
        .into_iter()
        .filter(|t| !row(d, *t)["owner"].is_null())
        .collect();
    kids.sort_by_key(|t| name(d, *t));
    kids
}

/// Max's queue, in order: each task, with the ranking's reason.
fn queue(d: &Device) -> Vec<(TaskId, Option<String>)> {
    let Some(FaceRow::Queue(Value::Array(items))) = d.rows().get(&FaceKey::Queue) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|q| {
            let task = TaskId::from_str(q["task"].as_str()?).ok()?;
            Some((task, q["why"].as_str().map(String::from)))
        })
        .collect()
}

fn print_queue(d: &Device) {
    for (task, why) in queue(d) {
        println!(
            "          {}: {}",
            name(d, task),
            why.as_deref().unwrap_or("unranked")
        );
    }
}

/// A request whose response is streaming now.
fn streaming(d: &Device) -> Option<ModelCallId> {
    tasks(d).into_iter().find_map(|t| {
        let w = &row(d, t)["waiting"];
        if w["kind"] == "model" && w["streaming"] == true {
            w["call"].as_str()?.parse().ok()
        } else {
            None
        }
    })
}

fn text_of(live: &[Live], call: ModelCallId) -> String {
    live.iter()
        .filter_map(|l| match l {
            Live::Model { call: c, chunk } if *c == call => Some(chunk.as_str()),
            _ => None,
        })
        .collect()
}

/// How many times the database holds the delivery `id`.
fn deliveries(db: &Path, id: DeliveryId) -> usize {
    let store = Store::open(db).unwrap();
    let records = Storage::<Harness>::load(&store).unwrap();
    records
        .iter()
        .filter_map(|r| match &r.what {
            What::Said(batch) => Some(batch),
            _ => None,
        })
        .flat_map(|b| &b.facts)
        .filter(|f| matches!(f, Fact::Delivery(d) if d.id == id))
        .count()
}

/// A brain log line without its timing.
fn trim(line: &str) -> &str {
    let trimmed = line.trim_start();
    trimmed
        .split_once("  ")
        .map_or(trimmed, |(_, l)| l.trim_start())
}

/// The brain, its log followed line by line.
struct Brain {
    child: Child,
    lines: Receiver<String>,
    /// Every line so far.
    log: Vec<String>,
}

impl Brain {
    /// Started here rather than with `dev/drive.mjs up`: the demo reads
    /// the brain's output line by line to time each control against what
    /// the brain is doing, and ends the process itself.
    fn launch(db: &Path, roles: &[String], port: u16) -> Brain {
        let mut child = Command::new("target/release/harness-brain")
            .args(roles)
            .arg("--db")
            .arg(db)
            .args(["--port", &port.to_string()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("target/release/harness-brain runs");
        let (send, lines) = mpsc::channel();
        let out = child.stdout.take().unwrap();
        let err = child.stderr.take().unwrap();
        let log = db.with_file_name("brain.log");
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
        started();
        println!(
            "  brain launched (pid {}), log {}",
            child.id(),
            log.display()
        );
        Brain {
            child,
            lines: rx,
            log: Vec::new(),
        }
    }

    /// The lines logged since the last poll.
    fn poll(&mut self) -> Vec<String> {
        let new: Vec<String> = self.lines.try_iter().collect();
        for line in &new {
            if line.starts_with("stderr: harness-brain") {
                println!("  {}", line.trim_start_matches("stderr: "));
            }
        }
        self.log.extend(new.iter().cloned());
        new
    }

    /// Wait until the face is listening.
    fn serving(&mut self) {
        let until = Instant::now() + Duration::from_secs(30);
        while !self.poll().iter().any(|l| l.starts_with("face: ")) {
            assert!(Instant::now() < until, "the face never served");
            thread::sleep(Duration::from_millis(100));
        }
    }

    /// Keep the tab current until the brain logs `pattern`: that line.
    fn until(&mut self, max: &mut Device, pattern: &str) -> String {
        let until = Instant::now() + Duration::from_secs(600);
        loop {
            if let Some(line) = self.poll().into_iter().find(|l| l.contains(pattern)) {
                return line;
            }
            assert!(Instant::now() < until, "the brain never said {pattern:?}");
            max.listen(Duration::from_millis(300), |_| false);
        }
    }

    fn signal(&self, name: &str) {
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
            self.poll();
            if let Ok(Some(_)) = self.child.try_wait() {
                break;
            }
            thread::sleep(Duration::from_millis(200));
        }
        let status = self.child.wait().unwrap();
        self.log.extend(self.lines.try_iter());
        say(&format!("brain exited: {status}"));
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
