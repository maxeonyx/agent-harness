#!/usr/bin/env bash
# The map as the context graph. Two scripted chats on the test brain
# `graph`. The first, A, has every shape a past can have: it spawns B, C
# and D, whose results join; it compacts and reports; Max writes twice and
# edits his second message, which leaves the first version as an old leg;
# a background `sleep 5` joins; then A forks itself. The second, Waits,
# spawns three agents that each run `sleep 600`, and starts a background
# `sleep 600` of its own: the three work in a row beside it, its next leg
# hangs below them with a 3, and the running call is a chip.
# Open the link it prints, on the phone and the desktop: the past rises
# above the present row as small dots joined by tapering lines; a tap on a
# dot opens that chat at that stretch (`#<task>@<entry>`). The brain stays
# up.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain graph "${@:2}"; }
at=.drive.ignore/graph
drive down --wipe
drive up
mkdir -p "$at/a" "$at/b"
port=$(cat "$at/port")

a=$(drive chat chat "$PWD/$at/a" 'tool spawn {"tasks": [{"name": "B", "mandate": "say B is done."}, {"name": "C", "mandate": "say C is done."}, {"name": "D", "mandate": "say D is done."}]}
tool compact_and_continue {"carry": "B, C and D are done."}
say Carried on.')
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

work='tool bash {"command": "sleep 600", "timeout_ms": 600000, "summary": "work for ten minutes"}
say Done.'
w=$(drive chat chat "$PWD/$at/b" "tool background {\"name\": \"watch\", \"tool\": \"bash\", \"input\": {\"command\": \"sleep 600\", \"timeout_ms\": 600000}}
tool spawn {\"tasks\": [{\"name\": \"one\", \"mandate\": $(jq -Rs . <<<"$work")}, {\"name\": \"two\", \"mandate\": $(jq -Rs . <<<"$work")}, {\"name\": \"three\", \"mandate\": $(jq -Rs . <<<"$work")}]}
say All three are done.")
sleep 3

echo "Open https://mx-hv-mj.local:$port/ (A is #$a, Waits is #$w); end it with: node dev/drive.mjs down --brain graph"
