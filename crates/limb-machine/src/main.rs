//! `harness-limb`: the limb, one process per brain, machine and user, with
//! every tool set this build has; and relaunching a brain by hand:
//!
//!   harness-limb attach [--existing] LIMB   reach the limb, starting it (harness_limb)
//!   harness-limb serve LIMB                 the limb itself, as attach starts it
//!   harness-limb relaunch UNIT CLONE [--back] [--one-way]
//!   harness-limb install CLONE              prints the build's directory
//!   harness-limb start UNIT HARNESS-BRAIN ARG...   durably, see relaunch::start
//!
//! `relaunch` runs as the `relaunch` tool does, saying each step on stderr,
//! and exits 0 once the new build has opened and outlived its window, 1
//! otherwise.
use std::path::Path;
use std::process::ExitCode;

use harness_limb::Open;
use harness_limb_builds::relaunch::{self, Asked, Ended};

/// Every tool set a limb of this build can offer, by name.
const SETS: &[(&str, Open)] = &[
    (
        harness_limb_workspace::WORKSPACE,
        harness_limb_workspace::open,
    ),
    (
        harness_limb_workspace::user::SET,
        harness_limb_workspace::user::open,
    ),
    (harness_limb_browser::SET, harness_limb_browser::open),
    (harness_limb_builds::SET, harness_limb_builds::open),
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let say = |line: &str| eprintln!("{line}");
    let done = match args.as_slice() {
        ["relaunch", unit, clone, flags @ ..]
            if flags.iter().all(|f| ["--back", "--one-way"].contains(f)) =>
        {
            let asked = Asked {
                back: flags.contains(&"--back"),
                one_way: flags.contains(&"--one-way"),
            };
            let ended = relaunch::relaunch(unit, Path::new(clone), asked, None, &say);
            say(&format!("{}: {}", ended.stage(), ended.words()));
            Ok(match ended {
                Ended::Opened(_) => ExitCode::SUCCESS,
                Ended::Refused(_) | Ended::Failed(_) | Ended::WentBack(_) => ExitCode::FAILURE,
            })
        }
        ["install", clone] => relaunch::install(Path::new(clone), &say).map(|build| {
            println!("{}", build.display());
            ExitCode::SUCCESS
        }),
        ["start", unit, argv @ ..] if !argv.is_empty() => {
            let argv: Vec<String> = argv.iter().map(|a| a.to_string()).collect();
            relaunch::start(unit, &argv, None).map(|()| ExitCode::SUCCESS)
        }
        ["relaunch" | "install" | "start", ..] => Err(
            "usage: harness-limb relaunch UNIT CLONE [--back] [--one-way] | install CLONE | start UNIT ARGV..."
                .into(),
        ),
        _ => return harness_limb::main(&args, SETS),
    };
    done.unwrap_or_else(|why| {
        eprintln!("harness-limb: {why}");
        ExitCode::FAILURE
    })
}
