//! Builds the tab (`harness-face-tab`) for `wasm32`, for the page to run.
//!
//! A cargo of its own, into a target directory of its own under `OUT_DIR`,
//! so it never waits on the lock of the build that runs this script.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
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
