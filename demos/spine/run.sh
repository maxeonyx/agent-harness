#!/usr/bin/env bash
# The spine: a brain on its database, a PC tab and a phone tab linked over
# wss, and three brain restarts. Episodes 1, 6, 5 and 2 from the milestone.
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build -q --release -p harness-brain -p harness-limb-workspace --bins --example spine
exec target/release/examples/spine
