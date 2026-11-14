#!/usr/bin/env bash
# A fork of a member compacts onto a fresh root, on the scripted test brain
# `fresh`. Lead starts the member `l` and tells it, with `message`, to fork
# `f`: so `f` begins with `l`'s context, which holds its owner's message to
# `l`. While `f` runs `sleep 6`, the demo edits the `chat` role, so `f`'s
# root is stale and its `compact_and_continue` starts a fresh one. The
# carry keeps Max's words and the owner's messages to `f` itself; it keeps
# no message told to `l`, which the brain's own fold would refuse.
# Watch the last lines: `f` carried on, rather than failing with "its
# compaction was refused, a bug in the brain". Its fresh context repeats
# its mandate, which is its script, so it runs it again, compacting onto
# its checkpoint the second time, before it says it carried on.
# Free, about 30 s; it leaves the brain up:
# `node dev/drive.mjs down --brain fresh`.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain fresh "${@:2}"; }
# A scripted `tool NAME INPUT` line, its input built by jq.
tool() { echo "tool $1 $(jq -cn "${@:3}" "$2")"; }
at=$PWD/.drive.ignore/fresh
drive down --wipe >/dev/null
rm -rf "$at/roles"
mkdir -p "$at/dir"
cp -r roles "$at/roles"
drive up --roles "$at/roles"
port=$(cat "$at/port")
view() { curl -sk "https://127.0.0.1:$port/view"; }

f="$(tool bash '{command: "sleep 6", summary: "work for six seconds"}')
$(tool compact_and_continue '{carry: "say f carried on.", summary: "carry on afresh"}')"
l="$(tool spawn '{summary: "fork f", fork: [{name: "f", mandate: $f}]}' --arg f "$f")
say l is done."
lead=$(drive chat chat "$at/dir" "Lead
$(tool background '{name: "l", tool: "spawn", summary: "start l", input: {summary: "l", fork: [{name: "l", mandate: "tool wait {\"summary\": \"wait to be told\"}"}]}}')
$(tool message '{to: "l", words: $l, summary: "tell l to fork f"}' --arg l "$l")
$(tool wait '{until: "all", summary: "wait for l"}')
say Lead is done.")

task() { view | jq -r --arg n "$1" '.rows[] | .Task? | select(.name? == $n) | .id'; }
until [ -n "$(task f)" ]; do sleep 0.5; done
echo "== f started; the chat role changes under it"
echo "A line that makes every chat's root stale." >>"$at/roles/chat.md"
drive settle "$lead" >/dev/null
echo "== f's log"
drive log "$(task f)"
echo "== the brain on f's compaction"
grep -E "refused its own batch|failing" "$at/brain.log" || echo "(nothing refused)"
echo
echo "Open https://mx-hv-mj.local:$port/#$lead; end it with: node dev/drive.mjs down --brain fresh"
