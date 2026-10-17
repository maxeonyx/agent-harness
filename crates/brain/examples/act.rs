//! What Max does at a tab, for a script: a new tab replica on the face at
//! `PORT`, one act, and how the brain took it.
//!
//! ```text
//! act PORT done           Done on the task Max started
//! act PORT stop MS        Stop it, with MS milliseconds to finish
//! ```
//!
//! It prints `included`, or `refused: <why>` and exits 1.

mod device;

use std::env;
use std::process::exit;
use std::time::Duration;

use device::Device;
use harness_substrate::Status;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let usage = || -> ! {
        eprintln!("usage: act PORT done | act PORT stop MS");
        exit(2)
    };
    let Some(port) = args.first().and_then(|p| p.parse().ok()) else {
        usage()
    };
    let mut tab = Device::connect("act", port);
    tab.pump_until(|d| d.root().is_some());
    let root = tab.root().expect("a root");
    let n = match (args.get(1).map(String::as_str), args.get(2)) {
        (Some("done"), None) => tab.done(root),
        (Some("stop"), Some(ms)) => match ms.parse() {
            Ok(ms) => tab.stop(root, Duration::from_millis(ms)),
            Err(_) => usage(),
        },
        _ => usage(),
    };
    tab.pump_until(|d| d.tab.status(n) != Status::Pending);
    match tab.tab.status(n) {
        Status::Refused(why) => {
            println!("refused: {why}");
            exit(1)
        }
        _ => println!("included"),
    }
}
