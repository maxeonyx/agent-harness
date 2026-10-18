#!/usr/bin/env bash
# A chat starts a browser agent on a chore in real pages: an expense claim
# filled in from a receipts page, left unsubmitted, its tab handed to Max.
# It runs on the test brain `browser` (dev/drive.mjs), whose link it
# prints. Every role runs on Sonnet at low effort (MODEL=… for another).
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain browser "${@:2}"; }
at=.drive.ignore/browser
dir=$PWD/target/browser-demo
rm -rf "$dir"
mkdir -p "$dir"
site=$PWD/demos/browser/site
sed -e "s|RECEIPTS|file://$site/receipts.html|" -e "s|CLAIM|file://$site/claim.html|" \
  demos/browser/mandate.md > "$dir/mandate.md"
trap 'drive down' EXIT
drive down --wipe
drive up "${MODEL:-sonnet/low}"
port=$(cat "$at/port")
c() { curl -sS --resolve "mx-hv-mj.local:$port:127.0.0.1" "https://mx-hv-mj.local:$port$1"; }
root='.rows[] | .Task // empty | select(.owner == null)'
waits_on_max() { [ "$(c /view 2>/dev/null | jq -r "$root | .waiting.kind")" = max ]; }

echo "== Max asks a chat, in the chat role:"
cat "$dir/mandate.md"
echo
chat=$(drive chat chat "$dir" "$(cat "$dir/mandate.md")")
until waits_on_max; do sleep 1; done
grep -E 'browser|tab|passed|spawn|responded' "$at/brain.log" | cut -c1-170
echo
# The browser: the one HARNESS_BROWSER names, or the one the limb launched,
# whose profile is in the limbs' state.
browser=${HARNESS_BROWSER:-$(ls -d "$at"/state/harness/limbs/*/profile)}
echo
echo "== What the browser shows, read over DevTools"
node demos/browser/look.mjs "$browser"

echo
echo "== Handed to Max: the tab, still open"
handed=$(c /view | jq -c "$root | .handed[]")
echo "$handed" | jq -r '"  \(.id)  \(.tool) `\(.name // "")`"' 2>/dev/null || echo "$handed"
call=$(echo "$handed" | jq -r 'select(.tool == "tab") | .id' | head -1)
echo
echo "== Max has checked it and closes the tab: Stop on the call"
target/release/examples/act "$port" stop "$call"
until [ -z "$(c /view | jq -c "$root | .handed[]")" ]; do sleep 0.5; done
node demos/browser/look.mjs "$browser"
echo
echo "== Max presses Done, and the chat writes its result"
target/release/examples/act "$port" done "$chat"
until grep -q 'the task is done' "$at/brain.log"; do sleep 0.5; done
sleep 0.5
sed -n '/the task is done/,$p' "$at/brain.log"
