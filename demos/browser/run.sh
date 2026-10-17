#!/usr/bin/env bash
# A chat starts a browser agent on a chore in real pages: an expense claim
# filled in from a receipts page, left unsubmitted, its tab handed to Max.
# Watch it at https://mx-hv-mj.local:4342 (PORT=… for another port).
# Every role runs on Sonnet (MODEL=… for another; the effort is the role's).
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build -q --release -p harness-brain -p harness-limb-workspace -p harness-limb-browser --bins --example act
port=${PORT:-4342}
dir=$PWD/target/browser-demo
brain=target/release/harness-brain
rm -rf "$dir"
mkdir -p "$dir"
site=$PWD/demos/browser/site
sed -e "s|RECEIPTS|file://$site/receipts.html|" -e "s|CLAIM|file://$site/claim.html|" \
  demos/browser/mandate.md > "$dir/mandate.md"
# The limbs' state, the browser's profile among it, stays in the demo.
export XDG_STATE_HOME=$dir/state
mark=HARNESS_OWNER_browser-demo-$$
trap 'demos/end.sh "$mark"' EXIT
c() { curl -sS --resolve "mx-hv-mj.local:$port:127.0.0.1" "https://mx-hv-mj.local:$port$1"; }
root='.rows[] | .Task // empty | select(.owner == null)'
waits_on_max() { [ "$(c /view 2>/dev/null | jq -r "$root | .waiting.kind")" = max ]; }

echo "== Max asks a chat, in the chat role:"
cat "$dir/mandate.md"
echo
env "$mark=1" "$brain" --roles roles --db "$dir/brain.sqlite" --port "$port" --model "${MODEL:-sonnet}" > "$dir/brain.log" 2>&1 &
chat=$(target/release/examples/act "$port" chat chat - "$(cat "$dir/mandate.md")" | sed -n 's/^chat //p')
until waits_on_max; do sleep 1; done
grep -E 'browser|tab|passed|spawn|responded' "$dir/brain.log" | cut -c1-170
echo
# The browser: the one HARNESS_BROWSER names, or the one the limb launched.
browser=${HARNESS_BROWSER:-$(ls -d "$XDG_STATE_HOME"/harness/limbs/*/profile)}
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
until grep -q 'the task is done' "$dir/brain.log"; do sleep 0.5; done
sleep 0.5
sed -n '/the task is done/,$p' "$dir/brain.log"
