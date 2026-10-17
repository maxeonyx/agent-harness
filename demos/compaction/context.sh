#!/usr/bin/env bash
# The root task's context as the face reads it: one line per entry, its
# role, who wrote its blocks, and its text, cut. Usage: context.sh PORT
set -euo pipefail
port=$1
c() { curl -sS --resolve "mx-hv-mj.local:$port:127.0.0.1" "https://mx-hv-mj.local:$port$1"; }
head=$(c /view | jq -r '.rows[] | .Task // empty | select(.owner == null) | .head')
c "/context/$head" | jq -r '.[] |
  if .role == "root" then "root      \(.model)"
  else
    (.content | fromjson? // []) as $blocks |
    ((.by // []) | map(.by // "-") | unique | join(",")) as $by |
    ($blocks | map(if .type == "text" then .text elif .type == "tool_use" then "[\(.name)]" elif .type == "tool_result" then "[result]" else "[\(.type)]" end) | join(" | ")) as $text |
    "\(.role | .[0:9] | . + (" " * (10 - length)))\(if $by == "" then "" else "(\($by)) " end)\($text | gsub("\n"; " ") | .[0:150])"
  end'
