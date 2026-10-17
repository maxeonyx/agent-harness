#!/bin/sh
# The dev process on the harness: an orchestrator started from the role
# files in roles/, on a small real mandate in a scratch repository, spawns a
# designer and an engineer, each in its own clone. Watch the tree at
# https://mx-hv-mj.local:4345. A new run starts from scratch (FRESH=1, or no
# database yet); otherwise the brain carries on its database and serves it.
set -eu
cd "$(dirname "$0")/../.."
cargo build -q --release -p harness-brain -p harness-limb-workspace --bins --example act
dir=${DIR:-target/dev-process}
if [ -n "${FRESH:-}" ] || [ ! -e "$dir/brain.sqlite" ]; then
  rm -rf "$dir"
  mkdir -p "$dir"
  demos/dev-process/scratch.sh "$dir"
  # Max starts the orchestrator at a tab, once the brain below serves one.
  target/release/examples/act "${PORT:-4345}" chat orchestrator "$PWD/$dir/textstats" \
    "$(cat demos/dev-process/mandate.md)" &
fi
exec target/release/harness-brain --roles roles --db "$dir/brain.sqlite" --port "${PORT:-4345}"
