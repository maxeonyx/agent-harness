#!/usr/bin/env bash
# The brain and two of Max's devices, in memory, on the one data flow.
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo run -q --release -p harness-core --example replicas
