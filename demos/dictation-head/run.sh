#!/usr/bin/env bash
# Dictation keeps its first words on a slow connection: a phone-sized page
# whose microphone plays demos/home/sample.wav, on a network with 1.5 s of
# latency (DevTools' network emulation), so the dictation socket opens well
# after the microphone does, as it can on Max's phone through the tunnel.
# The box must start with the recording's first words, "This is the
# harness's own compose box"; it fails, printing what the box holds, if
# they are lost. Uses the dictation endpoint on the home brain's login.
set -euo pipefail
cd "$(dirname "$0")/../.."
B=(--brain dictation-head)
node dev/drive.mjs down "${B[@]}" --wipe
node dev/drive.mjs up "${B[@]}"
dir=$PWD/.drive.ignore/dictation-head/wd
mkdir -p "$dir"
chat=$(node dev/drive.mjs chat "${B[@]}" chat "$dir" "hello")
node dev/drive.mjs settle "${B[@]}" "$chat"
box='document.querySelector(".compose textarea").value'
node dev/drive.mjs page "${B[@]}" --size phone --mic demos/home/sample.wav open "$chat" wait ".compose .mic" \
  cdp Network.enable '{}' \
  cdp Network.emulateNetworkConditions '{"offline":false,"latency":1500,"downloadThroughput":-1,"uploadThroughput":-1}' \
  click ".compose .mic" sleep 20000 click ".compose .mic" sleep 3000 \
  eval "(() => { const v = $box; if (!v.startsWith(\"This is the harness\")) throw new Error(\"the start was lost: \" + JSON.stringify(v)); return v; })()"
node dev/drive.mjs down "${B[@]}"
