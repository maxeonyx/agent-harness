#!/usr/bin/env bash
# A chat in a plain directory hands a fresh `chat` something to do: it
# runs in the same directory, with no clone made. Then it asks for a role
# it cannot start, and is told which it can. Scripted: no model call.
set -euo pipefail
cd "$(dirname "$0")/../.."
node dev/drive.mjs down --brain chat-fresh --wipe
node dev/drive.mjs up --brain chat-fresh
dir=$PWD/.drive.ignore/chat-fresh/dir
mkdir -p "$dir"
echo "a note in the chat's directory" >"$dir/note.txt"
chat=$(node dev/drive.mjs chat --brain chat-fresh chat "$dir" "$(cat demos/chat-fresh/mandate.md)")
node dev/drive.mjs settle --brain chat-fresh "$chat" >/dev/null
node dev/drive.mjs log --brain chat-fresh "$chat" | tee .drive.ignore/chat-fresh/chat.log
echo
child=$(sed -n 's/^child \([^ ]*\) "pal".*/\1/p' .drive.ignore/chat-fresh/chat.log)
node dev/drive.mjs log --brain chat-fresh "$child" | grep -A4 '→ bash'
echo
echo "end it with: node dev/drive.mjs down --brain chat-fresh"
