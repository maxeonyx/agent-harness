mod agent;
mod bench;
mod face;
mod framing;
mod limb;
mod record;
mod session;
mod wire;

use agent::{Config, Mode, Outcome};
use framing::{Cut, Words};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;

const HELP: &str = "\
forks — agents as structured concurrency

  forks run  --dir <path> \"<task>\"   one autonomous root, run to completion
  forks chat --dir <path>            talk to the root; it forks when it wants to
  forks bench                        the discipline benchmark over a grid of framings

Framing (the two knobs the benchmark sweeps):
  --cut full|own|before   what a forked child inherits (default full)
      full    the parent's messages through the `task` turn, then a tool
              result addressed to this child
      own     the same, but this child's copy of the `task` arguments holds
              only its own entry
      before  the parent's messages up to, not including, the `task` turn,
              then a user message with the assignment
  --words stop|explained  what the assignment says (default explained)
      stop       the assignment, and \"do only this, then stop\"
      explained  also: that it is one branch of a split, that its siblings
                 hold the other assignments, and that its final message is
                 exactly what its parent receives
  --mode fork|fresh|declared   force every child's mode (default declared:
                               honour each `task` entry's `fresh`)

Provider:
  --model <id>        default anthropic/claude-sonnet-5
  --provider <slug>   default amazon-bedrock; empty string sends no routing
  --base-url <url>    default https://openrouter.ai/api/v1
  --keys <file>       default <experiment>/keys.ignore.env; or $OPENROUTER_API_KEY

Limits:
  --max-cost <usd>    stop the run before the request that would exceed it (default 0.50)
  --max-depth <n>     levels of agents below the root (default 3)
  --max-turns <n>     requests one agent may make (default 20)
  --runs-dir <path>   default <experiment>/runs.ignore

Benchmark only:
  --grid <model@provider,...>   default <model>@<provider>
  --reps <n>                    trials per combination (default 1)
  --budget <usd>                total for the whole benchmark; stops cleanly (default 5.00)
  --cut / --words / --mode      accept comma-separated lists here

In chat: a line is a message to the root; /tree, /cancel, /quit.
";

#[tokio::main]
async fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match run(argv).await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("forks: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(argv: Vec<String>) -> Result<ExitCode, String> {
    if argv.is_empty() || argv[0] == "--help" || argv[0] == "-h" || argv[0] == "help" {
        print!("{HELP}");
        return Ok(ExitCode::SUCCESS);
    }
    let args = Args::parse(argv)?;
    match args.command.as_str() {
        "run" => command_run(&args).await,
        "chat" => command_chat(&args).await,
        "bench" => bench::command(&args).await,
        other => Err(format!("unknown command `{other}`; try --help")),
    }
}

async fn command_run(args: &Args) -> Result<ExitCode, String> {
    args.known(&[COMMON, &["dir"]].concat())?;
    let dir = args.required("dir")?;
    let task = args
        .positional
        .first()
        .ok_or("forks run needs a task: forks run --dir <path> \"<task>\"")?
        .clone();
    let mut session = session::Session::open(
        args.config()?,
        &PathBuf::from(&dir),
        &args.runs_dir(),
        "run",
        true,
        args.session_id(),
    )?;
    session.say(&task);
    let run = session.run.clone();
    let outcome = {
        let turn = session.turn();
        tokio::pin!(turn);
        loop {
            tokio::select! {
                outcome = &mut turn => break outcome,
                _ = tokio::signal::ctrl_c(), if !run.cancel.is_cancelled() => {
                    run.face.say("");
                    run.face.say("cancelling: nothing new starts; in-flight responses are kept");
                    run.cancel.cancel();
                }
            }
        }
    };
    session.report(&outcome);
    Ok(exit_code(&outcome))
}

async fn command_chat(args: &Args) -> Result<ExitCode, String> {
    args.known(&[COMMON, &["dir"]].concat())?;
    let dir = args.required("dir")?;
    let mut session = session::Session::open(
        args.config()?,
        &PathBuf::from(&dir),
        &args.runs_dir(),
        "chat",
        true,
        args.session_id(),
    )?;
    let run = session.run.clone();
    run.face
        .say("chat: a line is a message to the root. /tree, /cancel, /quit.");

    let (lines_tx, mut lines) = tokio::sync::mpsc::channel::<String>(8);
    tokio::spawn(async move {
        use tokio::io::AsyncBufReadExt;
        let mut reader = tokio::io::BufReader::new(tokio::io::stdin()).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if lines_tx.send(line).await.is_err() {
                break;
            }
        }
    });

    let mut last = Outcome::Completed;
    while let Some(line) = lines.recv().await {
        let line = line.trim().to_string();
        match line.as_str() {
            "" => continue,
            "/quit" => break,
            "/tree" => {
                run.face.say(&run.snapshot());
                continue;
            }
            "/cancel" => {
                run.face.say("nothing is running");
                continue;
            }
            _ => {}
        }
        session.say(&line);
        let turn = session.turn();
        tokio::pin!(turn);
        last = loop {
            tokio::select! {
                outcome = &mut turn => break outcome,
                line = lines.recv() => match line.as_deref().map(str::trim) {
                    Some("/tree") => run.face.say(&run.snapshot()),
                    Some("/cancel") => {
                        run.face.say("cancelling: nothing new starts; in-flight responses are kept");
                        run.cancel.cancel();
                    }
                    Some(_) => run.face.say("a turn is running; only /tree and /cancel are accepted"),
                    None => {}
                },
            }
        };
        if !matches!(last, Outcome::Completed) {
            break;
        }
        run.face.say(&run.snapshot());
    }
    session.report(&last);
    Ok(exit_code(&last))
}

