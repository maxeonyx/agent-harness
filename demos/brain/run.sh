#!/usr/bin/env bash
# The harness on a root task in a scratch git repo, killed -9 mid-run and
# relaunched onto the same database, on the test brain `brain`
# (dev/drive.mjs): it prints the link to watch. Every role runs on the
# scripted model, from demos/brain/script.md, unless MODEL names another
# (MODEL=sonnet/low: real agents on mandate.md).
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain brain "${@:2}"; }
at=.drive.ignore/brain
dir=target/brain-demo
rm -rf "$dir"
mkdir -p "$dir"
demos/brain/scratch.sh "$dir/repo"
model=${MODEL:-scripted}
mandate=demos/brain/mandate.md
[ "$model" = scripted ] && mandate=demos/brain/script.md
# The brain's limbs outlive it, so what is left is ended when it finishes.
trap 'drive down' EXIT
drive down --wipe

echo "== launch 1: the orchestrator starts on $mandate, every role on $model"
drive up "$model"
port=$(cat "$at/port")
chat=$(drive chat orchestrator "$dir/repo" "$(cat "$mandate")")
echo "chat $chat"
# Kill it while a check.sh is running: a bash call, which may not be rerun.
# The face's view, read as the page reads it.
running_check() {
  curl -sS --resolve "mx-hv-mj.local:$port:127.0.0.1" "https://mx-hv-mj.local:$port/view" |
    jq -e '[.rows[] | .Task // empty | .waiting.calls[]? | select(.tool == "bash" and (.input | contains("check.sh")))] | length > 0' >/dev/null
}
until running_check 2>/dev/null; do sleep 0.2; done
sleep 4
kill -9 "$(cat "$at/pid")"
cat "$at/brain.log"
from=$(($(stat -c %s "$at/brain.log") + 1))
echo
echo "** kill -9 the brain"
echo

echo "== launch 2: the same database and limbs, nothing else"
drive up "$model"
demos/brain/done.sh "$port" "$chat"
until tail -c "+$from" "$at/brain.log" | grep -q 'the task is done'; do sleep 0.5; done
sleep 0.5
tail -c "+$from" "$at/brain.log"

echo "== the repository afterwards"
git -C "$dir/repo" log --stat --format='%h %s'
echo
echo "To serve the finished run again: node dev/drive.mjs up --brain brain $model"
