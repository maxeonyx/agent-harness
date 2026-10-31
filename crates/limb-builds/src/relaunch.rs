//! Relaunching a brain's service onto the commit of a clone: the procedure
//! the `relaunch` tool and the command line share.
//!
//! A brain runs as the durable systemd user unit `harness-NAME.service`
//! (see [`start`]), from an installed build:
//! `$XDG_DATA_HOME/agent-harness/builds/COMMIT/`, its executables and
//! `roles/`, read-only and never overwritten. What the unit runs is read
//! off its process: its command line names the build's directory, the
//! database (`--db`), the port and its flags, and a relaunch starts the
//! same command line in another build's directory. `NAME/build` beside
//! the database links to the build a restart or a reboot runs: the last
//! one to open the database.
//!
//! The relaunch from the running build A onto a new build B
//! ([`relaunch`]), each stage judged by A's code and by what any build
//! shows, never by B's records:
//!
//! ```mermaid
//! flowchart TD
//!   R["Rehearsal, beside A, which serves on:<br/>B --check on a copy (gate: lines),<br/>its page, served read-only, loaded in headless Chrome:<br/>no uncaught exception;<br/>sign-in read alike by A and B;<br/>two scratch brains of B, through A's limb and B's own:<br/>spawn, a bash token, a copy A checks;<br/>A folds what they wrote (readback)"]
//!   R -->|"a pathway fails, or one-way without one_way"| Refused["Refused: A never stopped"]
//!   R --> T["Takeover: A drains and stops;<br/>B starts with --candidate"]
//!   T -->|"its process ends, or 3 min pass (killed)"| Failed["Failed: the unit restarts the link, A,<br/>on the bytes A left"]
//!   T -->|"the link names B"| W["Opened: watched for 3 min"]
//!   W -->|"B's process ends"| Back["Went back: the link is A again,<br/>and A starts, skipping and logging<br/>what it cannot read"]
//!   W -->|"B outlives the window"| Opened["Opened"]
//! ```
//!
//! A relaunch is two-way when A folds everything B's scratch brains wrote:
//! going back is then lossless. Otherwise it is one-way, and refused unless
//! asked for ([`Asked::one_way`]): a way back after it skips what A cannot
//! read and may run a call a second time. The way back is never refused.
//! Max (31 Oct): "Sending two is better than bricking."

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use harness_core::build::{Pathway, Start, Through};
use harness_core::id::CallId;
use harness_core::schema;
use harness_provider::Login;
use harness_store::Store;
use serde_json::json;

/// How long a candidate gets to open the live database before it is
/// killed: its gate's pathways take a few seconds, and retry provider
/// trouble for a minute.
const OPENING: Duration = Duration::from_secs(3 * 60);
/// How long an opened build is watched, so that its process ending goes
/// back to the build before.
const WINDOW: Duration = Duration::from_secs(3 * 60);
/// How long a scratch brain gets to serve, and then to make its round trip.
const ROUND_TRIP: Duration = Duration::from_secs(90);
/// How long the page gets after it loads to render its first view.
const PAGE_SETTLES: Duration = Duration::from_secs(3);
/// How often a stage looks at the unit.
const EVERY: Duration = Duration::from_millis(500);

/// Where builds and brains live: `$XDG_DATA_HOME/agent-harness`.
pub fn home() -> PathBuf {
    match std::env::var_os("XDG_DATA_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"),
    }
    .join("agent-harness")
}

/// A brain's service as it runs now.
#[derive(Clone, Debug)]
pub struct Running {
    pub unit: String,
    /// Its command line, the executable first, as an installed start: with
    /// no [`Start::FLAG`].
    argv: Vec<String>,
    /// The installed build it runs from.
    pub build: PathBuf,
    pub commit: String,
    db: PathBuf,
}

impl Running {
    pub fn of(unit: &str) -> Result<Running, String> {
        let pid = main_pid(unit)?.ok_or_else(|| format!("{unit} is not running"))?;
        let cmdline = std::fs::read(format!("/proc/{pid}/cmdline"))
            .map_err(|e| format!("{unit}'s process {pid}: {e}"))?;
        let argv: Vec<String> = cmdline
            .split(|b| *b == 0)
            .filter(|a| !a.is_empty())
            .map(|a| String::from_utf8_lossy(a).into_owned())
            .filter(|a| a != Start::FLAG)
            .collect();
        let exe = std::fs::read_link(format!("/proc/{pid}/exe"))
            .map_err(|e| format!("{unit}'s process {pid}: {e}"))?;
        let build = exe
            .parent()
            .and_then(Path::parent)
            .filter(|b| b.parent() == Some(&home().join("builds")))
            .ok_or_else(|| format!("{unit} runs {}, not an installed build", exe.display()))?
            .to_path_buf();
        let commit = name(&build);
        let db = flag(&argv, "--db")
            .map(PathBuf::from)
            .ok_or_else(|| format!("{unit}'s command line names no --db"))?;
        // Started by the unit file, it names its build through the link.
        let link = db.with_file_name("build");
        let argv = argv
            .iter()
            .map(|a| rebase(a, &link.to_string_lossy(), &build.to_string_lossy()))
            .collect();
        Ok(Running {
            unit: unit.into(),
            argv,
            build,
            commit,
            db,
        })
    }

