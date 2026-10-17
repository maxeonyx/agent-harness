//! What Max does at a tab, for a script: a new tab replica on the face at
//! `PORT`, one act, and how the brain took it.
//!
//! ```text
//! act PORT done           Done on the task Max started
//! act PORT abandon MS     Abandon it, with MS milliseconds to finish
//! act PORT stop CALL      Stop a call, such as one passed to Max
//! act PORT chat ROLE DIR TEXT
//!                         Start a chat in ROLE's environment, in a new
//!                         workspace over DIR (`-` for a role with none),
//!                         with TEXT as its mandate
//! act PORT say CHAT TEXT  Send TEXT to the chat CHAT, as its next words
//! act PORT back CHAT ENTRY
//!                         Move the chat CHAT back to ENTRY, where it has been
//! ```
//!
//! It prints `included` (for a chat, `chat <id>`), or `refused: <why>` and
//! exits 1.

mod device;

use std::env;
use std::process::exit;
use std::time::Duration;

use device::Device;
use harness_substrate::Status;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let usage = || -> ! {
        eprintln!(
            "usage: act PORT done | act PORT abandon MS | act PORT stop CALL | act PORT chat ROLE DIR TEXT | act PORT say CHAT TEXT | act PORT back CHAT ENTRY"
        );
        exit(2)
    };
    let Some(port) = args.first().and_then(|p| p.parse().ok()) else {
        usage()
    };
    let mut tab = Device::connect("act", port);
    let mut started = None;
    let n = if let [_, chat, role, dir, text] = &args[..]
        && chat == "chat"
    {
        tab.pump_until(|d| d.tab.view().is_some());
        let Some(env) = tab.environment(role) else {
            println!("refused: no role {role}");
            exit(1)
        };
        let (n, task) = tab.chat(env, (dir != "-").then_some(dir.as_str()), text);
        started = Some(task);
        n
    } else if let [_, say, chat, text] = &args[..]
        && say == "say"
    {
        let Ok(chat) = chat.parse() else { usage() };
        tab.pump_until(|d| d.tab.view().is_some());
        tab.message(chat, text)
    } else if let [_, back, chat, entry] = &args[..]
        && back == "back"
    {
        let (Ok(chat), Ok(entry)) = (chat.parse(), entry.parse()) else {
            usage()
        };
        tab.pump_until(|d| d.tab.view().is_some());
        tab.back(chat, entry)
    } else {
        tab.pump_until(|d| d.root().is_some());
        let root = tab.root().expect("a root");
        match (args.get(1).map(String::as_str), args.get(2)) {
            (Some("done"), None) => tab.done(root),
            (Some("abandon"), Some(ms)) => match ms.parse() {
                Ok(ms) => tab.abandon(root, Duration::from_millis(ms)),
                Err(_) => usage(),
            },
            (Some("stop"), Some(call)) => match call.parse() {
                Ok(call) => tab.stop(call),
                Err(_) => usage(),
            },
            _ => usage(),
        }
    };
    tab.pump_until(|d| d.tab.status(n) != Status::Pending);
    match tab.tab.status(n) {
        Status::Refused(why) => {
            println!("refused: {why}");
            exit(1)
        }
        _ => match started {
            Some(task) => println!("chat {task}"),
            None => println!("included"),
        },
    }
}
