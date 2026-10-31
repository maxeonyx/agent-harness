#!/usr/bin/env bash
# Each task's legs, as /view gives them: the stretches of its context, and
# what each depends on. One chat, A, on the scripted model: it spawns B and
# C, whose results join; it compacts and reports; Max writes twice and edits
# his second message, which runs `sleep 5` in the background, whose result
# joins; then A forks itself. The legs are printed, the brain is killed -9
# and relaunched, and the legs are compared: they are rebuilt from facts, so
# they must be identical. It runs on the test brain `legs`, whose link it
# prints, and leaves it running.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain legs "${@:2}"; }
at=.drive.ignore/legs
drive down --wipe
drive up
mkdir -p "$at/dir"
port=$(cat "$at/port")

a=$(drive chat chat "$PWD/$at/dir" 'tool spawn {"tasks": [{"name": "B", "mandate": "say B is done."}, {"name": "C", "mandate": "say C is done."}]}
tool compact_and_report {"carry": "B and C are done.", "report": "B and C are done."}')
drive settle "$a" >/dev/null
drive say "$a" "say First words." >/dev/null
drive settle "$a" >/dev/null
drive say "$a" "say Second words." >/dev/null
drive settle "$a" >/dev/null
# Max edits his second message, as he does on the page.
edited='tool background {"name": "nap", "tool": "bash", "input": {"command": "sleep 5"}}
say Napping.
tool spawn {"tasks": [{"name": "f", "mandate": "say A fork of A."}]}
say Forked.'
drive page open "$a" click '.moves .mv[title^="Edit this message"] @last' \
  eval "(() => { const box = document.querySelector('.field textarea'); box.value = $(jq -Rs . <<<"$edited"); box.dispatchEvent(new Event('input', {bubbles: true})); })()" \
  click ".field .send, .send" sleep 300 click ".send" sleep 1000 >/dev/null
sleep 7
drive settle "$a" >/dev/null
sleep 3
drive settle "$a" >/dev/null

# A leg's key is <task>/<began>[:<entry>]: tasks by name (the chat is A),
# entries cut to 8 characters.
legs() {
  curl -sk "https://localhost:$port/view" | jq -r --arg a "$a" '
    [.rows[] | .Task? | select(.)] as $tasks |
    ($tasks | map({key: .id, value: (if .id == $a then "A" else .name end)}) | from_entries) as $names |
    def cut: gsub("(?<x>[0-9a-f]{8})[0-9a-f]{56}"; .x) |
      reduce ($names | to_entries[]) as $n (.; gsub($n.key | gsub("\\."; "\\."); $n.value));
    $tasks | sort_by(.legs[0].at)[] |
    "\($names[.id])  head leg: \(.leg | cut)",
    (.legs[] | "  \(.key | cut)" + (if .on == [] then "" else "  <- " + ([.on[] | if .leg then "\(.leg | cut) @\(.at[0:8])" else "work \(.work)" end] | join(", ")) end)),
    (.work[] | "  work \(.name): on \(.leg | cut) @\(.at[0:8]), running \(.running)")'
}
echo "== the legs, after the run"
legs | tee "$at/legs-before.txt"
echo
echo "== killed -9 and relaunched"
kill -9 "$(cat "$at/pid")"
drive up >/dev/null
port=$(cat "$at/port")
legs >"$at/legs-after.txt"
if cmp -s "$at/legs-before.txt" "$at/legs-after.txt"; then echo "the legs are identical"; else diff "$at/legs-before.txt" "$at/legs-after.txt"; exit 1; fi
echo
echo "Open https://mx-hv-mj.local:$port/#$a; end it with: node dev/drive.mjs down --brain legs"
