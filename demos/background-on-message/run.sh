#!/usr/bin/env bash
# Max's words reach an agent waiting on a spawn. A scripted orchestrator
# spawns a fork that sleeps 20 s; Max writes 2 s in. A few seconds later
# the spawn moves to the background: the orchestrator's tool use is
# answered with the call's label, and it replies to Max while the fork
# still runs. When the fork ends, its whole result reaches the orchestrator
# as a message that wakes it. Free (scripted model), about 40 s; it leaves
# the brain up: `node dev/drive.mjs down --brain background`.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain background "${@:2}"; }
at() { TZ=Pacific/Auckland date +%T; }
drive down --wipe >/dev/null
drive up
repo=$PWD/.drive.ignore/background/repo
demos/brain/scratch.sh "$repo" >/dev/null
mandate='tool spawn {"tasks": [{"name": "sleeper", "mandate": "tool bash {\"command\": \"sleep 20; echo the sleeper woke\"}\nsay I slept 20 s and woke."}]}'
chat=$(drive chat orchestrator "$repo" "$mandate")
echo "== $(at) the orchestrator $chat spawns a fork that sleeps 20 s"
sleep 2
echo "== $(at) Max writes while it waits"
drive say "$chat" "say Heard you, Max: the sleeper runs on in the background."
drive settle "$chat" >/dev/null
echo "== $(at) the orchestrator answered Max; the fork still runs:"
drive log "$chat"
echo "== $(at) waiting for the fork's result to wake it"
until drive log "$chat" | grep -q "has settled"; do sleep 1; done
drive settle "$chat" >/dev/null
echo "== $(at) the fork's result arrived as a message:"
drive log "$chat"
