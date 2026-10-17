#!/usr/bin/env bash
# A brain opens a database holding a record from a later build, whose fact
# it has no word for. See README.md.
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build -q --release -p harness-brain -p harness-limb-workspace --bins
dir=target/unread-demo
port=${PORT:-4396}
rm -rf "$dir" && mkdir -p "$dir"
db=$dir/brain.sqlite

# Run the brain until it has launched, then stop it as Ctrl-C would.
launch() {
  target/release/harness-brain --db "$db" --port "$port" "$@" > "$dir/launch$n.log" 2>&1 &
  local pid=$!
  until grep -q '^face:' "$dir/launch$n.log"; do sleep 0.1; done
  kill -INT $pid && wait $pid
  sed 's/^/  | /' "$dir/launch$n.log"
}

echo "1. A new brain launches on a fresh database, and stops."
n=1 launch --roles roles

brain=$(sqlite3 "$db" "SELECT replica FROM meta")
next=$(( $(sqlite3 "$db" "SELECT max(n) FROM record WHERE replica = '$brain'") + 1 ))
later='{"at":"'$brain.$next'","clock":'$(date +%s000)',"what":{"Said":{"values":[],"facts":[{"Note":"a fact only a later build knows"}]}}}'
echo
echo "2. A later build writes the brain's record $brain.$next, with a fact this build has no word for:"
sqlite3 "$db" "INSERT INTO record (replica, n, clock, vals, record) VALUES ('$brain', $next, 0, x'', '$later')"
echo "  $later"

echo
echo "3. This build launches on that database, and stops."
n=2 launch

echo
echo "4. The database afterwards:"
sqlite3 "$db" "SELECT '  ' || replica || '.' || n || '  ' || substr(record, 1, 110) FROM record WHERE replica = '$brain' AND n >= $next"
held=$(sqlite3 "$db" "SELECT record FROM record WHERE replica = '$brain' AND n = $next")
[ "$held" = "$later" ] && echo "OK: the later build's record is held exactly as it was written" || echo "FAIL: the record changed"