    /// The same command line in another build's directory.
    fn on(&self, build: &Path) -> Vec<String> {
        let (old, new) = (self.build.to_string_lossy(), build.to_string_lossy());
        self.argv.iter().map(|a| rebase(a, &old, &new)).collect()
    }

    fn data(&self) -> PathBuf {
        self.db.parent().unwrap_or(Path::new("/")).to_path_buf()
    }

    /// The login its brain reads: the one `--login` names, or the home
    /// brain's, as the brain defaults.
    fn login(&self) -> Option<&str> {
        flag(&self.argv, "--login")
    }
}

/// What a relaunch call asks besides its clone.
#[derive(Clone, Copy, Debug, Default)]
pub struct Asked {
    /// Allow a commit that does not descend from the running one.
    pub back: bool,
    /// Allow a one-way relaunch: one the running build could not go back
    /// from without skipping what the new one writes.
    pub one_way: bool,
}

/// How a relaunch ended: the stage it reached, in words.
#[derive(Debug)]
pub enum Ended {
    /// Refused before the running build was stopped: nothing changed.
    Refused(String),
    /// The candidate did not open on the live database, and wrote nothing:
    /// the unit runs the build before again, on the bytes it left.
    Failed(String),
    /// The new build opened and outlived the window.
    Opened(String),
    /// The new build opened, then its process ended within the window, so
    /// the build before runs again.
    WentBack(String),
}

impl Ended {
    /// The stage, as the call's result begins: the face's view of the
    /// relaunch reads these words (`face-web`'s `view.rs`).
    pub fn stage(&self) -> &'static str {
        match self {
            Ended::Refused(_) => "refused at rehearsal",
            Ended::Failed(_) => "failed its gate",
            Ended::Opened(_) => "opened",
            Ended::WentBack(_) => "went back",
        }
    }

    pub fn words(&self) -> &str {
        match self {
            Ended::Refused(w) | Ended::Failed(w) | Ended::Opened(w) | Ended::WentBack(w) => w,
        }
    }
}

/// Relaunch `unit` onto the commit `clone` has checked out, for the call
/// `by` if one asked: rehearse it beside the running build, then start it
/// as a candidate on the live database, then watch it for a window and go
/// back if its process ends. Returns once, after the window or at the
/// stage that ended it.
pub fn relaunch(
    unit: &str,
    clone: &Path,
    asked: Asked,
    by: Option<CallId>,
    say: &dyn Fn(&str),
) -> Ended {
    let (_lock, old, build, one_way) = match prepare(unit, clone, asked, say) {
        Ok(prepared) => prepared,
        Err(why) => {
            return Ended::Refused(format!("{why}\nNothing changed: {unit} was never stopped."));
        }
    };
    let commit = name(&build);
    say(&format!(
        "stopping {unit} on {}: its requests in flight drain first",
        old.commit
    ));
    if let Err(why) = stop(unit) {
        return Ended::Failed(format!("{unit} would not stop: {why}"));
    }
    let since = SystemTime::now();
    say(&format!("starting {commit} on {unit} as a candidate"));
    let mut candidate = old.on(&build);
    candidate.push(Start::FLAG.into());
    if let Err(why) = start(unit, &candidate, by) {
        let again = match start(unit, &old.argv, by) {
            Ok(()) => format!("{unit} runs {} again", old.commit),
            Err(e) => format!("nor would {} start again: {e}", old.commit),
        };
        return Ended::Failed(format!("{commit} would not start ({why}); {again}."));
    }
    let pid = match open(unit, &old, &build, since) {
        Ok(pid) => pid,
        Err(why) => {
            return Ended::Failed(format!(
                "{commit} did not open: {why}\n{}",
                restarted(unit, &old)
            ));
        }
    };
    say(&format!(
        "{commit} opened; watching it for {}s",
        WINDOW.as_secs()
    ));
    if let Some(why) = died(unit, pid, since) {
        say(&format!("{commit} {why}; going back to {}", old.commit));
        return Ended::WentBack(format!(
            "{commit} opened, then {why}.\n{}",
            back(unit, &old, by)
        ));
    }
    Ended::Opened(format!(
        "{unit} runs {commit}, which took over from {} and ran for {}s after opening; every \
         session carries on, on it.{one_way}",
        old.commit,
        WINDOW.as_secs()
    ))
}

