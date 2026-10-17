#!/bin/sh
# The harness Max uses at home: one brain, one journal kept outside the
# repo, chats started from the page. Run it again and everything carries on.
set -eu
cd "$(dirname "$0")/../.."
cargo build -q --release -p harness-brain -p harness-limb-workspace
dir=${DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/agent-harness/home}
mkdir -p "$dir"
# SAMPLE=1 offers "Dictate the sample", for a machine with no microphone.
exec target/release/harness-brain --roles roles --journal "$dir/journal.sqlite" --port "${PORT:-4320}" \
  ${SAMPLE:+--sample demos/home/sample.wav}
