#!/usr/bin/env bash
# Stopping work, end to end, on real brains: Max abandons a task whose
# background worker saves its state; Max abandons one whose process ignores
# SIGTERM, with a deadline too short for it; Ctrl-C, once and then twice,
# on a brain in a terminal, then once more to see it carry on; and an
# agent that passes its background worker to Max and finishes, while the
# worker runs on until Max stops it; and a brain run as a systemd service,
# restarted while a command runs. After each, every process the brain
# started is looked for, with the unit it is in.
#
# Every role runs on the scripted model, each mandate carrying the script
# of the calls it asks for (MODEL=sonnet/low for real agents, who read only
# the words). A, B and D run on test brains cancel-a, -b, -d
# (dev/drive.mjs), which print their links. C and E own their brain's
# process, as the point is what reaches it: C runs it in a terminal to
# press Ctrl-C at, E as a systemd unit to restart. They serve on PORT
# (4346 unless told).
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain "cancel-$2" "${@:3}"; }
cargo build -q --release -p harness-brain -p harness-limb-workspace --bins --example act
dir=target/cancel-demo
brain=target/release/harness-brain
model=${MODEL:-scripted}
rm -rf "$dir"
mkdir -p "$dir"
demos/cancel/scratch.sh "$dir/repo"
start=$(date +%s.%N)

# Everything a brain starts inherits a mark, however it escapes its process
# group, so what is left of them is found by it: each test brain's, and
# this run's own for C and E.
mark=HARNESS_OWNER_cancel_demo_$$
clone=$(printf %s "$PWD" | sha256sum | cut -c1-8)
marks="^($mark|HARNESS_OWNER_drive_cancel-[abd]_$clone)="
unit=cancel-demo-$$
trap 'systemctl --user stop "$unit" 2>/dev/null || true; demos/end.sh "$mark"; for b in a b d; do drive down $b; done' EXIT
for b in a b d; do drive down $b --wipe; done
# C's and E's limbs keep their state here, not with Max's own.
export XDG_STATE_HOME=$PWD/$dir/state
# The scripted model's part of a mandate: its lines, on the scripted model.
script() { [ "$model" = scripted ] && printf '\n\n%s' "$@"; true; }
left() {
  local any= e p
  printf "          %7s %7s %7s %-4s %-40s %s\n" PID PGID SID STAT UNIT COMMAND
  for e in $(grep -lzE "$marks" /proc/[0-9]*/environ 2>/dev/null); do
    p=$(basename "$(dirname "$e")")
    # The unit is the last part of its cgroup: a service, or a limb's scope.
    ps -o pid=,pgid=,sid=,stat=,args= -p "$p" | awk -v u="$(sed 's|.*/||' "/proc/$p/cgroup" 2>/dev/null)" \
      '{ printf "          %7s %7s %7s %-4s %-40s", $1, $2, $3, $4, u; $1=$2=$3=$4=""; sub(/^ +/, ""); print " " $0 }' ||
      continue # it exited while being looked at
    any=1
  done
  [ -n "$any" ] || echo "          (none)"
}
c() { curl -sS --resolve "mx-hv-mj.local:$port:127.0.0.1" "$@"; }
say() { printf "%6.1fs  %s\n" "$(awk "BEGIN { print $(date +%s.%N) - $start }")" "$*"; }
q() { c "https://mx-hv-mj.local:$port/view" | jq -r "$1"; }
root='.rows[] | .Task // empty | select(.owner == null)'
# Max abandons the root at a tab, with $1 ms to finish.
abandon() { target/release/examples/act "$port" abandon "$chat" "$1" | sed 's/^/          the brain: /'; }
until_root() { # jq condition on the root task, seconds
  for _ in $(seq $(($2 * 2))); do
    [ "$(q "$root | $1" 2>/dev/null)" = true ] && return 0
    sleep 0.5
  done
  say "!! the root never reached: $1"
  return 1
}
# Start test brain cancel-NAME on a new database, Max starts the task, and
# once its first turn has ended, say what is running.
launch() { # name mandate
  drive up "$1" "$model" | sed 's/^/          /'
  port=$(cat ".drive.ignore/cancel-$1/port")
  chat=$(drive chat "$1" engineer "$dir/repo" "$2")
  echo "          the brain: chat $chat"
  until_root '.waiting.kind == "max"' 180
  say "its turn has ended with this running in the background:"
  left
}
# Shut the brain down with SIGTERM, as systemd does, and show its log; then
# end the limbs it leaves, so the next part starts clean.
finish() { # name
  local pid
  pid=$(cat ".drive.ignore/cancel-$1/pid")
  kill -TERM "$pid"
  while [ -e "/proc/$pid" ]; do sleep 0.2; done
  say "the brain's log:"
  sed 's/^/          /' ".drive.ignore/cancel-$1/brain.log"
  drive down "$1"
}

say "== A. Max abandons a task whose background worker saves its state on SIGTERM"
launch a "Start \`./worker.sh\` in the background: call the \`background\` tool to run \`bash\` with the command \`./worker.sh\` and \`timeout_ms\` 600000. Then say in one line that it is running, and end your turn.$(script \
  'tool background {"name": "worker", "tool": "bash", "input": {"command": "./worker.sh", "timeout_ms": 600000}}' \
  'say It is running.' 'say Stopping: nothing to tidy.')"
say "Max presses Abandon: two minutes to stop and tidy up"
abandon 120000
until_root '.outcome != null' 180
say "abandoned; worker.log:"
sed 's/^/          /' "$dir/repo/worker.log"
say "left running:"
left
finish a

