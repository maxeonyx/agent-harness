#!/usr/bin/env bash
# A test brain whose every role runs on the scripted model: no model call,
# no cost. Its first chat runs demos/scripted/mandate.md; it prints the
# chat's transcript and leaves the brain at https://mx-hv-mj.local:${PORT:-4370}/
# for you to script more.
set -euo pipefail
cd "$(dirname "$0")/../.."
port=${PORT:-4370}
node dev/drive.mjs down "$port" --wipe
node dev/drive.mjs up "$port"
repo=$PWD/.drive.ignore/$port/repo
demos/brain/scratch.sh "$repo"
echo
echo "== the models on offer, read from the provider (one call, no cost)"
target/release/harness-api GET /v1/models 2>/dev/null | jq -r '.data[] | "\(.id)  \(.display_name)"'
echo
chat=$(node dev/drive.mjs chat "$port" orchestrator "$repo" "$(cat demos/scripted/mandate.md)" | cut -d' ' -f2)
echo "== its first chat: https://mx-hv-mj.local:$port/#$chat"
node dev/drive.mjs settle "$port" "$chat" >/dev/null
node dev/drive.mjs log "$port" "$chat"
echo
echo "end it with: node dev/drive.mjs down $port"
