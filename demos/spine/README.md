# spine: the brain on its database, Max's tabs as replicas, and three restarts

```sh
demos/spine/run.sh
```

It runs real brains (Opus, on the subscription) on one database, `target/spine-demo/brain.sqlite`, on port 4398 (`PORT=` for another), and plays two of Max's tabs, his PC and his phone, with the same `harness-face-tab` the page runs as wasm. Each tab is a replica with its own log: it says records, ships them over `wss://…/link?replica=<id>` (TLS checked against the dev CA, no bypass), and reads the brain's verdict on each from its own standing, in the view the brain ships it. It takes about a minute and a half.

What it shows, in order:

- **1. Dictate on the phone, finish and send on the PC.** The phone says a draft beside the chat; the brain holds it, and the PC's replica shows it. The PC sends it with its own words. Both tabs show it sent: its row is gone from both replicas.
- **6. A request interrupted before its first byte.** Max sends, and at once interrupts the turn, naming it. The response is cut without a byte.
- **5. Ctrl-C mid-command.** The agent runs `sleep 8`; the brain gets SIGINT, drains and exits. The next launch, given only the database, carries the agent on: the tabs link again and ship what it hadn't included, and the first request reads the prompt cache (`cache_read`), its new tail written to the hour-long cache (`cache_write_long`).
- **5. Ctrl-C twice mid-request.** The second SIGINT stops it at once; the next launch sends the lost request again, reading the cache.
- **2. Done on a chat the brain already finished.** The PC presses Done, and the task ends. The phone, which had seen the brain only as of before, presses Done too: its record is refused, in its own standing.

The brain reads nothing from disk but its database: the roles and the workspace limb are defined in it at launch 1, and launches 2 and 3 are given only `--db`.

## Real output

A run at 23:40 NZDT on 2026-10-17.

```

== launch 1: a new database, and a chat with Max
  brain 1 launched (pid 1560500), log target/spine-demo/launch1.log
  pc: tab baa9df9bb1f2b483 linked
  phone: tab cfcef0ca3abbda54 linked

== 1. The phone dictates; the PC finishes the words and sends them
  the PC's replica holds the phone's draft cfcef0ca3abbda54.1.0: "[{\"text\":\"Reply with one word:\",\"type\":\"text\"}]"
  PC: its Send is included; its replica has no draft row: sent
  phone: the draft row is gone from its replica too (as of e7f3d7a56c7687fe.13): sent
  agent: chat: responded, EndTurn, 0 calls (4 in, 9652 cached, 6 out)

== 6. A request interrupted before its first byte
  pc: Max sends "Count from 1 to 40, one number per line."
  the interrupt naming e7f3d7a56c7687fe.22.0: is included
  brain: chat: response cut
  it had a first byte before it ended: false

== 5. Ctrl-C mid-command: the brain drains, exits, and the next launch carries on
  pc: Max sends "Use bash to run `sleep 8 && echo slept`, then reply with what it printed, or how it ended."
  ** kill -INT the brain
  harness-brain: SIGINT: shutting down; requests in flight have 300s to finish (again to stop now)
  harness-brain: shut down
  brain exited: exit status: 0
  brain 2 launched (pid 1560541), log target/spine-demo/launch2.log
  pc: linked again
  phone: linked again
  agent, after the restart: chat: responded, EndTurn, 0 calls (2 in, 9769 cached, 117 out)
  usage of e7f3d7a56c7687fe.37.0, the first request of this launch: {"cache_read":9769,"cache_write_long":152,"cache_write_short":0,"input":2,"output":117}

== 5. Ctrl-C twice mid-request: the brain exits at once, and the next launch sends it again
  pc: Max sends "Name three fruits, one per line."
  ** kill -INT the brain
  ** kill -INT the brain
  harness-brain: SIGINT: shutting down; requests in flight have 300s to finish (again to stop now)
  harness-brain: SIGINT again: stopping now
  harness-brain: shut down
  brain exited: exit status: 0
  brain 3 launched (pid 1560569), log target/spine-demo/launch3.log
  pc: linked again
  phone: linked again
  agent, after the restart: chat: responded, EndTurn, 0 calls (4 in, 9921 cached, 14 out)
  usage of e7f3d7a56c7687fe.54.0, the first request of this launch: {"cache_read":9921,"cache_write_long":199,"cache_write_short":0,"input":4,"output":14}

== 2. Done on a chat the brain already finished
  pc: Max presses Done
  PC: its Done is included
  phone: Max presses Done
  phone, which had seen the brain as of e7f3d7a56c7687fe.13: its Done is refused, in its own standing: fact 0 refused: addresses something already finished
  ** kill -INT the brain
  harness-brain: SIGINT: shutting down; requests in flight have 300s to finish (again to stop now)
  harness-brain: shut down
  brain exited: exit status: 0

The database: target/spine-demo/brain.sqlite
```
