#!/usr/bin/env bash
# A real brain run (Sonnet and Opus), driven only by records said at one of
# Max's tabs, the way the page drives it: hold, interrupt, the queue, focus,
# words, stop, refusals in band, and live text. Watch it at
# https://mx-hv-mj.local:4400 (another port: PORT=...).
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build -q --release -p harness-brain -p harness-limb-workspace --bins --example face_controls
exec target/release/examples/face_controls
