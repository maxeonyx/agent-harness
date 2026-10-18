#!/bin/sh
# Retry, edit, going back and a reply's versions, on a fresh test brain on
# the scripted model, so nothing is paid for. One chat's reply varies
# (`vary`): its retry is a second version, and the demo goes back to the
# first. Another's is the same words each time (`say`): its retry shows
# "said 2×". The brain stays running for you: open the link it prints,
# and see README.md.
set -eu
cd "$(dirname "$0")/../.."
retry='.log button[title="Ask for this response again"]'
node dev/drive.mjs down --brain chat-moves --wipe
node dev/drive.mjs up --brain chat-moves
dir=$PWD/.drive.ignore/chat-moves/dir
mkdir -p "$dir"
node dev/drive.mjs page --brain chat-moves \
  chat chat "$dir" "vary A four-word poem." reply \
  click "$retry" text "(try 2)" wait ".versions .n" shot demos/chat-moves/versions.ignore.png \
  click '.versions button[title="The version before"]' text "(try 1)" \
  chat chat "$dir" "say 612" reply \
  click "$retry" wait ".versions .tries" shot demos/chat-moves/same-words.ignore.png
echo "open its link above (shots: demos/chat-moves/versions.ignore.png, same-words.ignore.png)"
echo "end it with: node dev/drive.mjs down --brain chat-moves"
