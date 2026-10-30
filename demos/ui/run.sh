#!/bin/sh
# The UI after Max's feedback of 29 Oct, on a fresh test brain on the
# scripted model, so nothing is paid for: a tool block folded to its calls'
# summaries, two parts staged and sent as one message, a compaction, a busy
# map, and several chats waiting on Max. Each step is a screenshot; the
# brain stays running for you: open the link it prints, and see README.md.
set -eu
cd "$(dirname "$0")/../.."
node dev/drive.mjs down --brain ui --wipe
node dev/drive.mjs up --brain ui
repo=$PWD/.drive.ignore/ui-work/repo
demos/brain/scratch.sh "$repo" >/dev/null

chat() { node dev/drive.mjs chat --brain ui "$1" "$repo" "$2"; }
tools=$(chat chat "$(cat demos/ui/tools.md)")
compacted=$(chat chat "$(cat demos/ui/compaction.md)")
busy=$(chat orchestrator "$(cat demos/ui/busy.md)")
names=$(chat chat "A chat asking Max to choose a name.
say Which name do you prefer for the package: textstats or wordy?")
lunch=$(chat chat "A chat asking Max whether to go on.
say Shall I go on after lunch, or wait for you?")
for c in $tools $compacted $busy $names $lunch; do node dev/drive.mjs settle --brain ui "$c" >/dev/null; done
echo "chats, each waiting on Max: tools $tools, compacted $compacted, busy $busy, names $names, lunch $lunch"

shots=demos/ui
block='#panel .fold:has(.row[data-summary])'
bottom='document.querySelectorAll("#panel *").forEach((e) => { if (e.scrollHeight > e.clientHeight + 5) e.scrollTop = 1e9; })'
node dev/drive.mjs page --brain ui \
  open "$tools" sleep 1500 shot $shots/folded.ignore.png \
  click "$block > .head" sleep 300 shot $shots/calls.ignore.png \
  click "$block .row .line" sleep 300 shot $shots/call.ignore.png \
  type ".compose textarea" "The first part: start with wordcount." click ".compose .send" sleep 400 \
  type ".compose textarea" "say Both parts arrived as one message." click ".compose .send" sleep 400 shot $shots/staged.ignore.png \
  click ".compose .send" reply sleep 500 eval "$bottom" sleep 300 shot $shots/sent.ignore.png \
  open "$compacted" sleep 1500 shot $shots/compacted.ignore.png \
  open "$busy" sleep 2500 shot $shots/card.ignore.png \
  open "$names" sleep 2500 shot $shots/edges.ignore.png \
  click "#panel .bar .x" sleep 2000 shot $shots/map.ignore.png
echo "shots: $shots/folded calls call staged sent compacted card edges map (.ignore.png)"
echo "end it with: node dev/drive.mjs down --brain ui"
