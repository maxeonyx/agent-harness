#!/bin/sh
# The page's modules live under their build, so a cached tab never runs an
# old one. On a fresh test brain (scripted, free): the headers of the
# document (never stored) and of its modules and wasm (stored forever, under
# /page/<build>/), and a fixed path or another build's path not found. Then
# one browser loads the page, reloads it (every module from its cache), and
# reloads it once more after main.js changes on disk, as a deploy changes
# it: every module comes from the new build's path, and the change runs.
# main.js is put back as it was.
set -eu
cd "$(dirname "$0")/../.."
node dev/drive.mjs down --brain build-paths --wipe
node dev/drive.mjs up --brain build-paths
port=$(cat .drive.ignore/build-paths/port)
face=https://localhost:$port
main=$(curl -sk "$face/" | grep -o '/page/[0-9a-f]*/main.js')
for path in / "$main" "${main%main.js}tab.wasm" /page/sw.js /page/main.js /page/0000000000000000/main.js; do
  echo "== GET $path"
  curl -sk -D- -o /dev/null "$face$path" | grep -iE '^HTTP|^cache-control'
done

page=crates/face-web/src/page/main.js
cp "$page" .drive.ignore/build-paths/main.js
trap 'cp .drive.ignore/build-paths/main.js "$page"' EXIT
(sleep 20 && echo 'globalThis.pageMark = "the new build";' >>"$page" && echo "== main.js changed on disk") &
loaded='JSON.stringify({ mark: globalThis.pageMark ?? null, modules: performance.getEntriesByType("resource").filter((e) => /\/page\/.*\/(main|tab)\.(js|wasm)$/.test(e.name)).map((e) => `${new URL(e.name).pathname} from ${e.transferSize === 0 ? "cache" : "network"}`) })'
node dev/drive.mjs page --brain build-paths \
  sleep 4000 eval "$loaded" \
  cdp Page.reload '{}' sleep 4000 eval "$loaded" \
  sleep 14000 cdp Page.reload '{}' sleep 4000 eval "$loaded" 2>&1 | grep --line-buffered -v '^  warning'
wait
echo "end it with: node dev/drive.mjs down --brain build-paths"
