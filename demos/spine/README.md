# spine: the brain on its database, Max's tabs as replicas, and three restarts

```sh
demos/spine/run.sh
```

It runs brains on the scripted model (no cost; each message is a script) on one database, `target/spine-demo/brain.sqlite`, on port 4398 (`PORT=` for another), and plays two of Max's tabs, his PC and his phone, with the same `harness-face-tab` the page runs as wasm. Each tab is a replica with its own log: it says records, ships them over `wss://…/link?replica=<id>` (TLS checked against the dev CA, no bypass), and reads the brain's verdict on each from its own standing, in the view the brain ships it. It takes about half a minute.

What it shows, in order:

- **1. Dictate on the phone, finish and send on the PC.** The phone says a draft beside the chat; the brain holds it, and the PC's replica shows it. The PC sends it with its own words. Both tabs show it sent: its row is gone from both replicas.
- **6. A request interrupted before its first byte.** Max sends, and at once interrupts the turn, naming it. The response is cut without a byte.
- **5. Ctrl-C mid-command.** The agent runs `sleep 8`; the brain gets SIGINT, drains and exits. The next launch, given only the database, carries the agent on: the tabs link again and ship what it hadn't included, and on a real model the first request reads the prompt cache (`cache_read`), its new tail written to the hour-long cache (`cache_write_long`).
- **5. Ctrl-C twice mid-request.** The second SIGINT stops it at once; the next launch sends the lost request again, reading the cache on a real model.
- **2. Done on a chat the brain already finished.** The PC presses Done, and the task ends. The phone, which had seen the brain only as of before, presses Done too: its record is refused, in its own standing.

The brain reads nothing from disk but its database and its flags: the roles and the workspace limb are defined in it at launch 1, and launches 2 and 3 keep them.

`MODEL=sonnet/low demos/spine/run.sh` runs every role on Sonnet at low effort instead, each message asking in words: then the requests after each restart read the prompt cache (`cache_read`), which the scripted model does not report. It costs a few cents of usage and takes about a minute.

## Real output

A run on the scripted model at 04:03 NZDT on 2026-10-18.

```
== launch 1: a new database, and a chat with Max
  brain 1 launched (pid 2317604), log target/spine-demo/launch1.log
  pc: tab 0968dd1cb484b94a linked
  phone: tab e62186900d223e0a linked
  pc: Max starts a chat in /home/maxeonyx/agent-harness-workspace/agent-harness-r2-retry-same/target/spine-demo/repo: "A demonstration chat.\nsay ready"

== 1. The phone dictates; the PC finishes the words and sends them
  the PC's replica holds the phone's draft e62186900d223e0a.1.0: "[{\"text\":\"Reply with one word:\",\"type\":\"text\"}]"
  PC: its Send is included; its replica has no draft row: sent
  phone: the draft row is gone from its replica too (as of 62b64bb8964e321e.5): sent
  agent: A demonstration chat: responded, EndTurn, 0 calls (5321 in, 0 cached, 53 out)

== 6. A request interrupted before its first byte
  pc: Max sends "slow 2000 long 40"
  the interrupt naming 62b64bb8964e321e.21.0: is included
  brain: A demonstration chat: response cut
  it had a first byte before it ended: false

== 5. Ctrl-C mid-command: the brain drains, exits, and the next launch carries on
  pc: Max sends "tool bash {\"command\": \"sleep 8 && echo slept\"}\nsay It printed what the result says."
  ** kill -INT the brain
  harness-brain: SIGINT: shutting down; requests in flight have 300s to finish (again to stop now)
  harness-brain: shut down
  brain exited: exit status: 0
  brain 2 launched (pid 2318549), log target/spine-demo/launch2.log
  pc: linked again
  phone: linked again
  agent, after the restart: A demonstration chat: responded, EndTurn, 0 calls (5561 in, 0 cached, 8 out)
  usage of 62b64bb8964e321e.36.0, the first request of this launch: {"cache_read":0,"cache_write_long":0,"cache_write_short":0,"input":5561,"output":8}

== 5. Ctrl-C twice mid-request: the brain exits at once, and the next launch sends it again
  pc: Max sends "slow 500 say Apple\\nBanana\\nCherry"
  ** kill -INT the brain
  ** kill -INT the brain
  harness-brain: SIGINT: shutting down; requests in flight have 300s to finish (again to stop now)
  harness-brain: SIGINT again: stopping now
  harness-brain: shut down
  brain exited: exit status: 0
  brain 3 launched (pid 2318811), log target/spine-demo/launch3.log
  pc: linked again
  phone: linked again
  agent, after the restart: A demonstration chat: responded, EndTurn, 0 calls (5633 in, 0 cached, 5 out)
  usage of 62b64bb8964e321e.45.0, the first request of this launch: {"cache_read":0,"cache_write_long":0,"cache_write_short":0,"input":5633,"output":5}

== 2. Done on a chat the brain already finished
  pc: Max presses Done
  PC: its Done is included
  phone: Max presses Done
  phone, which had seen the brain as of 62b64bb8964e321e.5: its Done is refused, in its own standing: fact 0 refused: addresses something already finished
  ** kill -INT the brain
  harness-brain: SIGINT: shutting down; requests in flight have 300s to finish (again to stop now)
  harness-brain: shut down
  brain exited: exit status: 0

The database: target/spine-demo/brain.sqlite
```