/// Everything before the running brain is stopped: the refusals, the
/// install and the rehearsal. The lock held until the relaunch ends, the
/// running brain, the new build's directory, and what the result says of a
/// one-way relaunch it was asked to make.
fn prepare(
    unit: &str,
    clone: &Path,
    asked: Asked,
    say: &dyn Fn(&str),
) -> Result<(File, Running, PathBuf, String), String> {
    let old = Running::of(unit)?;
    let lock = lock(&old.data().join("relaunch.lock"))?
        .ok_or_else(|| format!("a relaunch of {unit} is under way; wait for it to end"))?;
    let commit = clean_commit(clone)?;
    if !asked.back && !descends(clone, &old.commit, &commit)? {
        return Err(format!(
            "{commit} does not descend from {}, which {unit} runs: relaunching would undo commits. \
             Merge it first, or say `back` to go back on purpose",
            old.commit
        ));
    }
    let build = install(clone, say)?;
    let help = run(Command::new(build.join("bin/harness-brain")).arg("--help"))?;
    if !help.contains(Start::FLAG) {
        return Err(format!(
            "{commit}'s harness-brain does not know `{}`, so it would open with no gate",
            Start::FLAG
        ));
    }
    let skipped = rehearse(&build, &old, say)?;
    let one_way = match skipped.split_first() {
        None => String::new(),
        Some((first, rest)) => {
            let unread = match rest.len() {
                0 => format!("the record at {first}"),
                more => format!("the record at {first}, and {more} more"),
            };
            if !asked.one_way {
                return Err(format!(
                    "this relaunch would be one-way: {} could not read what {commit} wrote in the \
                     rehearsal ({unread}), so going back after {commit} opens would skip those \
                     records and may run a call a second time. Call `relaunch` again with \
                     `one_way: true` to go ahead",
                    old.commit
                ));
            }
            format!(
                " It is one-way, as asked: {} could not read what {commit} wrote in the \
                 rehearsal ({unread}), so a way back would skip such records and may run a call \
                 a second time.",
                old.commit
            )
        }
    };
    Ok((lock, old, build, one_way))
}

