#!/usr/bin/env bash
# A real brain run (Sonnet), driven only through the face's HTTP endpoints,
# the way the page drives it: live text, hold, interrupt, the queue, focus,
# words, stop, and the core's refusals. Watch it at https://mx-hv-mj.local:4323.
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build -q --release -p harness-brain
dir=target/face-controls-demo
port=${PORT:-4323}
url=https://mx-hv-mj.local:$port
rm -rf "$dir"
mkdir -p "$dir"
demos/brain/scratch.sh "$dir/repo"
target/release/harness-brain --journal "$dir/journal.sqlite" --port "$port" \
  --workspace "$dir/repo" --mandate demos/brain/mandate.md > "$dir/brain.log" 2>&1 &
brain=$!
trap 'kill $brain 2>/dev/null || true' EXIT
start=$(date +%s.%N)

# The face, over its real TLS certificate, on this machine.
c() { curl -sS --resolve "mx-hv-mj.local:$port:127.0.0.1" "$@"; }
say() { printf "%6.1fs  %s\n" "$(awk "BEGIN { print $(date +%s.%N) - $start }")" "$*"; }
# The page's view, as of the face's last fold.
view() { c "$url/view"; }
q() { view | jq -r "$1"; }
FACE=$(head -c16 /dev/urandom | od -An -tx1 | tr -d ' \n')
# One of Max's facts, as the page posts it; prints the face's answer.
act() {
  local body
  body=$(jq -nc --arg face "$FACE" --argjson seen "$(q .seq)" --argjson fact "$1" \
    '{face: $face, seen: $seen, fact: $fact}')
  echo "$body" > "$dir/last-act.json"
  c -X POST "$url/act" --data-binary "@$dir/last-act.json" -w ' -> %{http_code}' | sed 's/^/          answer: /'
  echo
}
waiting() { q ".tasks[] | select(.id == \"$1\") | .waiting.kind"; }
until_waiting() { # task kind seconds
  for _ in $(seq $(($3 * 2))); do
    [ "$(waiting "$1")" = "$2" ] && return 0
    sleep 0.5
  done
  say "!! $1 never reached $2"; return 1
}
name() { q ".tasks[] | select(.id == \"$1\") | .name"; }
queue() { q '.tasks as $t | .queue[] | "          \(.task as $id | $t[] | select(.id == $id) | .name): \(.why // "unranked")"'; }

until c -s -o /dev/null "$url/" 2>/dev/null; do sleep 0.2; done
root=$(q '.tasks[] | select(.owner == null) | .id')
say "the root task is $(name "$root")"

say "== live text: a request of the root's as it streams, from /live"
# Read 2 s of a streaming request, then reconnect the way a browser does,
# with the last event id it got, and read on until the request ends. Try
# again on the next request if this one ended inside the 2 s.
while :; do
  call=$(q ".tasks[] | select(.id == \"$root\" and .waiting.streaming == true) | .waiting.call")
  [ -n "$call" ] || { sleep 0.2; continue; }
  { c -m 2 -N "$url/live?model=$call" 2>/dev/null || true; } > "$dir/live1.txt"
  n=$(grep -c '^id:' "$dir/live1.txt" || true)
  [ "$n" -gt 0 ] && ! grep -q '^event: end' "$dir/live1.txt" && break
done
say "first connection to /live?model=$call, cut after 2 s: events 1 to $n"
c -N -H "Last-Event-ID: $n" "$url/live?model=$call" 2>/dev/null > "$dir/live2.txt" || true
ids=$(grep '^id:' "$dir/live2.txt" | cut -c5- | tr '\n' ' ')
say "reconnected with Last-Event-ID: $n: events $(echo $ids | cut -d' ' -f1) to $(echo $ids | awk '{print $NF}'), then: $(grep -o '^event: end' "$dir/live2.txt")"
say "the streamed text, from both connections:"
cat "$dir/live1.txt" "$dir/live2.txt" | grep '^data:' | cut -c7- |
  jq -rj 'select(.type == "content_block_delta") | .delta | (.text // .thinking // .partial_json // "")' |
  fold -w 100 | head -12 | sed 's/^/          /' || true
echo

say "== hold the root: nothing under it sends a request until it is released"
act "{\"Task\": [\"$root\", {\"Held\": true}]}"
until [ -n "$(q '.tasks[] | select(.waiting.kind == "held") | .id')" ]; do sleep 0.5; done
sent=$(grep -c 'request sent' "$dir/brain.log")
say "held now: $(q '[.tasks[] | select(.waiting.kind == "held") | .name] | join(", ")')"
sleep 5
say "5 s later: $(( $(grep -c 'request sent' "$dir/brain.log") - sent )) requests sent meanwhile; release it"
act "{\"Task\": [\"$root\", {\"Held\": false}]}"

say "== interrupt both children as soon as they exist"
until [ "$(q '[.tasks[] | select(.owner != null)] | length')" = 2 ]; do sleep 0.2; done
mapfile -t kids < <(q '[.tasks[] | select(.owner != null)] | sort_by(.name) | .[].id')
for kid in "${kids[@]}"; do
  say "interrupt $(name "$kid") while it is on '$(waiting "$kid")'"
  act "{\"Task\": [\"$kid\", {\"Drain\": \"ToMax\"}]}"
done
for kid in "${kids[@]}"; do until_waiting "$kid" max 120; done
say "both wait on Max; his queue, oldest first:"
queue

say "== reorder the queue: the second to the top"
order=$(q '[.queue | reverse | .[] | {task, why: "placed by you"}]')
act "{\"Ranking\": {\"order\": $order}}"
queue
top=$(q '.queue[0].task')
other=$(q '.queue[1].task')

say "== focus the top item, $(name "$top"), and tell it to carry on"
act "{\"Focus\": {\"face\": \"$FACE\", \"task\": \"$top\"}}"
words=$(jq -nc '[{type: "text", text: "Sorry for the interruption: carry on where you left off."}] | tojson')
delivery=$(head -c16 /dev/urandom | od -An -tx1 | tr -d ' \n')
act "{\"Delivery\": {\"id\": \"$delivery\", \"to\": \"$top\", \"content\": {\"Words\": $words}}}"
say "the same post again, as a page whose connection dropped would retry it:"
act "{\"Delivery\": {\"id\": \"$delivery\", \"to\": \"$top\", \"content\": {\"Words\": $words}}}"

say "== stop the other, $(name "$other")"
act "{\"Task\": [\"$other\", {\"Drain\": {\"Stop\": \"Max\"}}]}"
until_waiting "$other" done 60
say "$(name "$other"): $(q ".tasks[] | select(.id == \"$other\") | .outcome | \"\(.kind) by \(.by)\"")"

say "== what the core refuses"
say "hold the stopped task:"
act "{\"Task\": [\"$other\", {\"Held\": true}]}"
say "words to the stopped task:"
act "{\"Delivery\": {\"id\": \"$(head -c16 /dev/urandom | od -An -tx1 | tr -d ' \n')\", \"to\": \"$other\", \"content\": {\"Words\": $words}}}"
say "a fact that is not Max's to author (failing a task):"
act "{\"Task\": [\"$top\", {\"Drain\": {\"Fail\": \"because\"}}]}"

say "== wait for the root's result"
until grep -q 'the root is done' "$dir/brain.log"; do sleep 0.5; done
sleep 0.5
say "done; the brain's own log:"
echo
cat "$dir/brain.log"
