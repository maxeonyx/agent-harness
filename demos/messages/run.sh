#!/usr/bin/env bash
# Agents tell each other things with the `message` tool. Scripted: no
# model call. A message's words become its reader's script, so each
# message here says what its reader does next.
#
# 1. A chat starts a background fork, `kid`, and waits on Max. `kid` tells
#    its owner something: the chat wakes, reads "`kid`, which you started,
#    wrote this", answers, and keeps its place in Max's queue.
# 2. Max has the chat tell `kid`, mid `sleep 120`, something with his
#    words quoted by label: `kid` reads it, "which started you", with the
#    quote, within 30 s.
# 3. One of Max's chats tells another, by task id; a name that is no one
#    near it fails saying whom it can reach.
# 4. The circle: an owner waits on `listener`, a member that waits for
#    words with a `wait` naming nothing. The owner's wait ends at once,
#    saying so; its message ends the member's wait.
#
# Free, about 1 min; it leaves the brain up:
# `node dev/drive.mjs down --brain messages`.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain messages "${@:2}"; }
at() { TZ=Pacific/Auckland date +%T; }
# A scripted `tool NAME INPUT` line, its input built by jq.
tool() { echo "tool $1 $(jq -cn "${@:3}" "$2")"; }
drive down --wipe >/dev/null
drive up
dir=$PWD/.drive.ignore/messages/dir
mkdir -p "$dir"

kid="$(tool message '{to: "owner", words: "say Thanks, kid: I read you.", summary: "tell the owner"}')
$(tool bash '{command: "sleep 120; echo kid slept", summary: "sleep"}')
say I slept and woke."
chat=$(drive chat orchestrator "$dir" "$(tool background '{name: "kid", tool: "spawn", input: {fork: [{name: "kid", mandate: $m}]}, summary: "start kid"}' --arg m "$kid")
say Started kid.")
echo "== $(at) 1. kid tells the chat $chat, which waits on Max, something"
until drive log "$chat" | grep -q "^agent: Thanks, kid"; do sleep 0.5; done
drive log "$chat" | grep -B1 -A3 "which you started"
echo "== $(at) the chat: $(drive settle "$chat")"

echo
echo "== $(at) 2. Max has the chat tell kid, mid sleep 120, something, quoting him"
drive say "$chat" 'Tell kid to say it heard you.' >/dev/null
drive settle "$chat" >/dev/null
quote=$(drive log "$chat" --full | grep -o 'max-[0-9a-f]\{8\}' | tail -1)
drive say "$chat" "$(tool message '{to: "kid", words: "say Heard you, owner.", quotes: [$q], summary: "tell kid"}' --arg q "$quote")
say Told kid." >/dev/null
drive settle "$chat" >/dev/null
drive log "$chat" | grep "Delivered"
told=$(date +%s)
port=$(cat .drive.ignore/messages/port)
view() { curl -sk "https://127.0.0.1:$port/view"; }
kid_id=$(view | jq -r '.rows | to_entries[] | select(.value.Task.name? == "kid") | .value.Task.id')
until drive log "$kid_id" | grep -q "^agent: Heard you, owner"; do sleep 1; done
echo "== $(at) kid read it $(($(date +%s) - told)) s after it was told:"
drive log "$kid_id" --full | grep -A6 "which started you"

echo
echo "== $(at) 3. Max's chat A tells his chat B something, by task id"
b=$(drive chat orchestrator "$dir" "say I am B.")
drive settle "$b" >/dev/null
drive say "$chat" "$(tool message '{to: "nobody", words: "say Hi.", summary: "tell nobody"}')
$(tool message '{to: $b, words: "say B heard A.", summary: "tell B"}' --arg b "$b")
say Told B." >/dev/null
drive settle "$chat" >/dev/null
drive log "$chat" | grep -E "No agent|Delivered" | tail -2
until drive log "$b" | grep -q "^agent: B heard A"; do sleep 0.5; done
drive log "$b" | grep -B1 -A2 "another of Max's chats"

echo
echo "== $(at) 4. an owner waits on a member that waits for words"
listener="$(tool wait '{summary: "wait for words"}')
say Owner answered."
owner=$(drive chat orchestrator "$dir" "$(tool background '{name: "listener", tool: "spawn", input: {fork: [{name: "listener", mandate: $m}]}, summary: "start listener"}' --arg m "$listener")
$(tool wait '{on: ["listener"], summary: "wait on listener"}')
$(tool message '{to: "listener", words: "say Owner answered.", summary: "answer listener"}')
$(tool wait '{on: ["listener"], summary: "wait on listener"}')
say Listener done.")
until drive log "$owner" | grep -q "^agent: Listener done"; do sleep 0.5; done
drive log "$owner" | sed -n '/waits to hear from you/,$p'
listener=$(view | jq -r '.rows | to_entries[] | select(.value.Task.name? == "listener") | .value.Task.id')
echo "== $(at) listener: $(drive settle "$listener")"
drive log "$listener" | sed -n '/wait for words/,$p'
echo "== $(at) owner: $(drive settle "$owner")"
