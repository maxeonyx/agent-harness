#!/bin/sh
# Kill a brain mid-commit, reopen it from its SQLite file. See README.md.
set -e
cd "$(dirname "$0")/../.."
exec cargo run -q --release -p harness-store --example crash -- target/store-demo
