//! Relaunching a brain's service onto the commit of a clone, and the watch
//! that goes back when the new build fails: the procedure the `relaunch`
//! tool and the command line share.
//!
//! A brain runs as the durable systemd user unit `harness-NAME.service`
//! (see [`start`]), from an installed build:
//! `$XDG_DATA_HOME/agent-harness/builds/COMMIT/`, its executables and
//! `roles/`, read-only and never overwritten. What the unit runs is read
//! off its process: its command line names the build's directory, the
//! database (`--db`), the port and its flags, and a relaunch starts the
//! same command line in another build's directory. `NAME/build` beside
//! the database links to the build a restart or a reboot runs: the last
//! one proven.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use harness_core::build::Verdict;
use harness_core::id::{CallId, Seq};
use harness_core::schema;
use harness_store::Store;

/// How long a new build gets to state its launch.
const STARTING: Duration = Duration::from_secs(120);
/// How long a request of the new build's span may go without a first
/// byte before the build is taken to hang.
const UNANSWERED: Duration = Duration::from_secs(5 * 60);
/// How often the watch folds the database.
const EVERY: Duration = Duration::from_secs(2);

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
    pid: u32,
    /// Its command line, the executable first.
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
        let db = argv
            .iter()
            .skip_while(|a| *a != "--db")
            .nth(1)
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
            pid,
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
}

/// A relaunch that has taken over: the new build serves, on probation, and
/// no other relaunch of the unit can start until its watch ends.
pub struct Takeover {
    _lock: File,
    pub old: Running,
    pub new: Running,
    /// The brain's last launch before the new one.
    before: Option<Seq>,
    /// The `relaunch` call that started it, which the way back names too.
    by: Option<CallId>,
    started: Instant,
}

/// How a watch ended.
pub enum Ended {
    Proven,
    /// It went back to the old build, for this reason.
    Back(String),
}

/// Relaunch `unit` onto the commit `clone` has checked out: refuse a tree
/// with changes, a commit that does not descend from the running one
/// unless `back`, and a second relaunch; build and install it; rehearse it;
/// stop the running brain (it drains) and start the new build on the same
/// database. What it returns as an error is why nothing changed, unless
/// it says otherwise.
pub fn take_over(
    unit: &str,
    clone: &Path,
    back: bool,
    by: Option<CallId>,
    say: &dyn Fn(&str),
) -> Result<Takeover, String> {
    let old = Running::of(unit)?;
    let lock = lock(&old.data().join("relaunch.lock"))?
        .ok_or_else(|| format!("a relaunch of {unit} is under way; wait for it to end"))?;
    let commit = clean_commit(clone)?;
    if !back && !descends(clone, &old.commit, &commit)? {
        return Err(format!(
            "{commit} does not descend from {}, which {unit} runs: relaunching would undo commits. \
             Merge it first, or say `back` to go back on purpose",
            old.commit
        ));
    }
    let build = install(clone, say)?;
    rehearse(&build, &old, say).map_err(|why| {
        format!(
            "{why}\n{unit} runs {} still: it was never stopped.",
            old.commit
        )
    })?;
    let before = fold_span(&old.db)?.0;
    say(&format!(
        "stopping {unit} on {}: its requests in flight drain first",
        old.commit
    ));
    stop(unit)?;
    say(&format!("starting {unit} on {commit}"));
    if let Err(why) = start(unit, &old.on(&build), by) {
        return Err(match start(unit, &old.argv, by) {
            Ok(()) => format!(
                "{commit} would not start ({why}), so {unit} runs {} again",
                old.commit
            ),
            Err(again) => format!(
                "{commit} would not start ({why}), nor {} again: {again}",
                old.commit
            ),
        });
    }
    let argv = old.on(&build);
    let new = Running {
        pid: started(unit, Path::new(&argv[0])),
        argv,
        build,
        commit,
        ..old.clone()
    };
    Ok(Takeover {
        _lock: lock,
        old,
        new,
        before,
        by,
        started: Instant::now(),
    })
}

