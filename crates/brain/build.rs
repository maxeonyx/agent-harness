//! The commit the brain is built from, as `HARNESS_SOURCE`: the short
//! hash, `+dirty` if the tree had changes, `unknown` outside a checkout.
//! Run again when the branch moves, the index changes or a source file
//! does.

use std::process::Command;

fn main() {
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let source = match (
        git(&["rev-parse", "--short=12", "HEAD"]),
        git(&["status", "--porcelain", "--untracked-files=no"]),
    ) {
        (Some(commit), Some(changes)) if changes.is_empty() => commit,
        (Some(commit), Some(_)) => format!("{commit}+dirty"),
        _ => "unknown".into(),
    };
    println!("cargo:rustc-env=HARNESS_SOURCE={source}");
    if let Some(dir) = git(&["rev-parse", "--absolute-git-dir"]) {
        println!("cargo:rerun-if-changed={dir}/HEAD");
        println!("cargo:rerun-if-changed={dir}/index");
        if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]) {
            println!("cargo:rerun-if-changed={dir}/{branch}");
        }
    }
    if let Some(top) = git(&["rev-parse", "--show-toplevel"]) {
        for path in ["crates", "Cargo.toml", "Cargo.lock"] {
            println!("cargo:rerun-if-changed={top}/{path}");
        }
    }
}
