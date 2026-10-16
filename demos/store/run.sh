#!/bin/sh
# Kill a writer mid-batch, reopen the store, fold it, query it. See README.md.
set -e
cd "$(dirname "$0")/../.."
exec cargo run -q --release -p harness-store --example crash -- target/store-demo
