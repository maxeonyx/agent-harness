#!/usr/bin/env bash
# An agent waits on and stops calls by the label Max's words gave them. A
# scripted chat runs `sleep 6`; Max writes 1 s in, so the call moves to the
# background, answered with its label, and it lands. The chat then waits on
# that label and is told it has landed. It runs `sleep 120`, Max writes
# again, and the chat waits on that call's label until Max writes once more,
# then stops it by its label. Free (scripted model), about 30 s; it leaves
# the brain up: `node dev/drive.mjs down --brain wait-label`.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain wait-label "${@:2}"; }
at() { TZ=Pacific/Auckland date +%T; }
label() { drive log "$chat" | grep -o 'background as `call-[0-9a-f]*`' | tail -1 | grep -o 'call-[0-9a-f]*'; }
drive down --wipe >/dev/null
drive up
repo=$PWD/.drive.ignore/wait-label/repo
demos/brain/scratch.sh "$repo" >/dev/null

chat=$(drive chat engineer "$repo" 'tool bash {"command": "sleep 6; echo the short sleep ended"}')
echo "== $(at) the chat $chat runs sleep 6; Max writes 1 s in"
sleep 1
drive say "$chat" "say Heard you."
drive settle "$chat" >/dev/null
first=$(label)
echo "== $(at) its round moved to the background as $first; waiting for it to land"
until drive log "$chat" | grep -q "has settled"; do sleep 1; done
drive settle "$chat" >/dev/null
echo "== $(at) Max has it wait on $first, which has landed"
drive say "$chat" "tool wait {\"on\": [\"$first\"], \"summary\": \"wait on the short sleep\"}"
drive settle "$chat" >/dev/null
drive log "$chat" | tail -4

echo "== $(at) the chat runs sleep 120; Max writes 1 s in"
drive say "$chat" 'tool bash {"command": "sleep 120"}'
sleep 1
drive say "$chat" "say Heard you again."
drive settle "$chat" >/dev/null
second=$(label)
echo "== $(at) its round moved to the background as $second; Max has it wait on that label"
drive say "$chat" "tool wait {\"on\": [\"$second\"], \"summary\": \"wait on the long sleep\"}"
sleep 3
echo "== $(at) Max writes, which ends the wait, and has it stop $second by its label"
drive say "$chat" "tool stop {\"call\": \"$second\", \"summary\": \"stop the long sleep\"}"
until drive log "$chat" | grep -q "$second.*has settled\|has settled.*$second"; do sleep 1; done
drive settle "$chat" >/dev/null
drive log "$chat" | tail -24
