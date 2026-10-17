#!/usr/bin/env bash
# Stopping work, end to end, on real brains: Max stops a task whose
# background worker saves its state; Max stops one whose process ignores
# SIGTERM, with a deadline too short for it; and Ctrl-C, once and then
# twice, on a brain in a terminal, then once more to see it carry on from
# the cache. After each, every process the brain started is looked for.
# Watch at https://mx-hv-mj.local:4326.
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build -q --release -p harness-brain -p harness-limb-workspace
dir=target/cancel-demo
port=${PORT:-4326}
url=https://mx-hv-mj.local:$port
brain=target/release/harness-brain
rm -rf "$dir"
mkdir -p "$dir"
demos/cancel/scratch.sh "$dir/repo"
start=$(date +%s.%N)

# Everything a demo brain starts inherits this mark, however it escapes
# its process group, so what is left of them is found by it.
mark=HARNESS_OWNER_cancel-demo-$$
left() {
  local pids=()
  for e in /proc/[0-9]*/environ; do
    if grep -qz "^$mark=" "$e" 2>/dev/null; then
      pids+=("$(basename "$(dirname "$e")")")
    fi
  done
  if [ ${#pids[@]} -eq 0 ]; then
    echo "          (none)"
  else
    ps -o pid,pgid,sid,stat,args -p "$(IFS=,; echo "${pids[*]}")" | sed 's/^/          /'
  fi
}

c() { curl -sS --resolve "mx-hv-mj.local:$port:127.0.0.1" "$@"; }
say() { printf "%6.1fs  %s\n" "$(awk "BEGIN { print $(date +%s.%N) - $start }")" "$*"; }
q() { c "$url/view" | jq -r "$1"; }
FACE=$(head -c16 /dev/urandom | od -An -tx1 | tr -d ' \n')
act() {
  jq -nc --arg face "$FACE" --argjson seen "$(q .seq)" --argjson fact "$1" \
    '{face: $face, seen: $seen, batch: {values: [], facts: [$fact]}}' > "$dir/act.json"
  c -X POST "$url/act" --data-binary "@$dir/act.json" -w ' -> %{http_code}' | sed 's/^/          answer: /'
  echo
}
until_root() { # jq condition on the root task, seconds
  for _ in $(seq $(($2 * 2))); do
    [ "$(q ".tasks[] | select(.owner == null) | $1" 2>/dev/null)" = true ] && return 0
    sleep 0.5
  done
  say "!! the root never reached: $1"
  return 1
}
# Start a brain in the background on a new journal, wait for the root to
# end its first turn, and say what is running.
launch() { # name mandate
  printf '%s\n' "$2" > "$dir/$1.md"
  env "$mark=1" "$brain" --roles roles --role engineer --port "$port" \
    --journal "$dir/$1.sqlite" --workspace "$dir/repo" --mandate "$dir/$1.md" > "$dir/$1.log" 2>&1 &
  pid=$!
  until c -s -o /dev/null "$url/" 2>/dev/null; do sleep 0.2; done
  until_root '.waiting.kind == "max"' 180
  say "its turn has ended with this running in the background:"
  left
}
# Shut the brain down with SIGTERM, as systemd does, and show its log.
finish() { # name
  kill -TERM "$pid"
  wait "$pid" || true
  say "the brain's log:"
  sed 's/^/          /' "$dir/$1.log"
}

say "== A. Max stops a task whose background worker saves its state on SIGTERM"
launch a "Start \`./worker.sh\` in the background: call the \`background\` tool to run \`bash\` with the command \`./worker.sh\` and \`timeout_ms\` 600000. Then say in one line that it is running, and end your turn."
say "Max presses Stop: two minutes to stop and tidy up"
act '{"Task": ["'"$(q '.tasks[] | select(.owner == null) | .id')"'", {"Drain": {"Stop": {"by": "Max", "within": 120000}}}]}'
until_root '.outcome != null' 180
say "cancelled; worker.log:"
sed 's/^/          /' "$dir/repo/worker.log"
say "left running:"
left
finish a

say "== B. Max stops a task whose process ignores SIGTERM, with 6 seconds to do it"
launch b "Start \`./stubborn.sh\` in the background: call the \`background\` tool to run \`bash\` with the command \`./stubborn.sh\` and \`timeout_ms\` 600000. Then say in one line that it is running, and end your turn."
say "Max stops it with 6 s: its call gets three quarters of that, then SIGKILL"
act '{"Task": ["'"$(q '.tasks[] | select(.owner == null) | .id')"'", {"Drain": {"Stop": {"by": "Max", "within": 6000}}}]}'
until_root '.outcome != null' 60
say "cancelled: $(q '.tasks[] | select(.owner == null) | .outcome | "by \(.by), \(if .report then "with" else "without" end) a report"')"
say "left running:"
left
finish b

say "== C1. Ctrl-C once on a brain in a terminal, while a command runs"
printf '%s\n' "Run \`./slow.sh\` with \`bash\` (it takes about a minute), then tell me in one line what its last line said." > "$dir/c.md"
python3 demos/cancel/terminal.py "$dir/c1.log" "./slow.sh" 3 1 0 -- \
  env "$mark=1" "$brain" --roles roles --role engineer --port "$port" \
  --journal "$dir/c.sqlite" --workspace "$dir/repo" --mandate "$dir/c.md"
say "left running:"
left

say "== C2. The same journal again; Ctrl-C twice while its request waits for the first byte"
python3 demos/cancel/terminal.py "$dir/c2.log" "request sent" 0.8 2 0.2 -- \
  env "$mark=1" "$brain" --roles roles --role engineer --port "$port" --journal "$dir/c.sqlite"
say "left running:"
left

say "== C3. Again: the request the last Ctrl-C cut is sent again, and reads the cache"
python3 demos/cancel/terminal.py "$dir/c3.log" "responded" 0 2 0.2 -- \
  env "$mark=1" "$brain" --roles roles --role engineer --port "$port" --journal "$dir/c.sqlite"
say "left running:"
left
