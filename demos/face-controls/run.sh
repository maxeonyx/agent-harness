#!/usr/bin/env bash
# A real brain run (Sonnet at low effort; MODEL=haiku/low for another), driven
# only by records said at one of Max's tabs, the way the page drives it: hold, interrupt, the queue, focus,
# words, stop, refusals in band, and live text. Watch it at
# https://mx-hv-mj.local:4400 (another port: PORT=...).
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build -q --release -p harness-brain -p harness-limb-workspace --bins --example face_controls
# Everything the demo starts carries this mark; its limbs outlive their
# brains, so what is left is ended when it finishes.
mark=HARNESS_OWNER_face-controls-demo-$$
trap 'demos/end.sh "$mark"' EXIT
env "$mark=1" target/release/examples/face_controls
