#!/usr/bin/env bash
# A relaunch hands the face over from the running build to the next with
# no gap: a request every 200 ms through the whole relaunch, none failing.
#
# A throwaway long-lived brain (`dev/serve`, unit harness-handover-demo,
# port 4393, the scripted model) runs this clone's HEAD, from a scratch
# clone of it, and is relaunched onto an empty commit on top. The old
# brain drains, starts the new one as its child on its own listener, and
# serves read-only until the new one has opened; the new one then becomes
# the unit's main process. Meanwhile curl asks for the page every 200 ms;
# at the end each request's status is tallied, and the brain's own words
# for the handover are printed. The rehearsal and the candidate's gate
# each have the provider count tokens and stream one short response: a few
# real requests. About 6 minutes, most of it the 3-minute watch.
set -euo pipefail
repo=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
name=${NAME:-handover-demo} port=${PORT:-4393}
unit=harness-$name.service
home=${XDG_DATA_HOME:-$HOME/.local/share}/agent-harness
work=$repo/.demo-handover.ignore
step() { printf '\n== %s\n' "$*"; }

systemctl --user -q is-active "$unit" && systemctl --user stop "$unit"
rm -rf "$work" "$home/$name"
mkdir -p "$work"
git clone -q "$repo" "$work/clone"
git -C "$work/clone" checkout -q --detach "$(git -C "$repo" rev-parse HEAD)"
# A target directory of its own, kept between runs so dependencies stay built.
export CARGO_TARGET_DIR=$repo/.demo-handover-target.ignore
git -C "$work/clone" config user.name demo
git -C "$work/clone" config user.email demo@localhost

base=$(git -C "$work/clone" rev-parse --short=12 HEAD)
[[ -d $home/builds/$base ]] && installed="" || installed=$base
step "serve $base"
"$work/clone/dev/serve" "$name" "$port" --model scripted
git -C "$work/clone" commit -q --allow-empty -m "demo: the next build"
next=$(git -C "$work/clone" rev-parse --short=12 HEAD)

step "relaunch onto $next, asking for the page every 200 ms"
since=$(date '+%Y-%m-%d %H:%M:%S')
while sleep 0.2; do
    printf '%s %s\n' "$(date +%T.%N | cut -c1-12)" \
        "$(curl -sk -o /dev/null -m 5 -w '%{http_code} %{time_total}s' "https://localhost:$port/" || true)"
done > "$work/requests.log" &
asking=$!
"$work/clone/dev/serve" "$name" "$port"
kill "$asking"

step "the requests"
echo "$(wc -l < "$work/requests.log") requests; by status:"
awk '{print $2}' "$work/requests.log" | sort | uniq -c
echo "the slowest:"
sort -k3 -t' ' -g "$work/requests.log" | tail -3

step "the brain's words"
journalctl --user -u "$unit" --since "$since" --no-pager -o short-precise |
    grep -E 'SIGUSR2|shut down|handed over|gate: [a-z]+: passed|face:|brain launched|Supervising' || true
systemctl --user show "$unit" -p MainPID -p NRestarts

step "end: retire the brain's unit and delete the demo's builds"
systemctl --user disable --now --quiet "$unit"
rm "${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/$unit"
systemctl --user daemon-reload
for build in $installed "$next"; do
    [[ -d $home/builds/$build ]] && chmod -R u+w "$home/builds/$build" && rm -rf "$home/builds/$build"
done
