#!/usr/bin/env bash
# What a chat shows when the provider fails it. A request the provider
# rejects as invalid (a 400) fails the chat at once, in the provider's
# words; one that fails for a reason a retry may cure (a 529 overload) is
# retried after 2, 4, 8 and 16 s, then fails, naming the last error.
# Watch each chat's red "Failed: ..." card on the page. Scripted: no
# model call.
set -euo pipefail
cd "$(dirname "$0")/../.."
brain=provider-errors
node dev/drive.mjs down --brain $brain --wipe
node dev/drive.mjs up --brain $brain
dir=$PWD/.drive.ignore/$brain/dir
mkdir -p "$dir"
rejected=$(node dev/drive.mjs chat --brain $brain chat "$dir" \
  'error 400 messages.5: `web_fetch` tool use with id `srvtoolu_x` was found without a corresponding `web_fetch_tool_result` block')
overloaded=$(node dev/drive.mjs chat --brain $brain chat "$dir" 'error 529 Overloaded')
node dev/drive.mjs page --brain $brain \
  open "$rejected" text "rejected the request" shot ".drive.ignore/$brain/rejected.png" \
  open "$overloaded" text "5 times in a row" --timeout 90 shot ".drive.ignore/$brain/overloaded.png"
grep -E "request failed|failing" ".drive.ignore/$brain/brain.log"
echo
echo "end it with: node dev/drive.mjs down --brain $brain"
