//! Relaunching a brain's service onto the commit of a clone, and the watch
//! that goes back when the new build fails: the procedure the `relaunch`
//! tool and the command line share.
//!
//! A brain runs as the systemd user unit `harness-NAME.service`, from an
//! installed build: `$XDG_DATA_HOME/agent-harness/builds/COMMIT/`, its
//! executables and `roles/`, read-only and never overwritten. What the
//! unit runs is read off its process: its command line names the build's
//! directory, the database (`--db`), the port and its flags, and a
//! relaunch starts the same command line in another build's directory.
//! `NAME/build` beside the database links to the build a restart runs: the
//! last one proven.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use harness_core::build::Verdict;
use harness_core::id::Seq;
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
        let exe = Path::new(
            argv.first()
                .ok_or_else(|| format!("{unit} has no command line"))?,
        );
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
        self.argv
            .iter()
            .map(|a| match a.strip_prefix(&*old) {
                Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("{new}{rest}"),
                _ => a.clone(),
            })
            .collect()
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
    rehearse(&build, &old)?;
    let before = fold_span(&old.db)?.0;
    say(&format!(
        "stopping {unit} on {}: its requests in flight drain first",
        old.commit
    ));
    stop(unit)?;
    say(&format!("starting {unit} on {commit}"));
    if let Err(why) = start(unit, &old.on(&build)) {
        return Err(match start(unit, &old.argv) {
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
        started: Instant::now(),
    })
}

/// Prove the new build beside the running brain without touching it.
/// Nothing yet: what it is to do is in [`harness_core::build`].
fn rehearse(_build: &Path, _running: &Running) -> Result<(), String> {
    Ok(())
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
        let how = match start(unit, &self.old.argv) {
            Ok(()) => format!("{unit} runs {} again", self.old.commit),
            Err(e) => format!("and {} would not start again: {e}", self.old.commit),
        };
        format!(
            "{} failed its probation: {why}. {how}; requests it left unanswered are sent again.",
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
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64);
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
    let stderr = cargo.stderr.take().expect("piped");
    let errors = thread::spawn(move || {
        let lines: Vec<String> = BufReader::new(stderr)
            .lines()
            .map_while(Result::ok)
            .collect();
        lines[lines.len().saturating_sub(20)..].join("\n")
    });
    let mut executables = Vec::new();
    for line in BufReader::new(cargo.stdout.take().expect("piped")).lines() {
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
        .stdin(tar.stdout.expect("piped"))
        .status()
        .map_err(|e| format!("tar: {e}"))?;
    if !untar.success() {
        return Err(format!("{commit}'s roles/ would not unpack"));
    }
    let built = run(Command::new(bin.join("harness-brain")).arg("--version"))?;
    if built != format!("harness-brain {commit}") {
        return Err(format!("the build says it is {built}, not {commit}"));
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

/// Start `unit` running `argv`, in the database's directory. Its stop
/// sends SIGTERM to the brain alone, which ends its links to limbs itself
/// (the limbs run in units of their own), and gives it 330 s to drain.
pub fn start(unit: &str, argv: &[String]) -> Result<(), String> {
    let _ = systemctl(&["reset-failed", unit]);
    let exe = Path::new(argv.first().ok_or("nothing to start")?);
    let commit = exe
        .parent()
        .and_then(Path::parent)
        .map(name)
        .unwrap_or_default();
    let db = argv.iter().skip_while(|a| *a != "--db").nth(1);
    let dir = db
        .and_then(|db| Path::new(db).parent())
        .ok_or("the command line names no --db")?;
    let name = unit
        .trim_start_matches("harness-")
        .trim_end_matches(".service");
    run(Command::new("systemd-run")
        .args(["--user", "--quiet", "--collect"])
        .arg(format!("--unit={unit}"))
        .arg(format!(
            "--description=agent-harness brain {name} ({commit})"
        ))
        .arg(format!("--working-directory={}", dir.display()))
        .args(["-p", "KillMode=mixed", "-p", "TimeoutStopSec=330"])
        .args(argv))
    .map(|_| ())
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

/// What the unit printed last since `since`, after a colon, if anything.
fn last_words(unit: &str, since: Instant) -> String {
    let secs = SystemTime::now()
        .checked_sub(since.elapsed())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    let out = Command::new("journalctl")
        .args(["--user", "-u", unit, "--no-pager", "-o", "cat", "-n", "3"])
        .arg(format!("--since=@{secs}"))
        .output();
    match out {
        Ok(out) if !out.stdout.is_empty() => {
            let text = String::from_utf8_lossy(&out.stdout);
            format!(", saying: {}", text.trim().replace('\n', " / "))
        }
        _ => String::new(),
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
