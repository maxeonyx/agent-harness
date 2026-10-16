//! Fork-framing experiment: does a forked child do only its own part and end
//! its turn, and does it read its parent's prompt cache while doing so?
//!
//! A trunk is one real chain of spawns: the root splits A, the fork for A.1
//! splits A.1, the fork for A.1.1 splits A.1.1, each framed by the framing
//! under test. A trial forks the middle sibling at depth 1, 2 or 3 of a
//! trunk, runs it on stub tools until it ends its turn or runs out of rounds,
//! and scores what it did: which pages it wrote, what it spawned, and whether
//! it ended its turn.

mod framing;
mod scenario;
mod wire;

use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{Value, json};

use framing::{Call, Fork, Framing};
use scenario::Part;
use wire::{Client, Token};

const USAGE: &str = "usage:
  fork-framing run [--models a,b] [--framings bare,user,user-remind,tool,tool-remind,tool-neutral,identity,rewind] [--depths 1,2,3]
                   [--trunks K] [--trials N] [--jobs J] [--rounds R] [--out DIR]
  fork-framing report RUN_DIR...";

/// Room for hidden thinking as well as a page.
const MAX_TOKENS: u32 = 8000;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("run") => run(&args[1..]),
        Some("report") => report(&args[1..]),
        _ => Err(USAGE.into()),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

struct Config {
    models: Vec<String>,
    framings: Vec<Framing>,
    depths: Vec<usize>,
    trunks: usize,
    trials: usize,
    jobs: usize,
    rounds: usize,
    out: PathBuf,
}

fn parse(args: &[String]) -> Result<Config, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut c = Config {
        models: vec!["claude-sonnet-5".into(), "claude-opus-5-5".into()],
        framings: framing::ALL.to_vec(),
        depths: vec![1, 2, 3],
        trunks: 2,
        trials: 5,
        jobs: 12,
        rounds: 5,
        out: PathBuf::from(format!("runs.ignore/{stamp}")),
    };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let v = it.next().ok_or(format!("{flag} needs a value\n{USAGE}"))?;
        let list = || v.split(',').map(str::to_owned);
        let num = || {
            v.parse::<usize>()
                .map_err(|_| format!("{flag}: not a number"))
        };
        match flag.as_str() {
            "--models" => c.models = list().collect(),
            "--framings" => {
                c.framings = list()
                    .map(|s| Framing::parse(&s).ok_or(format!("unknown framing {s}")))
                    .collect::<Result<_, _>>()?
            }
            "--depths" => {
                c.depths = list()
                    .map(|s| match s.parse() {
                        Ok(d @ 1..=3) => Ok(d),
                        _ => Err(format!("depth {s} is not 1, 2 or 3")),
                    })
                    .collect::<Result<_, _>>()?
            }
            "--trunks" => c.trunks = num()?,
            "--trials" => c.trials = num()?,
            "--jobs" => c.jobs = num()?,
            "--rounds" => c.rounds = num()?,
            "--out" => c.out = PathBuf::from(v),
            _ => return Err(format!("unknown flag {flag}\n{USAGE}")),
        }
    }
    Ok(c)
}

