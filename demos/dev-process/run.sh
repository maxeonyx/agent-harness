#!/usr/bin/env bash
# The dev process on the harness: an orchestrator started from the role
# files in roles/, on a small real mandate in a scratch repository, spawns a
# designer and an engineer, each in its own clone. It runs on the test
# brain `dev-process` (dev/drive.mjs), whose link it prints; every role on
# Sonnet at low effort unless MODEL names another (MODEL=scripted only shows
# the page). A new run starts from scratch (FRESH=1, or no database yet);
# otherwise the brain carries on its database. It shows the orchestrator's
# chat once it waits on you, then serves, printing the brain's log, until
# Ctrl-C, which also ends the workspaces it started.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain dev-process "${@:2}"; }
at=.drive.ignore/dev-process
dir=target/dev-process
trap 'drive down' EXIT
trap 'exit 130' INT TERM
fresh=
if [ -n "${FRESH:-}" ] || [ ! -e "$at/brain.sqlite" ]; then
  fresh=1
  drive down --wipe
  rm -rf "$dir"
  mkdir -p "$dir"
  demos/dev-process/scratch.sh "$dir"
fi
drive up "${MODEL:-sonnet/low}"
if [ -n "$fresh" ]; then
  # Max starts the orchestrator in his clone at a tab.
  chat=$(drive chat orchestrator "$dir/textstats" "$(cat demos/dev-process/mandate.md)")
  drive settle "$chat" --timeout 1200
  drive log "$chat"
fi
echo
echo "== serving until Ctrl-C; the brain's log:"
tail -n 0 -f "$at/brain.log"