say "== B. Max abandons a task whose process ignores SIGTERM, with 6 seconds to do it"
launch b "Start \`./stubborn.sh\` in the background: call the \`background\` tool to run \`bash\` with the command \`./stubborn.sh\` and \`timeout_ms\` 600000. Then say in one line that it is running, and end your turn.$(script \
  'tool background {"name": "stubborn", "tool": "bash", "input": {"command": "./stubborn.sh", "timeout_ms": 600000}}' \
  'say It is running.' 'slow 2000 say Stopping it now, and writing up what I did.')"
say "Max abandons it with 6 s: its call gets three quarters of that, then SIGKILL"
abandon 6000
until_root '.outcome != null' 60
say "abandoned: $(q "$root"' | .outcome | "by \(.by), \(if .report then "with" else "without" end) a report"')"
say "left running:"
left
finish b

say "== C1. Ctrl-C once on a brain in a terminal, while a command runs"
own=${PORT:-4346}
port=$own
{ printf '%s' "Run \`./slow.sh\` with \`bash\` (it takes about a minute), then tell me in one line what its last line said."
  script 'tool bash {"command": "./slow.sh", "timeout_ms": 120000}' 'slow 1500 say Its last line said where it stopped.'; } > "$dir/c.md"
# Max starts the task at a tab once the brain in the terminal serves.
target/release/examples/act "$port" chat engineer "$PWD/$dir/repo" "$(cat "$dir/c.md")" > /dev/null &
python3 demos/cancel/terminal.py "$dir/c1.log" "./slow.sh" 3 1 0 -- \
  env "$mark=1" "$brain" --roles roles --model "$model" --port "$port" --db "$dir/c.sqlite"
say "left running:"
left

say "== C2. The same database again; Ctrl-C twice while its request waits for the first byte"
python3 demos/cancel/terminal.py "$dir/c2.log" "request sent" 0.8 2 0.2 -- \
  env "$mark=1" "$brain" --roles roles --model "$model" --port "$port" --db "$dir/c.sqlite"
say "left running:"
left

say "== C3. Again: the request the last Ctrl-C cut is sent again, and reads the cache"
python3 demos/cancel/terminal.py "$dir/c3.log" "responded" 0 2 0.2 -- \
  env "$mark=1" "$brain" --roles roles --model "$model" --port "$port" --db "$dir/c.sqlite"
say "left running:"
left

# C's limbs are done with: end them, so what D leaves is D's.
demos/end.sh "$mark"

say "== D. The agent passes its background worker to Max and finishes; the worker runs on past its time limit until Max stops it"
: > "$dir/repo/worker.log"
launch d "Start \`./worker.sh\` in the background: call the \`background\` tool with the \`name\` \`worker\`, to run \`bash\` with the command \`./worker.sh\` and \`timeout_ms\` 10000. It is a server Max will go on using after you finish, so then pass it to Max with the \`pass\` tool, saying why. Then say in one line what you did, and end your turn.$(script \
  'tool background {"name": "worker", "tool": "bash", "input": {"command": "./worker.sh", "timeout_ms": 10000}}' \
  'tool pass {"call": "worker", "why": "A server Max goes on using after I finish."}' \
  'say Started worker.sh and passed it to Max.' 'say Done: worker.sh runs on with Max.')"
say "its own background work, and what it passed to Max:"
q "$root | {background: [.background[].name], handed: [.handed[] | {name, owner, why: .passed.why}]}" | sed 's/^/          /'
say "Max presses Done: it finishes, as nothing it owns still runs"
target/release/examples/act "$port" done "$chat" | sed 's/^/          the brain: /'
until_root '.outcome != null' 180
say "finished: $(q "$root | .outcome.kind"); left running:"
left
call=$(q "$root | .handed[0].id")
sleep 15
say "15 s on, past its 10 s time limit, it runs on, as Max keeps it for its lifetime. worker.log:"
sed 's/^/          /' "$dir/repo/worker.log"
say "the call is still Max's, running: $(q "$root | .handed[] | select(.id == \"$call\") | .name")"
say "Max stops the worker passed to him, $call"
target/release/examples/act "$port" stop "$call" | sed 's/^/          the brain: /'
until_root '(.handed | length) == 0' 120
say "stopped; worker.log:"
sed 's/^/          /' "$dir/repo/worker.log"
say "left running:"
left
finish d

say "== E. The brain as a systemd service, restarted while a command runs"
# Its own launch: a unit the demo restarts, which drive cannot be.
port=$own
cp "$dir/c.md" "$dir/e.md"
# Max starts the task at a tab when the service makes the database, and
# only then, once the brain serves.
cat > "$dir/e.sh" <<EOF
#!/usr/bin/env bash
[ -e $dir/e.sqlite ] || $PWD/target/release/examples/act $port chat engineer $PWD/$dir/repo "\$(cat $PWD/$dir/e.md)" &
exec $PWD/$brain --roles roles --model $model --port $port --db $dir/e.sqlite
EOF
chmod +x "$dir/e.sh"
systemd-run --user --quiet --unit="$unit" --same-dir \
  --setenv="$mark=1" --setenv=XDG_STATE_HOME="$XDG_STATE_HOME" \
  -p StandardOutput=append:"$PWD/$dir/e.log" -p StandardError=append:"$PWD/$dir/e.log" \
  "$PWD/$dir/e.sh"
until grep -q "bash dispatched" "$dir/e.log" 2>/dev/null; do sleep 0.2; done
sleep 2
say "./slow.sh runs; systemctl --user restart $unit"
systemctl --user restart "$unit"
say "restarted; left running:"
left
until_root '.waiting.kind == "max"' 180
say "the result reached the restarted brain; its log:"
sed 's/^/          /' "$dir/e.log"
systemctl --user stop "$unit"
