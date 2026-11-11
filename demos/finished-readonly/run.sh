#!/usr/bin/env bash
# Max talks to an agent's subagents only while they work. A scripted
# orchestrator spawns two forks whose last turns stream slowly. Max writes
# to each during that last turn: `talker` ends its turn with a reply, and
# `reporter` with compact_and_report. Neither finishes there: the words
# keep the turn open, the held-back answer is just a message in its log,
# and its next turn's answer is the result the orchestrator gets. Once
# they have finished, Max continuing one is refused (only the
# orchestrator can, with `spawn` and `resume`), while the orchestrator's
# own chat, which faced him, he can still continue. Free (scripted
# model), about 40 s; it leaves the brain up to look at the page:
# `node dev/drive.mjs down --brain readonly`.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain readonly "${@:2}"; }
at() { TZ=Pacific/Auckland date +%T; }
drive down --wipe >/dev/null
drive up
port=$(cat .drive.ignore/readonly/port)
view() { curl -sk "https://localhost:$port/view"; }
id_of() { view | jq -r --arg n "$1" '.rows[] | .Task? // empty | select(.name == $n) | .id'; }
waiting() { view | jq -r --arg id "$1" '.rows["task:" + $id].Task.waiting.kind'; }
act() { target/release/examples/act "$port" "$@" || true; }
repo=$PWD/.drive.ignore/readonly/repo
demos/brain/scratch.sh "$repo" >/dev/null
talker='slow 1000 say My first answer, which Max'"'"'s words hold back.'
reporter='slow 1000 tool compact_and_report {\"carry\": \"Nothing to carry.\", \"report\": \"My first report, which Max'"'"'s words hold back.\", \"summary\": \"report\"}'
mandate="tool spawn {\"summary\": \"two forks\", \"tasks\": [{\"name\": \"talker\", \"mandate\": \"$talker\"}, {\"name\": \"reporter\", \"mandate\": \"$reporter\"}]}
say Both forks have reported."
chat=$(drive chat orchestrator "$repo" "$mandate")
echo "== $(at) the orchestrator $chat spawns talker and reporter; their last turns stream slowly"
until [ -n "$(id_of talker)" ] && [ -n "$(id_of reporter)" ]; do sleep 0.2; done
talker=$(id_of talker) reporter=$(id_of reporter)
sleep 2
echo "== $(at) Max writes to each while its last turn is in flight"
act say "$talker" "say Heard you, Max: this is my real result."
act say "$reporter" "say Heard you, Max: this is my real report."
drive settle "$chat" >/dev/null
echo "== $(at) talker's log: its first answer stays a message, and its next is the result"
drive log "$talker"
echo "== $(at) reporter's log: the compaction and its report stay, and Max's words follow"
drive log "$reporter"
echo "== $(at) what the orchestrator got:"
drive log "$chat"
echo "== $(at) Max continues the finished talker: refused"
act continue "$talker" "say Can I keep going?"
echo "== $(at) Max marks the orchestrator's chat done, then continues it, since it faced him: a new chat"
act done "$chat"
until [ "$(waiting "$chat")" = done ]; do sleep 0.2; done
act continue "$chat" "say Carrying on with Max."
echo "== the page: https://mx-hv-mj.local:$port/#$talker (finished, read-only) and https://mx-hv-mj.local:$port/#$chat"
