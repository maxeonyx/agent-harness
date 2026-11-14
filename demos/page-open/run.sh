#!/usr/bin/env bash
# Max reaches an agent's page while the agent works. A scripted chat plans,
# writes its page, asks Max where CI keeps its cache (in its last step, so
# the question is a screen or more down the page) and presents; Max sends
# it more work, a long `bash`, so the page is no longer the one in front of
# him (`current: false`) while its ask stays open. On a phone: opening the
# chat shows the page, its lead on screen; tapping the ask's orange card
# on the map opens the page with the question on screen; and the present
# call's row in the chat has a link that scrolls to the page; and the page
# sits in the log where it was presented, so the bash's live output streams
# below it and tapping the blob (Go to bottom) puts that on screen. Fails at the
# first of those that doesn't hold. Screenshots into
# demos/page-open/shots.ignore. Scripted: no model call. Leaves the brain up.
set -euo pipefail
cd "$(dirname "$0")/../.."
b=page-open
node dev/drive.mjs down --brain $b --wipe
node dev/drive.mjs up --brain $b
dir=$PWD/.drive.ignore/$b/dir
shots=$PWD/demos/page-open/shots.ignore
mkdir -p "$dir" "$shots"
port=$(cat .drive.ignore/$b/port)
drive() { node dev/drive.mjs "$1" --brain $b "${@:2}"; }
page() { drive page --size phone "$@" 2>&1 | grep -v "WebGL"; }
view() { curl -sk "https://127.0.0.1:$port/view" | jq -c --arg t "$1" ".rows[\"task:\"+\$t].Task | $2"; }
a=$(drive chat chat "$dir" "$(cat demos/page-open/mandate.md)")
drive settle "$a" >/dev/null
drive say "$a" 'tool bash {"command": "for i in $(seq 600); do echo measuring clone $i; sleep 1; done", "summary": "measure every clone"}'
sleep 5
echo "== the page, while the agent works"
view "$a" '{ current: .page.current, lead: .page.lead, asks: [.work[] | select(.tool == "ask") | { name, running }] }'
# On screen: the top of what it matches lies in the card's scroll view.
seen="(s) => { const e = document.querySelector(s), v = document.querySelector('#panel .scroll').getBoundingClientRect(); if (!e || e.closest('[hidden]')) return false; const r = e.getBoundingClientRect(); return r.top >= v.top - 1 && r.top < v.bottom - 40; }"
need() { echo "(() => { const seen = $seen; if (!seen('$1')) throw new Error('not on screen: $2'); return 'on screen: $2'; })()"; }
echo "== Max opens the chat"
page open "$a" sleep 3000 shot "$shots/open.png" eval "$(need '.sheet .lead h1' 'the page lead')"
echo "== Max taps the ask's orange card on the map"
page sleep 3000 click ".node.work.yours" sleep 3000 shot "$shots/ask.png" eval "$(need '.sheet .ask .question' 'the ask question')"
echo "== Max taps the present row's link"
page open "$a" sleep 3000 eval "document.querySelector(\".open-file[title='Open the page']\").scrollIntoView({ block: 'center' }), 1" sleep 500 \
  click ".open-file[title='Open the page']" sleep 1000 shot "$shots/present.png" eval "$(need '.sheet .lead h1' 'the page lead')"
echo "== Max taps Go to bottom while the bash prints below the page"
page open "$a" sleep 3000 click ".pane > .dock .blob" sleep 2000 shot "$shots/bottom.png" eval "$(need '.row.live .res' 'the live output')"
ls "$shots"/*.png
echo "end it with: node dev/drive.mjs down --brain $b"
