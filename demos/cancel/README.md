# cancel: stopping work, and the brain itself, without leaving anything behind

```sh
demos/cancel/run.sh
```

It runs real brains on a scratch repo of three commands (`scratch.sh` makes it), and stops them the ways Max does. After each act it lists every process carrying the demo's mark: everything a brain starts inherits `HARNESS_OWNER_cancel-demo-<pid>`, however it escapes its process group, so what is listed is exactly what is left. Watch it at https://mx-hv-mj.local:4326 while it runs (another port: `PORT=4400`). It takes about a minute.

What it does, in order:

- **A. Stop, with time to tidy up.** The agent starts `./worker.sh` in the background and ends its turn. The worker starts a helper in a session of its own, the way a daemon escapes. Max presses Stop: two minutes. The call gets three quarters of that: SIGTERM to every process the call started, which the worker answers by saving its state, then SIGKILL to the rest (the escaped helper). Then the agent gets one last turn to say what it did and what it left, and the task ends cancelled with that report.
- **B. Stop, with a deadline too short.** The same with `./stubborn.sh`, which ignores SIGTERM, and 6 seconds. The call gets 4.5 s, then SIGKILL. The tidy-up turn's request is still running at the deadline, so it is abandoned and the task ends cancelled without a report.
- **C1. Ctrl-C once, in a terminal.** `terminal.py` runs a brain in a pseudo-terminal and presses Ctrl-C while `./slow.sh` runs. The limbs have process groups of their own, so only the brain gets SIGINT. It shuts down: nothing new starts, its limbs stop their calls, and with no request in flight it exits at once. The task isn't touched: it carries on at the next launch.
- **C2. Ctrl-C twice.** The same journal again. The relaunched brain sends the request the interrupted call left, and Ctrl-C comes twice before its first byte: the first gives requests in flight 5 minutes to finish, the second abandons them now.
- **C3. Carrying on.** The same journal again, and the cut request is sent again with the same bytes, so it reads the prompt cache (the default cache lasts 5 minutes). Ctrl-C after it responds.

A stop is a fact with a deadline, `within` milliseconds of the fact's time:

```json
{"Task": ["<task>", {"Drain": {"Stop": {"by": "Max", "within": 120000}}}]}
```

The other ways to stop work are the same fact: a parent stopping a child, the brain stopping the members of a task it stops (with three quarters of the time it has left), and Kill, which is a stop due now.

## Real output

A run at 13:41 NZT on 2026-10-17, trimmed where marked. The demo's own lines are timed from its start; the brain's log is timed from the brain's launch, and in C the terminal's lines from the command's start.