/// Prove the new build beside the running brain without touching it, in
/// `NAME/rehearsal/`: why not, in the words of the program that failed.
/// The directory is deleted once the build passes; when it fails it is kept,
/// with the logs its words name, until the next rehearsal.
fn rehearse(build: &Path, running: &Running, say: &dyn Fn(&str)) -> Result<(), String> {
    let dir = running.data().join("rehearsal");
    remove(&dir);
    std::fs::create_dir_all(dir.join("ws")).map_err(|e| format!("{}: {e}", dir.display()))?;
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

fn rehearse_in(dir: &Path, build: &Path, running: &Running) -> Result<(), String> {
    let commit = name(build);
    // The new build reads a copy of the live database as if it took over:
    // it folds every record, has the provider count each live session's
    // next request, and streams one short response.
    let copy = dir.join("copy.sqlite");
    Store::open(&running.db)
        .and_then(|store| store.copy_to(&copy))
        .map_err(|e| format!("copying {}: {e}", running.db.display()))?;
    let out = Command::new(build.join("bin/harness-brain"))
        .arg("--check")
        .arg(&copy)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("{commit}'s harness-brain --check: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{commit} failed its check on a copy of the database: {}{}",
            String::from_utf8_lossy(&out.stderr).trim(),
            failures(&String::from_utf8_lossy(&out.stdout)),
        ));
    }
    // The way back: this build, the old one, reads what the new one left.
    fold_span(&copy).map_err(|e| {
        format!(
            "{}, which {} runs, could not read the copy once {commit} had: {e}",
            running.commit, running.unit
        )
    })?;
    // A scratch brain of the new build, on the scripted model, makes a
    // round trip through a limb of the new build.
    let mark = format!("HARNESS_OWNER_rehearsal_{}", std::process::id());
    let done = scratch(dir, build, &mark);
    end(&mark);
    done.map_err(|why| format!("{commit}'s scratch brain {why}"))
}

/// What a `--check` report says failed, one line each. Of the records it
/// could not fold, only the first, in fold order: the rest may follow from
/// it.
fn failures(report: &str) -> String {
    let Ok(report) = serde_json::from_str::<serde_json::Value>(report) else {
        return format!("\n{}", report.trim());
    };
    let skipped = report["skipped"].as_array().map_or(&[][..], Vec::as_slice);
    let sessions = report["sessions"].as_array().into_iter().flatten();
    let said: Vec<String> = (skipped.first())
        .map(|s| match skipped.len() - 1 {
            0 => format!("\nit could not fold a record: {s}"),
            more => format!("\nit could not fold a record: {s}, nor {more} after it"),
        })
        .into_iter()
        .chain(
            sessions
                .filter(|s| s.get("tokens").is_none())
                .map(|s| format!("\na session's next request: {s}")),
        )
        .chain(
            report["ping"]
                .get("failed")
                .map(|f| format!("\nits streamed request: {f}")),
        )
        .collect();
    said.concat()
}

