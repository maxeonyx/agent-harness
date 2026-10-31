#!/usr/bin/env bash
# The harness relaunches itself through a rehearsal and a gated opening,
# and a broken build bricks nothing.
#
# A throwaway long-lived brain (`dev/serve`, unit harness-relaunch-demo,
# port 4392, the scripted model) runs this clone's HEAD, from a scratch
# clone of it. A session calls the builds limb's `relaunch` onto commits on
# top, and each call's result is printed once it settles:
#   1. a good build: rehearsed, opened as a candidate, watched for 3 min;
#   2. one whose --check fails: refused at rehearsal with its gate: line;
#   3. one whose gate fails only on the live database: it writes nothing,
#      and the build before runs again;
#   4. one whose brain dies 20 s after opening: the harness goes back;
#   5. one the running build cannot read back (it renames a field of
#      Launched): refused as one-way, then opened with `one_way: true`.
# Each rehearsal and each candidate has the provider count tokens and
# stream one short response: a few real requests.
set -euo pipefail
repo=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
name=${NAME:-relaunch-demo} port=${PORT:-4392}
unit=harness-$name.service
home=${XDG_DATA_HOME:-$HOME/.local/share}/agent-harness
db=$home/$name/brain.sqlite
work=$repo/.demo-relaunch.ignore
step() { printf '\n== %s\n' "$*"; }
runs() {
    local exe
    exe=$(readlink -f "/proc/$(systemctl --user show -p MainPID --value "$unit")/exe")
    echo "$unit runs $(basename "$(dirname "$(dirname "$exe")")"); $name/build -> $(basename "$(readlink "$home/$name/build")")"
}

systemctl --user -q is-active "$unit" && systemctl --user stop "$unit"
rm -rf "$work" "$home/$name"
mkdir -p "$work/ws"
git clone -q "$repo" "$work/clone"
git -C "$work/clone" checkout -q --detach "$(git -C "$repo" rev-parse HEAD)"
# A target directory of its own, kept between runs so dependencies stay
# built. Never this clone's: cargo would take one clone's build as fresh
# for the other, and this clone's binaries would be the demo's commits.
mkdir "$work/clone/.cargo"
printf '[build]\ntarget-dir = "%s/.demo-relaunch-target.ignore"\n' "$repo" > "$work/clone/.cargo/config.toml"
git -C "$work/clone" config user.name demo
git -C "$work/clone" config user.email demo@localhost

# Commit on top of $1 the change the sed script $3 makes to the file $2
# (none if it is empty), with the message $4; print the commit.
variant() {
    git -C "$work/clone" checkout -q --detach "$1"
    [[ -z $2 ]] || sed -i "$3" "$work/clone/$2"
    git -C "$work/clone" commit -q --allow-empty -am "$4"
    git -C "$work/clone" rev-parse --short=12 HEAD
}

# A session calls `relaunch` onto commit $1 with the extra input $2, and
# this prints the call's result once it settles: its stage, then its words.
relaunch() {
    "$work/act" "$port" chat orchestrator "$work/ws" "tool relaunch {\"clone\":\"$work/clone\"$2}" >/dev/null
    local said=""
    for _ in $(seq 1800); do
        said=$(sqlite3 "$db" "select cast(bytes as text) from value" 2>/dev/null |
            grep -a -o -E '(refused at rehearsal|failed its gate|opened|went back): ([^"\\]|\\.)*' |
            grep -a -F "$1" | grep -a -v -F "${3:-no such words}" | head -1) || true
        [[ -n $said ]] && break
        sleep 1
    done
    printf '%b\n' "${said:-no result in 30 minutes}"
}

step "serve $(git -C "$work/clone" rev-parse --short=12 HEAD)"
"$work/clone/dev/serve" "$name" "$port" --model scripted
cp "$home/$name/build/bin/act" "$work/act"
base=$(git -C "$work/clone" rev-parse --short=12 HEAD)
runs

step "1. relaunch onto a good build"
good=$(variant "$base" "" "" "demo: a good build")
relaunch "$good" ""
runs

step "2. onto a build whose --check fails: refused at rehearsal"
check=$(variant "$good" crates/brain/src/main.rs \
    's|^    if let Some(copy) = check {$|&\n        return Err("gate: decide: the demo build fails its check".into());|' \
    "demo: a build whose check fails")
relaunch "$check" ""
runs

step "3. onto a build whose gate fails only on the live database"
live=$(variant "$good" crates/brain/src/main.rs \
    's|^        Start::Candidate => {$|&\n            return Err("gate: decide: the demo build fails on the live database".into());|' \
    "demo: a build whose gate fails on the live database")
relaunch "$live" ""
runs
echo "records naming $live in the database: $(sqlite3 "$db" "select count(*) from record where record like '%$live%'")"

step "4. onto a build whose brain dies 20 s after opening: the way back"
dies=$(variant "$good" crates/brain/src/main.rs \
    's|^            install(&db)?;$|&\n            std::thread::spawn(\|\| {\n                std::thread::sleep(Duration::from_secs(20));\n                std::process::exit(3);\n            });|' \
    "demo: a build whose brain dies 20 s after opening")
relaunch "$dies" ""
runs

step "5. onto a one-way build: refused, then opened with one_way"
oneway=$(variant "$good" crates/core/src/build.rs \
    's|^    pub build: BuildId,$|    #[serde(rename = "build_id", alias = "build")]\n&|' \
    "demo: a build that writes Launched's build as build_id")
relaunch "$oneway" ""
relaunch "$oneway" ',"one_way":true' "refused at rehearsal"
runs

step "end: retire the brain's unit and delete the demo's builds"
systemctl --user disable --now --quiet "$unit"
rm "${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/$unit"
systemctl --user daemon-reload
for build in "$base" "$good" "$check" "$live" "$dies" "$oneway"; do
    [[ -d $home/builds/$build ]] && chmod -R u+w "$home/builds/$build" && rm -rf "$home/builds/$build"
done
echo "its limbs end themselves 15 minutes after their brain"
