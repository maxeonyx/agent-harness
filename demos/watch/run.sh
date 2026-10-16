#!/bin/sh
# A real tree of agents at work, to watch on the face: the brain demo's
# mandate in a fresh scratch repo, served until you stop it.
set -eu
cd "$(dirname "$0")/../.."
cargo build -q --release -p harness-brain
dir=${DIR:-target/watch}
rm -rf "$dir"
mkdir -p "$dir"
demos/brain/scratch.sh "$dir/repo"
exec target/release/harness-brain --journal "$dir/journal.sqlite" --port "${PORT:-4324}" \
  --workspace "$dir/repo" --mandate demos/brain/mandate.md
