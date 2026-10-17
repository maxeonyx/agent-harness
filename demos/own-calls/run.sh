#!/bin/sh
# An agent is not refused for its own calls; when it is refused for someone
# else's change, the refusal shows the file as it is now, so its retry goes
# through. Runs on the scripted model: no model call, no cost. Prints the
# chat's transcript, each tool call with its whole result.
set -eu
cd "$(dirname "$0")/../.."
port=${PORT:-4391}
dir=$PWD/target/own-calls
rm -rf "$dir"
mkdir -p "$dir"
printf 'one\ntwo\n' > "$dir/f.txt"
echo theirs > "$dir/other.txt"
echo draft > "$dir/shared.txt"
node dev/drive.mjs up "$port"
chat=$(node dev/drive.mjs chat "$port" engineer "$dir" "$(cat demos/own-calls/mandate.md)" | awk '{print $NF}')
# Another agent in the same directory, while A's second response streams.
other=$(node dev/drive.mjs chat "$port" engineer "$dir" "$(cat demos/own-calls/other.md)" | awk '{print $NF}')
node dev/drive.mjs settle "$port" "$other"
node dev/drive.mjs settle "$port" "$chat"
node dev/drive.mjs log "$port" "$other" --full
node dev/drive.mjs log "$port" "$chat" --full
node dev/drive.mjs down "$port" --wipe
