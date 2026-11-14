#!/bin/sh
# A tab open across a relaunch onto a new build reloads onto it, once that
# loses nothing. On a fresh test brain (scripted, free): one browser opens a
# chat's card and types half a thought into its box, leaving it focused.
# Meanwhile sheet.js changes on disk and the brain restarts, as a deploy
# relaunches it onto a new build. The tab relinks and learns the face serves
# another build (`/wiring`), but stays on its own while Max is typing; once
# the box loses focus it reloads, onto the new build, with the card still
# open and the half thought still in the box. sheet.js is put back as it was.
# Fails if the tab is still on its old build at the end.
set -eu
cd "$(dirname "$0")/../.."
node dev/drive.mjs down --brain build-reload --wipe
node dev/drive.mjs up --brain build-reload
unit=$(systemctl --user list-units --plain --no-legend 'harness-drive-build-reload-*' | awk '{print $1}')

page=crates/face-web/src/page/sheet.js
cp "$page" .drive.ignore/build-reload/sheet.js
trap 'cp .drive.ignore/build-reload/sheet.js "$page"' EXIT
(sleep 25 && echo '// a new build' >>"$page" && systemctl --user restart "$unit" && echo "== sheet.js changed on disk and the brain restarted") &

state='JSON.stringify({ build: document.querySelector("script[type=module]").src.split("/")[4], card: location.hash, box: document.querySelector(".compose textarea")?.value ?? null, focused: document.activeElement?.tagName })'
node dev/drive.mjs page --brain build-reload \
  chat orchestrator "$PWD/.drive.ignore/build-reload" "Say hello" reply \
  eval "sessionStorage.setItem('firstBuild', document.querySelector('script[type=module]').src.split('/')[4]); $state" \
  type ".compose textarea" "half a thought" \
  sleep 30000 eval "$state" \
  eval "document.activeElement.blur(), 'the box loses focus'" \
  sleep 5000 eval "$state" \
  shot .drive.ignore/build-reload/after.png \
  eval "(() => { const was = sessionStorage.getItem('firstBuild'), now = document.querySelector('script[type=module]').src.split('/')[4]; if (was === now) throw new Error('still on build ' + now); return 'reloaded from ' + was + ' onto ' + now; })()" \
  2>&1 | grep --line-buffered -v '^  warning'
