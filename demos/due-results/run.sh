#!/usr/bin/env bash
# Which results are still due to a chat, on the scripted test brain `due`:
# a chat sits below the work whose results it has still to read, and only
# that. Five chats:
# - Abandon starts victim (in its own clone, so it is still starting) and
#   fine in the background and waits on both; Max abandons victim before
#   it has said a word. Abandon reads the spawn's result and waits on Max.
# - Unread starts quick in the background, then runs `sleep 600`: quick
#   is done, but its result is unread while the sleep runs.
# - Carry starts quick in the background, then waits on it alongside a
#   compaction, so the carry reads its result.
# - Fore spawns quick and waits for it.
# - Pass starts `sleep 600` and slow in the background and passes both to
#   Max.
# Open the link it prints, on the phone and the desktop, and watch:
# - Abandon, Carry, Fore and Pass each sit at their last leg, a big solid
#   blob, with no dotted line anywhere: victim is a grey dot hanging from
#   the spawn point, the members dots joined into the chat's last leg.
# - Pass's sleep and slow branch off to the side, never joined.
# - Unread's big hollow node sits below quick, joined to it by a dotted
#   line, until the sleep ends.
# The brain stays up.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain due "${@:2}"; }
at=.drive.ignore/due
drive down --wipe
drive up
cargo build -q --release -p harness-brain --example act
port=$(cat "$at/port")
act() { target/release/examples/act "$port" "$@" >/dev/null; }
view() { curl -sk "https://localhost:$port/view"; }
q() { python3 -c 'import json, sys; print(json.dumps(sys.argv[1]))' "$1"; }
for d in a u c f p; do mkdir -p "$at/$d"; done
git init -q "$at/a/repo" && git -C "$at/a/repo" commit -q --allow-empty -m init

# A spawn's input, its tasks as JSON.
spawn() { echo "{\"tasks\": [$1]}"; }
bg() { echo "tool background {\"name\": \"$1\", \"tool\": \"$2\", \"input\": $3}"; }
quick="{\"name\": \"quick\", \"mandate\": \"say Quick is done.\"}"
sleep600='{"command": "sleep 600", "timeout_ms": 600000}'

a=$(drive chat chat "$PWD/$at/a" "Abandon
$(bg two spawn "$(spawn "{\"name\": \"victim\", \"role\": \"chat\", \"branch\": \"victim\", \"base\": \"master\", \"from\": \"repo\", \"mandate\": \"say Victim is done.\"}, {\"name\": \"fine\", \"mandate\": \"say Fine is done.\"}")")
tool wait {\"until\": \"all\"}
say Read them both.")
victim=""
for _ in $(seq 200); do
  victim=$(view | jq -r '.rows[] | .Task? // empty | select(.name | endswith("victim")) | .id')
  [ -n "$victim" ] && break
  sleep .05
done
act abandon "$victim" 0
drive settle "$a" >/dev/null

u=$(drive chat chat "$PWD/$at/u" "Unread
$(bg one spawn "$(spawn "$quick")")
tool bash $sleep600
say Slept.")

c=$(drive chat chat "$PWD/$at/c" "Carry
$(bg one spawn "$(spawn "$quick")")
tool wait {} tool compact_and_continue {\"carry\": $(q 'say Carried.')}")
drive settle "$c" >/dev/null

f=$(drive chat chat "$PWD/$at/f" "Fore
tool spawn $(spawn "$quick")
say Quick is back.")
drive settle "$f" >/dev/null

slow="tool bash $sleep600
say Done."
p=$(drive chat chat "$PWD/$at/p" "Pass
$(bg nap bash "$sleep600") $(bg slow spawn "$(spawn "{\"name\": \"slow\", \"mandate\": $(q "$slow")}")")
tool pass {\"call\": \"nap\", \"why\": \"Yours.\"} tool pass {\"call\": \"slow\", \"why\": \"Yours.\"}
say Passed both.")
drive settle "$p" >/dev/null

echo "Open https://mx-hv-mj.local:$port/ (Abandon #$a, Unread #$u, Carry #$c, Fore #$f, Pass #$p); end it with: node dev/drive.mjs down --brain due"
