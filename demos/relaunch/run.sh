#!/usr/bin/env bash
# The harness relaunches itself, and a broken build bricks nothing.
#
# A throwaway long-lived brain (`dev/serve`, unit harness-relaunch-demo,
# port 4392, the scripted model) runs this clone's HEAD, from a scratch
# clone of it. A session calls the builds limb's `relaunch` onto a commit
# on top: the brain drains, the new build takes over, the session carries
# on into a tool round trip, and the face's view says it is proven. Then
# onto a commit whose brain exits 20 s after it starts: the old build's
# builds limb sees it die, starts the old bytes again, and the session
# is woken to read why.
set -euo pipefail
repo=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
name=${NAME:-relaunch-demo} port=${PORT:-4392}
unit=harness-$name.service
home=${XDG_DATA_HOME:-$HOME/.local/share}/agent-harness
work=$repo/.demo-relaunch.ignore
step() { printf '\n== %s\n' "$*"; }
view() { curl -sk "https://localhost:$port/view" | jq -c '.rows.harness.Harness | {source, probation}'; }

systemctl --user -q is-active "$unit" && systemctl --user stop "$unit"
rm -rf "$work" "$home/$name"
mkdir -p "$work/ws"
git clone -q "$repo" "$work/clone"
git -C "$work/clone" checkout -q --detach "$(git -C "$repo" rev-parse HEAD)"
# Share this clone's target directory, so nothing builds from scratch.
mkdir "$work/clone/.cargo"
printf '[build]\ntarget-dir = "%s/target"\n' "$repo" > "$work/clone/.cargo/config.toml"
# A role that holds the builds limb: no shipped role does.
cp -r "$repo/roles" "$work/roles"
sed -i 's/^limbs: \[meta, user, web\]/limbs: [meta, user, web, builds]/' "$work/roles/engineer.md"
git -C "$work/clone" config user.name demo
git -C "$work/clone" config user.email demo@localhost

step "serve $(git -C "$work/clone" rev-parse --short=12 HEAD)"
"$work/clone/dev/serve" "$name" "$port" --model scripted --roles "$work/roles"
act=$home/$name/build/bin/act
view

step "relaunch onto a good commit"
git -C "$work/clone" commit -q --allow-empty -m "demo: a good build"
good=$(git -C "$work/clone" rev-parse --short=12 HEAD)
"$act" "$port" chat engineer "$work/ws" "tool relaunch {\"clone\":\"$work/clone\"}
tool bash {\"command\":\"echo a round trip on the new build\"}
say done" | tail -1
# Proven, the builds limb moves NAME/build, and its watch ends.
for _ in $(seq 300); do
    [[ $(readlink "$home/$name/build") == */$good ]] && break
    sleep 1
done
sleep 1
view
echo "NAME/build -> $(readlink "$home/$name/build")"

step "relaunch onto a brain that exits 20 s after it starts"
sed -i 's/^fn main() -> ExitCode {$/fn main() -> ExitCode {\n    if !std::env::args().any(|a| a == "--version") {\n        std::thread::spawn(|| {\n            std::thread::sleep(std::time::Duration::from_secs(20));\n            std::process::exit(3);\n        });\n    }/' "$work/clone/crates/brain/src/main.rs"
git -C "$work/clone" commit -q -am "demo: a brain that exits 20 s after it starts"
bad=$(git -C "$work/clone" rev-parse --short=12 HEAD)
chat=$("$act" "$port" chat engineer "$work/ws" "tool relaunch {\"clone\":\"$work/clone\"}
slow 30000 say hung
say after going back" | tail -1)
chat=${chat#chat }
for _ in $(seq 300); do
    journalctl --user -u "$unit" --no-pager -o cat | grep -q "service $unit changed" && break
    sleep 1
done
view
echo "NAME/build -> $(readlink "$home/$name/build")"
step "the session wakes, unasked, and reads why"
for _ in $(seq 240); do
    said=$(sqlite3 "$home/$name/brain.sqlite" "select cast(bytes as text) from value" |
        grep -a '(scripted) Noted' | grep -a -o "\[harness\] $unit: [^\"]*" | tail -1) || true
    [[ -n $said ]] && break
    sleep 1
done
echo "${said%%\\n*}"

step "end: stop the brain and delete the demo's builds"
systemctl --user stop "$unit"
for build in "$good" "$bad"; do
    [[ -d $home/builds/$build ]] && chmod -R u+w "$home/builds/$build" && rm -rf "$home/builds/$build"
done
echo "its limbs end themselves 15 minutes after their brain"