fn run(args: &[String]) -> Result<(), String> {
    let c = parse(args)?;
    let home = std::env::var("HOME").map_err(|_| "HOME is not set")?;
    let token = Token::from_claude_code(&Path::new(&home).join(".claude/.credentials.json"))
        .map_err(|e| e.to_string())?;
    let client = Client::new(token);
    std::fs::create_dir_all(&c.out).map_err(|e| e.to_string())?;
    let outline = scenario::outline();
    let deepest = *c.depths.iter().max().unwrap_or(&1);
    eprintln!("writing to {}", c.out.display());

    // The root's split is shared by every framing of a model, so depth-1
    // forks of every framing share one prefix.
    let roots: Vec<(String, usize)> = c
        .models
        .iter()
        .flat_map(|m| (0..c.trunks).map(move |k| (m.clone(), k)))
        .collect();
    let splits = Mutex::new(File::create(c.out.join("splits.jsonl")).map_err(|e| e.to_string())?);
    let roots = par(roots, c.jobs, |(model, k)| {
        let messages = vec![
            json!({ "role": "user", "content": [{ "type": "text", "text": scenario::task() }] }),
        ];
        let ctx = split(
            &client,
            &model,
            messages,
            outline.find("A").unwrap(),
            "root",
            &splits,
        );
        if let Err(e) = &ctx {
            eprintln!("[{model} trunk{k}] root split failed: {e}");
        }
        (model, k, ctx.ok())
    });

    let chains: Vec<_> = roots
        .into_iter()
        .filter_map(|(m, k, ctx)| ctx.map(|ctx| (m, k, ctx)))
        .flat_map(|(m, k, ctx)| {
            c.framings
                .iter()
                .map(move |&f| (m.clone(), k, f, ctx.clone()))
        })
        .collect();
    let trunks = par(chains, c.jobs, |(model, k, f, root)| {
        let mut ctxs = vec![root];
        for depth in 2..=deepest {
            let parent = scenario::forker(depth - 1);
            let own = scenario::forker(depth);
            let next = ctxs
                .last()
                .ok_or("no context".to_owned())
                .and_then(|ctx| fork(&outline, ctx, parent, own).map(|fk| f.open(ctx, &fk)))
                .and_then(|msgs| {
                    split(
                        &client,
                        &model,
                        msgs,
                        outline.find(own).unwrap(),
                        f.name,
                        &splits,
                    )
                });
            match next {
                Ok(ctx) => ctxs.push(ctx),
                Err(e) => {
                    eprintln!("[{model} trunk{k} {}] split of {own} failed: {e}", f.name);
                    break;
                }
            }
        }
        (model, k, f, ctxs)
    });
    let mut file = File::create(c.out.join("trunks.json")).map_err(|e| e.to_string())?;
    let dump: Vec<_> = trunks
        .iter()
        .map(|(m, k, f, ctxs)| json!({ "model": m, "trunk": k, "framing": f.name, "contexts": ctxs }))
        .collect();
    writeln!(file, "{}", Value::Array(dump)).map_err(|e| e.to_string())?;

    let jobs: Vec<_> = trunks
        .iter()
        .flat_map(|(m, k, f, ctxs)| {
            c.depths
                .iter()
                .filter(|&&d| d <= ctxs.len())
                .flat_map(move |&d| (0..c.trials).map(move |i| (m, *k, *f, d, i, &ctxs[d - 1])))
        })
        .collect();
    eprintln!("{} trunks, {} trials", trunks.len(), jobs.len());
    let path = c.out.join("trials.jsonl");
    let out = Mutex::new(File::create(&path).map_err(|e| e.to_string())?);
    par(jobs, c.jobs, |(model, k, f, depth, i, ctx)| {
        let mut record = match trial(&client, &outline, model, f, depth, ctx, c.rounds) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[{model} {} d{depth} t{k}.{i}] failed: {e}", f.name);
                return;
            }
        };
        record["trunk"] = json!(k);
        record["i"] = json!(i);
        eprintln!(
            "[{model} {} d{depth} t{k}.{i}] lane={} stop={} writes={} spawns={} cache_read={}",
            f.name,
            record["lane"],
            record["stop"],
            record["writes"],
            record["spawns"],
            record["cache_read"]
        );
        let mut out = out.lock().unwrap();
        let _ = writeln!(out, "{record}");
    });
    report(&[c.out.display().to_string()])
}

