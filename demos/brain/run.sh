#!/usr/bin/env bash
# The harness on a root task in a scratch git repo, killed -9 mid-run and
# relaunched onto the same database. Watch it at https://mx-hv-mj.local:4341.
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build -q --release -p harness-brain -p harness-limb-workspace --bins --example act
dir=target/brain-demo
brain=target/release/harness-brain
rm -rf "$dir"
mkdir -p "$dir"
demos/brain/scratch.sh "$dir/repo"
args=(--roles roles --db "$dir/brain.sqlite" --port "${PORT:-4341}")
# Everything the demo starts carries this mark; its limbs outlive their
# brains, so what is left is ended when it finishes.
mark=HARNESS_OWNER_brain-demo-$$
trap 'demos/end.sh "$mark"' EXIT

echo "== launch 1: the orchestrator starts on demos/brain/mandate.md"
env "$mark=1" "$brain" "${args[@]}" > "$dir/launch1.log" 2>&1 &
pid=$!
target/release/examples/act "${PORT:-4341}" chat orchestrator "$PWD/$dir/repo" "$(cat demos/brain/mandate.md)" >> "$dir/launch1.log"
# Kill it while a check.sh is running: a bash call, which may not be rerun.
# The face's view, read as the page reads it.
running_check() {
  curl -sS --resolve "mx-hv-mj.local:${PORT:-4341}:127.0.0.1" "https://mx-hv-mj.local:${PORT:-4341}/view" |
    jq -e '[.rows[] | .Task // empty | .waiting.calls[]? | select(.tool == "bash" and (.input | contains("check.sh")))] | length > 0' >/dev/null
}
until running_check 2>/dev/null; do sleep 0.2; done
sleep 4
kill -9 "$pid"
wait "$pid" 2>/dev/null || true
cat "$dir/launch1.log"
echo
echo "** kill -9 the brain"
echo

echo "== launch 2: the same database, nothing else"
env "$mark=1" "$brain" "${args[@]}" > "$dir/launch2.log" 2>&1 &
pid=$!
demos/brain/done.sh "${PORT:-4341}"
until grep -q 'the task is done' "$dir/launch2.log"; do sleep 0.5; done
sleep 0.5
kill "$pid"
wait "$pid" 2>/dev/null || true
cat "$dir/launch2.log"

echo "== the repository afterwards"
git -C "$dir/repo" log --stat --format='%h %s'
echo
echo "To keep serving the finished run: $brain ${args[*]}"