/// Prove the new build beside the running brain without touching it, in
/// `NAME/rehearsal/`: what the running build could not read of what the
/// new one wrote, or why it failed, as `{pathway}: {why}` in the words of
/// the program that failed. The directory is deleted once the build
/// passes; when it fails it is kept, with the logs its words name, until
/// the next rehearsal.
fn rehearse(build: &Path, running: &Running, say: &dyn Fn(&str)) -> Result<Vec<String>, String> {
    let dir = running.data().join("rehearsal");
    remove(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    say(&format!(
        "rehearsing {} beside {}",
        name(build),
        running.unit
    ));
    let done = rehearse_in(&dir, build, running);
    if done.is_ok() {
        remove(&dir);
    }
    done
}

fn rehearse_in(dir: &Path, build: &Path, running: &Running) -> Result<Vec<String>, String> {
    let commit = name(build);
    let login: Vec<&str> = running
        .login()
        .map(|l| vec!["--login", l])
        .unwrap_or_default();
    // The gate, on a copy of the live database.
    let copy = dir.join("copy.sqlite");
    Store::open(&running.db)
        .and_then(|store| store.copy_to(&copy))
        .map_err(|e| format!("copying {}: {e}", running.db.display()))?;
    let said = check(dir, build, &copy, &login)?;
    // Both builds read the login alike.
    let theirs = said.lines().find_map(|l| l.strip_prefix("signed in: "));
    let ours = match running.login() {
        Some(db) => Ok(Login::at(PathBuf::from(db))),
        None => Login::home(),
    }
    .and_then(|login| login.signed_in())
    .map(|signed_in| if signed_in { "yes" } else { "no" })
    .map_err(|e| {
        format!(
            "{}: {} reading the login: {e}",
            Pathway::SignIn,
            running.commit
        )
    })?;
    if theirs != Some(ours) {
        return Err(format!(
            "{}: {} reads the login as signed in: {ours}, and {commit} as {}",
            Pathway::SignIn,
            running.commit,
            theirs.map_or("nothing: it printed no `signed in:` line".into(), |t| {
                format!("signed in: {t}")
            })
        ));
    }
    // A round trip through each build's limb, and the readback of what
    // each scratch brain wrote.
    let mut skipped = Vec::new();
    for (through, limbs) in [
        (Through::Running, running.build.join("bin")),
        (Through::Own, build.join("bin")),
    ] {
        let pathway = Pathway::RoundTrip(through);
        let at = dir.join(match through {
            Through::Running => "through-running",
            Through::Own => "through-own",
        });
        let mark = format!("HARNESS_OWNER_rehearsal_{}", std::process::id());
        let done = round_trip(&at, build, &limbs, &login, &mark);
        end(&mark);
        done.map_err(|why| format!("{pathway}: {commit}'s scratch brain {why}"))?;
        skipped.extend(readback(&at.join("scratch.sqlite"))?);
    }
    Ok(skipped)
}

/// Run the gate of `build` on `copy`, then load the page it serves
/// read-only in headless Chrome, and stop it: what it printed, or why it
/// failed. Its stderr is kept in `dir`.
fn check(dir: &Path, build: &Path, copy: &Path, login: &[&str]) -> Result<String, String> {
    let commit = name(build);
    let errors = dir.join("check.log");
    let mut child = Command::new(build.join("bin/harness-brain"))
        .arg("--check")
        .arg(copy)
        .args(login)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(File::create(&errors).map_err(|e| format!("{}: {e}", errors.display()))?)
        .spawn()
        .map_err(|e| format!("{commit}'s harness-brain --check: {e}"))?;
    let mut said = String::new();
    let mut passed = false;
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            said.push_str(&line);
            said.push('\n');
            if line.starts_with("gate: passed;") {
                passed = true;
                break;
            }
        }
    }
    if !passed {
        let _ = child.wait();
        let why = std::fs::read_to_string(&errors).unwrap_or_default();
        return Err(format!(
            "{commit} failed its gate on a copy of the database:\n{}",
            why.trim()
        ));
    }
    // The port is the face's line's last word.
    let port = said
        .lines()
        .find_map(|l| l.strip_prefix("face: "))
        .and_then(|url| url.rsplit(':').next())
        .unwrap_or_default()
        .to_owned();
    let page = harness_limb_browser::uncaught(
        &format!("https://localhost:{port}/"),
        &dir.join("chrome"),
        PAGE_SETTLES,
    );
    // SIGTERM, which a checking brain ends at.
    unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGTERM) };
    let _ = child.wait();
    let pathway = Pathway::Page;
    match page {
        Ok(thrown) if thrown.is_empty() => Ok(said),
        Ok(thrown) => Err(format!(
            "{pathway}: {commit}'s page threw: {}",
            thrown.join("; ")
        )),
        Err(why) => Err(format!("{pathway}: {why}")),
    }
}

/// What this build, the running one, could not read of the database at
/// `db`: each record it skips, by its position and why.
fn readback(db: &Path) -> Result<Vec<String>, String> {
    let store = Store::read(db).map_err(|e| format!("{}: {e}", db.display()))?;
    let (_, divergences) = schema::Brain::open(store.replica(), store)
        .map_err(|e| format!("reading back {}: {e}", db.display()))?;
    Ok(divergences
        .iter()
        .map(|d| format!("{}: {}", d.at, d.why))
        .collect())
}

