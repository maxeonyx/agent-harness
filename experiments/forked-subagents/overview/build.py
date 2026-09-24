"""Build the forked-subagents overview: one self-contained HTML file, from the recorded evidence and the source.

Usage: python3 overview/build.py [out.html]   (run from experiments/forked-subagents; default overview.ignore.html)
Needs `target/release/forks` built, and the benchmark runs under runs.ignore/.
"""

import datetime
import glob
import hashlib
import json
import pathlib
import re
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
EXPERIMENT = HERE.parent
REPO = EXPERIMENT.parent.parent
RUNS = EXPERIMENT / "runs.ignore"

FEATURED = {
    "clean": "20260925-003053-trial001-before-stop-fork-rep1",
    "runaway": "20260924-232819-trial001-full-stop-fork-rep1",
}

PRICES = {
    "anthropic/claude-sonnet-5": {"in": 2.0, "read": 0.2, "write": 2.5, "out": 10.0},
    "openai/gpt-5.6-luna": {"in": 0.2, "read": 0.02, "write": 0.25, "out": 1.2},
}

SOURCES = [
    "src/main.rs", "src/session.rs", "src/agent.rs", "src/framing.rs", "src/wire.rs", "src/limb.rs",
    "src/record.rs", "src/face.rs", "src/bench.rs", "src/rescore.rs", "src/bin/fake_provider.rs",
    "tests/scenario.rs", "Cargo.toml", "README.md", "probe/probe.py", "probe/runs.md",
]
DOCS = [
    "docs/process/experiments/forked-subagents-brief.md",
    "docs/process/experiments/forked-subagents-outcome.md",
    "docs/design/lifecycle.md",
    "docs/design/model-framing.md",
    "docs/process/PLAN.md",
    "docs/source-notes/agent-hierarchy.md",
    "docs/source-notes/handoff-improvements.md",
]


def run(*command, cwd=EXPERIMENT):
    return subprocess.run(command, cwd=cwd, capture_output=True, text=True).stdout


def rows():
    """Every benchmark trial, as scored by `forks rescore` — the same scorer the outcome doc used."""
    found = []
    for bench in sorted(RUNS.glob("*-bench")):
        out = HERE / "rows.ignore.json"
        subprocess.run(["target/release/forks", "rescore", str(bench.relative_to(EXPERIMENT)), "--json", str(out)], cwd=EXPERIMENT, capture_output=True)
        if out.exists():
            found += json.loads(out.read_text())
            out.unlink()
    return found


def trial(row):
    directory = EXPERIMENT / row["dir"]
    summary = json.loads((directory / "summary.json").read_text())
    agents = []
    for agent in summary["agents"]:
        agents.append({
            "path": agent["path"], "depth": agent["depth"], "state": agent["state"], "fresh": agent["fresh"],
            "requests": agent["requests"], "cached": agent["cached_in"], "written": agent["written_in"],
            "uncached": agent["uncached_in"], "out": agent["out"], "cost": agent["cost"], "millis": agent["millis"],
            "task": agent.get("task"), "handoff": agent.get("handoff", ""),
            "tools": [{"name": call["name"], "args": call.get("arguments", ""), "result": (call.get("result") or "")[:4000]} for call in agent["tool_calls"]],
        })
    price = PRICES[row["model"]]
    tokens = {
        "read": sum(a["cached"] for a in agents),
        "write": sum(a["written"] for a in agents),
        "plain": sum(a["uncached"] - a["written"] for a in agents),
        "out": sum(a["out"] for a in agents),
    }
    dollars = {
        "read": tokens["read"] * price["read"] / 1e6,
        "write": tokens["write"] * price["write"] / 1e6,
        "plain": tokens["plain"] * price["in"] / 1e6,
        "out": tokens["out"] * price["out"] / 1e6,
    }
    keep = ["model", "cut", "words", "mode", "outcome", "valid", "fault_kind", "detail", "structure_ok", "correct", "cost", "millis",
            "overreached", "unscoreable", "leaves", "leaf_overreach", "regions", "region_overreach", "child_first_cached_in", "child_first_uncached_in"]
    return {"id": directory.name, "bench": directory.parent.name, **{k: row[k] for k in keep}, "agents": agents, "tokens": tokens, "dollars": dollars, "root_handoff": summary.get("root_handoff", "")}


