#!/bin/sh
# Retry, edit, going back and a reply's versions, on a fresh test brain
# whose every role runs on the scripted model, at no cost (MODEL=sonnet
# for a real one, whose retries differ). It starts a chat and leaves the
# brain running for you: open the link it prints, and see README.md.
set -eu
cd "$(dirname "$0")/../.."
port=${PORT:-4362}
node dev/drive.mjs down "$port" --wipe
node dev/drive.mjs up "$port" "${MODEL:-scripted}"
chat=$(node dev/drive.mjs chat "$port" chat - "Write a new four-word poem.
say Quiet rain, waking earth." | cut -d' ' -f2)
node dev/drive.mjs settle "$port" "$chat" >/dev/null
node dev/drive.mjs log "$port" "$chat"
echo "open https://mx-hv-mj.local:$port/#$chat"
echo "end it with: node dev/drive.mjs down $port"
