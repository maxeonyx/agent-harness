//! The builds limb's process, and relaunching a brain by hand:
//!
//!   harness-limb-builds attach|serve LIMB     the limb (harness_limb)
//!   harness-limb-builds relaunch UNIT CLONE [--back]
//!   harness-limb-builds install CLONE         prints the build's directory
//!   harness-limb-builds start UNIT HARNESS-BRAIN ARG...
//!
//! `relaunch` runs until the new build is proven (exit 0) or has gone back
//! (exit 1), saying each step on stderr.
use std::path::Path;
use std::process::ExitCode;

use harness_limb_builds::relaunch::{self, Ended};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let say = |line: &str| eprintln!("{line}");
    let done = match args.as_slice() {
        ["relaunch", unit, clone, rest @ ..] if rest.is_empty() || rest == ["--back"] => {
            relaunch::take_over(unit, Path::new(clone), !rest.is_empty(), &say).map(|takeover| {
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
            relaunch::start(unit, &argv).map(|()| ExitCode::SUCCESS)
        }
        ["relaunch" | "install" | "start", ..] => Err(
            "usage: harness-limb-builds relaunch UNIT CLONE [--back] | install CLONE | start UNIT ARGV..."
                .into(),
        ),
        _ => return harness_limb::main::<harness_limb_builds::Builds>(harness_limb_builds::KIND),
    };
    done.unwrap_or_else(|why| {
        eprintln!("harness-limb-builds: {why}");
        ExitCode::FAILURE
    })
}
