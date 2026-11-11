#!/usr/bin/env bash
# A background spawn whose results are read by a `wait` alongside
# `compact_and_continue`, on the scripted test brain `carry`. Kids spawns
# h0 and h1 in the background (each runs `sleep 3`), then in one response
# waits for them and compacts, so their results reach its context only in
# the compaction's carry. That carry is where Kids takes them in: its carry
# leg depends on each member, as a join does.
# Open the link it prints and watch the map: h0 and h1 are finished dots
# joined to Kids by solid lines, with Kids's blob below them, and no
# dashed planned join.
# The brain stays up.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain carry "${@:2}"; }
at=.drive.ignore/carry
drive down --wipe
drive up
port=$(cat "$at/port")
mkdir -p "$at/k"

member='tool bash {\"command\": \"sleep 3\", \"summary\": \"work for three seconds\"}\nsay Done.'
k=$(drive chat chat "$PWD/$at/k" "Kids
tool background {\"name\": \"kids\", \"tool\": \"spawn\", \"summary\": \"start h0 and h1\", \"input\": {\"summary\": \"two kids\", \"tasks\": [{\"name\": \"h0\", \"mandate\": \"$member\"}, {\"name\": \"h1\", \"mandate\": \"$member\"}]}}
tool wait {\"until\": \"all\", \"summary\": \"wait for the kids\"} tool compact_and_continue {\"carry\": \"say Both kids are done.\", \"summary\": \"carry their results\"}")
drive settle "$k" >/dev/null

echo "== Kids's legs"
curl -sk "https://localhost:$port/view" | jq -r --arg k "$k" '
  [.rows[] | .Task? | select(.)] as $tasks |
  ($tasks | map({key: .id, value: (if .id == $k then "Kids" else .name end)}) | from_entries) as $names |
  def cut: gsub("(?<x>[0-9a-f]{8})[0-9a-f]{56}"; .x) |
    reduce ($names | to_entries[]) as $n (.; gsub($n.key | gsub("\\."; "\\."); $n.value));
  $tasks[] | select(.id == $k) | .legs[] |
  "  \(.key | cut)" + (if .on == [] then "" else "  <- " + ([.on[] | if .leg then (.leg | cut) else "work \(.work)" end] | join(", ")) end)'
echo
echo "Open https://mx-hv-mj.local:$port/#$k; end it with: node dev/drive.mjs down --brain carry"