def wire(trial_id):
    """The featured runs' every request and response, with identical messages stored once."""
    directory = next(RUNS.glob(f"*-bench/{trial_id}"))
    messages, requests, pending = {}, [], {}

    def intern(message):
        text = json.dumps(message, sort_keys=True)
        key = hashlib.sha1(text.encode()).hexdigest()[:12]
        messages.setdefault(key, message)
        return key

    for line in (directory / "wire.jsonl").read_text().splitlines():
        entry = json.loads(line)
        at = datetime.datetime.fromisoformat(entry["at"]).timestamp()
        if entry["kind"] == "request":
            pending[entry["agent"]] = {"agent": entry["agent"], "sent": at, "messages": [intern(m) for m in entry["body"]["messages"]]}
        elif entry["kind"] == "response":
            request = pending.pop(entry["agent"])
            body = entry["body"]
            usage = body["usage"]
            message = body["choices"][0]["message"]
            request.update({
                "returned": at,
                "prompt": usage["prompt_tokens"], "cached": usage["prompt_tokens_details"]["cached_tokens"],
                "written": usage["prompt_tokens_details"]["cache_write_tokens"], "out": usage["completion_tokens"],
                "cost": usage["cost"], "via": body.get("provider"),
                "content": message.get("content") or "", "reasoning": message.get("reasoning") or "",
                "calls": [{"name": c["function"]["name"], "args": c["function"]["arguments"]} for c in message.get("tool_calls") or []],
            })
            requests.append(request)
    start = min(r["sent"] for r in requests)
    for request in requests:
        request["sent"] -= start
        request["returned"] -= start
    return {"messages": messages, "requests": requests}


def modules():
    found = []
    for path in [p for p in SOURCES if p.endswith(".rs")]:
        text = (EXPERIMENT / path).read_text()
        doc_lines = []
        for line in text.splitlines():
            if not line.startswith("//!"):
                break
            if not line[3:].strip():
                if doc_lines:
                    break
                continue
            doc_lines.append(line[3:].strip())
        doc = " ".join(doc_lines)
        uses = set(re.findall(r"crate::(\w+)", text)) | set(re.findall(r"^mod (\w+);", text, re.M))
        fns = [{"name": m.group(2), "line": text[: m.start()].count("\n") + 1, "pub": bool(m.group(1))}
               for m in re.finditer(r"^\s*(pub(?:\(crate\))? )?(?:async )?fn (\w+)", text, re.M)]
        found.append({"path": path, "name": pathlib.Path(path).stem, "lines": text.count("\n") + 1, "doc": doc, "uses": uses, "fns": fns})
    names = {module["name"] for module in found}
    for module in found:
        module["uses"] = sorted(module["uses"] & names - {module["name"]})
    return found


def first_request(trial_id, depth):
    """The first request an agent at `depth` sent in a recorded trial: exactly what that child was shown."""
    directory = next(RUNS.glob(f"*-bench/{trial_id}"))
    for line in (directory / "wire.jsonl").read_text().splitlines():
        entry = json.loads(line)
        if entry["kind"] == "request" and entry["agent"].count("›") == depth:
            return entry["agent"], entry["body"]["messages"]
    raise LookupError(f"{trial_id}: no request at depth {depth}")


def tool_message(trial_id, starts):
    directory = next(RUNS.glob(f"*-bench/{trial_id}"))
    for line in (directory / "wire.jsonl").read_text().splitlines():
        entry = json.loads(line)
        if entry["kind"] == "request":
            for message in entry["body"]["messages"]:
                if message["role"] == "tool" and str(message.get("content", "")).startswith(starts):
                    return entry["agent"], message
    raise LookupError(f"{trial_id}: no tool message starting {starts!r}")


