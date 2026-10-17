# face-controls: Max's controls, his queue and live text, from a tab

```sh
demos/face-controls/run.sh
```

It starts a real brain run (Sonnet and Opus) on the brain demo's scratch repo and mandate (`demos/brain/scratch.sh`, `demos/brain/mandate.md`), then drives it as Max, only from one tab: a replica on the face's link, `wss://…/link?replica=<id>`, played by the same `Tab` the page runs as wasm. Watch it at https://mx-hv-mj.local:4400 while it runs (another port: `PORT=…`). It takes a minute or two and ends when the root has its result. The script is `crates/brain/examples/face_controls.rs`; its files are in `target/face-controls-demo`.

What it does, in order:

1. **Hold.** It holds the root. Nothing under it sends a request, and the root shows "held". Then it releases the root.
2. **Interrupt.** Once both children are under way, it interrupts each, naming the request it was watching. A streaming response is abandoned, and both wait on Max: they are in his queue.
3. **The queue.** It reorders the queue, focuses the new top item, and sends it words. It then ships the same record again, as a tab would after its link dropped: the brain skips it, so the record stays included and the database holds the delivery once.
4. **Stop.** It stops the other child, which takes its tidy-up turn and ends cancelled by Max.
5. **Refusals.** Holding the stopped task, or sending it words, is refused as `Finished`. Failing a task isn't Max's to say, and is refused as `Author`. Each refusal is in band, in the tab's own standing.
6. **Live text.** It presses **Done** on the root once it waits on Max, and reads a streaming response's visible text, which arrives as `Down::Live` on the same link, for 2 seconds. Then it links again: the new link gets the response's text again from the start and carries on to its end.

Every control is one record said at the tab: a `harness_core::fact::Batch` of Max's facts, with the values they name. An id a fact creates is minted from the record's own time (`tab.next().event(0)`). There is no reply: the brain's verdict arrives in the tab's standing, `tab.status(n)`, as included or refused with the core's reason.

```rust
Fact::Task(task, TaskEvent::Held(true))
Fact::Task(task, TaskEvent::Drain(Drain::ToMax { turn }))       // turn: the request Max watched
Fact::Task(task, TaskEvent::Drain(Drain::Stop { by: Stopper::Max, within: 120_000 }))
Fact::Ranking(Ranking { order: vec![Ranked { task, why: "placed by you".into() }] })
Fact::Focus(Focus { task: Some(task) })
Fact::Delivery(Delivery { id, to: task, content: Content::Words(words.id()), read: Read::Next })
Fact::Delivery(Delivery { id, to: root, content: Content::Prompt(Prompt::Done), read: Read::Next })
```

## On the page

- **Open a task's node** and you focus on it: that's a `Focus` fact from this tab.
- **The controls** sit beside the open node's box as small icons, drawn from one list of the task's actions: ‖ **Pause** (`Held(true)`) while it works; once paused, ■ **Stop** (for now the interrupt, `Drain::ToMax`) and ▶ **Resume** (`Held(false)`); ✕ **Abandon** (`Drain::Stop`, within 120 s), then **End now** (within 0); ✓ **Done** in a chat. Abandon, End now and Done ask once more. A refusal shows under the box. Hovering a node shows the same actions; right-clicking it shows them as a menu.
- **Sending** takes two presses, staged then sent: Enter, Enter for **Send**, Alt+Enter twice for **Queue** (at turn end). Words sent to a task Max paused write `Held(false)` in the same record.
- **While the task's request runs**, its visible text streams in at the foot of its log.
- **The needs-you pill** opens the queue in ranked order. The ↑ ↓ buttons reorder it, and **Next** opens the top item.

## Real output

A run at 16:40 NZT on 2026-10-17, its streamed text and the root's result trimmed:

```
  brain launched (pid 1301242), log target/face-controls-demo/brain.log
  watch it at https://mx-hv-mj.local:4400
  tab: tab 314c619f5e8c5b04 linked
   0.1s  the root task is orchestrator
   0.1s  == hold the root: nothing under it sends a request until it is released
          hold: is included
   2.2s  held now: orchestrator
   7.2s  5 s later: 0 requests sent meanwhile; release it
          release: is included
   7.2s  == interrupt both children as soon as they are under way
  16.8s  interrupt orchestrator-palindromes while it is on 'model', naming its request 42c0c49ceafa7760.27.0
          interrupt: is included
  16.8s  interrupt orchestrator-wordcount while it is on 'model', naming its request 42c0c49ceafa7760.23.0
          interrupt: is included
          brain: orchestrator-palindromes interrupted
          brain: orchestrator-wordcount interrupted
          brain: orchestrator-wordcount: response abandoned
          brain: orchestrator-palindromes: response abandoned
  16.8s  both wait on Max; his queue, oldest first:
          orchestrator-wordcount: unranked
          orchestrator-palindromes: unranked
  16.8s  == reorder the queue: the second to the top
          the ranking: is included
          orchestrator-palindromes: placed by you
          orchestrator-wordcount: placed by you
  16.9s  == focus the top item, orchestrator-palindromes, and tell it to carry on
          focus: is included
          words: is included
  16.9s  the same record again, as a tab whose link dropped would resend it:
          the record is included; the database holds the delivery 314c619f5e8c5b04.7.0 1 time(s): the resend was skipped
  18.0s  == stop the other, orchestrator-wordcount
  tab: Max presses Stop, with 120s to finish
          stop: is included
  23.3s  orchestrator-wordcount: cancelled by Max
  23.3s  == what the brain refuses, in the tab's standing
          hold the stopped task: is refused, in its own standing: fact 0 refused: addresses something already finished
          words to the stopped task: is refused, in its own standing: fact 0 refused: addresses something already finished
          failing a task, which is not Max's to say: is refused, in its own standing: fact 0 refused: its author may not state it
  23.4s  == live text: a streaming response, and a new link carrying on
  tab: Max presses Done
  88.8s  the first link, for 2 s of 42c0c49ceafa7760.81.0: 193 characters
  tab: linked again
  93.3s  a new link, until the response ended: 1819 characters, which start with the first link's text: true
  93.3s  the streamed text:
          Only half the package is built. `textstats/palindromes.py` and its tests are on `main`, and `./check.sh` passes. `textstats/wordcount.py` and its tests don't exist, because you cancelled the agent writing them.

          **What I did:**
          - I read the README and `check.sh`, then started both agents in one `spawn` call:
            - a fork of me to write wordcount;
            - a fresh engineer on the branch `palindromes`, given the full palindromes spec.
  93.3s  == wait for the root to end its turn, and mark it done
  harness-brain: SIGINT: shutting down; requests in flight have 300s to finish (again to stop now)
  harness-brain: shut down
  94.7s  brain exited: exit status: 0
  94.7s  the brain's log of the controls, and the root's result:
    0.1s 314c619f5e8c5b04.1  orchestrator held
    7.2s 314c619f5e8c5b04.2  orchestrator released
   16.8s 314c619f5e8c5b04.3  orchestrator-palindromes interrupted
   16.8s 314c619f5e8c5b04.4  orchestrator-wordcount interrupted
   16.8s 42c0c49ceafa7760.30  orchestrator-wordcount: response abandoned
   16.8s 42c0c49ceafa7760.31  orchestrator-palindromes: response abandoned
   16.8s 314c619f5e8c5b04.5  queue ranked: 2 placed
   16.9s 314c619f5e8c5b04.6  a face changed focus
   16.9s 314c619f5e8c5b04.7  words delivered to orchestrator-palindromes
   17.9s 314c619f5e8c5b04.8  orchestrator-wordcount stopped by Max, to exit within 120s
   17.9s 42c0c49ceafa7760.37  Stopping prompt delivered to orchestrator-wordcount
   85.1s 314c619f5e8c5b04.12  Done prompt delivered to orchestrator
== the task is done; its result:

Only half the package is built. `textstats/palindromes.py` and its tests are on `main`, and `./check.sh` passes. `textstats/wordcount.py` and its tests don't exist, because you cancelled the agent writing them.
…
```

Each line of the brain's log names the record it folded: `314c619f5e8c5b04.N` is the tab's Nth record, and `42c0c49ceafa7760.N` the brain's own.
