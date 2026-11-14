#!/usr/bin/env bash
# An agent shows Max pictures on its page. A scripted chat writes a page,
# its page is screenshotted on a desktop and a phone into the chat's
# directory, and the desktop shot is cut into six tiles. The chat `read`s
# all eight, so it sees what Max will, and cites each `read`'s label as a
# picture's destination: `![what it shows](call-3fa2c1)`. Its page shows
# one picture across the column, a phone and a desktop shot sharing a row,
# and the six tiles in rows, each named by its alt; an ask shows its two
# options' pictures above them. A label that showed no picture is drawn as
# its alt's words, and page_write says why. Then the files are deleted,
# and the page still shows every picture: the page version holds each as
# the value the `read` returned. Each picture holds its room before it
# loads, and each loads once scrolled to: the demo fails on a picture
# with no room or one that drew 0x0. Screenshots wide and on a phone into
# demos/page-pictures/shots.ignore. Scripted: no model call. Leaves the
# brain up.
set -euo pipefail
cd "$(dirname "$0")/../.."
b=page-pictures
node dev/drive.mjs down --brain $b --wipe
node dev/drive.mjs up --brain $b
dir=$PWD/.drive.ignore/$b/dir
shots=$PWD/demos/page-pictures/shots.ignore
mkdir -p "$dir" "$shots"
drive() { node dev/drive.mjs "$1" --brain $b "${@:2}"; }
page() { drive page "$@" 2>&1 | grep -v "WebGL"; }
# Each picture's room as first drawn, before any has loaded, then each
# scrolled to and loaded, as its natural size.
room="'room ' + [...document.querySelectorAll('.sheet img')].map((i) => Math.round(i.getBoundingClientRect().width) + 'x' + Math.round(i.getBoundingClientRect().height)).join(' ')"
sizes="(async () => { const out = []; for (const i of document.querySelectorAll('.sheet img')) { i.scrollIntoView({ block: 'center' }); await i.decode().catch(() => {}); out.push(i.naturalWidth + 'x' + i.naturalHeight); } scrollTo(0, 0); return 'loaded ' + out.join(' '); })()"
# Fails the demo on a picture with no room, or one that drew nothing.
drew() { echo "$1"; if grep -qE '(room|loaded)( [0-9]+x[0-9]+)* (0x[0-9]+|[0-9]+x0)\b' <<<"$1"; then echo "a picture had no room or drew 0x0" >&2; exit 1; fi; }
at() { echo "document.querySelector('$1').scrollIntoView({ block: 'start' }), scrollBy(0, -40), 1"; }

first=$(jq -cn '{ lead: "What the page looks like", now: "| Shot | Taken |\n|---|---|\n| desktop | next |" }')
a=$(drive chat chat "$dir" "tool page_write $first tool present {}")
drive settle "$a" >/dev/null

echo "== the page, shot on a desktop and a phone, the desktop cut into six tiles"
page open "$a" wait ".sheet:not([hidden]) h1" sleep 1500 shot "$dir/desktop.png"
page --size phone open "$a" wait ".sheet:not([hidden]) h1" sleep 1500 shot "$dir/phone.png"
magick "$dir/desktop.png" -crop 3x2@ +repage "$dir/tile-%d.png"
ls "$dir"/*.png

echo "== the chat reads all eight, and each read's label is what it cites"
reads="tool read {\"path\":\"desktop.png\"} tool read {\"path\":\"phone.png\"}"
for n in 0 1 2 3 4 5; do reads+=" tool read {\"path\":\"tile-$n.png\"}"; done
drive say "$a" "$reads" >/dev/null
drive settle "$a" >/dev/null
mapfile -t l < <(drive log "$a" | grep -oE "call-[0-9a-f]{6}" | awk '!seen[$0]++' | tail -8)
echo "${l[@]}"

now="The page on a desktop:

![The page on a desktop](${l[0]})

On a phone and on a desktop:

![On a phone](${l[1]}) ![On a desktop](${l[0]})

The desktop shot in six tiles:

![Top left](${l[2]}) ![Top middle](${l[3]}) ![Top right](${l[4]}) ![Bottom left](${l[5]}) ![Bottom middle](${l[6]}) ![Bottom right](${l[7]})

![A shot nobody read](call-000000)"
second=$(jq -cn --arg now "$now" '{ lead: "The page, as it drew", now: $now }')
ask=$(jq -cn --arg shows "![The phone](${l[1]}) ![The desktop](${l[0]})" '{
  key: "shot", question: "Which shot goes in the PR?", end: "The PR shows one shot",
  now: "Both taken", why: "Only you know which reads better to you", want: "choose", shows: $shows,
  options: [{ key: "phone", label: "The phone" }, { key: "desktop", label: "The desktop" }] }')
drive say "$a" "tool page_write $second tool ask $ask tool present {}" >/dev/null
drive settle "$a" >/dev/null

echo "== the page with its pictures, wide and on a phone"
drew "$(page open "$a" wait ".sheet .picture img" eval "$room" eval "$sizes" sleep 500 shot "$shots/page-wide.png")"
page --size phone open "$a" wait ".sheet .picture img" sleep 2500 shot "$shots/page-phone.png"
page --size phone open "$a" wait ".sheet .pictures img" sleep 2500 eval "$(at '.sheet .now .pictures')" sleep 800 shot "$shots/rows-phone.png"
page open "$a" wait ".sheet .ask .pictures img" sleep 2500 eval "$(at '.sheet .ask .pictures')" sleep 800 shot "$shots/ask-wide.png"

echo "== the files deleted: the page still shows every picture"
rm "$dir"/*.png
drew "$(page --size phone open "$a" wait ".sheet .picture img" eval "$room" eval "$sizes" sleep 500 shot "$shots/after-delete-phone.png")"
ls "$shots"/*.png
echo "== what page_write said of the label that showed no picture"
drive say "$a" "ok" >/dev/null # the log shows a turn's results once the next request carries them; the quick answer folds the page, so this comes last
drive settle "$a" >/dev/null
drive log "$a" --full | grep -m1 -o "a picture drawn as its alt's words, since [^:]*"
echo "end it with: node dev/drive.mjs down --brain $b"