/// Serve a scratch brain of `build` on a new database in `dir`, starting
/// the `harness-limb` in `limbs`, every process of it carrying `mark`: its
/// root spawns a child, whose `bash` writes a token to a file, and the
/// root's next call copies it. Then this build checks the copy. Why not,
/// as the brain's own words end.
fn round_trip(
    dir: &Path,
    build: &Path,
    limbs: &Path,
    login: &[&str],
    mark: &str,
) -> Result<(), String> {
    let ws = dir.join("ws");
    std::fs::create_dir_all(&ws).map_err(|e| format!("{}: {e}", ws.display()))?;
    let log_path = dir.join("scratch.log");
    let log = File::create(&log_path).map_err(|e| format!("{}: {e}", log_path.display()))?;
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map_err(|e| format!("found no free port: {e}"))?
        .port()
        .to_string();
    let mut brain = Command::new(build.join("bin/harness-brain"))
        .arg("--db")
        .arg(dir.join("scratch.sqlite"))
        .args(["--port", &port, "--model", "scripted", "--roles"])
        .arg(build.join("roles"))
        .arg("--limbs")
        .arg(limbs)
        .args(login)
        .env(mark, "1")
        .env("XDG_STATE_HOME", dir.join("state"))
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log)
        .spawn()
        .map_err(|e| format!("would not start: {e}"))?;
    let said = || std::fs::read_to_string(&log_path).unwrap_or_default();
    let mut ended = |what: &str| {
        let lines: Vec<String> = said().lines().map(String::from).collect();
        let tail = lines[lines.len().saturating_sub(5)..].join("\n");
        match brain.try_wait() {
            Ok(Some(status)) => Some(format!("exited ({status}) {what}, saying:\n{tail}")),
            _ => None,
        }
    };
    let until = Instant::now() + ROUND_TRIP;
    while !said().contains("face: ") {
        if let Some(why) = ended("before it served") {
            return Err(why);
        }
        if Instant::now() > until {
            return Err(format!("did not serve in {}s", ROUND_TRIP.as_secs()));
        }
        thread::sleep(EVERY);
    }
    let token = format!("{mark}_{}", harness_limb::now().0);
    let child = json!({ "command": format!("printf %s {token} > token") });
    let spawn = json!({ "tasks": [{
        "name": "child",
        "mandate": format!("tool bash {child}\nsay done"),
    }]});
    let copy = json!({ "command": "cp token copy" });
    // The brain serves before it has read its role files, so a chat is
    // refused until it has.
    let until = Instant::now() + ROUND_TRIP;
    loop {
        let chat = Command::new(build.join("bin/act"))
            .args([&port, "chat", "engineer"])
            .arg(&ws)
            .arg(format!("tool spawn {spawn}\ntool bash {copy}\nsay done"))
            .env(mark, "1")
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("could not be acted on: {e}"))?;
        if chat.status.success() {
            break;
        }
        let refused = String::from_utf8_lossy(&chat.stdout);
        if !refused.contains("no role engineer") || Instant::now() > until {
            return Err(format!("refused a chat: {}", refused.trim()));
        }
        thread::sleep(EVERY);
    }
    let until = Instant::now() + ROUND_TRIP;
    loop {
        if std::fs::read_to_string(ws.join("copy")).is_ok_and(|c| c == token) {
            return Ok(());
        }
        if let Some(why) = ended("during its round trip") {
            return Err(why);
        }
        if Instant::now() > until {
            return Err(format!(
                "left no copy of its child's token in {}s; its log: {}",
                ROUND_TRIP.as_secs(),
                log_path.display()
            ));
        }
        thread::sleep(EVERY);
    }
}

/// End every process carrying `mark`: SIGTERM, and SIGKILL to what is
/// left after 10 s.
fn end(mark: &str) {
    let marked = || -> Vec<i32> {
        let procs = std::fs::read_dir("/proc").into_iter().flatten().flatten();
        procs
            .filter_map(|e| {
                let pid = e.file_name().to_str()?.parse().ok()?;
                let environ = std::fs::read(e.path().join("environ")).ok()?;
                let wanted = format!("{mark}=1");
                environ
                    .split(|b| *b == 0)
                    .any(|v| v == wanted.as_bytes())
                    .then_some(pid)
            })
            .collect()
    };
    let signal = |sig| {
        marked().into_iter().for_each(|pid| {
            // SAFETY: kill takes plain integers.
            unsafe { libc::kill(pid, sig) };
        })
    };
    signal(libc::SIGTERM);
    let until = Instant::now() + Duration::from_secs(10);
    while !marked().is_empty() && Instant::now() < until {
        thread::sleep(Duration::from_millis(200));
    }
    signal(libc::SIGKILL);
}

/// Wait for the candidate `unit` runs from `build` to open: the installed
/// link names it. Its process, or why it did not open: its process ended
/// (in its own words), or [`OPENING`] passed and it was killed.
fn open(unit: &str, old: &Running, build: &Path, since: SystemTime) -> Result<u32, String> {
    let link = old.data().join("build");
    let opened = || std::fs::canonicalize(&link).is_ok_and(|to| to == build);
    let pid = started(unit, &build.join("bin/harness-brain"));
    let until = Instant::now() + OPENING;
    loop {
        if opened() {
            return Ok(pid);
        }
        let gone = pid == 0 || main_pid(unit).is_ok_and(|now| now != Some(pid));
        if gone && !opened() {
            return Err(format!("its process ended{}", last_words(unit, since)));
        }
        if Instant::now() > until {
            let _ = systemctl(&["kill", "--kill-whom=main", "-s", "SIGKILL", unit]);
            return Err(format!(
                "it had not opened in {}s, so it was killed{}",
                OPENING.as_secs(),
                last_words(unit, since)
            ));
        }
        thread::sleep(EVERY);
    }
}

/// Once a candidate has ended unopened, the unit's restart runs the
/// installed link, still the build before: whether it did.
fn restarted(unit: &str, old: &Running) -> String {
    let exe = old.build.join("bin/harness-brain");
    match started(unit, &exe) {
        0 => format!(
            "{} has not started again within 30s: see `systemctl --user status {unit}`.",
            old.commit
        ),
        _ => format!(
            "{unit} runs {} again, on the bytes it left: the candidate wrote nothing.",
            old.commit
        ),
    }
}

