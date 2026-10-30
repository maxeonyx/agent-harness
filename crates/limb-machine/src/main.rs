//! `harness-limb`: the limb, one process per brain, machine and user, with
//! every tool set this build has; and relaunching a brain by hand:
//!
//!   harness-limb attach [--existing] LIMB   reach the limb, starting it (harness_limb)
//!   harness-limb serve LIMB                 the limb itself, as attach starts it
//!   harness-limb relaunch UNIT CLONE [--back]
//!   harness-limb install CLONE              prints the build's directory
//!   harness-limb start UNIT HARNESS-BRAIN ARG...   durably, see relaunch::start
//!
//! `relaunch` runs until the new build is proven (exit 0) or has gone back
//! (exit 1), saying each step on stderr.
use std::path::Path;
use std::process::ExitCode;

use harness_limb::Open;
use harness_limb_builds::relaunch::{self, Ended};

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
        ["relaunch", unit, clone, rest @ ..] if rest.is_empty() || rest == ["--back"] => {
            relaunch::take_over(unit, Path::new(clone), !rest.is_empty(), None, &say).map(|takeover| {
                say(&format!(
                    "{unit} runs {}, on probation until it has a response and a limb's settlement",
                    takeover.new.commit
                ));
                match takeover.watch() {
                    Ended::Proven => {
                        say("proven");
                        ExitCode::SUCCESS
                    }
                    Ended::Back(why) => {
                        say(&why);
                        ExitCode::FAILURE
                    }
                }
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
            "usage: harness-limb relaunch UNIT CLONE [--back] | install CLONE | start UNIT ARGV..."
                .into(),
        ),
        _ => return harness_limb::main(&args, SETS),
    };
    done.unwrap_or_else(|why| {
        eprintln!("harness-limb: {why}");
        ExitCode::FAILURE
    })
}
