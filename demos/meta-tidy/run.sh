#!/usr/bin/env bash
# The meta tools refuse only what means nothing, and return at once when
# there is nothing to wait for. A scripted orchestrator in a repository:
# - spawns a fork given a misspelt `task_base`, forks given a `brief`, a
#   fork given a `branch`, and an engineer whose `repo` is outside its
#   directory: each is said back, and nothing starts; then an engineer
#   whose `repo` is under it, which works there;
# - starts a quick background command, lets it land, then `wait`s on it
#   by name, and on nothing: both return at once;
# - starts an engineer in the background whose script sleeps, stops it,
#   and waits: the engineer's tidy-up turn ends with `compact_and_report`,
#   and its report is what the orchestrator gets.
# Last it prints the roles part of the orchestrator's instructions (each
# role with its tool sets), and the engineer's `spawn` task schema: an
# engineer starts nothing fresh, so it has no `role`, `branch`, `base` or
# `repo`. Free (scripted model), about 30 s; it leaves the brain up:
# `node dev/drive.mjs down --brain meta-tidy`.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain meta-tidy "${@:2}"; }
drive down --wipe >/dev/null
drive up
repo=$PWD/.drive.ignore/meta-tidy/repo
demos/brain/scratch.sh "$repo" >/dev/null
mandate=$(python3 - <<'PY'
import json
def tool(name, input):
    return f"tool {name} {json.dumps(input)}"
engineer = "\n".join([
    tool("bash", {"command": "sleep 60"}),
    tool("compact_and_report", {"carry": "Nothing done.", "report": "Stopped before I began: nothing was built."}),
])
print("\n".join([
    tool("spawn", {"tasks": [{"name": "step", "branch": "MC-x", "task_base": "main", "mandate": "say hi"}]}),
    tool("spawn", {"brief": "Context.", "tasks": [{"name": "step", "mandate": "say hi"}], "summary": "fork a step"}),
    tool("spawn", {"tasks": [{"name": "step", "branch": "MC-x", "mandate": "say hi"}]}),
    tool("spawn", {"tasks": [{"name": "far", "role": "engineer", "repo": "../..", "mandate": "say hi"}]}),
    tool("bash", {"command": "mkdir -p sub/repo"}),
    tool("spawn", {"tasks": [{"name": "near", "role": "engineer", "repo": "sub/repo", "mandate": "tool bash {\"command\": \"pwd\"}\nsay Worked there."}]}),
    tool("background", {"name": "quick", "tool": "bash", "input": {"command": "echo quick done"}}),
    tool("bash", {"command": "sleep 2"}),
    tool("wait", {"on": ["quick"]}),
    tool("wait", {}),
    tool("background", {"name": "eng", "tool": "spawn", "input": {"tasks": [{"name": "eng", "role": "engineer", "mandate": engineer}]}}),
    tool("bash", {"command": "sleep 3"}),
    tool("stop", {"call": "eng"}),
    tool("wait", {"on": ["eng"]}),
    "say Done.",
]))
PY
)
chat=$(drive chat orchestrator "$repo" "$mandate")
drive settle "$chat" --timeout 90 >/dev/null
drive log "$chat"
port=$(cat .drive.ignore/meta-tidy/port)
context() { curl -sk "https://localhost:$port/context/$1"; }
eng=$(curl -sk "https://localhost:$port/view" | jq -r '.rows[] | .Task? | select(.name == "eng") | .id' | head -1)
echo "== The engineer, stopped mid-sleep, ends its tidy-up turn with compact_and_report:"
drive log "$eng"
echo "== The roles in the orchestrator's instructions:"
context "$chat" | jq -r '.[0].system | fromjson | .[].text | select(contains("The roles you can start fresh"))' \
  | grep -A4 '^The roles you can start fresh'
echo "== spawn's task fields, for the orchestrator and for the engineer (which starts nothing fresh):"
fields() { context "$1" | jq -c '.[0].tools | fromjson | .[] | select(.name == "spawn") | .input_schema.properties | {spawn: keys, task: (.tasks.items.properties | keys)}'; }
fields "$chat"
fields "$eng"
