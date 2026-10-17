//! Builds the tab (`harness-face-tab`) for `wasm32`, for the page to run,
//! and lists the page's files for the face to build in.
//!
//! The tab is a cargo of its own, into a target directory of its own under
//! `OUT_DIR`, so it never waits on the lock of the build that runs this
//! script.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    page(&out);
    let tab = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../face-tab");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let target = out.join("tab");
    let status = Command::new(cargo)
        .args(["build", "--release", "--target", "wasm32-unknown-unknown"])
        .arg("--manifest-path")
        .arg(tab.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&target)
        // Small, since every tab downloads it.
        .env("CARGO_PROFILE_RELEASE_OPT_LEVEL", "s")
        .env("CARGO_PROFILE_RELEASE_LTO", "true")
        .env("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "1")
        .env("CARGO_PROFILE_RELEASE_PANIC", "abort")
        .env("CARGO_PROFILE_RELEASE_STRIP", "true")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .status()
        .expect("cargo runs");
    assert!(
        status.success(),
        "building the tab for wasm32 failed; is the target installed? \
         rustup target add wasm32-unknown-unknown"
    );
    let wasm = target.join("wasm32-unknown-unknown/release/harness_face_tab.wasm");
    std::fs::copy(&wasm, out.join("tab.wasm")).expect("the tab's wasm was built");
    println!("cargo::rerun-if-changed=../face-tab/src");
    println!("cargo::rerun-if-changed=../core/src");
    println!("cargo::rerun-if-changed=../substrate/src");
}

/// Every file in `src/page`, as `&[(&str, &str)]` of name and text, so a
/// new file is built in without naming it anywhere else.
fn page(out: &Path) {
    let page = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/page");
    let mut names: Vec<String> = std::fs::read_dir(&page)
        .expect("src/page is readable")
        .map(|e| {
            e.expect("src/page is readable")
                .file_name()
                .into_string()
                .expect("UTF-8 names")
        })
        .collect();
    names.sort();
    let entries: String = names
        .iter()
        .map(|n| format!("    ({n:?}, include_str!({:?})),\n", page.join(n)))
        .collect();
    std::fs::write(out.join("page.rs"), format!("&[\n{entries}]\n")).expect("OUT_DIR is writable");
    println!("cargo::rerun-if-changed=src/page");
}
