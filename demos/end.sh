#!/usr/bin/env bash
# End what a demo left running: every process carrying its mark, an
# environment variable everything the demo started inherits, however it
# escaped its process group. A brain's limbs live on after it exits, for
# the next brain, so a demo that is finished ends them itself: SIGTERM, as
# Max would stop a limb, which stops its calls; SIGKILL to what is left
# after 10 s.
#
# Usage: demos/end.sh MARK
mark=$1
pids() {
  for e in /proc/[0-9]*/environ; do
    grep -qz "^$mark=" "$e" 2>/dev/null && basename "$(dirname "$e")"
  done
}
left=$(pids)
[ -z "$left" ] && exit 0
kill -TERM $left 2>/dev/null
for _ in $(seq 50); do
  left=$(pids)
  [ -z "$left" ] && exit 0
  sleep 0.2
done
kill -KILL $left 2>/dev/null
exit 0
