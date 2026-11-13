#!/usr/bin/env bash
# Facing Max is a set an agent holds: `max`. Max's chat holds it, so its
# root offers `open_chat` and tells it how to talk with him. It spawns a
# fresh engineer and a fork, and opens a chat for Max: the engineer
# holds no `max` (no `open_chat`, no facing text), the fork continues
# the chat's root and holds it, and the opened chat holds it. Scripted:
# no model call.
set -euo pipefail
cd "$(dirname "$0")/../.."
b=max-set
node dev/drive.mjs down --brain $b --wipe
node dev/drive.mjs up --brain $b
dir=$PWD/.drive.ignore/$b/dir
mkdir -p "$dir"
port=$(cat .drive.ignore/$b/port)
view() { curl -sk "https://127.0.0.1:$port/view"; }
id() { view | jq -r --arg n "$1" '.rows | to_entries[] | select(.value.Task.name? == $n) | .value.Task.id'; }
# What a task's root holds: its sets (from the tool names), whether it
# offers `open_chat`, and whether its system prompt has the facing text.
root() {
  curl -sk "https://127.0.0.1:$port/context/$1" | jq -c --arg who "$2" '
    [.. | objects | select(.role? == "root")][0]
    | (.tools | fromjson | map(.name)) as $tools
    | { who: $who,
        open_chat: ($tools | index("open_chat") != null),
        facing: (.system | contains("This chat is Max'"'"'s")),
        both_ways: (.system | contains("when you have answered him")) }'
}
chat=$(node dev/drive.mjs chat --brain $b chat "$dir" "$(cat demos/max-set/mandate.md)")
node dev/drive.mjs settle --brain $b "$chat" >/dev/null
for n in builder twin helper; do
  until [ -n "$(id $n)" ]; do sleep 0.2; done
done
node dev/drive.mjs log --brain $b "$chat"
echo
echo "== each task: its mode, and what its root holds"
for n in "$chat" builder twin helper; do
  t=$n; [[ $n == "$chat" ]] || t=$(id $n)
  name=$n; [[ $n == "$chat" ]] && name="Max's chat"
  mode=$(view | jq -r --arg t "$t" '.rows["task:"+$t].Task.mode')
  root "$t" "$name ($mode)"
done
echo
echo "== the facing text in Max's chat's root"
curl -sk "https://127.0.0.1:$port/context/$chat" | jq -r '[.. | objects | select(.role? == "root")][0].system
    | fromjson | map(.text) | join("\n\n") | split("\n\n")[] | select(test("Max.s\\. End|talk to him as you go"))'
echo
echo "end it with: node dev/drive.mjs down --brain $b"