/// Watch the opened build's process `pid` for [`WINDOW`]: why it ended,
/// if it did.
fn died(unit: &str, pid: u32, since: SystemTime) -> Option<String> {
    let until = Instant::now() + WINDOW;
    while Instant::now() < until {
        thread::sleep(EVERY);
        if main_pid(unit).is_ok_and(|now| now != Some(pid)) {
            return Some(format!("its process ended{}", last_words(unit, since)));
        }
    }
    None
}

/// Go back to the build before: end the new build's brain without its
/// drain (it is not trusted), point the installed link at the old build,
/// and start it, as an installed start, which is never refused: it folds
/// what it can of what the new build wrote, and skips and logs the rest.
/// What it says is how it went.
fn back(unit: &str, old: &Running, by: Option<CallId>) -> String {
    let _ = systemctl(&["kill", "--kill-whom=main", "-s", "SIGKILL", unit]);
    let _ = systemctl(&["stop", unit]);
    let relinked = link(&old.data().join("build"), &old.build);
    match relinked.and_then(|()| start(unit, &old.argv, by)) {
        Ok(()) => format!(
            "{unit} runs {} again; it skips any record it cannot read of what the new build \
             wrote, logging each, and may run a call a second time.",
            old.commit
        ),
        Err(e) => format!("{} would not start again: {e}", old.commit),
    }
}

/// The value of `name` on a command line.
fn flag<'a>(argv: &'a [String], name: &str) -> Option<&'a str> {
    argv.iter()
        .skip_while(|a| *a != name)
        .nth(1)
        .map(String::as_str)
}

/// The commit `clone` has checked out, refusing a tree with changes: it
/// could not be built again.
fn clean_commit(clone: &Path) -> Result<String, String> {
    let status = git(clone, &["status", "--porcelain", "--untracked-files=no"])?;
    if !status.is_empty() {
        return Err(format!(
            "{} has uncommitted changes; commit them, or stash them",
            clone.display()
        ));
    }
    git(clone, &["rev-parse", "--short=12", "HEAD"])
}

fn descends(clone: &Path, ancestor: &str, commit: &str) -> Result<bool, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(clone)
        .args(["merge-base", "--is-ancestor", ancestor, commit])
        .output()
        .map_err(|e| format!("git: {e}"))?;
    match out.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(format!(
            "{} cannot tell whether {commit} descends from {ancestor}: {}",
            clone.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        )),
    }
}

/// Build `clone`'s commit and install it, once: its executables and
/// `roles/`, read-only. Then delete the builds nothing uses. Its directory.
pub fn install(clone: &Path, say: &dyn Fn(&str)) -> Result<PathBuf, String> {
    let commit = clean_commit(clone)?;
    let builds = home().join("builds");
    let build = builds.join(&commit);
    if build.is_dir() {
        return Ok(build);
    }
    say(&format!("building {commit}"));
    let staging = builds.join(format!(".{commit}.{}", std::process::id()));
    let made = stage(clone, &commit, &staging);
    match made {
        // Another install may have made it meanwhile: the first one stands.
        Ok(()) if std::fs::rename(&staging, &build).is_ok() => {}
        _ => remove(&staging),
    }
    made?;
    prune(&builds, say);
    Ok(build)
}

fn stage(clone: &Path, commit: &str, staging: &Path) -> Result<(), String> {
    let bin = staging.join("bin");
    std::fs::create_dir_all(&bin).map_err(|e| format!("{}: {e}", bin.display()))?;
    let mut cargo = Command::new("cargo")
        .current_dir(clone)
        .args([
            "build",
            "--release",
            "--workspace",
            "--bins",
            "--example",
            "act",
        ])
        .arg("--message-format=json-render-diagnostics")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cargo: {e}"))?;
    let (Some(stderr), Some(stdout)) = (cargo.stderr.take(), cargo.stdout.take()) else {
        return Err("cargo's output was not piped".into());
    };
    let errors = thread::spawn(move || {
        let lines: Vec<String> = BufReader::new(stderr)
            .lines()
            .map_while(Result::ok)
            .collect();
        lines[lines.len().saturating_sub(20)..].join("\n")
    });
    let mut executables = Vec::new();
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        let Ok(message) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if message["reason"] == "compiler-artifact"
            && let Some(exe) = message["executable"].as_str()
        {
            executables.push(PathBuf::from(exe));
        }
    }
    let status = cargo.wait().map_err(|e| format!("cargo: {e}"))?;
    let errors = errors.join().unwrap_or_default();
    if !status.success() {
        return Err(format!("{commit} does not build:\n{errors}"));
    }
    for exe in executables {
        let to = bin.join(exe.file_name().unwrap_or_default());
        std::fs::copy(&exe, &to).map_err(|e| format!("{}: {e}", exe.display()))?;
    }
    let tar = Command::new("git")
        .arg("-C")
        .arg(clone)
        .args(["archive", "HEAD", "roles"])
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git archive: {e}"))?;
    let untar = Command::new("tar")
        .arg("-x")
        .arg("-C")
        .arg(staging)
        .stdin(tar.stdout.ok_or("git archive's output was not piped")?)
        .status()
        .map_err(|e| format!("tar: {e}"))?;
    if !untar.success() {
        return Err(format!("{commit}'s roles/ would not unpack"));
    }
    let built = run(Command::new(bin.join("harness-brain")).arg("--version"))?;
    if built != format!("harness-brain {commit}") {
        // Cargo judges freshness by paths relative to the package and by
        // mtimes, so a target directory another clone built into last can
        // look fresh here and hold that clone's binary.
        return Err(format!(
            "the build says it is {built}, not {commit}: does another clone share {}'s target directory?",
            clone.display()
        ));
    }
    run(Command::new("chmod").arg("-R").arg("a-w").arg(staging)).map(|_| ())
}

