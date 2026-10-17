#!/usr/bin/env bash
# A brain survives a reboot and a crash.
#
# `dev/serve` serves a throwaway brain (unit harness-reboot-demo, port
# 4471, the scripted model) as a durable user unit. The demo shows the unit
# file and that it is enabled for default.target, then does what a boot
# does: stops it, reloads systemd and starts it from its file alone. Then
# it kills the brain with SIGKILL and systemd starts it again. Each time
# the face answers with the same commit, from the build NAME/build links
# to. At the end the unit is retired: disabled, its file and its data
# deleted.
set -euo pipefail
repo=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
name=${NAME:-reboot-demo} port=${PORT:-4471}
unit=harness-$name.service
file=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/$unit
step() { printf '\n== %s\n' "$*"; }
serves() {
    for _ in $(seq 60); do
        source=$(curl -sk --max-time 2 "https://localhost:$port/view" | jq -r .rows.harness.Harness.source 2>/dev/null) || true
        [[ -n $source && $source != null ]] && break
        sleep 0.5
    done
    pid=$(systemctl --user show -p MainPID --value "$unit")
    echo "process $pid runs $(readlink "/proc/$pid/exe"), and the face says it is $source"
}
retire() {
    systemctl --user disable --now --quiet "$unit" 2>/dev/null || true
    rm -f "$file"
    systemctl --user daemon-reload
    rm -rf "${XDG_DATA_HOME:-$HOME/.local/share}/agent-harness/$name"
}
retire
trap retire EXIT

step "serve it: dev/serve $name $port --model scripted"
"$repo/dev/serve" "$name" "$port" --model scripted
cat "$file"
echo "is-enabled: $(systemctl --user is-enabled "$unit"); wanted by: $(systemctl --user show -p WantedBy --value "$unit")"

step "a boot: stop it, reload systemd, start it from its file"
systemctl --user stop "$unit"
echo "is-active: $(systemctl --user is-active "$unit" || true)"
systemctl --user daemon-reload
systemctl --user start "$unit"
serves

step "a crash: SIGKILL the brain"
kill -9 "$pid"
sleep 7
echo "restarts: $(systemctl --user show -p NRestarts --value "$unit")"
serves

step "end: retire the unit"
