#!/usr/bin/env bash
# The chat's blob: a scripted chat runs a tool for 20 s, then streams a
# long reply slowly. Open its link, on the phone or the
# desktop: while it works, the end of its log shows its blob (the gear
# for the tool, the Claude mark while streaming, a disk for each piece of
# data), and the blob goes when the chat waits on you. The chat's
# top-left corner is its name and a dot in its state's colour.
# The brain stays up.
set -euo pipefail
cd "$(dirname "$0")/../.."
node dev/drive.mjs down --brain chat-spinner --wipe
node dev/drive.mjs up --brain chat-spinner
dir=$PWD/.drive.ignore/chat-spinner/wd
mkdir -p "$dir"
script=$'tool bash {"command":"sleep 20","summary":"wait 20 s"}\nslow 200 long 80'
chat=$(node dev/drive.mjs chat --brain chat-spinner chat "$dir" "$script")
echo "== the chat: the link above, then #$chat"
echo "end it with: node dev/drive.mjs down --brain chat-spinner"
