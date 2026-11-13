#!/usr/bin/env bash
# The advocate reads Max's words across chats with the `history` set. Max
# tells a chat what the boat's hull should be; a max-advocate, in a chat
# of its own, is asked what he would say about painting it blue. Watch its
# `find` list his words under their `max-…` label, with when and the chat
# he said them to, then its `context` show that chat as text, and its
# answer quote him by label. Each transcript prints as it ends; the brain
# stays up.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain history "${@:2}"; }
at=.drive.ignore/history
drive down --wipe
drive up "${MODEL:-sonnet/low}"
mkdir -p "$at/boat"

boat=$(drive chat chat "$PWD/$at/boat" "Note this for later: the boat we're building is called Kererū Dawn, and its hull should be ochre, never blue. Just acknowledge in one line.")
drive settle "$boat" --timeout 120
drive log "$boat"

advocate=$(drive chat max-advocate - "What would Max say about painting the boat's hull blue? Look up what he said about the boat in his other chats with your find and context tools, and quote him with his max- label.")
drive settle "$advocate" --timeout 300
drive say "$advocate" "Now open that chat with context, at its task, and tell me what the agent replied."
drive settle "$advocate" --timeout 300
drive log "$advocate"
