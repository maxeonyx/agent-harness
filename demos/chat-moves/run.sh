#!/bin/sh
# Retry, edit, going back and a reply's versions, on a fresh test brain
# whose roles run on Sonnet at low effort so each try is cheap. It starts a
# chat, asks for its reply again (a second version, unless the model says
# the same words again), and leaves the brain running for you:
# open the link it prints, and see README.md.
set -eu
cd "$(dirname "$0")/../.."
port=${PORT:-4362}
node dev/drive.mjs down "$port" --wipe
node dev/drive.mjs up "$port"
node dev/drive.mjs page "$port" \
  chat chat - "Write a new four-word poem." reply \
  click '.log button[title="Ask for this response again"]' \
  wait "#panel .mini.working" reply shot demos/chat-moves/retried.ignore.png
echo "open https://mx-hv-mj.local:$port/ (a retried reply: demos/chat-moves/retried.ignore.png)"
echo "end it with: node dev/drive.mjs down $port"
