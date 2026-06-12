//! Walking skeleton: face + brain + limb as logical roles in one process.
//! This file is the face: an append-only CLI. It never talks to the
//! provider; it stages context with the brain and ends turns.

mod brain;
mod limb;
mod provider;
mod recorder;

use std::io::BufRead;

fn main() {
    let config = brain::Config::from_env();
    println!(
        "walking-skeleton — provider {} model {}",
        config.base_url, config.model
    );
    println!("  <text>        stage a user message (appends, never triggers)");
    println!("  /open <path>  simulate user file-open activity (appends, never triggers)");
    println!("  /end          end the turn (triggers inference)");
    println!("  /quit         exit");

    let mut brain = brain::Brain::new(config, limb::Limb::new(), recorder::Recorder::from_env());

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "/quit" {
            break;
        }
        if line == "/end" {
            if let Err(e) = brain.end_turn() {
                println!("[brain] turn failed: {e}");
            }
            continue;
        }
        if let Some(path) = line.strip_prefix("/open ") {
            // User-tool dual surface: rich output for the user here,
            // compressed summary appended for the model.
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    println!("[face] opened {path} ({} bytes)", content.len());
                    let head: Vec<&str> = content.lines().take(20).collect();
                    let summary = format!(
                        "opened file {path}; first {} lines:\n{}",
                        head.len(),
                        head.join("\n")
                    );
                    brain.append_user_activity(summary);
                }
                Err(e) => println!("[face] could not open {path}: {e}"),
            }
            continue;
        }
        brain.append_user_message(line.to_string());
        println!("[face] staged user message");
    }
}
