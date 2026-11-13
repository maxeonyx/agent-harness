#!/bin/sh
# The five themes an area wears, held to the design's bars by their check:
# every identity slot's ground painted behind a page lead at phone and
# desktop size, timed, measured and painted twice, and each theme's map
# region with live agents' state marks. No brain and no model: it serves
# the face's theme files itself. Watch the table it prints (a bar missed
# says MISSED, and the run fails), then open the pictures it names: each
# theme's slots.png (its agents side by side, as thumbnails), its covers
# and its region.png.
set -eu
cd "$(dirname "$0")/../.."
node dev/theme-check.mjs --out demos/themes/shots.ignore "$@"
