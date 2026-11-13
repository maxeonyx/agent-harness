#!/usr/bin/env bash
# A brain reads message records before any brain writes them. No tool
# writes one yet, so the fixture `tell` (crates/brain/examples/tell.rs)
# writes each into the killed brain's database, as the brain will, from
# the newest `message` call (which fails for now: no such tool). Each
# relaunch folds the database again.
#
# 1. A scripted chat starts a background fork, `kid`, and waits on Max.
#    `kid` tells it something: the chat wakes, reads "`kid`, which you
#    started, wrote this", answers, and keeps its place in Max's queue (the
#    same "since" before and after).
# 2. The chat tells `kid`, mid `sleep 100`, something with Max's words
#    quoted by label: 30 s after it came, the round moves to the
#    background, and `kid` reads it, "which started you", with the quote.
# 3. A last relaunch folds every record again, skipping none.
#
# Free (scripted model), about 2 min; it leaves the brain up:
# `node dev/drive.mjs down --brain messages`.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain messages "${@:2}"; }
at() { TZ=Pacific/Auckland date +%T; }
db=$PWD/.drive.ignore/messages/brain.sqlite
# The brain is killed rather than stopped, so its limbs' calls run on, as
# in a crash.
tell() { kill -9 "$(cat .drive.ignore/messages/pid)"; cargo run -q -p harness-brain --example tell -- "$db" "$@"; drive up >/dev/null; }
cargo build -q -p harness-brain --example tell
drive down --wipe >/dev/null
drive up
repo=$PWD/.drive.ignore/messages/repo
demos/brain/scratch.sh "$repo" >/dev/null

kid='tool message {\"to\": \"owner\", \"words\": \"x\", \"summary\": \"tell the owner\"}\ntool bash {\"command\": \"sleep 100; echo kid slept\"}\nsay I slept and woke.'
chat=$(drive chat orchestrator "$repo" "tool background {\"name\": \"kid\", \"tool\": \"spawn\", \"input\": {\"fork\": [{\"name\": \"kid\", \"mandate\": \"$kid\"}]}, \"summary\": \"start kid\"}
say Started kid.")
drive settle "$chat" >/dev/null
sleep 5
echo "== $(at) the chat $chat waits on Max, with kid running: $(drive settle "$chat")"
echo "== $(at) kid tells it something, written while the brain is dead"
tell owner "say Thanks, kid: I read you."
echo "== $(at) relaunched; the chat wakes and answers:"
drive settle "$chat"
drive log "$chat" | grep -B1 -A6 "which you started"

echo "== $(at) Max has the chat tell kid something, quoting him; written while the brain is dead"
drive say "$chat" 'tool message {"to": "kid", "words": "x", "summary": "tell kid"}
say Told kid.'
drive settle "$chat" >/dev/null
quote=$(drive log "$chat" --full | grep -o 'max-[0-9a-f]\{8\}' | tail -1)
told=$(tell kid "say Heard you, owner." "$quote")
echo "$told"
kid_id=$(echo "$told" | awk '{print $2}')
echo "== $(at) relaunched; kid's sleep runs on until 30 s after the message came"
for _ in $(seq 60); do drive log "$kid_id" | grep -q "Heard you, owner" && break; sleep 2; done
echo "== $(at) kid has read it:"
drive log "$kid_id" --full | grep -A12 "written to while this ran" | head -24

echo "== $(at) one more relaunch folds everything again"
drive up >/dev/null
if grep -q "skipping the record" .drive.ignore/messages/brain.log; then
  grep "skipping the record" .drive.ignore/messages/brain.log
  exit 1
fi
echo "== $(at) no record skipped; chat: $(drive settle "$chat")"