/// Serve a scratch brain of `build` on a new database in `dir`, every
/// process of it carrying `mark`, and have one chat make a tool call
/// through a limb: why not, as the brain's own words end.
fn scratch(dir: &Path, build: &Path, mark: &str) -> Result<(), String> {
    const WITHIN: Duration = Duration::from_secs(90);
    let db = dir.join("scratch.sqlite");
    let log_path = dir.join("scratch.log");
    let log = File::create(&log_path).map_err(|e| format!("{}: {e}", log_path.display()))?;
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map_err(|e| format!("found no free port: {e}"))?
        .port()
        .to_string();
    let mut brain = Command::new(build.join("bin/harness-brain"))
        .arg("--db")
        .arg(&db)
        .args(["--port", &port, "--model", "scripted", "--roles"])
        .arg(build.join("roles"))
        .env(mark, "1")
        .env("XDG_STATE_HOME", dir.join("state"))
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log)
        .spawn()
        .map_err(|e| format!("would not start: {e}"))?;
    let said = || std::fs::read_to_string(&log_path).unwrap_or_default();
    let ended = |brain: &mut std::process::Child, what: &str| {
        let lines: Vec<String> = said().lines().map(String::from).collect();
        let tail = lines[lines.len().saturating_sub(5)..].join("\n");
        match brain.try_wait() {
            Ok(Some(status)) => Some(format!("exited ({status}) {what}, saying:\n{tail}")),
            _ => None,
        }
    };
    let until = Instant::now() + WITHIN;
    while !said().contains("face: ") {
        if let Some(why) = ended(&mut brain, "before it served") {
            return Err(why);
        }
        if Instant::now() > until {
            return Err(format!("did not serve in {}s", WITHIN.as_secs()));
        }
        thread::sleep(Duration::from_millis(200));
    }
    let chat = Command::new(build.join("bin/act"))
        .args([&port, "chat", "engineer"])
        .arg(dir.join("ws"))
        .arg("tool bash {\"command\":\"echo a round trip\"}\nsay done")
        .env(mark, "1")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("could not be acted on: {e}"))?;
    let chatted = String::from_utf8_lossy(&chat.stdout);
    if !chat.status.success() {
        return Err(format!("refused a chat: {}", chatted.trim()));
    }
    loop {
        thread::sleep(Duration::from_millis(500));
        if let Some(why) = ended(&mut brain, "during a scripted round trip") {
            return Err(why);
        }
        match fold_span(&db)?.1 {
            Verdict::Proven { .. } => return Ok(()),
            Verdict::Refused(r) => {
                return Err(format!(
                    "had its request refused with {}: {}",
                    r.status, r.message
                ));
            }
            Verdict::Proving { .. } if Instant::now() > until => {
                return Err(format!(
                    "made no scripted round trip through a limb in {}s; its log: {}",
                    WITHIN.as_secs(),
                    log_path.display()
                ));
            }
            Verdict::Proving { .. } => {}
        }
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

impl Takeover {
    /// Watch the new build until it is proven, then move the installed link
    /// to it; or until it fails, then start the old build's bytes again.
    /// The judge is this process's own code, the old build's.
    pub fn watch(self) -> Ended {
        loop {
            thread::sleep(EVERY);
            match self.check() {
                None => continue,
                Some(Ok(())) => {
                    return match link(&self.new.data().join("build"), &self.new.build) {
                        Ok(()) => Ended::Proven,
                        Err(e) => Ended::Back(format!("it was proven, but its link: {e}")),
                    };
                }
                Some(Err(why)) => return Ended::Back(self.back(why)),
            }
        }
    }

    fn check(&self) -> Option<Result<(), String>> {
        let unit = &self.new.unit;
        match main_pid(unit) {
            Ok(Some(pid)) if pid == self.new.pid => {}
            _ => {
                return Some(Err(format!(
                    "its brain exited{}",
                    last_words(unit, self.started)
                )));
            }
        }
        let (launch, verdict, unanswered) = match fold_span(&self.new.db) {
            Ok(folded) => folded,
            Err(e) => return Some(Err(e)),
        };
        if launch <= self.before {
            return (self.started.elapsed() > STARTING).then(|| {
                Err(format!(
                    "its brain did not launch in {}s",
                    STARTING.as_secs()
                ))
            });
        }
        match verdict {
            Verdict::Proven { .. } => Some(Ok(())),
            Verdict::Refused(r) => Some(Err(format!(
                "the provider refused its request {} with {}: {}",
                r.request, r.status, r.message
            ))),
            Verdict::Proving { .. } if unanswered => Some(Err(format!(
                "a request it sent had no answer in {}s",
                UNANSWERED.as_secs()
            ))),
            Verdict::Proving { .. } => None,
        }
    }

    /// End the new build's brain without its drain (it is not trusted) and
    /// start the old one's bytes again. What it says is why, and how.
    fn back(&self, why: String) -> String {
        let unit = &self.new.unit;
        let _ = systemctl(&["kill", "-s", "SIGKILL", unit]);
        let _ = systemctl(&["stop", unit]);
        let how = match start(unit, &self.old.argv, self.by) {
            Ok(()) => format!("{unit} runs {} again", self.old.commit),
            Err(e) => format!("and {} would not start again: {e}", self.old.commit),
        };
        format!(
            "{} failed its probation: {why}. {how}; what it left unanswered is sent again.",
            self.new.commit
        )
    }
}

/// Fold the database with this build's code: the last launch's position,
/// its build's verdict, and whether a request of its span has gone
/// unanswered too long.
fn fold_span(db: &Path) -> Result<(Option<Seq>, Verdict, bool), String> {
    let store = Store::open(db).map_err(|e| format!("{}: {e}", db.display()))?;
    let (decider, _) = schema::Brain::open(store.replica(), store)
        .map_err(|e| format!("folding {}: {e}", db.display()))?;
    let state = decider.state();
    let launch = state.launch().map(|(mark, _)| mark);
    let now = harness_limb::now().0;
    let unanswered = launch.is_some_and(|l| {
        state.model_calls().any(|(_, m)| {
            m.sent.seq > l.seq
                && m.first_byte.is_none()
                && m.ended.is_none()
                && now - m.sent.at.0 > UNANSWERED.as_millis() as i64
        })
    });
    Ok((
        launch.map(|l| l.seq),
        state.probation().verdict(),
        unanswered,
    ))
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
/// line runs the build `NAME/build` links to, the last proven, so it is
/// the same file whatever build `argv` names. This one start runs `argv`
/// itself, with `by` in its environment, through a drop-in in
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
/// if it does not within 30 s: the watch then sees it gone.
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

/// Why the unit's process ended since `since`, after a colon: systemd's
/// word on how it exited, and a panic if it printed one.
fn last_words(unit: &str, since: Instant) -> String {
    let secs = SystemTime::now()
        .checked_sub(since.elapsed())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    let out = Command::new("journalctl")
        .args(["--user", "-u", unit, "--no-pager", "-o", "cat"])
        .arg(format!("--since=@{secs}"))
        .output();
    let text = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    let said: Vec<&str> = text
        .as_deref()
        .unwrap_or_default()
        .lines()
        .filter(|l| l.contains("panicked") || l.contains("Main process exited"))
        .map(|l| l.trim_start_matches(&format!("{unit}: ")).trim())
        .collect();
    match said.is_empty() {
        true => String::new(),
        false => format!(", saying: {}", said.join(" / ")),
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
