//! The brain's database: one record per line, appended, never rewritten.
//! Every build opens it: a record whose payload this build cannot parse is
//! held as [`Unread`].

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

use harness_substrate::{Record, Storage, Time, Timestamp, Unread, What};
use serde::Deserialize;

use crate::schema::{Said, Spike};

pub struct Lines {
    pub path: PathBuf,
}

/// A record's envelope, which every build reads, around a payload this
/// build may not.
#[derive(Deserialize)]
struct Envelope {
    at: Time,
    clock: Timestamp,
    what: serde_json::Value,
}

impl Storage<Spike> for Lines {
    type Error = String;

    fn load(&self) -> Result<Vec<Record<Spike>>, String> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.to_string()),
        };
        text.lines()
            .map(|line| {
                let envelope: Envelope = serde_json::from_str(line)
                    .map_err(|e| format!("an envelope does not parse: {e}"))?;
                let what = serde_json::from_value(envelope.what.clone()).unwrap_or_else(|e| {
                    What::Said(Said::Unread(Unread {
                        bytes: envelope.what.to_string().into_bytes(),
                        why: e.to_string(),
                    }))
                });
                Ok(Record {
                    at: envelope.at,
                    clock: envelope.clock,
                    what,
                })
            })
            .collect()
    }

    fn commit(&mut self, put: &[Record<Spike>], _: &[Time]) -> Result<(), String> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| e.to_string())?;
        for record in put {
            let line = serde_json::to_string(record).map_err(|e| e.to_string())?;
            writeln!(file, "{line}").map_err(|e| e.to_string())?;
        }
        file.sync_data().map_err(|e| e.to_string())
    }
}
