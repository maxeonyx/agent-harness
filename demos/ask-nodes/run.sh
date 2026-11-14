#!/usr/bin/env bash
# An ask is Max's on the map. A scripted chat asks three things in the
# background (`ask`), withdraws one, and presents its page; Max answers a
# second by pressing it and sending. On the map each ask hangs from the
# chat on Max's orange line: the one still open is his orange card with
# the mic, like work handed to him, and the two settled ones are small
# gears at the end of their orange lines. `present` draws nothing of its
# own: the chat's blob waits on Max. Fails unless exactly the one open ask
# is drawn as Max's card. Screenshots wide and on a phone into
# demos/ask-nodes/shots.ignore. Scripted: no model call. Leaves the brain
# up.
set -euo pipefail
cd "$(dirname "$0")/../.."
b=ask-nodes
node dev/drive.mjs down --brain $b --wipe
node dev/drive.mjs up --brain $b
dir=$PWD/.drive.ignore/$b/dir
shots=$PWD/demos/ask-nodes/shots.ignore
mkdir -p "$dir" "$shots"
port=$(cat .drive.ignore/$b/port)
drive() { node dev/drive.mjs "$1" --brain $b "${@:2}"; }
page() { drive page "$@" 2>&1 | grep -v "WebGL"; }
view() { curl -sk "https://127.0.0.1:$port/view" | jq -c --arg t "$1" ".rows[\"task:\"+\$t].Task | $2"; }
a=$(drive chat chat "$dir" "$(cat demos/ask-nodes/mandate.md)")
drive settle "$a" >/dev/null
echo "== Max answers the docs ask: pressed, then sent"
page open "$a" wait ".sheet:not([hidden]) h1" sleep 1000 \
  eval "const q = [...document.querySelectorAll('.sheet .ask')].find((x) => x.textContent.includes('build notes')); q.querySelector('.press').click(), 1" sleep 500 \
  type ".compose textarea" "text \"AGENTS.md.\" tool present {}" \
  click ".compose .send" sleep 500 click ".compose .send" reply sleep 1000 >/dev/null
drive settle "$a" >/dev/null
echo "== the chat's asks, as its work"
view "$a" '[.work[] | { name, tool, running }]'
echo
echo "== the map, wide and on a phone: Max's cards"
check="const y = [...document.querySelectorAll('.node.work.yours .name')].map((e) => e.textContent); if (y.join() !== 'ci-cache') throw new Error('Max\\'s cards: ' + JSON.stringify(y)); 'Max\\'s cards: ' + y.join()"
page sleep 3000 shot "$shots/map-wide.png" eval "$check"
page --size phone sleep 3000 shot "$shots/map-phone.png" eval "$check"
ls "$shots"/*.png
echo "end it with: node dev/drive.mjs down --brain $b"
