#!/usr/bin/env bash
# A workspace limb is let go once nothing holds it. Two chats share one
# directory, so one limb, in one systemd scope. Max closes the first: the
# second still holds the limb, and its scope is still there after the
# grace. He closes the second: the scope is gone within the grace (60 s).
# A third chat there starts the limb again. Scripted: no model call.
# About three minutes.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain limb-release "${@:2}"; }
drive down --wipe
drive up
port=$(cat .drive.ignore/limb-release/port)
dir=$PWD/.drive.ignore/limb-release/dir
mkdir -p "$dir"
scopes() {
  systemctl --user list-units 'harness-limb-*' --no-legend --plain |
    grep -F "$PWD/target/release/harness-limb-workspace" | awk '{print $1}' || true
}
at() { echo "[$(TZ=Pacific/Auckland date +%T)] $*"; }
first=$(drive chat chat "$dir" "Say hello.")
second=$(drive chat chat "$dir" "Say hello too.")
drive settle "$first" >/dev/null
drive settle "$second" >/dev/null
at "two chats in $dir; its limb's scope:"
scopes
target/release/examples/act "$port" done "$first"
at "closed the first; waiting out the grace"
sleep 75
at "the second still holds it; its scope:"
scopes
target/release/examples/act "$port" done "$second"
at "closed the second; waiting out the grace"
for i in $(seq 90); do
  [ -z "$(scopes)" ] && break
  sleep 1
done
left=$(scopes)
if [ -n "$left" ]; then
  at "still there: $left"
  exit 1
fi
at "its scope is gone, ${i}s after"
third=$(drive chat chat "$dir" "Say hello once more.")
drive settle "$third" >/dev/null
at "a third chat in the directory; its limb started again:"
scopes
drive down
