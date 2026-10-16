//! Making a workspace's clone.

use std::io;
use std::path::Path;
use std::process::Command;

use crate::Clone;

/// Clone `clone.from` into `dir` and check out `clone.branch`: from
/// `origin/<branch>` if it exists, otherwise new from `origin/<base>`. A
/// `dir` that exists already is the clone made before, and is left as it
/// is, so making it is idempotent across restarts.
pub fn clone(clone: &Clone, dir: &Path) -> io::Result<()> {
    if dir.exists() {
        return Ok(());
    }
    let partial = dir.with_extension("cloning");
    if partial.exists() {
        std::fs::remove_dir_all(&partial)?;
    }
    let path = partial.to_string_lossy();
    git(
        None,
        &[
            "clone",
            "--recursive",
            "--filter=blob:none",
            "--quiet",
            &clone.from,
            &path,
        ],
    )?;
    let remote = format!("origin/{}", clone.branch);
    let exists = git(
        Some(&partial),
        &["rev-parse", "--verify", "--quiet", &remote],
    )
    .is_ok();
    let base = format!("origin/{}", clone.base);
    if exists {
        git(
            Some(&partial),
            &["checkout", "--quiet", "-B", &clone.branch, &remote],
        )?;
    } else {
        git(
            Some(&partial),
            &[
                "checkout",
                "--quiet",
                "--no-track",
                "-b",
                &clone.branch,
                &base,
            ],
        )?;
    }
    std::fs::rename(&partial, dir)
}

/// Run git; its stderr is the error when it fails.
fn git(dir: Option<&Path>, args: &[&str]) -> io::Result<()> {
    let mut command = Command::new("git");
    if let Some(dir) = dir {
        command.arg("-C").arg(dir);
    }
    let out = command.args(args).output()?;
    if out.status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "git {}: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr).trim()
    )))
}
