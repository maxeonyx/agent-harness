#!/bin/sh
# A chat with the harness: a brain whose root task talks to Max on the face,
# in the brain demo's scratch repo. The journal persists: run it again and
# the chat carries on; delete target/chat for a new one.
set -eu
cd "$(dirname "$0")/../.."
cargo build -q --release -p harness-brain -p harness-limb-workspace
dir=${DIR:-target/chat}
mkdir -p "$dir"
[ -e "$dir/journal.sqlite" ] || demos/brain/scratch.sh "$dir/repo"
exec target/release/harness-brain --roles roles --journal "$dir/journal.sqlite" --port "${PORT:-4320}" \
  --workspace "$dir/repo" --mandate demos/chat/mandate.md --name chat \
  --sample demos/chat/sample.wav
