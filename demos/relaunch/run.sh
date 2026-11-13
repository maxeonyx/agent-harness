#!/usr/bin/env bash
# The harness relaunches itself through a rehearsal and a gated opening,
# and a broken build bricks nothing.
#
# A throwaway long-lived brain (`dev/serve`, unit harness-relaunch-demo,
# port 4392, the scripted model) runs this clone's HEAD, from a scratch
# clone of a scratch upstream whose main is that commit; the brain's source
# clone is a clone of the upstream too. A session calls the builds limb's
# `relaunch` onto targets, each a commit pushed to its own branch on top
# of main unless it says otherwise, and each call's result is printed once
# it settles:
#   1. `main`, after a good build is merged: rehearsed, opened as a
#      candidate, watched for 3 min;
#   then `previous`, back to the build before it, though that lacks some
#   of main; then, refused at once with nothing built: a commit that lacks
#   some of main, and one no branch has;
#   2. one whose --check fails: refused at rehearsal with its gate: line;
#   3. one whose page throws as it loads: refused at rehearsal, naming the
#      exception headless Chrome saw;
#   4. one whose gate fails only on the live database: it writes nothing,
#      and the build before runs again;
#   5. one whose brain dies 20 s after opening: the harness goes back;
#   6. one the running build cannot read back (it renames a field of
#      Launched): refused as one-way, then opened with `one_way: true`;
#   7. `previous` after that: refused at rehearsal, since the build before
#      cannot read what the one-way build wrote.
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
git init -q --bare "$work/upstream.git"
git -C "$repo" push -q "$work/upstream.git" HEAD:refs/heads/main
git clone -q "$work/upstream.git" "$work/clone"
git clone -q "$work/upstream.git" "$home/$name/source"
# A target directory of its own for each, kept between runs so
# dependencies stay built. Never shared: cargo would take one clone's build
# as fresh for the other, and this clone's binaries would be the demo's
# commits.
for dir in clone source; do
    to=$work/$dir
    [[ $dir == clone ]] || to=$home/$name/source
    mkdir "$to/.cargo"
    printf '[build]\ntarget-dir = "%s/.demo-relaunch-%s-target.ignore"\n' "$repo" "$dir" > "$to/.cargo/config.toml"
done
git -C "$work/clone" config user.name demo
git -C "$work/clone" config user.email demo@localhost

# Commit on top of $1 the change the sed script $3 makes to the file $2
# (none if it is empty), with the message $4, and push it to a branch of
# its own; print the commit.
variant() {
    git -C "$work/clone" checkout -q --detach "$1"
    [[ -z $2 ]] || sed -i "$3" "$work/clone/$2"
    git -C "$work/clone" commit -q --allow-empty -am "$4"
    local commit
    commit=$(git -C "$work/clone" rev-parse --short=12 HEAD)
    git -C "$work/clone" push -q origin "HEAD:refs/heads/demo-$commit"
    echo "$commit"
}

# A session calls `relaunch` onto the target $1 with the extra input $2,
# and this prints the call's result naming $3 (the target if none) once it
# settles, and not saying $4: its stage, then its words.
relaunch() {
    "$work/act" "$port" chat orchestrator "$work/ws" "tool relaunch {\"target\":\"$1\"$2}" >/dev/null
    local said=""
    for _ in $(seq 1800); do
        said=$(sqlite3 "$db" "select cast(bytes as text) from value" 2>/dev/null |
            grep -a -o -E '(refused at rehearsal|failed its gate|opened|went back): ([^"\\]|\\.)*' |
            grep -a -F "${3:-$1}" | grep -a -v -F "${4:-no such words}" | head -1) || true
        [[ -n $said ]] && break
        sleep 1
    done
    printf '%b\n' "${said:-no result in 30 minutes}"
}

base=$(git -C "$work/clone" rev-parse --short=12 HEAD)
# A build of HEAD installed before the demo may be another brain's: keep it.
[[ -d $home/builds/$base ]] && installed="" || installed=$base
step "serve $base"
"$work/clone/dev/serve" "$name" "$port" --model scripted
cp "$home/$name/build/bin/act" "$work/act"
runs

step "1. relaunch onto main, once a good build is merged"
good=$(variant "$base" "" "" "demo: a good build")
git -C "$work/clone" push -q origin "$good:refs/heads/main"
relaunch main "" "$good"
runs

step "back to previous"
relaunch previous "" "took over from $good"
runs

step "refused with nothing built: a commit that lacks some of main, and one no branch has"
relaunch "$base" "" "$base does not contain"
unpushed=$(git -C "$work/clone" commit-tree -m "demo: never pushed" "$good^{tree}" | cut -c1-12)
relaunch "$unpushed" ""

step "2. onto a build whose --check fails: refused at rehearsal"
check=$(variant "$good" crates/brain/src/main.rs \
    's|^        gate::gate(&db, &client, roles.as_deref(), port, passed, serve)?;$|        return Err("gate: decide: the demo build fails its check".into());\n&|' \
    "demo: a build whose check fails")
relaunch "$check" ""
runs

step "3. onto a build whose page throws: refused at rehearsal"
page=$(variant "$good" crates/face-web/src/page/builds.js \
    's|^import { can } from "./tab.js";$|&\nthrow new Error("demo: builds.js breaks on load");|' \
    "demo: a build whose page throws")
relaunch "$page" ""
runs

step "4. onto a build whose gate fails only on the live database"
live=$(variant "$good" crates/brain/src/main.rs \
    's|^        None => {$|&\n            return Err("gate: decide: the demo build fails on the live database".into());|' \
    "demo: a build whose gate fails on the live database")
relaunch "$live" ""
runs
echo "records naming $live in the database: $(sqlite3 "$db" "select count(*) from record where record like '%$live%'")"

step "5. onto a build whose brain dies 20 s after opening: the way back"
dies=$(variant "$good" crates/brain/src/main.rs \
    's|^            install(&db)?;$|&\n            std::thread::spawn(\|\| {\n                std::thread::sleep(Duration::from_secs(20));\n                std::process::exit(3);\n            });|' \
    "demo: a build whose brain dies 20 s after opening")
relaunch "$dies" ""
runs

step "6. onto a one-way build: refused, then opened with one_way"
oneway=$(variant "$good" crates/core/src/build.rs \
    's|^    pub build: BuildId,$|    #[serde(rename = "build_id", alias = "build")]\n&|' \
    "demo: a build that writes Launched's build as build_id")
relaunch "$oneway" ""
relaunch "$oneway" ',"one_way":true' "" "refused at rehearsal"
runs

step "7. back to previous after a one-way relaunch: refused"
relaunch previous "" "$base failed its gate"
runs

step "end: retire the brain's unit and delete the demo's builds"
systemctl --user disable --now --quiet "$unit"
rm "${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/$unit"
systemctl --user daemon-reload
for build in $installed "$good" "$check" "$page" "$live" "$dies" "$oneway"; do
    [[ -d $home/builds/$build ]] && chmod -R u+w "$home/builds/$build" && rm -rf "$home/builds/$build"
done
echo "its limbs end themselves 15 minutes after their brain"
