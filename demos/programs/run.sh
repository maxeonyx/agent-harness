#!/usr/bin/env bash
# One brain, many builds. A session relaunches the harness onto new builds
# and carries on with the provider's cache still hit; a build that crashes
# on open changes nothing; a build whose requests are refused is rolled
# back, and the records it wrote are held by the build before it; a stale
# relaunch is refused; a killed brain is restarted on the proven build.
#
# Real requests on the Claude subscription: five small ones.
set -euo pipefail
cd "$(dirname "$0")/../.."
domain=$PWD/demos/programs/domain.ignore
port=${PORT:-4390}
rm -rf "$domain"
mkdir -p "$domain"

step() { printf '\n\033[1m== %s\033[0m\n' "$*"; }
build() {
  cargo build -q --release -p harness-relaunch-spike --features "$1"
  target/release/spike install "$domain"
}

step "Five builds, each installed under its digest"
A=$(build "")
B=$(build render-v2)
C=$(build crash-on-open)
D=$(build "new-fact bad-requests")
E=$(build "render-v2 new-fact")
echo "A ${A:0:8}  the base"
echo "B ${B:0:8}  a fix to how notices are rendered"
echo "C ${C:0:8}  panics once it has opened the database"
echo "D ${D:0:8}  writes a fact older builds have no word for; the provider refuses its requests"
echo "E ${E:0:8}  the fix, and a word for D's fact"
names="s/${A:0:8}/A:${A:0:8}/g; s/${B:0:8}/B:${B:0:8}/g; s/${C:0:8}/C:${C:0:8}/g; s/${D:0:8}/D:${D:0:8}/g; s/${E:0:8}/E:${E:0:8}/g"
spike() { target/release/spike "$@" | sed "$names"; }

step "The supervisor takes port $port and starts A"
target/release/spike supervise "$domain" "$port" "$A" 2> >(sed -u "$names" >&2) &
supervisor=$!
trap 'kill $supervisor 2>/dev/null' EXIT

step "Session s1 asks its first question: the cache is written"
spike call "$port" open s1
spike call "$port" ask s1 "One short line: which brig carried the most tons?"

step "s1 relaunches the harness onto B: B opens the database while A serves, A hands over, and s1 carries on with the cache read"
spike call "$port" relaunch s1 "$B"
spike await "$port"

step "s1 relaunches onto C, which crashes once it has opened the database: B never stopped"
spike call "$port" relaunch s1 "$C"
spike await "$port"

step "s1 relaunches onto D: D takes over and writes its new fact, the provider refuses its request, and the supervisor goes back to B, which holds D's fact unread"
spike call "$port" relaunch s1 "$D"
spike await "$port"

step "s1 carries on, on B, and is told what happened"
spike call "$port" ask s1 "One short line: what did the harness tell you?"

step "A relaunch built on A, while B runs, is refused as stale"
spike call "$port" relaunch s1 "$E" "$A"
spike await "$port"

step "s1 relaunches onto E, which reads D's fact"
spike call "$port" relaunch s1 "$E"
spike await "$port"

step "The brain is killed: the supervisor starts the proven build, and s1 carries on"
pid=$(target/release/spike call "$port" status | head -1 | sed 's/.*pid=//')
kill -9 "$pid"
spike await "$port"
spike call "$port" ask s1 "One short line: how many builds has the harness run in this conversation?"
