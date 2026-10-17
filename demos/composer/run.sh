#!/bin/sh
# The composer, on a fresh test brain on the scripted model, so nothing is
# paid for: a chat with a long draft in its box, the model pill's hint, and
# its model menu, each as a screenshot at desktop size and the effort menu
# on a phone. The brain stays running for you: open the link it prints,
# and see README.md.
set -eu
cd "$(dirname "$0")/../.."
long=$(i=0; while [ $i -lt 30 ]; do printf 'Line %s of a draft long enough that the box scrolls. ' $i; i=$((i + 1)); done)
node dev/drive.mjs down --brain composer --wipe
node dev/drive.mjs up --brain composer
mkdir -p .drive.ignore/composer-work
out=$(node dev/drive.mjs page --brain composer chat chat .drive.ignore/composer-work "say Hello." reply sleep 500 \
  type ".compose textarea" "$long" shot demos/composer/draft.ignore.png \
  hover ".tune .pick" sleep 700 shot demos/composer/hint.ignore.png \
  click ".tune .pick" sleep 200 shot demos/composer/menu.ignore.png)
echo "$out"
chat=$(echo "$out" | sed -n 's/^chat //p')
node dev/drive.mjs page --brain composer --size phone open "$chat" sleep 500 \
  click ".tune .pick @2" sleep 200 shot demos/composer/phone.ignore.png
echo "open its link above (shots: demos/composer/draft.ignore.png, hint.ignore.png, menu.ignore.png, phone.ignore.png)"
echo "end it with: node dev/drive.mjs down --brain composer"
