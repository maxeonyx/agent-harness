#!/usr/bin/env bash
# The map's framing fits what is drawn, on the scripted test brain
# `overview-frame`: one chat, so the whole map is one node, opened on a
# phone (390x844) and a desktop (1440x900), and framed again by Fit (F)
# after a wheel moves it away. Each time it prints how much empty paper
# lies above the map and below it, down to the bottom bar, and fails
# unless the two are within 24 px: the map sits in the middle of the room the bottom bar leaves, not
# low with the top of the screen empty. The pictures it takes are
# .drive.ignore/overview-frame/{open,fit}-{phone,desktop}.png.
# The brain stays up.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain overview-frame "${@:2}"; }
at=.drive.ignore/overview-frame
drive down --wipe
drive up
mkdir -p "$at/w"
chat=$(drive chat chat "$PWD/$at/w" "Hello there")
drive settle "$chat" >/dev/null

# Empty paper above the drawn nodes and below them, to the bottom bar.
gaps='(() => {
  const r = [...document.querySelectorAll("#nodes .node .hit, #nodes .node .name")].map((e) => e.getBoundingClientRect()).filter((b) => b.height);
  const top = Math.min(...r.map((b) => b.top)), bottom = Math.max(...r.map((b) => b.bottom));
  const room = document.querySelector("#bottom").getBoundingClientRect().top;
  return `${Math.round(top)} ${Math.round(room - bottom)}`;
})()'
fail=0
for size in phone:390x844 desktop:1440x900; do
  name=${size%%:*}
  out=$(drive page --size "${size#*:}" wait "#nodes .node" sleep 1500 \
    shot "$PWD/$at/open-$name.png" eval "$gaps" \
    eval "document.querySelector('#map').dispatchEvent(new WheelEvent('wheel', { deltaY: 400, clientX: 100, clientY: 100, bubbles: true, cancelable: true }))" \
    sleep 600 key f sleep 1200 \
    shot "$PWD/$at/fit-$name.png" eval "$gaps" 2>&1 | grep -E '^"[0-9-]+ [0-9-]+"$' | tr -d '"')
  [ "$(wc -l <<<"$out")" -eq 2 ] || { echo "$name: no measurement"; exit 1; }
  for when in open fit; do
    read -r above below <<<"$(sed -n "$([ $when = open ] && echo 1 || echo 2)p" <<<"$out")"
    verdict=ok; [ $(( above > below ? above - below : below - above )) -le 24 ] || { verdict=FAIL; fail=1; }
    echo "$name, $when: ${above}px empty above the map, ${below}px below it: $verdict"
  done
done
port=$(cat "$at/port")
echo "Open https://mx-hv-mj.local:$port/; end it with: node dev/drive.mjs down --brain overview-frame"
exit $fail
