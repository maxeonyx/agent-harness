#!/usr/bin/env bash
# One agent reads another's report through the face, as its system prompt
# says to. An engineer on Sonnet low spawns two forks at once: `alpha`
# reports a codeword it makes with `date +%N`; `beta` is not told it, and
# finds alpha's chat with `/find` and reads its report with `/context`.
# Then the demo shows the line every agent's system prompt carries, and
# reads alpha's report itself the same way. About 6 requests, 1 min; it
# leaves the brain up: `node dev/drive.mjs down --brain reads`.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain reads "${@:2}"; }
drive down --wipe >/dev/null
drive up sonnet/low
port=$(cat .drive.ignore/reads/port)
dir=$PWD/.drive.ignore/reads/work
mkdir -p "$dir"
chat=$(drive chat engineer "$dir" "Spawn one scope of two forks at once. Fork \`alpha\`: run \`date +%N\` and end with a report that says 'The codeword is ' followed by that number. Fork \`beta\`: its sibling \`alpha\` reports a codeword; beta must not be told it. Beta reads alpha's report the way its instructions say to read another agent's work (retrying every 15 seconds until alpha has reported), and reports the codeword and the exact commands it ran. Then end with both results.")
drive settle "$chat" --timeout 300 >/dev/null
drive log "$chat"
face=https://localhost:$port
echo "== the line in the agent's system prompt"
curl -s "$face/context/$chat" | jq -r '.[0].system' | grep -o "To read another agent's work[^\"]*"
echo "== curl -s '$face/find?q=codeword+is'"
curl -s "$face/find?q=codeword+is" | jq -c '.[]'
alpha=$(curl -s "$face/find?q=codeword+is" | jq -r 'map(select(.snippet.after | test("^ [0-9]+$"))) | .[0].task')
echo "== curl -s $face/context/$alpha, its last entry"
curl -s "$face/context/$alpha" | jq -c '.[-1] | {role, content}'
