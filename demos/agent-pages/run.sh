#!/usr/bin/env bash
# An agent facing Max answers him with a page. A scripted chat describes
# its plan, writes its page (a lead, where it ends, where it stands, why
# this route) with an ask in the plan's CI step, and presents it: its turn
# ends facing Max, and the face shows the page instead of the chat, open
# at its top, the chat above it on scrolling up. Prints the page as the
# view carries it, and screenshots it wide and on a phone into
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
chat=$(node dev/drive.mjs chat --brain $b chat "$dir" "$(cat demos/agent-pages/mandate.md)")
node dev/drive.mjs settle --brain $b "$chat" >/dev/null
node dev/drive.mjs log --brain $b "$chat"
echo
echo "== the chat's page, as the view carries it"
curl -sk "https://127.0.0.1:$port/view" | jq --arg t "$chat" '.rows["task:"+$t].Task
  | { page: (.page | { version, current, lead, end: .end.says, now: .now.says,
      plan: [.plan[] | { title, state, asks: [.asks[] | { question, want, open, otherwise }] }] }) }'
echo
echo "== the face, wide and on a phone"
node dev/drive.mjs page --brain $b open "$chat" wait ".sheet:not([hidden]) h1" sleep 1500 shot "$shots/wide.png"
node dev/drive.mjs page --brain $b --size phone open "$chat" wait ".sheet:not([hidden]) h1" sleep 1500 shot "$shots/phone.png"
echo "$shots/wide.png"
echo "$shots/phone.png"
echo "end it with: node dev/drive.mjs down --brain $b"
