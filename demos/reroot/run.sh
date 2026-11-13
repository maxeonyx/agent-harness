#!/usr/bin/env bash
# The harness, not the agent, picks where a compaction starts. A scripted
# `chat` compacts onto its checkpoint; then its role changes, its next
# turn's opening line says its next compaction starts a new context, and
# that compaction does, under the new role. A second chat whose kept
# checkpoints pass half the context limit is re-rooted the same way. Free
# (scripted), on the test brain `reroot`, with its roles in a scratch copy.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain reroot "${@:2}"; }
at=.drive.ignore/reroot
dir=target/reroot-demo
rm -rf "$dir"
mkdir -p "$dir/work"
cp -r roles "$dir/roles"
trap 'drive down' EXIT
drive down --wipe
drive up --roles "$dir/roles"
port=$(cat "$at/port")
context() { curl -sk "https://localhost:$port/context/$1"; }
# The root's prompt, and each user message's harness lines.
root() { context "$1" | jq -r '.[0].system | fromjson | map(.text) | join("\n")' | grep -c 'rerooted' || true; }
said() { context "$1" | jq -r '.[] | select(.role == "user") | .content | fromjson? // [] | .[] | .text? // empty' | grep -o "$2" || true; }

echo "== a chat works a step and compacts: onto its checkpoint"
chat=$(drive chat chat "$dir/work" 'tool bash {"command": "echo step one"}
tool compact_and_continue {"carry": "Step one done.\nsay Step one is done; waiting for you."}')
drive settle "$chat"
grep -E 'compacted' "$at/brain.log" | cut -c1-160
echo "root lines saying 'rerooted': $(root "$chat")"

echo
echo "== its role changes"
printf '\nSign every report "rerooted".\n' >> "$dir/roles/chat.md"
sleep 2
drive say "$chat" 'say Noted.' >/dev/null
drive settle "$chat" >/dev/null
echo "-- what its next turn opened with:"
said "$chat" '\[harness\] [^[]*changed[^[]*' | sort -u
drive say "$chat" 'tool bash {"command": "echo step two"}
tool compact_and_report {"carry": "Step two done.", "report": "Both steps are done, under the role as it is now."}'
drive settle "$chat"
echo "-- the compaction:"
grep -E 'compacted' "$at/brain.log" | tail -1 | cut -c1-160
echo "root lines saying 'rerooted': $(root "$chat")"

echo
echo "== a chat whose checkpoints keep 165k tokens, past half of the 300k limit"
long=$(drive chat chat "$dir/work" 'say ready')
drive settle "$long"
filler=$(python3 -c 'print(("filler " * 15 + "\n") * 1000)')
for i in 1 2 3 4 5 6; do
  drive say "$long" "say noted $i
$filler" >/dev/null
  drive settle "$long" >/dev/null
done
drive say "$long" 'say Noted.' >/dev/null
drive settle "$long" >/dev/null
echo "-- what its next turn opened with:"
said "$long" '\[harness\] [^[]*next compaction starts a new context[^[]*' | sort -u
drive say "$long" 'tool bash {"command": "echo tidy"}
tool compact_and_report {"carry": "Kept what matters.", "report": "Carried on in a new context."}'
drive settle "$long"
echo "-- the compaction:"
grep -E 'compacted' "$at/brain.log" | tail -1 | cut -c1-160
echo "-- its context now:"
context "$long" | jq -r '.[] | "\(.role)\t\(.content // "" | fromjson? // [] | map(.text? // "[\(.type)]") | join(" | ") | gsub("\n"; " ") | .[0:120])"'