/// Runs a fork for `own` at the head of `ctx` (the response of `forker`)
/// until it ends its turn or `rounds` tool rounds have passed.
fn trial(
    client: &Client,
    outline: &Part,
    model: &str,
    f: Framing,
    depth: usize,
    ctx: &[Value],
    rounds: usize,
) -> Result<Value, String> {
    let fork = fork(
        outline,
        ctx,
        scenario::forker(depth),
        scenario::target(depth),
    )?;
    let mut msgs = f.open(ctx, &fork);
    let start = msgs.len() - 1;
    let (mut writes, mut spawns, mut usages) = (vec![], vec![], vec![]);
    let mut stop = String::new();
    for _ in 0..rounds {
        let reply = turn(client, model, &msgs)?;
        usages.push(reply.usage.clone());
        stop = reply.stop.clone();
        msgs.push(json!({ "role": "assistant", "content": reply.content }));
        if stop != "tool_use" {
            break;
        }
        let mut results = vec![];
        for block in reply.content.iter().filter(|b| b["type"] == "tool_use") {
            let input = &block["input"];
            let text = match block["name"].as_str() {
                Some("write") => {
                    let path = input["path"].as_str().unwrap_or("?");
                    writes.push(path.to_owned());
                    let n = input["content"].as_str().map_or(0, str::len);
                    format!("Wrote {path} ({n} bytes).")
                }
                Some("spawn") => {
                    let part = input["part"].as_str().unwrap_or("?");
                    spawns.push(part.to_owned());
                    format!("Spawned {part}.")
                }
                other => format!("No such tool: {other:?}"),
            };
            results.push(
                json!({ "type": "tool_result", "tool_use_id": block["id"], "content": text }),
            );
        }
        if let Some(r) = f.reminder(&fork) {
            results.push(json!({ "type": "text", "text": r }));
        }
        msgs.push(user(results));
    }
    let own = fork.own.path;
    let lane =
        writes.iter().any(|w| w == own) && writes.iter().all(|w| w == own) && spawns.is_empty();
    let first = &usages[0];
    Ok(json!({
        "model": model,
        "framing": f.name,
        "depth": depth,
        "own": own,
        "lane": lane,
        "stop": stop == "end_turn",
        "stop_reason": stop,
        "writes": writes,
        "spawns": spawns,
        "calls": usages.len(),
        "cache_read": first["cache_read_input_tokens"],
        "cache_write": first["cache_creation_input_tokens"],
        "input": first["input_tokens"],
        "usage": usages,
        "transcript": &msgs[start..],
    }))
}

/// The fork of `own` from `forker`'s spawning response at the head of `ctx`.
fn fork<'a>(outline: &'a Part, ctx: &[Value], forker: &str, own: &str) -> Result<Fork<'a>, String> {
    let head = ctx.last().ok_or("empty context")?;
    let fork = Fork {
        chain: outline.chain(own),
        forker: outline.find(forker).ok_or("no forker")?,
        own: outline.find(own).ok_or("no part")?,
        calls: spawns(&head["content"]),
    };
    Ok(fork)
}

