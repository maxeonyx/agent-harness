#!/bin/sh
# The face as Max's phone reaches it: through a TLS-terminating proxy that
# stands in for Cloudflare Access (access.mjs), on a phone-sized Chromium.
# It checks Chrome would offer to install it as an app, then expires the
# session and watches the page leave for the login. Scripted model, free.
set -eu
cd "$(dirname "$0")/../.."
dir=.drive.ignore/deploy-demo
mkdir -p "$dir"
rm -f "$dir/expired"
node dev/drive.mjs down --brain deploy-demo --wipe
node dev/drive.mjs up --brain deploy-demo
node demos/deploy/access.mjs "$(cat .drive.ignore/deploy-demo/port)" "$dir/expired" > "$dir/access.log" &
access=$!
trap 'kill $access' EXIT
sleep 1
[ -s "$dir/access.log" ] || { echo "the stand-in for Access did not start" >&2; exit 1; }
url=https://mx-hv-mj.local:$(head -1 "$dir/access.log")/
echo "the stand-in for Access serves the brain at $url"
(sleep 15; echo "the session expires"; touch "$dir/expired") &
node dev/drive.mjs page --brain deploy-demo --size phone \
  eval "setTimeout(() => location.href = '$url', 0)" sleep 3000 wait "#new" \
  eval location.href \
  cdp Page.getInstallabilityErrors '{}' \
  shot demos/deploy/phone.ignore.png \
  sleep 15000 eval location.href
echo "what reached the brain, and what the stand-in turned away:"
tail -n +2 "$dir/access.log" | grep -v '^fwd GET /page/'
node dev/drive.mjs down --brain deploy-demo
