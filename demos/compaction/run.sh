#!/usr/bin/env bash
# One agent works in two steps and compacts between them, then compacts and
# reports; Max answers, and it compacts and reports again.
# Watch it at https://mx-hv-mj.local:4321 (another port: PORT=4400).
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build -q --release -p harness-brain -p harness-limb-workspace --bins --example act
port=${PORT:-4321}
dir=target/compaction-demo
brain=target/release/harness-brain
rm -rf "$dir"
mkdir -p "$dir"
demos/brain/scratch.sh "$dir/repo"
mark=HARNESS_OWNER_compaction-demo-$$
trap 'demos/end.sh "$mark"' EXIT
c() { curl -sS --resolve "mx-hv-mj.local:$port:127.0.0.1" "https://mx-hv-mj.local:$port$1"; }
root='.rows[] | .Task // empty | select(.owner == null)'
waits() { [ "$(c /view 2>/dev/null | jq -r "$root | .waiting.kind")" = max ]; }

echo "== the agent starts on demos/compaction/mandate.md"
env "$mark=1" "$brain" --roles roles --db "$dir/brain.sqlite" --port "$port" \
  --workspace "$dir/repo" --mandate demos/compaction/mandate.md > "$dir/brain.log" 2>&1 &
pid=$!
until waits; do sleep 1; done
grep -E 'compacted|Max sends' "$dir/brain.log" || true
echo
echo "== its context now: its first message and each carry are kept; the steps' tool calls are gone"
demos/compaction/context.sh "$port"

echo
echo "== Max answers"
target/release/examples/act "$port" say "Thanks. Add \`textstats.wordcount.unique(text: str) -> int\`, the number of distinct words, with a test, run the check, commit, and report again."
sleep 2
until waits; do sleep 1; done
echo
echo "== its context now: Max's words and both reports are kept verbatim"
demos/compaction/context.sh "$port"

echo
target/release/examples/act "$port" done
until grep -q 'the task is done' "$dir/brain.log"; do sleep 0.5; done
kill "$pid"
wait "$pid" 2>/dev/null || true
echo
echo "== what the brain said of its tasks' moves"
grep -E 'compacted|done' "$dir/brain.log" | cut -c1-160
echo
echo "== the repository afterwards"
git -C "$dir/repo" log --format='%h %s'
echo
echo "To keep serving the finished run: $brain --roles roles --db $dir/brain.sqlite --port $port"