/// Delete the builds nothing uses: none a `NAME/build` links to, none a
/// running process (a brain, or a limb that outlived its brain) runs from,
/// and not the three newest of the rest.
fn prune(builds: &Path, say: &dyn Fn(&str)) {
    let mut used: Vec<PathBuf> = Vec::new();
    let links = std::fs::read_dir(home()).into_iter().flatten().flatten();
    used.extend(links.filter_map(|e| std::fs::canonicalize(e.path().join("build")).ok()));
    let procs = std::fs::read_dir("/proc").into_iter().flatten().flatten();
    used.extend(procs.filter_map(|e| std::fs::read_link(e.path().join("exe")).ok()));
    let mut unused: Vec<(SystemTime, PathBuf)> = std::fs::read_dir(builds)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| !name(p).starts_with('.') && !used.iter().any(|u| u.starts_with(p)))
        .filter_map(|p| Some((p.metadata().ok()?.modified().ok()?, p)))
        .collect();
    unused.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
    for (_, old) in unused.into_iter().skip(3) {
        remove(&old);
        say(&format!("pruned {}", name(&old)));
    }
}

fn remove(dir: &Path) {
    if dir.exists() {
        let _ = Command::new("chmod").arg("-R").arg("u+w").arg(dir).status();
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Start `unit` running `argv`, an installed build's brain, telling it the
/// `relaunch` call `by` that started it, if one did.
///
/// The unit is durable: `$XDG_CONFIG_HOME/systemd/user/UNIT`, enabled for
/// `default.target`, so it starts at boot (with lingering, before a login)
/// and again after any exit systemd did not ask for: a crash, or a signal
/// from anything but `systemctl stop`, which a relaunch uses. Its command
/// line runs the build `NAME/build` links to, the last to open the
/// database, as an installed start (no [`Start::FLAG`]), so it is the same
/// file whatever build `argv` names. This one start runs `argv` itself, with `by` in its environment, through a drop-in in
/// `$XDG_RUNTIME_DIR` that is deleted once it has started: a restart or a
/// reboot runs the link again.
///
/// Its stop sends SIGTERM to the brain alone, which ends its links to
/// limbs itself (the limbs run in units of their own), and gives it 330 s
/// to drain.
pub fn start(unit: &str, argv: &[String], by: Option<CallId>) -> Result<(), String> {
    let build = Path::new(argv.first().ok_or("nothing to start")?)
        .parent()
        .and_then(Path::parent)
        .ok_or("the command line names no installed build")?;
    let data = argv
        .iter()
        .skip_while(|a| *a != "--db")
        .nth(1)
        .and_then(|db| Path::new(db).parent())
        .ok_or("the command line names no --db")?;
    let linked: Vec<String> = argv
        .iter()
        .filter(|a| *a != Start::FLAG)
        .map(|a| {
            rebase(
                a,
                &build.to_string_lossy(),
                &data.join("build").to_string_lossy(),
            )
        })
        .collect();
    let name = unit
        .trim_start_matches("harness-")
        .trim_end_matches(".service");
    write(
        &config().join("systemd/user").join(unit),
        &format!(
            "[Unit]\nDescription=agent-harness brain {name}\n\n\
             [Service]\nWorkingDirectory={}\nExecStart={}\n\
             KillMode=mixed\nTimeoutStopSec=330\nRestart=always\nRestartSec=5\n\n\
             [Install]\nWantedBy=default.target\n",
            data.to_string_lossy().replace('%', "%%"),
            exec_line(&linked),
        ),
    )?;
    let once = runtime().join(format!("systemd/user/{unit}.d/start.conf"));
    write(
        &once,
        &format!(
            "[Service]\nExecStart=\nExecStart={}\n{}",
            exec_line(argv),
            by.map(|call| format!(
                "Environment={}\n",
                exec_quote(&format!("{}={call}", crate::RELAUNCHED_BY))
            ))
            .unwrap_or_default()
        ),
    )?;
    let started = systemctl(&["daemon-reload"])
        .and_then(|_| systemctl(&["enable", "--quiet", unit]))
        .and_then(|_| {
            let _ = systemctl(&["reset-failed", unit]);
            systemctl(&["start", unit])
        });
    let _ = std::fs::remove_file(&once);
    let _ = once.parent().map(std::fs::remove_dir);
    systemctl(&["daemon-reload"])?;
    started.map(|_| ())
}

/// `arg` with the leading directory `from` replaced by `to`.
fn rebase(arg: &str, from: &str, to: &str) -> String {
    match arg.strip_prefix(from) {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("{to}{rest}"),
        _ => arg.to_string(),
    }
}

/// A command line as systemd reads `ExecStart=`.
fn exec_line(argv: &[String]) -> String {
    argv.iter()
        .map(|a| exec_quote(a))
        .collect::<Vec<_>>()
        .join(" ")
}

/// One word, quoted for a unit file: no specifier, variable or escape in
/// it is expanded.
fn exec_quote(word: &str) -> String {
    let escaped = word
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%")
        .replace('$', "$$");
    format!("\"{escaped}\"")
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    path.parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(path, text))
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn config() -> PathBuf {
    match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"),
    }
}

fn runtime() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        // SAFETY: getuid has no preconditions.
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", unsafe { libc::getuid() })))
}

