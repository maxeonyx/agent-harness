#!/bin/sh
# A chat with the harness: a brain whose root task talks to Max on the face,
# in the brain demo's scratch repo. The journal persists: run it again and
# the chat carries on; delete target/chat for a new one.
set -eu
cd "$(dirname "$0")/../.."
cargo build -q --release -p harness-brain -p harness-limb-workspace
dir=${DIR:-target/chat}
mkdir -p "$dir"
start=""
if [ ! -e "$dir/journal.sqlite" ]; then
  demos/brain/scratch.sh "$dir/repo"
  start="--workspace $dir/repo --mandate demos/chat/mandate.md --name chat"
fi
# shellcheck disable=SC2086
exec target/release/harness-brain --roles roles --journal "$dir/journal.sqlite" --port "${PORT:-4320}" \
  --sample demos/chat/sample.wav $start
