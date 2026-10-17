#!/usr/bin/env bash
# A chat in a workspace directory (one holding a clone, the way Max's
# `workspaces` skill lays them out) starts an engineer, whose own clone is
# made beside that one, on its branch. It runs on the test brain
# `chat-engineer` (dev/drive.mjs), Sonnet at low effort unless MODEL names
# another, prints the chat and the engineer's commit, and leaves the brain
# serving: `node dev/drive.mjs down --brain chat-engineer` ends it.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain chat-engineer "${@:2}"; }
root=$PWD/target/chat-engineer
rm -rf "$root"
mkdir -p "$root/origin" "$root/ws"
git -C "$root/origin" init -q -b main
echo hi >"$root/origin/README"
git -C "$root/origin" add README
git -C "$root/origin" -c user.email=demo@local -c user.name=demo commit -qm init
git clone -q "file://$root/origin" "$root/ws/demo"
drive down --wipe
drive up "${MODEL:-sonnet/low}"
chat=$(drive chat chat "$root/ws" "In this workspace there is a clone \`demo\`. Start an engineer on branch \`hello\` from it, with the mandate: add a file hello.txt saying hello, commit it on its branch (no PR, no push, there is no AGENTS.md), and report the commit hash. Then tell me its result.")
drive settle "$chat" --timeout 600
drive log "$chat"
echo
echo "== the workspace: $(ls "$root/ws" | tr '\n' ' ')"
git -C "$root/ws/origin-hello" log --oneline -1 hello
echo "end it with: node dev/drive.mjs down --brain chat-engineer"
