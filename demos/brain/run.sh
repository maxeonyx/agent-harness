#!/usr/bin/env bash
# The harness on a root task in a scratch git repo, killed -9 mid-run and
# relaunched onto the same journal. Watch it at https://mx-hv-mj.local:4321.
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build -q --release -p harness-brain
dir=target/brain-demo
brain=target/release/harness-brain
rm -rf "$dir"
mkdir -p "$dir"
demos/brain/scratch.sh "$dir/repo"
args=(--roles roles --journal "$dir/journal.sqlite" --port "${PORT:-4321}")

echo "== launch 1: the orchestrator starts on demos/brain/mandate.md"
"$brain" "${args[@]}" --workspace "$dir/repo" --mandate demos/brain/mandate.md > "$dir/launch1.log" 2>&1 &
pid=$!
# Kill it while a check.sh is running: a bash call, which may not be rerun.
# The face's view, read as the page reads it.
running_check() {
  curl -sS --resolve "mx-hv-mj.local:${PORT:-4321}:127.0.0.1" "https://mx-hv-mj.local:${PORT:-4321}/view" |
    jq -e '[.tasks[].waiting.calls[]? | select(.tool == "bash" and (.input | contains("check.sh")))] | length > 0' >/dev/null
}
until running_check 2>/dev/null; do sleep 0.2; done
sleep 4
kill -9 "$pid"
wait "$pid" 2>/dev/null || true
cat "$dir/launch1.log"
echo
echo "** kill -9 the brain"
echo

echo "== launch 2: the same journal, nothing else"
"$brain" "${args[@]}" > "$dir/launch2.log" 2>&1 &
pid=$!
until grep -q 'the task is done' "$dir/launch2.log"; do sleep 0.5; done
sleep 0.5
kill "$pid"
wait "$pid" 2>/dev/null || true
cat "$dir/launch2.log"

echo "== the repository afterwards"
git -C "$dir/repo" log --stat --format='%h %s'
echo
echo "To keep serving the finished run: $brain ${args[*]}"
