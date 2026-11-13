#!/usr/bin/env bash
# An orchestrator opens a chat for Max with `open_chat`: a fresh engineer
# that is his, in his queue and waiting on him, owned by the orchestrator
# as background work. Max talks to it, marks it done, and its last
# message reaches the orchestrator as a message. Scripted: no model call.
set -euo pipefail
cd "$(dirname "$0")/../.."
b=open-chat
node dev/drive.mjs down --brain $b --wipe
node dev/drive.mjs up --brain $b
dir=$PWD/.drive.ignore/$b/dir
mkdir -p "$dir"
port=$(cat .drive.ignore/$b/port)
view() { curl -sk "https://127.0.0.1:$port/view"; }
opener=$(node dev/drive.mjs chat --brain $b orchestrator "$dir" "$(cat demos/open-chat/mandate.md)")
node dev/drive.mjs settle --brain $b "$opener" >/dev/null
node dev/drive.mjs log --brain $b "$opener"
chat=$(view | jq -r '.rows | to_entries[] | select(.value.Task.name? == "helper") | .value.Task.id')
node dev/drive.mjs settle --brain $b "$chat" >/dev/null
echo
echo "== helper ($chat), as the view has it, and Max's queue"
view | jq -c --arg c "$chat" '.rows["task:"+$c].Task | {name, mode, owner, waiting: .waiting.kind}'
view | jq -c '.rows as $r | [$r.queue.Queue[].task | $r["task:"+.].Task.name]'
echo
echo "== Max talks to helper"
node dev/drive.mjs say --brain $b "$chat" $'say A wordcount tool: noted.\nsay My result: Max wants a wordcount tool.'
node dev/drive.mjs settle --brain $b "$chat" >/dev/null
echo "== Max marks helper done"
target/release/examples/act "$port" done "$chat"
node dev/drive.mjs settle --brain $b "$chat" >/dev/null
node dev/drive.mjs log --brain $b "$chat"
until node dev/drive.mjs log --brain $b "$opener" | grep -q "I heard"; do sleep 0.2; done
echo
echo "== the orchestrator hears"
node dev/drive.mjs log --brain $b "$opener"
echo
echo "== the orchestrator opens another chat, then stops it"
node dev/drive.mjs say --brain $b "$opener" $'tool open_chat {"name":"spare","role":"designer","mandate":"say Hi."}\ntool stop {"call":"spare"}\nsay Stopped spare.'
node dev/drive.mjs settle --brain $b "$opener" >/dev/null
until node dev/drive.mjs log --brain $b "$opener" | grep -q "spare.*has settled"; do sleep 0.2; done
node dev/drive.mjs log --brain $b "$opener" | sed -n '/Stopped spare/,$p'
spare=$(view | jq -r '.rows | to_entries[] | select(.value.Task.name? == "spare") | .value.Task.id')
view | jq -c --arg c "$spare" '.rows["task:"+$c].Task | {name, mode, outcome: .outcome.kind}'
echo
echo "end it with: node dev/drive.mjs down --brain $b"
