#!/usr/bin/env bash
# A chat forks an agent that runs a command and attaches that call to its
# report, so the chat receives the command's real output, as the harness
# recorded it, after the fork's own words. It runs on the test brain
# `attach` (dev/drive.mjs), Sonnet at low effort unless MODEL names
# another, prints the chat (watch the `## probe` result: the report, then
# "[harness] Attached: the `bash` call …" and the output), and leaves the
# brain serving: `node dev/drive.mjs down --brain attach` ends it.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain attach "${@:2}"; }
dir=$PWD/target/attach
mkdir -p "$dir"
drive down --wipe
drive up "${MODEL:-sonnet/low}"
chat=$(drive chat engineer "$dir" "Fork one agent named probe, mandate: run \`uname -sr; echo marker-42\` with bash, then report what it printed, attaching that call. When its result arrives, end your turn with one line saying whether the real output came with it.")
drive settle "$chat" --timeout 300
drive log "$chat" --full
echo "end it with: node dev/drive.mjs down --brain attach"
