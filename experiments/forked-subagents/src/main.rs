mod agent;
mod anthropic;
mod bench;
mod face;
mod framing;
mod limb;
mod record;
mod rescore;
mod session;
mod wire;

use agent::{Config, Mode, Outcome};
use framing::{Cut, Framing, Words};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;
use wire::Backend;

const HELP: &str = "\
forks — agents as structured concurrency

  forks run  --dir <path> \"<task>\"   one autonomous root, run to completion
  forks chat --dir <path>            talk to the root; it forks when it wants to
  forks bench                        the discipline benchmark over a grid of framings
  forks rescore <bench-dir> [--json <file>]  score a recorded benchmark again, offline;
                                     --json also writes every trial row as JSON

Framing (the two knobs the benchmark sweeps):
  --cut full|own|before   what a forked child inherits (default before)
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
  --mode fork|fresh|declared   force every child's mode (run and chat default
                               to declared: honour each `task` entry's `fresh`.
                               bench defaults to fork)

Provider:
  --backend claude|openrouter   default claude
      claude      Anthropic's API on your Claude subscription, with the token
                  opencode keeps. Not billed per token, so costs show as $0
                  and --max-cost never trips
      openrouter  OpenRouter's chat-completions API, paid from a key
  --model <id>        default anthropic/claude-sonnet-5
  --base-url <url>    default https://api.anthropic.com or
                      https://openrouter.ai/api/v1
  --credentials <db>  claude: opencode's database, default
                      $XDG_DATA_HOME/opencode/opencode.db
  --provider <slug>   openrouter: default amazon-bedrock; empty string sends
                      no routing
  --keys <file>       openrouter: default <experiment>/keys.ignore.env; or
                      $OPENROUTER_API_KEY

Limits:
  --max-cost <usd>    stop before the request that would exceed it (default 0.50;
                      bench uses 0.15 per trial)
  --max-depth <n>     levels of agents below the root (default 3; bench uses 2,
                      which is exactly the shape its task asks for)
  --max-turns <n>     requests one agent may make (default 20)
  --request-timeout <seconds>  give up on a silent provider and retry (default 300)
  --rate-limit-patience <seconds>  how long to keep waiting out a 429 before
                      giving up on it (default 120; the first wait is a
                      twenty-fourth of it, doubling, and `Retry-After` wins)
  --runs-dir <path>   default <experiment>/runs.ignore
  --panic-in <agent path>  fault injection: make that agent's task panic

Benchmark only:
  --grid <model@provider,...>   default <model>@<provider>
  --reps <n>                    trials per combination (default 1)
  --budget <usd>                total for the whole benchmark; the only thing
                                that stops it early (default 5.00). A trial that
                                reaches its own --max-cost is a scored trial.
  --cut / --words / --mode      accept comma-separated lists here

In chat: a line is a message to the root; /tree, /cancel, /quit. A line typed
while a turn is running is refused and discarded, not queued.
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
        "rescore" => rescore::command(&args).await,
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
    let interrupts = watch_interrupts(run.clone());
    let outcome = session.turn().await;
    interrupts.abort();
    session.report(&outcome);
    Ok(exit_code(&run, &outcome))
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

    let interrupts = watch_interrupts(run.clone());
    let (lines_tx, mut lines) = tokio::sync::mpsc::channel::<String>(8);
    // A thread of its own, not tokio's stdin: that reads on the runtime's
    // blocking pool, and the runtime does not shut down until the read
    // returns, so chat sat after its final report until Enter was pressed.
    std::thread::spawn(move || {
        for line in std::io::stdin().lines().map_while(Result::ok) {
            if lines_tx.blocking_send(line).is_err() {
                break;
            }
        }
    });

    let mut last = Outcome::Completed;
    // Once stdin is gone it stays gone, and a closed channel is ready
    // forever. Selecting on it again would spin a core for as long as the
    // turn runs.
    let mut stdin_open = true;
    loop {
        let line = tokio::select! {
            line = lines.recv() => line,
            _ = run.cancel.cancelled() => {
                last = Outcome::Cancelled;
                break;
            }
        };
        let Some(line) = line else { break };
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
                line = lines.recv(), if stdin_open => match line.as_deref().map(str::trim) {
                    Some("/tree") => run.face.say(&run.snapshot()),
                    Some("/cancel") => {
                        run.face.say(CANCELLING);
                        run.cancel.cancel();
                    }
                    Some(_) => run.face.say("a turn is running; only /tree and /cancel are accepted"),
                    None => stdin_open = false,
                },
            }
        };
        if !matches!(last, Outcome::Completed) {
            break;
        }
        // No tree here: the root's reply is what Max is waiting for, and it
        // must be the last thing on the screen. `/tree` is a keystroke away.
        if !stdin_open {
            break;
        }
    }
    interrupts.abort();
    session.report(&last);
    Ok(exit_code(&run, &last))
}

