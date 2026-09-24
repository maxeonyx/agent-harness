//! One run: a root agent, its tree, and the run directory that records them.

use crate::agent::{AgentRecord, Config, Outcome, Run, render_context, run_agent};
use crate::face::Face;
use crate::framing;
use crate::limb::Limb;
use crate::record::Recorder;
use crate::wire::Message;

use std::path::Path;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio_util::sync::CancellationToken;

pub struct Session {
    pub run: Arc<Run>,
    pub root_index: usize,
    pub messages: Vec<Message>,
    started: Instant,
}

pub struct Ending {
    pub handoff: String,
    pub agents: Vec<AgentRecord>,
    pub cost: f64,
}

impl Session {
    pub fn open(
        config: Config,
        dir: &Path,
        runs_dir: &Path,
        label: &str,
        verbose: bool,
        session_id: Option<String>,
    ) -> Result<Session, String> {
        let limb = Limb::new(dir)?;
        let recorder = Recorder::create(runs_dir, label)?;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let session_id = match session_id {
            Some(given) => given,
            None => format!("forks-{nanos}"),
        };
        let run = Arc::new(Run::new(
            config,
            limb,
            Face::new(verbose),
            recorder,
            CancellationToken::new(),
            session_id,
        ));
        let root_index = run.register("root", 0, false, None, None);
        Ok(Session {
            run,
            root_index,
            messages: vec![Message::new("system", framing::SYSTEM_PROMPT)],
            started: Instant::now(),
        })
    }

    pub fn say(&mut self, text: &str) {
        self.messages.push(Message::new("user", text));
    }

    /// Drive the root until it ends its turn. The messages come back so a
    /// chat session can carry the same root forward into the next turn.
    pub async fn turn(&mut self) -> Outcome {
        let messages = std::mem::take(&mut self.messages);
        let end = run_agent(
            self.run.clone(),
            "root".to_string(),
            0,
            self.root_index,
            messages,
        )
        .await;
        self.messages = end.messages;
        end.outcome
    }

    pub fn finish(&self, outcome: &Outcome) -> Ending {
        let agents = self.run.agents();
        for record in &agents {
            let file = format!("agents/{}.md", record.path.replace(" › ", "."));
            self.run.recorder.write(&file, &render_context(record));
        }
        let handoff = agents
            .iter()
            .find(|record| record.path == "root")
            .map(|record| record.handoff.clone())
            .unwrap_or_default();
        let summary = serde_json::json!({
            "outcome": outcome.short(),
            "detail": outcome.label(),
            "fault": self.run.fault(),
            "model": self.run.config.model,
            "provider": self.run.config.provider,
            "cut": self.run.config.framing.cut.name(),
            "words": self.run.config.framing.words.name(),
            "mode": self.run.config.framing.mode.name(),
            "max_depth": self.run.config.max_depth,
            "max_cost": self.run.config.max_cost,
            "cost": self.run.total_cost(),
            "millis": self.started.elapsed().as_millis(),
            "root_handoff": handoff,
            "agents": agents.iter().map(|a| a.to_json()).collect::<Vec<_>>(),
        });
        self.run.recorder.write(
            "summary.json",
            &serde_json::to_string_pretty(&summary).unwrap(),
        );
        Ending {
            handoff,
            agents,
            cost: self.run.total_cost(),
        }
    }

    pub fn report(&self, outcome: &Outcome) -> Ending {
        let ending = self.finish(outcome);
        self.run.face.say("");
        self.run.face.say(&self.run.snapshot());
        self.run.face.say(&format!(
            "run: {} in {:.1}s · recorded in {}",
            outcome.label(),
            self.started.elapsed().as_secs_f64(),
            self.run.recorder.dir().display()
        ));
        if !ending.handoff.is_empty() {
            self.run.face.say("");
            self.run.face.say("root's report:");
            self.run.face.say(&ending.handoff);
        }
        ending
    }
}
