#!/usr/bin/env bash
# One limb process per machine: four chats in three directories, one
# nested in another, all run by one `harness-limb serve`, each agent's
# commands in a slice of its own. Scripted: no model call.
set -euo pipefail
cd "$(dirname "$0")/../.."
B="--brain limbs-per-machine"
node dev/drive.mjs down $B --wipe
node dev/drive.mjs up $B
cargo build -q --release -p harness-brain --example act
W=$PWD/.drive.ignore/limbs-per-machine/dirs
mkdir -p "$W/a/nested" "$W/b"
echo original >"$W/a/nested/note.txt"
git -C "$W/b" init -q -b main
git -C "$W/b" commit -q --allow-empty -m init

say() { printf '\n== %s\n' "$*"; }
A=$(node dev/drive.mjs chat $B engineer "$W/a" 'tool read {"path":"nested/note.txt"}
tool bash {"command":"sleep 12; echo A done"}
say A carried on')
C=$(node dev/drive.mjs chat $B engineer "$W/a/nested" 'tool bash {"command":"sleep 3; echo changed > note.txt; echo wrote"}
say C wrote')
S=$(node dev/drive.mjs chat $B engineer "$W/b" 'tool bash {"command":"setsid sleep 4242 & sleep 4243"}')
D=$(node dev/drive.mjs chat $B orchestrator "$W/b" 'tool spawn {"tasks":[{"name":"badbranch","role":"engineer","mandate":"say hi","branch":"x","base":"nope"}]}')
sleep 2

say "systemd: one limb scope, a scope per running command, a slice per agent"
# The brain reaches its limb through an `attach LIMB` child.
id=$(pgrep -a -P "$(cat .drive.ignore/limbs-per-machine/pid)" | sed -n 's/.*harness-limb attach //p')
limb=harness-limb-$id.scope
replica=${id%%.*}
systemctl --user list-units 'harness-*' --no-legend --plain |
    grep -E "$replica|${A%%.*}|${C%%.*}|${S%%.*}|${D%%.*}" | cut -c1-220

say "the limb's memory"
cgroup=$(systemctl --user show "$limb" -p ControlGroup --value)
for pid in $(cat "/sys/fs/cgroup$cgroup/cgroup.procs"); do
    echo "$(tr '\0' ' ' <"/proc/$pid/cmdline" | cut -c1-120) $(grep VmRSS "/proc/$pid/status")"
done

say "stopping the chat whose command ran setsid sleep 4242 & sleep 4243"
ps -eo pid,args | grep -E '[s]leep 424[23]'
port=$(cat .drive.ignore/limbs-per-machine/port)
target/release/examples/act "$port" abandon "$S" 0
sleep 3
echo "left running:"
ps -eo pid,args | grep -E '[s]leep 424[23]' || echo "  no sleep 4242 or 4243"

say "A, in a/, read nested/note.txt; C, in a/nested/, wrote it"
node dev/drive.mjs settle $B "$A" >/dev/null
node dev/drive.mjs log $B "$A" | grep -E '^  [←→]|^harness: .*Changed since|^agent: A'
node dev/drive.mjs log $B "$C" | grep -E '^agent: C wrote'

say "a child whose branch can't be made waits with git's words; the others carried on"
node dev/drive.mjs log $B "$D" | grep '^child'

echo
echo "end it with: node dev/drive.mjs down $B"
