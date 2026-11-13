#!/usr/bin/env bash
# A chat with Max ends its turn while its background work runs; an agent
# working on its own cannot. A scripted chat starts `job` (a 10 s sleep)
# in the background, is told it can end its turn meanwhile, and does: it
# waits on Max with `job` still running, and `job`'s result starts its
# next turn. Then Max asks for `job2` and a `wait`, and writes during the
# wait: the wait ends telling the chat to answer him and end its turn,
# and it does, with `job2` still running until it lands. Last the chat
# spawns a fork, which is autonomous: it is told it cannot finish until
# its background work settles, Max's words end its wait telling it to
# answer and wait again, and when it ends its turn anyway it is told that
# is not its result. Free (scripted model), about 40 s; it leaves the
# brain up: `node dev/drive.mjs down --brain chat-turns`.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain chat-turns "${@:2}"; }
at() { TZ=Pacific/Auckland date +%T; }
drive down --wipe >/dev/null
drive up
port=$(cat .drive.ignore/chat-turns/port)
view() { curl -sk "https://localhost:$port/view"; }
id_of() { view | jq -r --arg n "$1" '.rows[] | .Task? // empty | select(.name == $n) | .id'; }
# What a task waits on, and the background work it still owns.
state() {
  view | jq -r --arg id "$1" '.rows["task:" + $id].Task
    | "waiting: \(.waiting.kind // "nothing"); background: \([.background[] | .tool] | join(", ") | if . == "" then "none" else . end)"'
}
act() { target/release/examples/act "$port" "$@" || true; }
# The harness's words to the agent, from its full log after the line
# matching $2 (a fork's log starts with its parent's context).
told() { drive log "$1" --full | sed -n "/${2:-.}/,\$p" | grep -o '\[harness\] Max wrote to you[^"]*\|Started `[^`]*` ([^)]*) in the background[^"]*\|Your turn ended, but[^"]*' | sort -u; }
repo=$PWD/.drive.ignore/chat-turns/repo
demos/brain/scratch.sh "$repo" >/dev/null
bg() { printf 'tool background {"name": "%s", "tool": "bash", "input": {"command": "sleep %s; echo %s done"}}' "$1" "$2" "$1"; }

chat=$(drive chat orchestrator "$repo" "$(bg job 10)
say job runs in the background; I will tell you when it lands.
say job has landed.")
echo "== $(at) the chat $chat starts job in the background and ends its turn"
drive settle "$chat" >/dev/null
state "$chat"
echo "== $(at) waiting for job to land and start a new turn"
until drive log "$chat" | grep -q "^agent: job has landed"; do sleep 1; done
state "$chat"
drive log "$chat"

echo "== $(at) Max asks for job2 and a wait, then writes during the wait"
drive say "$chat" "$(bg job2 12)
tool wait {}"
sleep 3
drive say "$chat" "say Update: job2 still runs; I will tell you when it lands.
say job2 has landed."
drive settle "$chat" >/dev/null
echo "== $(at) answered at once, turn ended, job2 still running:"
state "$chat"
until drive log "$chat" | grep -q "^agent: job2 has landed"; do sleep 1; done
echo "== $(at) job2 landed and started a new turn:"
state "$chat"
echo "== what the chat was told:"
told "$chat"

echo "== $(at) the chat spawns an autonomous fork that backgrounds a job and waits"
fork_script="$(bg fjob 8)\ntool wait {}"
fork_script=${fork_script//\"/\\\"}
drive say "$chat" "tool spawn {\"fork\": [{\"name\": \"fork\", \"mandate\": \"$fork_script\"}]}
say The fork has reported."
until [ -n "$(id_of fork)" ]; do sleep 0.2; done
fork=$(id_of fork)
sleep 3
echo "== $(at) Max writes to the fork during its wait"
act say "$fork" "say Heard you, Max: fjob still runs.
tool wait {}
say fjob done: my result."
drive settle "$chat" >/dev/null
echo "== the fork's log:"
drive log "$fork"
echo "== what the fork was told:"
told "$fork" 'You are now `fork`'
echo "== the chat:"
drive log "$chat"