/// Stop `unit`: SIGTERM, and the brain drains.
fn stop(unit: &str) -> Result<(), String> {
    systemctl(&["stop", unit]).map(|_| ())
}

/// The process `unit` runs once it has become `exe`, or 0 (no process)
/// if it does not within 30 s.
fn started(unit: &str, exe: &Path) -> u32 {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if let Ok(Some(pid)) = main_pid(unit)
            && std::fs::read_link(format!("/proc/{pid}/exe")).is_ok_and(|e| e == exe)
        {
            return pid;
        }
        thread::sleep(Duration::from_millis(100));
    }
    0
}

fn main_pid(unit: &str) -> Result<Option<u32>, String> {
    let pid = systemctl(&["show", "-p", "MainPID", "--value", unit])?;
    Ok(pid.parse().ok().filter(|p| *p != 0))
}

fn systemctl(args: &[&str]) -> Result<String, String> {
    run(Command::new("systemctl").arg("--user").args(args))
}

/// Why the unit's process ended since `since`, after a colon: its `gate:`
/// lines, a panic if it printed one, and systemd's word on how it exited.
fn last_words(unit: &str, since: SystemTime) -> String {
    // The journal has its last lines a moment after it ends.
    thread::sleep(Duration::from_secs(1));
    let secs = (since.duration_since(UNIX_EPOCH)).map_or(0, |d| d.as_secs());
    let out = Command::new("journalctl")
        .args(["--user", "-u", unit, "--no-pager", "-o", "cat"])
        .arg(format!("--since=@{secs}"))
        .output();
    let text = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    let said: Vec<&str> = text
        .as_deref()
        .unwrap_or_default()
        .lines()
        .filter(|l| {
            l.contains("gate: ") || l.contains("panicked") || l.contains("Main process exited")
        })
        .map(|l| l.trim_start_matches(&format!("{unit}: ")).trim())
        .collect();
    match said.is_empty() {
        true => String::new(),
        false => format!(", saying:\n{}", said.join("\n")),
    }
}

/// Point `at` to `to`, in one step.
fn link(at: &Path, to: &Path) -> Result<(), String> {
    let staging = at.with_extension("new");
    let _ = std::fs::remove_file(&staging);
    std::os::unix::fs::symlink(to, &staging)
        .and_then(|()| std::fs::rename(&staging, at))
        .map_err(|e| format!("{}: {e}", at.display()))
}

/// The lock at `path`, if no one else holds it.
fn lock(path: &Path) -> Result<Option<File>, String> {
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    // SAFETY: flock on a descriptor this function owns.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        return Ok(Some(file));
    }
    Ok(None)
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    run(Command::new("git").arg("-C").arg(dir).args(args))
}

/// Run a command to its end: what it printed, or why it failed.
fn run(command: &mut Command) -> Result<String, String> {
    let out = command
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("{:?}: {e}", command.get_program()))?;
    if !out.status.success() {
        return Err(format!(
            "{:?} failed: {}",
            command.get_program(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}
