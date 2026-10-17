#!/usr/bin/env bash
# A brain opens a database holding a record from a later build, whose fact
# it has no word for. See README.md.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain unread "${@:2}" >/dev/null; }
at=.drive.ignore/unread
db=$at/brain.sqlite
trap 'drive down' EXIT
drive down --wipe

# Run the brain until it serves, then stop it, and show what it logged.
launch() {
  local from=$(( $( [ -f "$at/brain.log" ] && stat -c %s "$at/brain.log" || echo 0) + 1 ))
  drive up
  drive down
  tail -c "+$from" "$at/brain.log" | sed 's/^/  | /'
}

echo "1. A new brain launches on a fresh database, and stops."
launch

brain=$(sqlite3 "$db" "SELECT replica FROM meta")
next=$(( $(sqlite3 "$db" "SELECT max(n) FROM record WHERE replica = '$brain'") + 1 ))
later='{"at":"'$brain.$next'","clock":'$(date +%s000)',"what":{"Said":{"values":[],"facts":[{"Note":"a fact only a later build knows"}]}}}'
echo
echo "2. A later build writes the brain's record $brain.$next, with a fact this build has no word for:"
sqlite3 "$db" "INSERT INTO record (replica, n, clock, vals, record) VALUES ('$brain', $next, 0, x'', '$later')"
echo "  $later"

echo
echo "3. This build launches on that database, and stops."
launch

echo
echo "4. The database afterwards:"
sqlite3 "$db" "SELECT '  ' || replica || '.' || n || '  ' || substr(record, 1, 110) FROM record WHERE replica = '$brain' AND n >= $next"
held=$(sqlite3 "$db" "SELECT record FROM record WHERE replica = '$brain' AND n = $next")
[ "$held" = "$later" ] && echo "OK: the later build's record is held exactly as it was written" || echo "FAIL: the record changed"
