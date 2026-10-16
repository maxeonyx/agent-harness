#!/bin/sh
# The dev process on the harness: an orchestrator started from the role
# files in roles/, on a small real mandate in a scratch repository, spawns a
# designer and an engineer, each in its own clone. Watch the tree at
# https://mx-hv-mj.local:4325. A new run starts from scratch (FRESH=1, or no
# journal yet); otherwise the brain carries on the journal and serves it.
set -eu
cd "$(dirname "$0")/../.."
cargo build -q --release -p harness-brain -p harness-limb-workspace
dir=${DIR:-target/dev-process}
start=""
if [ -n "${FRESH:-}" ] || [ ! -e "$dir/journal.sqlite" ]; then
  rm -rf "$dir"
  mkdir -p "$dir"
  demos/dev-process/scratch.sh "$dir"
  start="--role orchestrator --workspace $dir/textstats --mandate demos/dev-process/mandate.md --words demos/dev-process/words.md"
fi
# shellcheck disable=SC2086
exec target/release/harness-brain --roles roles --journal "$dir/journal.sqlite" \
  --port "${PORT:-4325}" $start
