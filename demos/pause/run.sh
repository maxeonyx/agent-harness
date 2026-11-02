#!/usr/bin/env bash
# Pausing a tree: everything in it comes to rest in the brain, grey. One
# chat on the scripted model spawns two forks: `streamer`, whose slow reply
# asks for a tool, and `sleeper`, which runs `sleep 15`. On a phone-sized
# page, mid-stream and mid-call, it presses Pause on the chat. Each task
# shows what it is really doing until it comes to rest: the stream ends
# with its call left undispatched, the sleep ends, and every task in the
# tree waits `held`: a still grey blob in the brain's shape. It prints
# each task's waiting kind as it goes, leaves screenshots in
# .drive.ignore/pause/, and leaves the brain `pause` running.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain pause "${@:2}"; }
at=.drive.ignore/pause
drive down --wipe
drive up
mkdir -p "$at/dir"
port=$(cat "$at/port")
kinds() { curl -sk "https://localhost:$port/view" | jq -c '[.rows[] | objects | .Task? // empty | {(.name[0:12]): .waiting.kind}] | add'; }

id=$(drive chat chat "$PWD/$at/dir" 'tool spawn {"tasks":[{"name":"streamer","mandate":"slow 2500 tool bash {\"command\":\"echo hi\",\"summary\":\"Say hi\"}\nsay streamer done"},{"name":"sleeper","mandate":"tool bash {\"command\":\"sleep 15\",\"summary\":\"Sleep a while\"}\nsay sleeper done"}]}
say parent done')
drive page --size phone open "$id" sleep 3000 shot "$PWD/$at/1-running.png" \
  click '[aria-label=Pause]' sleep 800 shot "$PWD/$at/2-paused-busy.png" \
  click 'button.x' sleep 800 shot "$PWD/$at/3-map-busy.png"
echo "just paused: $(kinds)"
sleep 20
echo "at rest:     $(kinds)"
drive page --size phone open "$id" click 'button.x' sleep 1000 shot "$PWD/$at/4-map-rest.png"
