#!/bin/sh
# Max's day from his phone, on a fresh test brain on the scripted model, so
# nothing is paid for: a first chat, then a second started with +, in the
# first one's directory, dictated, sent with two taps, the page reloaded,
# and the first chat found again with the find button. Each step is a
# screenshot at phone size. The brain stays running for you: open the link
# it prints, and see README.md.
set -eu
cd "$(dirname "$0")/../.."
node dev/drive.mjs down --brain phone --wipe
node dev/drive.mjs up --brain phone --sample demos/home/sample.wav
mkdir -p .drive.ignore/phone-work
node dev/drive.mjs page --brain phone --size phone chat chat .drive.ignore/phone-work "say Hello from the first chat." reply
node dev/drive.mjs page --brain phone --size phone \
  click "#new" sleep 500 eval 'document.querySelector(".compose .place").value' shot demos/phone/new.ignore.png \
  click ".sample" sleep 13000 shot demos/phone/dictated.ignore.png \
  click ".compose .send" sleep 300 shot demos/phone/staged.ignore.png click ".compose .send" reply sleep 500 \
  eval 'location.reload()' sleep 2500 shot demos/phone/reloaded.ignore.png \
  click "#panel .bar .x" sleep 500 click "#bottom .icon @1" type "#find" "first chat" sleep 1500 shot demos/phone/find.ignore.png \
  click "#found .hit" sleep 1500 shot demos/phone/found.ignore.png
echo "shots: demos/phone/new.ignore.png dictated staged reloaded find found (.ignore.png)"
echo "end it with: node dev/drive.mjs down --brain phone"
