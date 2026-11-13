#!/usr/bin/env bash
# A chat's blob while its log loads. Two scripted chats: a long one (a
# tool call and a 200-line reply) and a short one. A headless page, its
# network slowed to 2.5 s a request, opens each, at desktop and phone
# size: while the page fetches the context its blob waits in the middle
# of the empty log, grey and bare, its balls circling (`loading`); once
# the log is drawn it glides to its place below the last entry, its
# buttons with it, and takes the chat's state. Reopening a chat the page
# already holds skips the loading. Shots go to .drive.ignore/chat-loading/.
# The brain stays up: open a chat's link with DevTools throttling on to
# watch it.
set -euo pipefail
cd "$(dirname "$0")/../.."
brain=chat-loading out=$PWD/.drive.ignore/chat-loading
node dev/drive.mjs down --brain $brain --wipe
node dev/drive.mjs up --brain $brain
mkdir -p "$out/wd"
long=$(python3 -c 'import json; print("tool bash " + json.dumps({"command": "seq 1 400", "summary": "count to 400"}) + "\nlong 200")')
long=$(node dev/drive.mjs chat --brain $brain chat "$out/wd" "$long")
short=$(node dev/drive.mjs chat --brain $brain chat "$out/wd" "say A short chat: one reply.")
node dev/drive.mjs settle --brain $brain "$long" >/dev/null
node dev/drive.mjs settle --brain $brain "$short" >/dev/null

slow='{"offline":false,"latency":2500,"downloadThroughput":200000,"uploadThroughput":200000}'
# Where the blob is: its centre, down the log pane and below the last
# entry drawn, and whether the pane is loading.
where='(() => { const p = document.querySelector(".pane"), b = document.querySelector(".pane > .dock .blob").getBoundingClientRect(), s = document.querySelector(".scroll").getBoundingClientRect(), y = b.top + b.height / 2, last = [...document.querySelectorAll(".log > *")].filter((e) => e.offsetHeight).at(-1); return `${p.classList.contains("loading") ? "loading" : "loaded "}: blob ${Math.round(y - s.top)} px down a ${Math.round(s.height)} px log, ` + (last ? `${Math.round(y - last.getBoundingClientRect().bottom)} px below its last entry` : "the log empty"); })()'
for size in desktop phone; do
  echo "== $size"
  flags=(); [ $size = phone ] && flags=(--size phone)
  node dev/drive.mjs page --brain $brain "${flags[@]}" \
    cdp Network.enable '{}' cdp Network.emulateNetworkConditions "$slow" \
    open "$long" wait ".pane.loading" eval "$where" shot "$out/$size-long-loading.png" \
    gone ".pane.loading" sleep 1000 eval "$where" shot "$out/$size-long-loaded.png" \
    open "$short" wait ".pane.loading" eval "$where" shot "$out/$size-short-loading.png" \
    gone ".pane.loading" sleep 1000 eval "$where" shot "$out/$size-short-loaded.png" \
    open "$long" sleep 100 eval "\"reopened, 0.1 s later: \" + $where" \
    2>&1 | grep -v '^{}$\|warning (rendering)\|^console, its last\|^the run'"'"'s log'
done
echo "== the chats: #$long (long), #$short (short), on the brain's link above"
echo "end it with: node dev/drive.mjs down --brain $brain"
