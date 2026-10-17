#!/bin/sh
# Make the cancel demo's scratch repository at $1: three commands that stop
# in different ways.
set -eu
rm -rf "$1"
mkdir -p "$1"
cd "$1"
cat > AGENTS.md <<'MD'
# A scratch repository for a demo

Do exactly what you are asked, and nothing more. Don't explore the repository.
MD
cat > worker.sh <<'SH'
#!/bin/bash
# A worker that saves its state when asked to stop. It starts a helper in a
# session of its own, the way a daemon escapes: no process group holds it.
setsid sleep 600 &
echo "worker: started; its helper $! left the process group" >> worker.log
trap 'echo "worker: SIGTERM: saving state" >> worker.log; sleep 2; echo "worker: state saved; exiting" >> worker.log; exit 0' TERM
while :; do sleep 1 & wait; done
SH
cat > stubborn.sh <<'SH'
#!/bin/bash
# A process that ignores SIGTERM, with an escaped helper that does too.
trap '' TERM
setsid bash -c "trap '' TERM; while :; do sleep 1; done" &
echo "stubborn: started; ignoring SIGTERM" >> stubborn.log
while :; do sleep 1; done
SH
cat > slow.sh <<'SH'
#!/bin/bash
# A minute of steps, saying where it was when it is stopped.
trap 'echo "slow: SIGTERM at step $i: stopping" | tee -a slow.log; exit 143' TERM
for i in $(seq 1 60); do echo "slow: step $i"; sleep 1 & wait; done
echo "slow: finished"
SH
chmod +x worker.sh stubborn.sh slow.sh
git init -q
git add .
git -c user.name=demo -c user.email=demo@example.com commit -qm "Three commands"
