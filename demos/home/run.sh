#!/bin/sh
# The harness Max uses at home: one long-lived brain, `dev/serve home`, a
# user unit on ~/.local/share/agent-harness/home/brain.sqlite, chats
# started from the page. Run it again (after a commit, say) and the new
# build carries everything on. NAME=… PORT=… for another brain.
set -eu
cd "$(dirname "$0")/../.."
# SAMPLE=1 offers "Dictate the sample", for a machine with no microphone.
exec dev/serve "${NAME:-home}" "${PORT:-4320}" ${SAMPLE:+--sample "$PWD/demos/home/sample.wav"}
