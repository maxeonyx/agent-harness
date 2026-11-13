#!/usr/bin/env bash
# A provider outage of minutes is ridden out, and a refusal for good is
# not retried. One chat's request is refused with 503 six times (the
# scripted model's `flaky 503x6`): one more than the five failures that
# used to end a task. The brain backs off (2, 4, 8, 16, 32 s, then a
# minute), the chat shows it is waiting on the provider and when it tries
# again, and the seventh try is answered. Another chat's request is refused
# with 400, which the same request would get every time: it fails at once.
# Scripted: no model call.
set -euo pipefail
cd "$(dirname "$0")/../.."
brain=provider-outage
drive() { node dev/drive.mjs "$1" --brain $brain "${@:2}"; }
node dev/drive.mjs down --brain $brain --wipe
node dev/drive.mjs up --brain $brain
at=.drive.ignore/$brain
port=$(cat "$at/port")
dir=$PWD/$at/dir
mkdir -p "$dir"
trouble() { curl -sk "https://localhost:$port/view" | jq -c --arg id "$1" '.. | objects | select(.id? == $id and has("trouble")) | {failures, trouble, outcome}' | head -1; }

echo "== a chat whose request the provider refuses with 503, six times over"
chat=$(drive chat chat "$dir" 'flaky 503x6 say Back, after the provider came back.')
seen=
until drive log "$chat" | grep -v '^max:' | grep -q 'Back, after'; do
  now=$(trouble "$chat")
  if [ "$now" != "$seen" ]; then echo "$(date +%T) $now"; seen=$now; fi
  if [ "$now" != "${now/\"failures\":3/}" ] && [ ! -e "$at/waiting.png" ]; then
    echo "   the page shows: $(drive page open "$chat" text "Waiting on the provider" shot "$at/waiting.png" \
      eval 'document.querySelector(".entry.card.ended").textContent' | grep -m1 'Waiting on the provider')"
  fi
  sleep 1
done
echo "$(date +%T) $(trouble "$chat")"
drive log "$chat" | tail -3

echo
echo "== a chat whose request the provider refuses with 400: no retry"
bad=$(drive chat chat "$dir" 'error 400 scripted: this request is invalid')
sleep 3
trouble "$bad"
grep -E 'failed|rejected' "$at/brain.log" | cut -c1-200 | tail -3
echo
echo "the page as it waited: $at/waiting.png"
echo "end it with: node dev/drive.mjs down --brain $brain"
