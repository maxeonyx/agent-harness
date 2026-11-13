#!/usr/bin/env bash
# Every tool takes its call's `summary` without knowing about it. The
# harness adds `summary` to every tool's schema; whatever runs a tool (a
# limb, or the brain for its own) takes it off first, so even a parser
# that refuses unknown fields, such as `relaunch`'s, never sees it.
#
# A scripted orchestrator calls every tool its root offers (each with an
# input schema) in one response, each with only a `summary`: most refuse
# for a missing argument, and none may name `summary`. Then it calls
# `relaunch` with a target no branch has and a summary: it gets past its
# input and is refused at once, building nothing. Scripted: no model call.
set -euo pipefail
cd "$(dirname "$0")/../.."
b=summary
node dev/drive.mjs down --brain $b --wipe
node dev/drive.mjs up --brain $b
dir=$PWD/.drive.ignore/$b/dir
mkdir -p "$dir"
port=$(cat .drive.ignore/$b/port)
tools() {
  curl -sk "https://127.0.0.1:$port/context/$1" | jq -r '[.. | objects | select(.role? == "root")][0]
    | .tools | fromjson | map(select(.input_schema)) | .[].name'
}
probe=$(node dev/drive.mjs chat --brain $b orchestrator "$dir" "say Listed.")
node dev/drive.mjs settle --brain $b "$probe" >/dev/null
mapfile -t offered < <(tools "$probe")
echo "== $probe's root offers ${#offered[@]} tools with an input schema: ${offered[*]}"
mandate=$(for t in "${offered[@]}"; do echo "tool $t {\"summary\":\"probe $t\"}"; done
  echo 'tool relaunch {"target":"0000000000000000000000000000000000000000","summary":"relaunch onto no commit"}'
  echo "say Probed.")
chat=$(node dev/drive.mjs chat --brain $b orchestrator "$dir" "$mandate")
node dev/drive.mjs settle --brain $b "$chat" >/dev/null
log=$(node dev/drive.mjs log --brain $b "$chat" --full)
echo "$log"
echo
echo "== results that name \`summary\` (there should be none)"
grep -n 'summary`' <<<"$log" || echo "none"
echo
echo "end it with: node dev/drive.mjs down --brain $b"