const CANCELLING: &str = "cancelling: nothing new starts; waiting for the responses already in flight, then closing. Ctrl-C again to force-cancel.";

/// Ctrl-C cancels, and the run closes once what is in flight has come back.
/// Ctrl-C on a run that is already cancelling force-cancels: the responses
/// still in flight are abandoned, and the run closes at once, still recording
/// every agent's outcome. One watcher for the whole session, so the interrupt
/// is never disarmed — with a guard on the listener, the second one went
/// nowhere and the run could not be interrupted at all.
fn watch_interrupts(run: std::sync::Arc<agent::Run>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while tokio::signal::ctrl_c().await.is_ok() {
            run.face.say("");
            if !run.cancel.is_cancelled() {
                run.face.say(CANCELLING);
                run.cancel.cancel();
            } else {
                run.face
                    .say("force-cancelling: abandoning the responses still in flight");
                run.force.cancel();
            }
        }
    })
}

/// 130 is what a shell reports for a process ended by Ctrl-C.
fn exit_code(run: &agent::Run, outcome: &Outcome) -> ExitCode {
    if run.force.is_cancelled() {
        return ExitCode::from(130);
    }
    match outcome {
        Outcome::Completed | Outcome::Cancelled => ExitCode::SUCCESS,
        Outcome::Faulted(_) | Outcome::Suspended | Outcome::Panicked(_) => ExitCode::FAILURE,
    }
}

const COMMON: &[&str] = &[
    "backend",
    "credentials",
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
    "request-timeout",
    "rate-limit-patience",
    "panic-in",
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

    /// OpenRouter routes to a pinned upstream by default; the subscription
    /// has no routes.
    pub fn provider(&self) -> String {
        let default = match self.one("backend", "claude").as_str() {
            "openrouter" => "amazon-bedrock",
            _ => "",
        };
        self.one("provider", default)
    }

    pub fn config(&self) -> Result<Config, String> {
        self.config_with(
            &self.one("model", "anthropic/claude-sonnet-5"),
            &self.provider(),
            Framing {
                cut: Cut::parse(&self.one("cut", "before"))?,
                words: Words::parse(&self.one("words", "explained"))?,
                mode: Mode::parse(&self.one("mode", "declared"))?,
            },
            3,
            0.50,
        )
    }

    pub fn config_with(
        &self,
        model: &str,
        provider: &str,
        framing: Framing,
        default_max_depth: usize,
        default_max_cost: f64,
    ) -> Result<Config, String> {
        let backend = match self.one("backend", "claude").as_str() {
            "openrouter" => {
                let base_url = self.one("base-url", "https://openrouter.ai/api/v1");
                let api_key = self.api_key()?;
                if api_key.is_none() && base_url.contains("openrouter.ai") {
                    return Err(
                        "no API key: set OPENROUTER_API_KEY or put it in keys.ignore.env"
                            .to_string(),
                    );
                }
                Backend::OpenRouter {
                    base_url,
                    api_key,
                    provider: (!provider.is_empty()).then(|| provider.to_string()),
                }
            }
            "claude" => {
                if !provider.is_empty() {
                    return Err(format!(
                        "provider {provider} is an OpenRouter route, and the claude backend has none"
                    ));
                }
                anthropic::model_id(model)?;
                let credentials = PathBuf::from(self.one("credentials", &opencode_db()));
                // Checked now so a run never starts on a token that cannot work.
                anthropic::access_token(&credentials)?;
                Backend::Claude {
                    base_url: self.one("base-url", "https://api.anthropic.com"),
                    credentials,
                }
            }
            other => {
                return Err(format!(
                    "unknown --backend {other}; expected claude or openrouter"
                ));
            }
        };
        Ok(Config {
            model: model.to_string(),
            backend,
            framing,
            max_cost: self.number("max-cost", default_max_cost)?,
            max_depth: self.number("max-depth", default_max_depth)?,
            max_turns: self.number("max-turns", 20usize)?,
            panic_in: self
                .flags
                .get("panic-in")
                .and_then(|values| values.last())
                .cloned(),
            request_timeout: std::time::Duration::from_secs_f64(
                self.number("request-timeout", 300.0)?,
            ),
            rate_limit_patience: std::time::Duration::from_secs_f64(
                self.number("rate-limit-patience", 120.0)?,
            ),
        })
    }
}

fn opencode_db() -> String {
    let data = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|dir| !dir.is_empty())
        .unwrap_or_else(|| format!("{}/.local/share", std::env::var("HOME").unwrap_or_default()));
    format!("{data}/opencode/opencode.db")
}
