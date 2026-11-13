#!/usr/bin/env bash
# Spawn takes one shape per kind of member, and the meta tools return at
# once when there is nothing to wait for. A scripted orchestrator in a
# repository:
# - spawns with the old `tasks` list, and a fork given a `branch`: each is
#   said back, and nothing starts; then a fork `helper` and an engineer
#   whose `dir` is beside its own directory, and one whose `dir` is under
#   it, each of which works there;
# - starts a quick background command, lets it land, then `wait`s on it
#   by name: it returns at once;
# - starts an engineer in the background whose script sleeps, stops it,
#   and waits: the engineer's tidy-up turn ends with `compact_and_report`,
#   and its report is what the orchestrator gets.
# Then Max writes again, and the orchestrator resumes `helper` quoting his
# first message by its label: the resumed helper is given his words
# verbatim. Last it prints the roles part of the orchestrator's
# instructions, and the `spawn` schema of the orchestrator and of an
# engineer, which starts nothing fresh, so has no `start`. Free (scripted
# model), about 30 s; it leaves the brain up:
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
    tool("spawn", {"tasks": [{"name": "step", "mandate": "say hi"}]}),
    tool("spawn", {"fork": [{"name": "step", "branch": "MC-x", "mandate": "say hi"}]}),
    tool("bash", {"command": "mkdir -p sub/repo"}),
    tool("spawn", {
        "fork": [{"name": "helper", "mandate": "say Helper is done."}],
        "start": {"brief": "A demo.", "engineer": [
            {"name": "beside", "dir": "../beside", "mandate": "tool bash {\"command\": \"pwd\"}\nsay Worked beside."},
            {"name": "near", "dir": "sub/repo", "mandate": "tool bash {\"command\": \"pwd\"}\nsay Worked under."},
        ]},
    }),
    tool("background", {"name": "quick", "tool": "bash", "input": {"command": "echo quick done"}}),
    tool("bash", {"command": "sleep 2"}),
    tool("wait", {"on": ["quick"]}),
    tool("background", {"name": "eng", "tool": "spawn", "input": {"start": {"engineer": [{"name": "eng", "mandate": engineer}]}}}),
    tool("bash", {"command": "sleep 3"}),
    tool("stop", {"call": "eng"}),
    tool("wait", {"on": ["eng"]}),
    "say Done.",
]))
PY
)
chat=$(drive chat orchestrator "$repo" "$mandate")
drive settle "$chat" --timeout 90 >/dev/null
port=$(cat .drive.ignore/meta-tidy/port)
context() { curl -sk "https://localhost:$port/context/$1"; }
first=$(context "$chat" | grep -o 'max-[0-9a-f]\{8\}' | head -1)
resume=$(python3 - "$first" <<'PY'
import json, sys
print("tool spawn " + json.dumps({"resume": [{"agent": "helper", "mandate": "say Resumed, with Max's words.", "quotes": [sys.argv[1]]}]}) + "\nsay Done again.")
PY
)
drive say "$chat" "$resume" >/dev/null
drive settle "$chat" --timeout 60 >/dev/null
drive log "$chat"
eng=$(curl -sk "https://localhost:$port/view" | jq -r '.rows[] | .Task? | select(.name == "eng") | .id' | head -1)
echo "== The engineer, stopped mid-sleep, ends its tidy-up turn with compact_and_report:"
drive log "$eng"
echo "== The roles in the orchestrator's instructions:"
context "$chat" | jq -r '.[0].system | fromjson | .[].text | select(contains("The roles you can start fresh"))' \
  | grep -A4 '^The roles you can start fresh'
helper=$(curl -sk "https://localhost:$port/view" | jq -r '.rows[] | .Task? | select(.name == "helper" and .origin == "resume") | .id' | head -1)
echo "== The resumed helper's messages from its spawn: the quote, then its mandate. It quotes Max's first message ($first):"
context "$helper" | jq -r '.[-3:-1][] | .content | fromjson | map(.text) | join("\n")' | cut -c1-160
echo "== spawn's schema, for the orchestrator and for the engineer (which starts nothing fresh):"
fields() { context "$1" | jq -c '.[0].tools | fromjson | .[] | select(.name == "spawn") | .input_schema.properties | {spawn: keys, fork: (.fork.items.properties | keys), resume: (.resume.items.properties | keys), start: (.start.properties // {} | with_entries(.value |= (.items.properties // null | if . then keys else "shared" end)))}'; }
fields "$chat"
fields "$eng"
