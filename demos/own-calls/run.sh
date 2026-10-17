#!/bin/sh
# An agent is not refused for its own calls, and is still refused for a
# file it has only seen through a command. Runs on the scripted model: no
# model call, no cost. Prints each call's outcome and the files it left.
set -eu
cd "$(dirname "$0")/../.."
cargo build -q -p harness-brain -p harness-limb-workspace
dir=target/own-calls
rm -rf "$dir"
mkdir -p "$dir/ws"
printf 'one\ntwo\n' > "$dir/ws/f.txt"
echo theirs > "$dir/ws/other.txt"
target/debug/harness-brain --roles roles --db "$dir/brain.sqlite" --port "${PORT:-4391}" \
  --model scripted --role engineer --workspace "$dir/ws" \
  --mandate demos/own-calls/mandate.md > "$dir/brain.log" 2>&1 &
brain=$!
until grep -q "EndTurn" "$dir/brain.log" 2>/dev/null; do
  kill -0 "$brain" 2>/dev/null || { cat "$dir/brain.log"; exit 1; }
  sleep 0.2
done
kill "$brain"
grep -E "made|settled|completed|failed|refused" "$dir/brain.log" | grep -v dispatched
for f in "$dir"/ws/*.txt; do echo "== $(basename "$f"):"; cat "$f"; done
