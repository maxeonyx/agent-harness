# cancel: stopping work, and the brain itself, without leaving anything behind

```sh
demos/cancel/run.sh
```

It runs real brains on a scratch repo of three commands (`scratch.sh` makes it), and stops them the ways Max does. After each act it lists every process carrying the demo's mark: everything a brain starts inherits `HARNESS_OWNER_cancel-demo-<pid>`, however it escapes its process group, so what is listed is exactly what is left. Watch it at https://mx-hv-mj.local:4326 while it runs (another port: `PORT=4400`). It takes about a minute.

What it does, in order:

- **A. Stop, with time to tidy up.** The agent starts `./worker.sh` in the background and ends its turn. The worker starts a helper in a session of its own, the way a daemon escapes. Max presses Stop: two minutes. The call gets three quarters of that: SIGTERM to every process the call started, which the worker answers by saving its state, then SIGKILL to the rest (the escaped helper). Then the agent gets one last turn to say what it did and what it left, and the task ends cancelled with that report.
- **B. Stop, with a deadline too short.** The same with `./stubborn.sh`, which ignores SIGTERM, and 6 seconds. The call gets 4.5 s, then SIGKILL. The tidy-up turn's request is still running at the deadline, so it is abandoned and the task ends cancelled without a report.
- **C1. Ctrl-C once, in a terminal.** `terminal.py` runs a brain in a pseudo-terminal and presses Ctrl-C while `./slow.sh` runs. The limbs have process groups of their own, so only the brain gets SIGINT. It shuts down: nothing new starts, its limbs stop their calls, and with no request in flight it exits at once. The task isn't touched: it carries on at the next launch.
- **C2. Ctrl-C twice.** The same database again. The relaunched brain sends the request the interrupted call left, and Ctrl-C comes twice before its first byte: the first gives requests in flight 5 minutes to finish, the second abandons them now.
- **C3. Carrying on.** The same database again, and the cut request is sent again with the same bytes, so it reads the prompt cache (every breakpoint's cache lasts an hour). Ctrl-C after it responds.

Max presses Stop at a tab: `target/release/examples/act PORT stop MS`, a place on the face's link that says one fact and reads the brain's verdict from its standing. A stop is a fact with a deadline, `within` milliseconds of the fact's time:

```json
{"Task": ["<task>", {"Drain": {"Stop": {"by": "Max", "within": 120000}}}]}
```

The other ways to stop work are the same fact: a parent stopping a child, the brain stopping the members of a task it stops (with three quarters of the time it has left), and Kill, which is a stop due now.

## Real output

A run at 16:58 NZT on 2026-10-17. The demo's own lines are timed from its start; the brain's log is timed from the brain's launch, and in C the terminal's lines from the command's start.

```
   0.0s  == A. Max stops a task whose background worker saves its state on SIGTERM
   5.8s  its turn has ended with this running in the background:
              PID    PGID     SID STAT COMMAND
          1297101 1297033 1296964 SNl  target/release/harness-brain --roles roles --role engineer --port 4326 --db target/cancel-demo/a.sqlite --workspace target/cancel-demo/repo --mandate target/canc
          1297394 1297394 1296964 SNl  .../harness-limb-user target/cancel-demo/a.limbs/f7802153a0184743.1.3 target/cancel-demo/a.limbs/limbs.sock {"id":"f7802153a0184743.1.3","name":"user","kind":{"P
          1297395 1297395 1296964 SNl  .../harness-limb-workspace target/cancel-demo/a.limbs/f7802153a0184743.4.0 target/cancel-demo/a.limbs/limbs.sock {"id":"f7802153a0184743.4.0","name":"/home/maxeo
          1297554 1297554 1296964 SN   /bin/bash ./worker.sh
          1297555 1297555 1297555 SNs  sleep 600
   6.2s  Max presses Stop: two minutes to stop and tidy up
          the brain:   act: tab 2dc527548ebcb207 linked
          the brain:   act: Max presses Stop, with 120s to finish
          the brain: included
  12.0s  cancelled; worker.log:
          worker: started; its helper 1297555 left the process group
          worker: SIGTERM: saving state
          worker: state saved; exiting
  12.0s  left running:
              PID    PGID     SID STAT COMMAND
          1297101 1297033 1296964 SNl  target/release/harness-brain --roles roles --role engineer --port 4326 --db target/cancel-demo/a.sqlite --workspace target/cancel-demo/repo --mandate target/canc
          1297394 1297394 1296964 SNl  .../harness-limb-user target/cancel-demo/a.limbs/f7802153a0184743.1.3 target/cancel-demo/a.limbs/limbs.sock {"id":"f7802153a0184743.1.3","name":"user","kind":{"P
          1297395 1297395 1296964 SNl  .../harness-limb-workspace target/cancel-demo/a.limbs/f7802153a0184743.4.0 target/cancel-demo/a.limbs/limbs.sock {"id":"f7802153a0184743.4.0","name":"/home/maxeo
  12.6s  the brain's log:
              0.0s f7802153a0184743.1  brain launched, build harness-brain 0.0.0
              0.0s f7802153a0184743.1  limb meta defined
              0.0s f7802153a0184743.1  limb max defined
              0.0s f7802153a0184743.1  limb user defined
              0.0s f7802153a0184743.1  limb roles defined
              0.0s f7802153a0184743.2  roles connected
              0.0s 778ee9682faf09e2.1  environment designer defined
              0.0s 778ee9682faf09e2.1  environment engineer defined
              0.0s 778ee9682faf09e2.1  environment orchestrator defined
              0.0s 778ee9682faf09e2.1  role /home/maxeonyx/agent-harness-workspace/ah-spine-a/roles/designer.md changed
              0.0s 778ee9682faf09e2.1  role /home/maxeonyx/agent-harness-workspace/ah-spine-a/roles/engineer.md changed
              0.0s 778ee9682faf09e2.1  role /home/maxeonyx/agent-harness-workspace/ah-spine-a/roles/orchestrator.md changed
              0.0s f7802153a0184743.4  limb /home/maxeonyx/agent-harness-workspace/ah-spine-a/target/cancel-demo/repo defined
              0.0s f7802153a0184743.4  tab 9c216134bb398d4a is Max's
              0.0s 9c216134bb398d4a.1  engineer created (engineer)
          face: https://mx-hv-mj.local:4326
              2.0s f7802153a0184743.6  /home/maxeonyx/agent-harness-workspace/ah-spine-a/target/cancel-demo/repo connected
              2.0s f7802153a0184743.7  user connected
              2.0s 319ce78cad50e34f.1  /home/maxeonyx/agent-harness-workspace/ah-spine-a/target/cancel-demo/repo described: 5 tools
              2.0s e169ead0c4b6e6d9.1  user described: 1 tools
              2.0s f7802153a0184743.10  engineer has a new root, on claude-opus-5-5
              2.3s f7802153a0184743.11  engineer: request sent
              3.5s f7802153a0184743.12  engineer: first byte
              4.3s f7802153a0184743.13  engineer's background made
              4.3s f7802153a0184743.13  engineer: responded, ToolUse, 1 calls (4 in, 7623 cached, 97 out)
              4.3s f7802153a0184743.14  engineer's background dispatched
              4.3s f7802153a0184743.14  engineer's bash made {"command":"./worker.sh","grep":"","timeout_ms":600000}
              4.3s f7802153a0184743.14  engineer's background completed
              4.6s f7802153a0184743.15  engineer: request sent
              4.6s f7802153a0184743.16  engineer's bash dispatched
              5.7s f7802153a0184743.17  engineer: first byte
              5.7s f7802153a0184743.18  engineer: responded, EndTurn, 0 calls (2 in, 7762 cached, 16 out)
              6.2s f7802153a0184743.19  tab 2dc527548ebcb207 is Max's
              6.2s 2dc527548ebcb207.1  engineer stopped by Max, to exit within 120s
              6.2s 2dc527548ebcb207.1  as seen at f7802153a0184743.18
              6.2s f7802153a0184743.21  engineer's bash stopping, within 89s
              8.2s 319ce78cad50e34f.2  engineer's bash interrupted
              8.2s f7802153a0184743.23  Stopping prompt delivered to engineer
              8.5s f7802153a0184743.24  engineer: request sent
              9.5s f7802153a0184743.25  engineer: first byte
             11.5s f7802153a0184743.26  engineer: responded, EndTurn, 0 calls (4 in, 7762 cached, 179 out)
          
          == the task is done; its result:
          
          Cancelled by Max. Its report:
          
          `./worker.sh` was stopped before it finished, when you stopped me. It may have done part of its work, and I didn't capture any of its output, so I don't know how far it got.
          
          - **Done:** I started `./worker.sh` in the background. That was the whole task.
          - **Not done:** the run didn't complete.
          - **Left running:** nothing. I didn't edit or commit any files and didn't start any agents.
          - **To know:** if the script changes anything, look for partial changes before running it again.
          
          harness-brain: SIGTERM: shutting down; requests in flight have 300s to finish (again to stop now)
             12.3s f7802153a0184743.27  brain shutting down, within 300s
          limb f7802153a0184743.1.4: its link ended
             12.3s f7802153a0184743.28  roles lost
          limb f7802153a0184743.1.3: its link ended
             12.3s f7802153a0184743.29  user lost
          limb f7802153a0184743.4.0: its link ended
             12.3s f7802153a0184743.30  /home/maxeonyx/agent-harness-workspace/ah-spine-a/target/cancel-demo/repo lost
          harness-brain: shut down
  12.6s  == B. Max stops a task whose process ignores SIGTERM, with 6 seconds to do it
  20.5s  its turn has ended with this running in the background:
              PID    PGID     SID STAT COMMAND
          1298946 1297033 1296964 SNl  target/release/harness-brain --roles roles --role engineer --port 4326 --db target/cancel-demo/b.sqlite --workspace target/cancel-demo/repo --mandate target/canc
          1299271 1299271 1296964 SNl  .../harness-limb-user target/cancel-demo/b.limbs/6cec61573f32cb17.1.3 target/cancel-demo/b.limbs/limbs.sock {"id":"6cec61573f32cb17.1.3","name":"user","kind":{"P
          1299272 1299272 1296964 SNl  .../harness-limb-workspace target/cancel-demo/b.limbs/6cec61573f32cb17.4.0 target/cancel-demo/b.limbs/limbs.sock {"id":"6cec61573f32cb17.4.0","name":"/home/maxeo
          1299460 1299460 1296964 SN   /bin/bash ./stubborn.sh
          1299461 1299461 1299461 SNs  bash -c trap '' TERM; while :; do sleep 1; done
          1299527 1299460 1296964 SN   sleep 1
          1299528 1299461 1299461 SN   sleep 1
  20.8s  Max stops it with 6 s: its call gets three quarters of that, then SIGKILL
          the brain:   act: tab 841517e8f0135a45 linked
          the brain:   act: Max presses Stop, with 6s to finish
          the brain: included
  27.6s  cancelled: by Max, without a report
  27.6s  left running:
              PID    PGID     SID STAT COMMAND
          1298946 1297033 1296964 SNl  target/release/harness-brain --roles roles --role engineer --port 4326 --db target/cancel-demo/b.sqlite --workspace target/cancel-demo/repo --mandate target/canc
          1299271 1299271 1296964 SNl  .../harness-limb-user target/cancel-demo/b.limbs/6cec61573f32cb17.1.3 target/cancel-demo/b.limbs/limbs.sock {"id":"6cec61573f32cb17.1.3","name":"user","kind":{"P
          1299272 1299272 1296964 SNl  .../harness-limb-workspace target/cancel-demo/b.limbs/6cec61573f32cb17.4.0 target/cancel-demo/b.limbs/limbs.sock {"id":"6cec61573f32cb17.4.0","name":"/home/maxeo
  28.2s  the brain's log:
              0.0s 6cec61573f32cb17.1  brain launched, build harness-brain 0.0.0
              0.0s 6cec61573f32cb17.1  limb meta defined
              0.0s 6cec61573f32cb17.1  limb max defined
              0.0s 6cec61573f32cb17.1  limb user defined
              0.0s 6cec61573f32cb17.1  limb roles defined
              0.0s 6cec61573f32cb17.2  roles connected
              0.0s 5ae3ddc596243389.1  environment designer defined
              0.0s 5ae3ddc596243389.1  environment engineer defined
              0.0s 5ae3ddc596243389.1  environment orchestrator defined
              0.0s 5ae3ddc596243389.1  role /home/maxeonyx/agent-harness-workspace/ah-spine-a/roles/designer.md changed
              0.0s 5ae3ddc596243389.1  role /home/maxeonyx/agent-harness-workspace/ah-spine-a/roles/engineer.md changed
              0.0s 5ae3ddc596243389.1  role /home/maxeonyx/agent-harness-workspace/ah-spine-a/roles/orchestrator.md changed
              0.0s 6cec61573f32cb17.4  limb /home/maxeonyx/agent-harness-workspace/ah-spine-a/target/cancel-demo/repo defined
              0.0s 6cec61573f32cb17.4  tab 33d31f334d62253f is Max's
              0.0s 33d31f334d62253f.1  engineer created (engineer)
          face: https://mx-hv-mj.local:4326
              2.0s 6cec61573f32cb17.6  /home/maxeonyx/agent-harness-workspace/ah-spine-a/target/cancel-demo/repo connected
              2.0s 6cec61573f32cb17.7  user connected
              2.0s 19e2c9a5cd9eeb3a.1  /home/maxeonyx/agent-harness-workspace/ah-spine-a/target/cancel-demo/repo described: 5 tools
              2.0s 8ad314d05fd6c898.1  user described: 1 tools
              2.0s 6cec61573f32cb17.10  engineer has a new root, on claude-opus-5-5
              2.3s 6cec61573f32cb17.11  engineer: request sent
              4.6s 6cec61573f32cb17.12  engineer: first byte
              5.6s 6cec61573f32cb17.13  engineer's background made
              5.6s 6cec61573f32cb17.13  engineer: responded, ToolUse, 1 calls (4 in, 7630 cached, 100 out)
              5.6s 6cec61573f32cb17.14  engineer's background dispatched
              5.6s 6cec61573f32cb17.14  engineer's bash made {"command":"./stubborn.sh","grep":"","timeout_ms":600000}
              5.6s 6cec61573f32cb17.14  engineer's background completed
              5.8s 6cec61573f32cb17.15  engineer: request sent
              5.8s 6cec61573f32cb17.16  engineer's bash dispatched
              7.8s 6cec61573f32cb17.17  engineer: first byte
              7.8s 6cec61573f32cb17.18  engineer: responded, EndTurn, 0 calls (2 in, 7772 cached, 28 out)
              8.3s 6cec61573f32cb17.19  tab 841517e8f0135a45 is Max's
              8.3s 841517e8f0135a45.1  engineer stopped by Max, to exit within 6s
              8.3s 841517e8f0135a45.1  as seen at 6cec61573f32cb17.18
              8.3s 6cec61573f32cb17.21  engineer's bash stopping, within 4s
             12.8s 19e2c9a5cd9eeb3a.2  engineer's bash interrupted
             12.8s 6cec61573f32cb17.23  Stopping prompt delivered to engineer
             13.1s 6cec61573f32cb17.24  engineer: request sent
             14.3s 6cec61573f32cb17.25  engineer is past its deadline: stopped hard
             14.6s 6cec61573f32cb17.26  engineer: response abandoned
          
          == the task is done; its result:
          
          Cancelled by Max, before it could write a report.
          
          harness-brain: SIGTERM: shutting down; requests in flight have 300s to finish (again to stop now)
             15.4s 6cec61573f32cb17.27  brain shutting down, within 300s
          limb 6cec61573f32cb17.1.4: its link ended
             15.4s 6cec61573f32cb17.28  roles lost
          limb 6cec61573f32cb17.4.0: its link ended
             15.4s 6cec61573f32cb17.29  /home/maxeonyx/agent-harness-workspace/ah-spine-a/target/cancel-demo/repo lost
          limb 6cec61573f32cb17.1.3: its link ended
             15.4s 6cec61573f32cb17.30  user lost
          harness-brain: shut down
  28.2s  == C1. Ctrl-C once on a brain in a terminal, while a command runs
   0.01      0.0s 0c0c0993fd06eef6.1  brain launched, build harness-brain 0.0.0
   0.01      0.0s 0c0c0993fd06eef6.1  limb meta defined
   0.01      0.0s 0c0c0993fd06eef6.1  limb max defined
   0.01      0.0s 0c0c0993fd06eef6.1  limb user defined
   0.01      0.0s 0c0c0993fd06eef6.1  limb roles defined
   0.01      0.0s 0c0c0993fd06eef6.2  roles connected
   0.01      0.0s ce5b9108cff29f75.1  environment designer defined
   0.01      0.0s ce5b9108cff29f75.1  environment engineer defined
   0.01      0.0s ce5b9108cff29f75.1  environment orchestrator defined
   0.01      0.0s ce5b9108cff29f75.1  role /home/maxeonyx/agent-harness-workspace/ah-spine-a/roles/designer.md changed
   0.01      0.0s ce5b9108cff29f75.1  role /home/maxeonyx/agent-harness-workspace/ah-spine-a/roles/engineer.md changed
   0.01      0.0s ce5b9108cff29f75.1  role /home/maxeonyx/agent-harness-workspace/ah-spine-a/roles/orchestrator.md changed
   0.01      0.0s 0c0c0993fd06eef6.4  limb /home/maxeonyx/agent-harness-workspace/ah-spine-a/target/cancel-demo/repo defined
   0.01      0.0s 0c0c0993fd06eef6.4  tab 677026f0f8056b4b is Max's
   0.02      0.0s 677026f0f8056b4b.1  engineer created (engineer)
   0.02  face: https://mx-hv-mj.local:4326
   2.03      2.0s 0c0c0993fd06eef6.6  user connected
   2.03      2.0s 0c0c0993fd06eef6.7  /home/maxeonyx/agent-harness-workspace/ah-spine-a/target/cancel-demo/repo connected
   2.03      2.0s 90675fd1a3475034.1  user described: 1 tools
   2.03      2.0s 568a2732577bad19.1  /home/maxeonyx/agent-harness-workspace/ah-spine-a/target/cancel-demo/repo described: 5 tools
   2.03      2.0s 0c0c0993fd06eef6.10  engineer has a new root, on claude-opus-5-5
   2.28      2.3s 0c0c0993fd06eef6.11  engineer: request sent
   3.72      3.7s 0c0c0993fd06eef6.12  engineer: first byte
   4.24      4.2s 0c0c0993fd06eef6.13  engineer's bash made {"command": "./slow.sh", "grep": "", "timeout_ms": 300000}
   4.24      4.2s 0c0c0993fd06eef6.13  engineer: responded, ToolUse, 1 calls (4 in, 7594 cached, 99 out)
   4.25      4.2s 0c0c0993fd06eef6.14  engineer's bash dispatched
   7.26  [the terminal] Ctrl-C (1)
   7.26  ^Charness-brain: SIGINT: shutting down; requests in flight have 300s to finish (again to stop now)
   7.26      7.3s 0c0c0993fd06eef6.15  brain shutting down, within 300s
   7.26  limb 0c0c0993fd06eef6.1.4: its link ended
   7.26      7.3s 0c0c0993fd06eef6.16  roles lost
   7.27  limb 0c0c0993fd06eef6.1.3: its link ended
   7.27      7.3s 0c0c0993fd06eef6.17  user lost
   7.28      7.3s 568a2732577bad19.2  engineer's bash interrupted
   7.28  limb 0c0c0993fd06eef6.4.0: its link ended
   7.28      7.3s 0c0c0993fd06eef6.19  /home/maxeonyx/agent-harness-workspace/ah-spine-a/target/cancel-demo/repo lost
   7.53  harness-brain: shut down
   7.53  [the terminal] the command exited with 0, 0.27s after the first Ctrl-C
  35.8s  left running:
          (none)
  36.1s  == C2. The same database again; Ctrl-C twice while its request waits for the first byte
   0.01      0.0s 0c0c0993fd06eef6.20  brain launched, build harness-brain 0.0.0
   0.01      0.0s 0c0c0993fd06eef6.21  roles connected
   0.01  face: https://mx-hv-mj.local:4326
   0.01      0.0s 0c0c0993fd06eef6.22  engineer: request sent
   0.81  [the terminal] Ctrl-C (1)
   0.81  ^Charness-brain: SIGINT: shutting down; requests in flight have 300s to finish (again to stop now)
   0.81      0.8s 0c0c0993fd06eef6.23  brain shutting down, within 300s
   0.81  limb 0c0c0993fd06eef6.1.4: its link ended
   0.81      0.8s 0c0c0993fd06eef6.24  roles lost
   1.01  [the terminal] Ctrl-C (2)
   1.01  ^Charness-brain: SIGINT again: stopping now
   1.01      1.0s 0c0c0993fd06eef6.25  brain shutting down, within 0s
   1.01      1.0s 0c0c0993fd06eef6.26  engineer: response abandoned
   1.01  harness-brain: shut down
   1.01  [the terminal] the command exited with 0, 0.20s after the first Ctrl-C
  37.2s  left running:
          (none)
  37.5s  == C3. Again: the request the last Ctrl-C cut is sent again, and reads the cache
   0.01      0.0s 0c0c0993fd06eef6.27  brain launched, build harness-brain 0.0.0
   0.01      0.0s 0c0c0993fd06eef6.28  roles connected
   0.01  face: https://mx-hv-mj.local:4326
   0.01      0.0s 0c0c0993fd06eef6.29  engineer: request sent
   1.53      1.5s 0c0c0993fd06eef6.30  engineer: first byte
   2.03      2.0s 0c0c0993fd06eef6.31  user connected
   2.03      2.0s 0c0c0993fd06eef6.32  /home/maxeonyx/agent-harness-workspace/ah-spine-a/target/cancel-demo/repo connected
   2.04      2.0s 90675fd1a3475034.2  user described: 1 tools
   2.04      2.0s 568a2732577bad19.3  /home/maxeonyx/agent-harness-workspace/ah-spine-a/target/cancel-demo/repo described: 5 tools
   2.90      2.9s 0c0c0993fd06eef6.35  engineer: responded, EndTurn, 0 calls (2 in, 7594 cached, 148 out)
   2.90  [the terminal] Ctrl-C (1)
   2.90  ^Charness-brain: SIGINT: shutting down; requests in flight have 300s to finish (again to stop now)
   2.90      2.9s 0c0c0993fd06eef6.36  brain shutting down, within 300s
   2.90  limb 0c0c0993fd06eef6.1.4: its link ended
   2.90      2.9s 0c0c0993fd06eef6.37  roles lost
   2.91  limb 0c0c0993fd06eef6.4.0: its link ended
   2.91      2.9s 0c0c0993fd06eef6.38  /home/maxeonyx/agent-harness-workspace/ah-spine-a/target/cancel-demo/repo lost
   2.91  limb 0c0c0993fd06eef6.1.3: its link ended
   2.91      2.9s 0c0c0993fd06eef6.39  user lost
   3.10  [the terminal] Ctrl-C (2)
   3.10  ^Charness-brain: SIGINT again: stopping now
   3.10      3.1s 0c0c0993fd06eef6.40  brain shutting down, within 0s
   3.10  harness-brain: shut down
   3.10  [the terminal] the command exited with 0, 0.20s after the first Ctrl-C
  40.6s  left running:
          (none)
```
