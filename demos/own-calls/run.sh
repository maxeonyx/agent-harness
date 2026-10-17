#!/bin/sh
# An agent is not refused for its own calls; when it is refused for someone
# else's change, the refusal shows the file as it is now, so its retry goes
# through. Runs on the scripted model: no model call, no cost. Prints the
# chat's transcript, each tool call with its whole result.
set -eu
cd "$(dirname "$0")/../.."
dir=$PWD/target/own-calls
rm -rf "$dir"
mkdir -p "$dir"
printf 'one\ntwo\n' > "$dir/f.txt"
echo theirs > "$dir/other.txt"
echo draft > "$dir/shared.txt"
node dev/drive.mjs up --brain own-calls
chat=$(node dev/drive.mjs chat --brain own-calls engineer "$dir" "$(cat demos/own-calls/mandate.md)")
# Another agent in the same directory, while A's second response streams.
other=$(node dev/drive.mjs chat --brain own-calls engineer "$dir" "$(cat demos/own-calls/other.md)")
for c in "$other" "$chat"; do node dev/drive.mjs settle --brain own-calls "$c"; done
for c in "$other" "$chat"; do node dev/drive.mjs log --brain own-calls "$c" --full; done
node dev/drive.mjs down --brain own-calls --wipe