```
   0.0s  == A. Max stops a task whose background worker saves its state on SIGTERM
   5.4s  its turn has ended with this running in the background:
              PID    PGID     SID STAT COMMAND
          1194388 1194057 1194057 SNl  target/release/harness-brain --roles roles --role engineer --port 4326 --journal target/cancel-demo/a.sqlite ...
          1194407 1194407 1194057 SNl  .../harness-limb-user target/cancel-demo/a.workspace-state/553938d97a787ab7e9d91cbb02e8d2eb
          1194409 1194409 1194057 SNl  .../harness-limb-workspace target/cancel-demo/a.workspace-state/9e0ca05dd2d8117d700c65b6e13b5084
          1194681 1194681 1194057 SN   /bin/bash ./worker.sh
          1194682 1194682 1194682 SNs  sleep 600
   6.0s  Max presses Stop: two minutes to stop and tidy up
          answer:  -> 200
  12.7s  cancelled; worker.log:
          worker: started; its helper 1194682 left the process group
          worker: SIGTERM: saving state
          worker: state saved; exiting
  12.7s  left running:
              PID    PGID     SID STAT COMMAND
          1194388 1194057 1194057 SNl  target/release/harness-brain ...
          1194407 1194407 1194057 SNl  .../harness-limb-user ...
          1194409 1194409 1194057 SNl  .../harness-limb-workspace ...
  13.0s  the brain's log:
              [the launch, trimmed]
              5.4s    28  engineer: responded, EndTurn, 0 calls (2 in, 7551 cached, 16 out)
              6.0s    29  engineer stopped by Max, to exit within 120s
              6.0s    30  engineer's bash stopping, within 89s
              8.0s    31  engineer's bash interrupted
              8.0s    32  Stopping prompt delivered to engineer
              8.3s    33  engineer: request sent
             10.1s    34  engineer: first byte
             12.2s    35  engineer: responded, EndTurn, 0 calls (4 in, 7691 cached, 177 out)

          == the task is done; its result:

          Cancelled by Max. Its report:

          I started `./worker.sh` in the background, and it was stopped when you stopped me, before it finished.

          - **Done:** only the start. I never saw any of its output, so I don't know how far it got.
          - **Not done:** the run itself. Because it was cut off partway, it may have left partial effects.
          - **Left running:** nothing. I edited no files, and there is nothing to clean up on my side.
          - **If you want the full run:** start `./worker.sh` again from the beginning.

          harness-brain: SIGTERM: shutting down; requests in flight have 300s to finish (again to stop now)
             13.0s    36  brain shutting down, within 300s
          limb 9e0ca05dd2d8117d700c65b6e13b5084: its process ended
             13.0s    37  /home/maxeonyx/agent-harness-workspace/ah-cancel/target/cancel-demo/repo lost
          limb 553938d97a787ab7e9d91cbb02e8d2eb: its process ended
             13.0s    38  user lost
          harness-brain: shut down
  13.0s  == B. Max stops a task whose process ignores SIGTERM, with 6 seconds to do it
  17.4s  its turn has ended with this running in the background:
              PID    PGID     SID STAT COMMAND
              [the brain and its two limbs, trimmed]
          1195783 1195783 1194057 SN   /bin/bash ./stubborn.sh
          1195784 1195784 1195784 SNs  bash -c trap '' TERM; while :; do sleep 1; done
          1195842 1195783 1194057 SN   sleep 1
          1195843 1195784 1195784 SN   sleep 1
  17.7s  Max stops it with 6 s: its call gets three quarters of that, then SIGKILL
          answer:  -> 200
  24.4s  cancelled: by Max, without a report
  24.4s  left running:
              [the brain and its two limbs, trimmed]
  24.7s  the brain's log:
              [the launch, trimmed]
              4.3s    28  engineer: responded, EndTurn, 0 calls (2 in, 7629 cached, 28 out)
              4.7s    29  engineer stopped by Max, to exit within 6s
              4.7s    30  engineer's bash stopping, within 4s
              9.2s    31  engineer's bash interrupted
              9.2s    32  Stopping prompt delivered to engineer
              9.4s    33  engineer: request sent
             10.7s    34  engineer: first byte
             10.7s    35  engineer is past its deadline: stopped hard
             10.9s    36  engineer: response abandoned

          == the task is done; its result:

          Cancelled by Max, before it could write a report.

          [its SIGTERM shutdown, as in A, trimmed]
  24.7s  == C1. Ctrl-C once on a brain in a terminal, while a command runs
   0.01      0.0s     1  brain launched, build harness-brain 0.0.0
              [the launch, trimmed]
   0.27      0.3s    18  engineer: request sent
   1.95      1.9s    19  engineer: first byte
   2.48      2.5s    20  engineer's bash made {"command": "./slow.sh", "grep": "", "timeout_ms": 180000}
   2.48      2.5s    21  engineer: responded, ToolUse, 1 calls (4 in, 7453 cached, 99 out)
   2.49      2.5s    22  engineer's bash dispatched
   5.50  [the terminal] Ctrl-C (1)
   5.50  ^Charness-brain: SIGINT: shutting down; requests in flight have 300s to finish (again to stop now)
   5.50      5.5s    23  brain shutting down, within 300s
   5.51  limb d631856c8afaaa0edd850a3274791deb: its process ended
   5.51      5.5s    24  user lost
   5.51      5.5s    25  engineer's bash interrupted
   5.52  limb 9ce8121646fb476da800cbb8d0f2425f: its process ended
   5.52      5.5s    26  /home/maxeonyx/agent-harness-workspace/ah-cancel/target/cancel-demo/repo lost
   5.52  harness-brain: shut down
   5.52  [the terminal] the command exited with 0, 0.02s after the first Ctrl-C
  30.3s  left running:
          (none)
  30.6s  == C2. The same journal again; Ctrl-C twice while its request waits for the first byte
   0.01      0.0s    27  brain launched, build harness-brain 0.0.0
   0.01  face: https://mx-hv-mj.local:4326
   0.01      0.0s    28  engineer: request sent
              [its limbs connect, trimmed]
   0.81  [the terminal] Ctrl-C (1)
   0.81  ^Charness-brain: SIGINT: shutting down; requests in flight have 300s to finish (again to stop now)
   0.81      0.8s    33  brain shutting down, within 300s
   0.82  limb 9ce8121646fb476da800cbb8d0f2425f: its process ended
   0.82      0.8s    34  /home/maxeonyx/agent-harness-workspace/ah-cancel/target/cancel-demo/repo lost
   0.83  limb d631856c8afaaa0edd850a3274791deb: its process ended
   0.83      0.8s    35  user lost
   1.01  [the terminal] Ctrl-C (2)
   1.01  ^Charness-brain: SIGINT again: stopping now
   1.01      1.0s    36  brain shutting down, within 0s
   1.02      1.0s    37  engineer: response abandoned
   1.02  harness-brain: shut down
   1.02  [the terminal] the command exited with 0, 0.20s after the first Ctrl-C
  31.6s  left running:
          (none)
  31.9s  == C3. Again: the request the last Ctrl-C cut is sent again, and reads the cache
   0.01      0.0s    38  brain launched, build harness-brain 0.0.0
   0.01  face: https://mx-hv-mj.local:4326
   0.01      0.0s    39  engineer: request sent
              [its limbs connect, trimmed]
   1.37      1.4s    44  engineer: first byte
   2.33      2.3s    45  engineer: responded, EndTurn, 0 calls (2 in, 7593 cached, 107 out)
   2.33  [the terminal] Ctrl-C (1)
   2.33  ^Charness-brain: SIGINT: shutting down; requests in flight have 300s to finish (again to stop now)
   2.34      2.3s    46  brain shutting down, within 300s
              [its limbs end, trimmed]
   2.35  harness-brain: shut down
   2.35  [the terminal] the command exited with 0, 0.02s after the first Ctrl-C
  34.3s  left running:
          (none)
```

In A the worker's escaped `sleep 600` is gone with it, and in B both stubborn processes went at SIGKILL, though one had left the call's process group. A call that ignores SIGTERM costs the task its report: the call took its three quarters of the time, and the tidy-up turn had 1.5 s.
