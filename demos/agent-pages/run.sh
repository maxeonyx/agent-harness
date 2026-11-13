#!/usr/bin/env bash
# An agent facing Max answers him with a page, in its area's theme. Two
# scripted chats are two areas, each given its own theme by the harness.
# The first describes its plan, writes its page, asks a choice in the plan's
# CI step and asks again in the docs step, withdraws that, and presents: the
# withdrawn ask stays where it was, with why. Max presses an answer, which
# is held until he sends it with his words; the agent's answer is a quick
# line above its page, which still holds, folded to its lead. Then the agent
# changes its page: where it ends, which Max's reply agreed with, folds to
# its claim. Last, the map: each area faint on its theme's ground.
# Screenshots wide and on a phone into
# demos/agent-pages/shots.ignore. Scripted: no model call. Leaves the
# brain up.
set -euo pipefail
cd "$(dirname "$0")/../.."
b=agent-pages
node dev/drive.mjs down --brain $b --wipe
node dev/drive.mjs up --brain $b
dir=$PWD/.drive.ignore/$b/dir
shots=$PWD/demos/agent-pages/shots.ignore
mkdir -p "$dir" "$shots"
port=$(cat .drive.ignore/$b/port)
drive() { node dev/drive.mjs "$1" --brain $b "${@:2}"; }
page() { drive page "$@" 2>&1 | grep -v "WebGL"; }
a=$(drive chat chat "$dir" "$(cat demos/agent-pages/mandate.md)")
c=$(drive chat chat "$dir" "$(cat demos/agent-pages/second.md)")
drive settle "$a" >/dev/null
drive settle "$c" >/dev/null
view() { curl -sk "https://127.0.0.1:$port/view" | jq -c --arg t "$1" ".rows[\"task:\"+\$t].Task | $2"; }
echo "== each area's theme"
view "$a" '{ name, look }'
view "$c" '{ name, look }'
echo "== the first page's asks, where they are drawn"
view "$a" '[.page.plan[] | { step: .key, asks: [.asks[] | { key, settled }] }]'
echo
echo "== each area's page, wide and on a phone"
page open "$a" wait ".sheet:not([hidden]) h1" sleep 1500 shot "$shots/first-wide.png"
page --size phone open "$a" wait ".sheet:not([hidden]) h1" sleep 1500 shot "$shots/first-phone.png"
page open "$c" wait ".sheet:not([hidden]) h1" sleep 1500 shot "$shots/second-wide.png"
page --size phone open "$c" wait ".sheet:not([hidden]) h1" sleep 1500 shot "$shots/second-phone.png"
echo
echo "== Max presses an answer: held, then sent with his words"
page open "$a" wait ".sheet:not([hidden]) h1" sleep 1000 \
  eval "document.querySelector('.sheet .press').scrollIntoView({ block: 'center' }), 1" \
  click ".sheet .press" sleep 500 \
  eval "[...document.querySelectorAll('.sheet .press')].map((b) => b.textContent + (b.classList.contains('armed') ? ' (held)' : '')).join(' | ')" \
  shot "$shots/press-held.png" \
  type ".compose textarea" "text \"Going with GitHub's cache. The page still holds.\" tool present {}" \
  click ".compose .send" sleep 500 click ".compose .send" reply wait ".sheet.folded .checked" sleep 1000 \
  eval "document.querySelector('.sheet.folded .checked')?.textContent" \
  shot "$shots/quick-wide.png"
page --size phone open "$a" wait ".sheet.folded" sleep 1500 shot "$shots/quick-phone.png"
drive log "$a" | grep -A2 "has settled" | tail -3
echo
echo "== the agent changes where it stands: the end Max's reply agreed with stays folded"
drive say "$a" 'tool page_edit {"edits": [{"old": "| 52 s |", "new": "| 50 s |"}]} tool present {}' >/dev/null
drive settle "$a" >/dev/null
view "$a" '.page | { version, held, end: (.end | { says, agreed, changed }) }'
page open "$a" wait ".sheet .end.agreed" sleep 1500 shot "$shots/agreed-wide.png"
page --size phone open "$a" wait ".sheet .end.agreed" sleep 1500 shot "$shots/agreed-phone.png"
echo
echo "== the map: each area on its ground, faint behind its part"
page sleep 3000 shot "$shots/map-wide.png"
ls "$shots"/*.png
echo "end it with: node dev/drive.mjs down --brain $b"
