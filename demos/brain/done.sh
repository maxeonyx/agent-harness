#!/usr/bin/env bash
# Max's part at the end of a run: once the root task (a chat with him, like
# every task he starts) has ended its turn and waits on him, press Done the
# way the page does, so its next turn writes its result. Usage: done.sh PORT
set -euo pipefail
port=$1
url=https://mx-hv-mj.local:$port
c() { curl -sS --resolve "mx-hv-mj.local:$port:127.0.0.1" "$@"; }
hex() { head -c16 /dev/urandom | od -An -tx1 | tr -d ' \n'; }
root=$(c "$url/view" | jq -r '.tasks[] | select(.owner == null) | .id')
until [ "$(c "$url/view" | jq -r ".tasks[] | select(.id == \"$root\") | .waiting.kind")" = max ]; do sleep 0.5; done
seen=$(c "$url/view" | jq .seq)
c -X POST "$url/act" --data-binary "$(jq -nc --arg face "$(hex)" --argjson seen "$seen" --arg id "$(hex)" --arg to "$root" \
  '{face: $face, seen: $seen, batch: {values: [], facts: [{Delivery: {id: $id, to: $to, content: {Prompt: "Done"}}}]}}')"
