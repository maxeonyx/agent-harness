#!/usr/bin/env bash
# Max's part at the end of a run: once the root task (a chat with him, like
# every task he starts) has ended its turn and waits on him, press Done at a
# tab, so its next turn writes its result. Usage: done.sh PORT CHAT
set -euo pipefail
port=$1 chat=$2
c() { curl -sS --resolve "mx-hv-mj.local:$port:127.0.0.1" "https://mx-hv-mj.local:$port$1"; }
until [ "$(c /view 2>/dev/null | jq -r ".rows[\"task:$chat\"].Task.waiting.kind")" = max ]; do sleep 0.5; done
target/release/examples/act "$port" done "$chat"
