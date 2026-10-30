#!/usr/bin/env bash
# One agent works in two steps and compacts between them, then compacts and
# reports; Max answers, and it compacts and reports again. It runs on the
# test brain `compaction` (dev/drive.mjs), whose link it prints. Every role
# runs on the scripted model, from script.md and answer.md, unless MODEL
# names another (MODEL=sonnet/low: a real agent on mandate.md).
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain compaction "${@:2}"; }
at=.drive.ignore/compaction
dir=target/compaction-demo
rm -rf "$dir"
mkdir -p "$dir"
demos/brain/scratch.sh "$dir/repo"
model=${MODEL:-scripted}
mandate=demos/compaction/mandate.md
answer="Thanks. Add \`textstats.wordcount.unique(text: str) -> int\`, the number of distinct words, with a test, run the check, commit, and report again."
if [ "$model" = scripted ]; then
  mandate=demos/compaction/script.md
  answer=$(cat demos/compaction/answer.md)
fi
# The brain's limbs outlive it, so what is left is ended when it finishes.
trap 'drive down' EXIT
drive down --wipe
drive up "$model"
port=$(cat "$at/port")

echo "== the agent starts on $mandate, every role on $model"
chat=$(drive chat orchestrator "$dir/repo" "$(cat "$mandate")")
drive settle "$chat"
grep -E 'compacted|Max sends' "$at/brain.log" | cut -c1-150 || true
echo
echo "== its context now: its first message and each carry are kept; the steps' tool calls are gone"
demos/compaction/context.sh "$port"

echo
echo "== Max answers"
drive say "$chat" "$answer"
drive settle "$chat"
echo
echo "== its context now: Max's words and both reports are kept verbatim"
demos/compaction/context.sh "$port"

echo
target/release/examples/act "$port" done "$chat"
until grep -q 'the task is done' "$at/brain.log"; do sleep 0.5; done
echo
echo "== what the brain said of its tasks' moves"
grep -E 'compacted|done' "$at/brain.log" | cut -c1-160
echo
echo "== the repository afterwards"
git -C "$dir/repo" log --format='%h %s'

if [ "$model" = scripted ]; then
  echo
  echo "== an agent compacting in a loop: a compact_and_continue with no call since the last compaction is refused"
  loop=$(drive chat chat "$dir/repo" 'tool compact_and_continue {"carry": "tool compact_and_continue {\"carry\": \"again\"}"}')
  drive settle "$loop"
  drive log "$loop" | grep -E '^  (→|←)'
fi
echo
echo "To serve the finished run again: node dev/drive.mjs up --brain compaction $model"
