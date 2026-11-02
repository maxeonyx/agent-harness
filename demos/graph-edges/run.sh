#!/usr/bin/env bash
# The map's edge cases, on the scripted test brain `edges`. Waits starts a
# background `sleep 600` (watch) and a `sleep 1` (nap), then spawns three
# agents: quick says it is done at once, slow-1 and slow-2 run `sleep 600`.
# Max writes to Waits twice while it waits, which opens an aside each time,
# and marks each aside done. Hi is a chat Max abandons after one reply.
# Pips compacts seven times.
# Open the link it prints, on the phone and the desktop, and watch:
# - Waits's planned join, under it, is a hollow grey blob with a dotted
#   outline and a 2 in it; the chat's own blob, opened, is purple.
# - quick, finished, is a dot still joined to the planned join by a dotted line.
# - both asides are green dots, each joined to the planned join by a dotted line.
# - watch, running, is a gear on a solid line; nap, finished, a faint small dot on a faint line.
# - Hi is a card in the archive, not a track of its own.
# - Pips's blob has seven compaction dots, from ten o'clock anticlockwise.
# The brain stays up.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain edges "${@:2}"; }
at=.drive.ignore/edges
drive down --wipe
drive up
cargo build -q --release -p harness-brain --example act
port=$(cat "$at/port")
act() { target/release/examples/act "$port" "$@" >/dev/null; }
mkdir -p "$at/w" "$at/h" "$at/p"
view() { curl -sk "https://localhost:$port/view"; }

slow='tool bash {"command": "sleep 600", "timeout_ms": 600000, "summary": "work for ten minutes"}
say Done.'
w=$(drive chat chat "$PWD/$at/w" "Waits
tool background {\"name\": \"watch\", \"tool\": \"bash\", \"input\": {\"command\": \"sleep 600\", \"timeout_ms\": 600000}} tool background {\"name\": \"nap\", \"tool\": \"bash\", \"input\": {\"command\": \"sleep 1\"}}
tool spawn {\"tasks\": [{\"name\": \"quick\", \"mandate\": \"say Quick is done.\"}, {\"name\": \"slow-1\", \"mandate\": $(jq -Rs . <<<"$slow")}, {\"name\": \"slow-2\", \"mandate\": $(jq -Rs . <<<"$slow")}]}
say All three are done.")
sleep 4

# An aside: Max's words to Waits while it waits; it replies, and he marks it done.
aside() {
  act say "$w" "say $1"
  local id=""
  for _ in $(seq 40); do
    id=$(view | jq -r --arg w "$w" --arg n "$2" '.rows[] | .Task? // empty | select(.origin == "aside" and .from == $w) | .id' | tail -n +"$2" | head -1)
    [ -n "$id" ] && break
    sleep .5
  done
  drive settle "$id" >/dev/null
  act done "$id"
  drive settle "$id" >/dev/null
}
aside "An aside." 1
aside "Another aside." 2

h=$(drive chat chat "$PWD/$at/h" 'Hi
say Hello.')
drive settle "$h" >/dev/null
act abandon "$h" 0

# Each carry holds the script that follows it: the scripted model reads
# the latest words, and after a compaction those are the carry.
steps="say Carried on seven times."
for i in 7 6 5 4 3 2 1; do
  steps="tool bash {\"command\": \"echo $i\", \"summary\": \"step $i\"}
tool compact_and_continue {\"carry\": $(jq -Rs . <<<"$steps")}"
done
p=$(drive chat chat "$PWD/$at/p" "Pips
$steps")
drive settle "$p" >/dev/null

echo "Open https://mx-hv-mj.local:$port/ (Waits is #$w, Hi #$h, Pips #$p); end it with: node dev/drive.mjs down --brain edges"