def samples():
    """What children were actually shown, per cut — the tails of real first requests."""
    found = {}
    for cut, trial_id in [("full", FEATURED["runaway"]), ("own", "20260924-233629-trial005-own-stop-fork-rep1"),
                          ("before", FEATURED["clean"]), ("fresh", "20260925-003334-trial001-full-explained-fresh-rep1")]:
        agent, messages = first_request(trial_id, 1)
        found[cut] = {"trial": trial_id, "agent": agent, "count": len(messages), "tail": messages if cut == "fresh" else messages[-2:]}
    agent, messages = first_request("20260925-003147-trial002-before-explained-fork-rep1", 2)
    found["explained"] = {"trial": "20260925-003147-trial002-before-explained-fork-rep1", "agent": agent, "message": messages[-1]}
    found["system"] = {"message": messages[0]}
    for name, trial_id, starts in [("refusal", "20260925-003147-trial002-before-explained-fork-rep1", "Error: this run allows"),
                                   ("scope", FEATURED["clean"], "Every agent you launched")]:
        agent, message = tool_message(trial_id, starts)
        found[name] = {"trial": trial_id, "agent": agent, "message": message}
    directory = next(RUNS.glob(f"*-bench/{FEATURED['clean']}"))
    body = json.loads((directory / "wire.jsonl").read_text().splitlines()[0])["body"]
    found["tools"] = body["tools"]
    found["root_task"] = body["messages"][1]
    policy = next(RUNS.glob("*-bench/fixture/ledgers/POLICY.md"))
    found["policy"] = policy.read_text()
    return found


def tests():
    output = run("cargo", "test", "--release", "--", "--test-threads=8")
    output += run("cargo", "test", "--release")
    passed = set(re.findall(r"^test (\w+) \.\.\. ok", output, re.M))
    failed = set(re.findall(r"^test (\w+) \.\.\. FAILED", output, re.M))
    text = (EXPERIMENT / "tests/scenario.rs").read_text()
    found = []
    for m in re.finditer(r"#\[(?:tokio::)?test[^\]]*\]\s*(?:async )?fn (\w+)", text):
        name = m.group(1)
        found.append({"name": name, "line": text[: m.start()].count("\n") + 2, "state": "passed" if name in passed else "failed" if name in failed else "unknown"})
    return found


def commits():
    log = run("git", "log", "--first-parent", "--format=%H%x09%aI%x09%an%x09%s", "origin/main..HEAD", cwd=REPO)
    found = []
    for line in log.splitlines():
        sha, date, author, subject = line.split("\t", 3)
        if author.startswith("github-actions") or subject.startswith("Merge"):
            continue
        found.append({"sha": sha, "date": date, "subject": subject})
    return found[::-1]


def main():
    out = EXPERIMENT / (sys.argv[1] if len(sys.argv) > 1 else "overview.ignore.html")
    scored = rows()
    trials = [trial(row) for row in scored]
    data = {
        "generated": datetime.datetime.now().astimezone().isoformat(timespec="minutes"),
        "head": run("git", "rev-parse", "--short", "HEAD", cwd=REPO).strip(),
        "trials": trials,
        "featured": FEATURED,
        "wire": {name: wire(trial_id) for name, trial_id in FEATURED.items()},
        "sources": {path: (EXPERIMENT / path).read_text() for path in SOURCES} | {path: (REPO / path).read_text() for path in DOCS},
        "modules": modules(),
        "samples": samples(),
        "tests": tests(),
        "commits": commits(),
        "prices": PRICES,
    }
    page = (HERE / "page.html").read_text()
    page = page.replace("/*CSS*/", (HERE / "app.css").read_text())
    page = page.replace("/*JS*/", "\n".join(path.read_text() for path in sorted((HERE / "js").glob("*.js"))))
    page = page.replace("/*DATA*/", "const DATA = " + json.dumps(data).replace("</", "<\\/") + ";")
    out.write_text(page)
    print(f"{out}  {len(page) / 1e6:.1f} MB  {len(trials)} trials  {len(data['tests'])} tests")


main()
