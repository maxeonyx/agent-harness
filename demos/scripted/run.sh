#!/usr/bin/env bash
# A test brain whose every role runs on the scripted model: no model call,
# no cost. Its first chat runs demos/scripted/mandate.md; it prints the
# chat's transcript and leaves the brain running for you to script more.
set -euo pipefail
cd "$(dirname "$0")/../.."
node dev/drive.mjs down --brain scripted --wipe
node dev/drive.mjs up --brain scripted
repo=$PWD/.drive.ignore/scripted/repo
demos/brain/scratch.sh "$repo"
echo
echo "== the models on offer, read from the provider (one call, no cost)"
target/release/harness-api GET /v1/models 2>/dev/null | jq -r '.data[] | "\(.id)  \(.display_name)"'
echo
chat=$(node dev/drive.mjs chat --brain scripted orchestrator "$repo" "$(cat demos/scripted/mandate.md)")
echo "== its first chat, $chat: the link above, then #$chat"
node dev/drive.mjs settle --brain scripted "$chat" >/dev/null
node dev/drive.mjs log --brain scripted "$chat"
echo
echo "end it with: node dev/drive.mjs down --brain scripted"