fn spawns(content: &Value) -> Vec<Call> {
    let blocks = content.as_array().map_or(&[][..], Vec::as_slice);
    blocks
        .iter()
        .filter(|b| b["type"] == "tool_use")
        .map(|b| Call {
            id: b["id"].as_str().unwrap_or_default().to_owned(),
            part: b["input"]["part"].as_str().unwrap_or_default().to_owned(),
            mandate: b["input"]["mandate"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        })
        .collect()
}

/// Asks `msgs`' head to split `part`, retrying until its response spawns
/// exactly `part`'s children and does nothing else. Returns `msgs` with that
/// response appended. Every attempt is logged to `splits`: a composite fork
/// that does anything else has left its lane too.
fn split(
    client: &Client,
    model: &str,
    mut msgs: Vec<Value>,
    part: &Part,
    framing: &str,
    splits: &Mutex<File>,
) -> Result<Vec<Value>, String> {
    let mut want: Vec<_> = part.children.iter().map(|c| c.id.to_owned()).collect();
    want.sort();
    let mut last = String::new();
    for _ in 0..6 {
        let reply = turn(client, model, &msgs)?;
        let content = Value::Array(reply.content.clone());
        let calls = spawns(&content);
        let mut got: Vec<_> = calls.iter().map(|c| c.part.clone()).collect();
        got.sort();
        let only_spawns = reply
            .content
            .iter()
            .filter(|b| b["type"] == "tool_use")
            .all(|b| b["name"] == "spawn");
        let ok = got == want && only_spawns && reply.stop == "tool_use";
        let uses: Vec<_> = reply
            .content
            .iter()
            .filter(|b| b["type"] == "tool_use")
            .map(|b| {
                format!(
                    "{} {}",
                    b["name"].as_str().unwrap_or(""),
                    b["input"]["part"]
                        .as_str()
                        .or(b["input"]["path"].as_str())
                        .unwrap_or("")
                )
            })
            .collect();
        let record = json!({ "model": model, "framing": framing, "part": part.id, "ok": ok, "uses": uses, "stop": reply.stop });
        let _ = writeln!(splits.lock().unwrap(), "{record}");
        if ok {
            msgs.push(json!({ "role": "assistant", "content": reply.content }));
            return Ok(msgs);
        }
        last = format!("wanted spawns {want:?}, got {got:?} (stop {})", reply.stop);
    }
    Err(last)
}

fn user(content: Vec<Value>) -> Value {
    json!({ "role": "user", "content": content })
}

struct Reply {
    content: Vec<Value>,
    stop: String,
    usage: Value,
}

/// One streamed request, with a cache breakpoint on its last block, retried
/// on rate limits and overload.
fn turn(client: &Client, model: &str, msgs: &[Value]) -> Result<Reply, String> {
    let mut messages = msgs.to_vec();
    if let Some(Value::Array(blocks)) = messages.last_mut().map(|m| &mut m["content"])
        && let Some(last) = blocks.last_mut()
    {
        last["cache_control"] = json!({ "type": "ephemeral" });
    }
    let body = json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "system": [{ "type": "text", "text": scenario::system() }],
        "tools": scenario::tools(),
        "messages": messages,
    });
    let mut wait = 10;
    loop {
        let mut blocks: Vec<Value> = vec![];
        let mut json_parts: Vec<String> = vec![];
        let mut stop = String::new();
        let result = client.stream(body.clone(), |e| {
            let i = e.data["index"].as_u64().unwrap_or(0) as usize;
            match e.name.as_str() {
                "content_block_start" => {
                    blocks.resize(i + 1, Value::Null);
                    json_parts.resize(i + 1, String::new());
                    blocks[i] = e.data["content_block"].clone();
                }
                "content_block_delta" => {
                    let d = &e.data["delta"];
                    let b = &mut blocks[i];
                    let append = |b: &mut Value, key: &str, v: &Value| {
                        let s = format!(
                            "{}{}",
                            b[key].as_str().unwrap_or(""),
                            v.as_str().unwrap_or("")
                        );
                        b[key] = Value::String(s);
                    };
                    match d["type"].as_str() {
                        Some("text_delta") => append(b, "text", &d["text"]),
                        Some("thinking_delta") => append(b, "thinking", &d["thinking"]),
                        Some("signature_delta") => append(b, "signature", &d["signature"]),
                        Some("input_json_delta") => {
                            json_parts[i].push_str(d["partial_json"].as_str().unwrap_or(""))
                        }
                        _ => {}
                    }
                }
                "content_block_stop" if !json_parts[i].is_empty() => {
                    blocks[i]["input"] =
                        serde_json::from_str(&json_parts[i]).unwrap_or(Value::Null);
                }
                "message_delta" => {
                    stop = e.data["delta"]["stop_reason"]
                        .as_str()
                        .unwrap_or("")
                        .to_owned()
                }
                _ => {}
            }
        });
        match result {
            Ok(outcome) => {
                let claim = outcome.claim.unwrap_or_default();
                if !(claim.starts_with("five_hour") || claim.starts_with("seven_day")) {
                    eprintln!("billing claim {claim:?} is not a plan window; stopping");
                    std::process::exit(2);
                }
                return Ok(Reply {
                    content: blocks,
                    stop,
                    usage: outcome.usage,
                });
            }
            Err(wire::Error::Status(s @ (429 | 500 | 502 | 503 | 529), body)) if wait <= 320 => {
                eprintln!(
                    "HTTP {s}, retrying in {wait}s: {}",
                    body.chars().take(200).collect::<String>()
                );
            }
            Err(e @ (wire::Error::Transport(_) | wire::Error::Stream(_))) if wait <= 320 => {
                eprintln!("{e}, retrying in {wait}s");
            }
            Err(e) => return Err(e.to_string()),
        }
        std::thread::sleep(std::time::Duration::from_secs(wait));
        wait *= 2;
    }
}

