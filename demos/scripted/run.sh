#!/usr/bin/env bash
# A brain whose every role runs on the scripted model: no model call, no
# cost. Its first chat runs demos/scripted/mandate.md; open
# https://mx-hv-mj.local:${PORT:-4370}/ and script more. Ctrl-C ends it.
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build -q --release -p harness-brain -p harness-limb-workspace -p harness-provider --bins
dir=target/scripted-demo
rm -rf "$dir"
mkdir -p "$dir"
demos/brain/scratch.sh "$dir/repo"
echo "== the models on offer, read from the provider (one call, no cost)"
target/release/harness-api GET /v1/models 2>/dev/null | jq -r '.data[] | "\(.id)  \(.display_name)"'
echo
mark=HARNESS_OWNER_scripted-demo-$$
trap 'demos/end.sh "$mark"' EXIT
echo "== a brain on --model scripted at https://mx-hv-mj.local:${PORT:-4370}/"
env "$mark=1" target/release/harness-brain --roles roles --db "$dir/brain.sqlite" --port "${PORT:-4370}" \
  --model scripted --workspace "$dir/repo" --mandate demos/scripted/mandate.md