fn exit_code(outcome: &Outcome) -> ExitCode {
    match outcome {
        Outcome::Completed | Outcome::Cancelled => ExitCode::SUCCESS,
        Outcome::Faulted(_) | Outcome::Suspended => ExitCode::FAILURE,
    }
}

const COMMON: &[&str] = &[
    "model",
    "provider",
    "base-url",
    "keys",
    "cut",
    "words",
    "mode",
    "max-cost",
    "max-depth",
    "max-turns",
    "runs-dir",
    "session-id",
];

/// The experiment's own directory, known at build time. The binary is always
/// built from this tree, so it can find its key file and its run directory
/// wherever it is invoked from.
const EXPERIMENT_DIR: &str = env!("CARGO_MANIFEST_DIR");

pub struct Args {
    pub command: String,
    pub positional: Vec<String>,
    flags: HashMap<String, Vec<String>>,
}

impl Args {
    fn parse(argv: Vec<String>) -> Result<Args, String> {
        let mut argv = argv.into_iter();
        let command = argv.next().expect("checked non-empty");
        let mut positional = Vec::new();
        let mut flags: HashMap<String, Vec<String>> = HashMap::new();
        let mut pending: Option<String> = None;
        for argument in argv {
            if let Some(name) = pending.take() {
                flags.entry(name).or_default().push(argument);
                continue;
            }
            if let Some(rest) = argument.strip_prefix("--") {
                match rest.split_once('=') {
                    Some((name, value)) => flags
                        .entry(name.to_string())
                        .or_default()
                        .push(value.to_string()),
                    None => pending = Some(rest.to_string()),
                }
            } else {
                positional.push(argument);
            }
        }
        if let Some(name) = pending {
            return Err(format!("--{name} needs a value"));
        }
        Ok(Args {
            command,
            positional,
            flags,
        })
    }

    fn known(&self, allowed: &[&str]) -> Result<(), String> {
        for name in self.flags.keys() {
            if !allowed.contains(&name.as_str()) {
                return Err(format!(
                    "unknown flag --{name} for `{}`; accepted: {}",
                    self.command,
                    allowed
                        .iter()
                        .map(|a| format!("--{a}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
        }
        Ok(())
    }

    pub fn one(&self, name: &str, default: &str) -> String {
        self.flags
            .get(name)
            .and_then(|values| values.last())
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }

    fn required(&self, name: &str) -> Result<String, String> {
        self.flags
            .get(name)
            .and_then(|values| values.last())
            .cloned()
            .ok_or_else(|| format!("--{name} is required"))
    }

    /// Comma-separated values, across every occurrence of the flag.
    pub fn list(&self, name: &str, default: &str) -> Vec<String> {
        let joined = match self.flags.get(name) {
            Some(values) => values.join(","),
            None => default.to_string(),
        };
        joined
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .collect()
    }

    pub fn number<T: std::str::FromStr>(&self, name: &str, default: T) -> Result<T, String> {
        match self.flags.get(name).and_then(|values| values.last()) {
            Some(text) => text
                .parse()
                .map_err(|_| format!("--{name} {text} is not a number")),
            None => Ok(default),
        }
    }

    /// `--session-id ""` sends no session id at all.
    pub fn session_id(&self) -> Option<String> {
        self.flags
            .get("session-id")
            .and_then(|values| values.last())
            .cloned()
    }

    pub fn runs_dir(&self) -> PathBuf {
        PathBuf::from(self.one("runs-dir", &format!("{EXPERIMENT_DIR}/runs.ignore")))
    }

    pub fn api_key(&self) -> Result<Option<String>, String> {
        if let Ok(key) = std::env::var("OPENROUTER_API_KEY")
            && !key.is_empty()
        {
            return Ok(Some(key));
        }
        let path = PathBuf::from(self.one("keys", &format!("{EXPERIMENT_DIR}/keys.ignore.env")));
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Ok(None);
        };
        Ok(text
            .lines()
            .filter_map(|line| line.trim().strip_prefix("OPENROUTER_API_KEY="))
            .map(|value| value.trim().to_string())
            .next())
    }

    pub fn config(&self) -> Result<Config, String> {
        self.config_with(
            &self.one("model", "anthropic/claude-sonnet-5"),
            &self.one("provider", "amazon-bedrock"),
            Cut::parse(&self.one("cut", "full"))?,
            Words::parse(&self.one("words", "explained"))?,
            Mode::parse(&self.one("mode", "declared"))?,
        )
    }

    pub fn config_with(
        &self,
        model: &str,
        provider: &str,
        cut: Cut,
        words: Words,
        mode: Mode,
    ) -> Result<Config, String> {
        let base_url = self.one("base-url", "https://openrouter.ai/api/v1");
        let api_key = self.api_key()?;
        if api_key.is_none() && base_url.contains("openrouter.ai") {
            return Err(
                "no API key: set OPENROUTER_API_KEY or put it in keys.ignore.env".to_string(),
            );
        }
        Ok(Config {
            model: model.to_string(),
            provider: (!provider.is_empty()).then(|| provider.to_string()),
            base_url,
            api_key,
            cut,
            words,
            mode,
            max_cost: self.number("max-cost", 0.50)?,
            max_depth: self.number("max-depth", 3usize)?,
            max_turns: self.number("max-turns", 20usize)?,
        })
    }
}