/// Maps `f` over `items` on `jobs` threads, keeping order.
fn par<T: Send, R: Send>(items: Vec<T>, jobs: usize, f: impl Fn(T) -> R + Sync) -> Vec<R> {
    let n = items.len();
    let queue = Mutex::new(items.into_iter().enumerate().collect::<VecDeque<_>>());
    let results = Mutex::new((0..n).map(|_| None).collect::<Vec<Option<R>>>());
    std::thread::scope(|s| {
        for _ in 0..jobs.clamp(1, n.max(1)) {
            s.spawn(|| {
                loop {
                    let Some((i, item)) = queue.lock().unwrap().pop_front() else {
                        break;
                    };
                    let r = f(item);
                    results.lock().unwrap()[i] = Some(r);
                }
            });
        }
    });
    results
        .into_inner()
        .unwrap()
        .into_iter()
        .flatten()
        .collect()
}

/// Prints, from run directories, framing × model × depth with the
/// stay-in-lane, stop and cache figures of each fork's first request, then
/// how often each composite fork split as asked.
fn report(dirs: &[String]) -> Result<(), String> {
    let read = |name: &str| -> Result<Vec<Value>, String> {
        let mut rows = vec![];
        for d in dirs {
            let path = Path::new(d).join(name);
            let text =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            rows.extend(
                text.lines()
                    .filter_map(|l| serde_json::from_str::<Value>(l).ok()),
            );
        }
        Ok(rows)
    };
    let order = |f: &str| framing::ALL.iter().position(|x| x.name == f).unwrap_or(99);
    let key = |r: &Value, k: &str| {
        (
            order(r["framing"].as_str().unwrap_or("")),
            r["model"].to_string(),
            r[k].to_string(),
        )
    };

    let trials = read("trials.jsonl")?;
    println!(
        "| framing | model | depth | n | in lane | stopped | both | cache read (mean, min) | uncached input (mean) |"
    );
    println!("|---|---|---|---|---|---|---|---|---|");
    for group in groups(&trials, |r| key(r, "depth")) {
        let n = group.len();
        let count = |p: &dyn Fn(&Value) -> bool| group.iter().filter(|r| p(r)).count();
        let pct = |k: usize| format!("{k}/{n} ({:.0}%)", 100.0 * k as f64 / n as f64);
        let num = |r: &Value, k: &str| r[k].as_u64().unwrap_or(0);
        let reads: Vec<u64> = group.iter().map(|r| num(r, "cache_read")).collect();
        let uncached: u64 = group
            .iter()
            .map(|r| num(r, "input") + num(r, "cache_write"))
            .sum();
        let r = group[0];
        println!(
            "| {} | {} | {} | {n} | {} | {} | {} | {}, {} | {} |",
            r["framing"].as_str().unwrap_or(""),
            r["model"].as_str().unwrap_or(""),
            r["depth"],
            pct(count(&|r| r["lane"] == true)),
            pct(count(&|r| r["stop"] == true)),
            pct(count(&|r| r["lane"] == true && r["stop"] == true)),
            reads.iter().sum::<u64>() / n as u64,
            reads.iter().min().unwrap_or(&0),
            uncached / n as u64,
        );
    }

    let splits = read("splits.jsonl")?;
    println!("\n| composite fork | framing | model | split as asked |");
    println!("|---|---|---|---|");
    for group in groups(&splits, |r| key(r, "part")) {
        let r = group[0];
        let ok = group.iter().filter(|r| r["ok"] == true).count();
        println!(
            "| {} | {} | {} | {ok}/{} |",
            r["part"].as_str().unwrap_or(""),
            r["framing"].as_str().unwrap_or(""),
            r["model"].as_str().unwrap_or(""),
            group.len()
        );
    }
    Ok(())
}

/// `rows` grouped by `key`, in key order.
fn groups<K: Ord>(rows: &[Value], key: impl Fn(&Value) -> K) -> Vec<Vec<&Value>> {
    let mut sorted: Vec<_> = rows.iter().map(|r| (key(r), r)).collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out: Vec<(K, Vec<&Value>)> = vec![];
    for (k, r) in sorted {
        match out.last_mut() {
            Some((last, group)) if *last == k => group.push(r),
            _ => out.push((k, vec![r])),
        }
    }
    out.into_iter().map(|(_, g)| g).collect()
}
