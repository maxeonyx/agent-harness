#!/bin/sh
# Retry, edit, going back and a reply's versions, on a fresh brain whose
# chat runs on Sonnet at low effort so each try is cheap. Open
# https://mx-hv-mj.local:${PORT:-4362}/, start a chat, and see README.md.
set -eu
cd "$(dirname "$0")/../.."
cargo build -q --release -p harness-brain -p harness-limb-workspace
dir=$(mktemp -d "${TMPDIR:-/var/tmp}/chat-moves.XXXXXX")
cp -r roles "$dir/roles"
sed -i 's/^model: .*/model: sonnet/; s/^effort: .*/effort: low/' "$dir/roles/chat.md"
echo "brain at https://mx-hv-mj.local:${PORT:-4362}/ (database in $dir)"
exec target/release/harness-brain --roles "$dir/roles" --db "$dir/brain.sqlite" --port "${PORT:-4362}"
