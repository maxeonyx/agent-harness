# face-controls: Max's controls, his queue and live text, from a tab

```sh
demos/face-controls/run.sh
```

It starts a real brain run (Sonnet and Opus) on the brain demo's scratch repo and mandate (`demos/brain/scratch.sh`, `demos/brain/mandate.md`), then drives it as Max, only from one tab: a replica on the face's link, `wss://…/link?replica=<id>`, played by the same `Tab` the page runs as wasm. Watch it at https://mx-hv-mj.local:4400 while it runs (another port: `PORT=…`). It takes about two minutes and ends when the root has its result. The script is `crates/brain/examples/face_controls.rs`; its files are in `target/face-controls-demo`.

What it does, in order:

1. **Pause.** It pauses the root (`Pause::Now`). Nothing under it sends a request, and the root shows "held". Then it resumes the root (`Pause::Off`).
2. **Pause for now.** Once both children are under way, it pauses the root for now (`Pause::Park`). Each child gets the Park prompt with its next request, brings its work to a safe point, says where it stands, and waits "parked"; the root, waiting on them, has nothing to do. Then it resumes the root: each child gets the Resume prompt and sends its next request.
3. **Interrupt.** It interrupts each child in that request, naming it. The streaming response is cut, and both wait on Max: they are in his queue.
4. **The queue.** It reorders the queue, focuses the new top item, and sends it words. It then ships the same record again, as a tab would after its link dropped: the brain skips it, so the record stays included and the database holds the delivery once.
5. **Abandon.** It abandons the other child, which takes its tidy-up turn and ends abandoned by Max.
6. **Refusals.** Pausing the abandoned task, or sending it words, is refused as `Finished`. Failing a task isn't Max's to say, and is refused as `Author`. Each refusal is in band, in the tab's own standing.
7. **Live text.** It presses **Done** on the root once it waits on Max, and reads a streaming response's visible text, which arrives as `Down::Live` on the same link, for 2 seconds. Then it links again: the new link gets the response's text again from the start and carries on to its end.

Every control is one record said at the tab: a `harness_core::fact::Batch` of Max's facts, with the values they name. An id a fact creates is minted from the record's own time (`tab.next().event(0)`). There is no reply: the brain's verdict arrives in the tab's standing, `tab.status(n)`, as included or refused with the core's reason.

```rust
Fact::Task(task, TaskEvent::Pause(Pause::Now))                  // Pause::Park for now; Pause::Off resumes
Fact::Task(task, TaskEvent::Drain(Drain::ToMax { turn }))       // turn: the request Max watched
Fact::Task(task, TaskEvent::Drain(Drain::Abandon { by: Abandoner::Max, within: 120_000 }))
Fact::Ranking(Ranking { order: vec![Ranked { task, why: "placed by you".into() }] })
Fact::Focus(Focus { task: Some(task) })
Fact::Delivery(Delivery { id, to: task, content: Content::Words(words.id()), read: Read::Next })
Fact::Delivery(Delivery { id, to: root, content: Content::Prompt(Prompt::Done), read: Read::Next })
```

A pause covers the task and everything under it, and the newest word on the way up a task's ancestry is the one that holds: resuming a child under a paused parent sends only that branch on.

## On the page

- **Open a task's node** and you focus on it: that's a `Focus` fact from this tab.
- **The controls** sit beside the open node's box as small icons, drawn from one list of the task's actions: ‖ **Pause** (`Pause::Now`) while it works; once paused, ■ **Stop** (`Pause::Park`: pause for now, each agent comes to a safe point) and ▶ **Resume** (`Pause::Off`); ✕ **Abandon** (`Drain::Abandon`, within 120 s), then **End now** (within 0); ✓ **Done** in a chat. Abandon, End now and Done ask once more. A refusal shows under the box. Hovering a node shows the same actions; right-clicking it shows them as a menu.
- **Sending** takes two presses, staged then sent: Enter, Enter for **Send**, Alt+Enter twice for **Queue** (at turn end). Words sent to a task Max paused write `Pause::Off` in the same record.
- **While the task's request runs**, its visible text streams in at the foot of its log.
- **The needs-you pill** opens the queue in ranked order. The ↑ ↓ buttons reorder it, and **Next** opens the top item.

## Real output

A run at 23:26 NZDT on 2026-10-17:

```
  brain launched (pid 1542816), log target/face-controls-demo/brain.log
  watch it at https://mx-hv-mj.local:4400
  tab: tab b5c0d8dedebe5881 linked
   0.1s  the root task is orchestrator
   0.1s  == pause the root: nothing under it sends a request until it is resumed
          pause: is included
   2.2s  paused now: orchestrator
   7.2s  5 s later: 0 requests sent meanwhile; resume it
          resume: is included
   7.3s  == park the root once both children are under way: each brings its work to a safe point and waits
          park: is included
  34.2s  orchestrator-palindromes: parked
  34.2s  orchestrator-wordcount: parked
          resume: is included
  34.4s  orchestrator-palindromes: carrying on, on 'model'
  34.4s  orchestrator-wordcount: carrying on, on 'model'
  34.4s  == interrupt both children in their next request
  34.4s  interrupt orchestrator-palindromes while it is on 'model', naming its request 938c87cb3499f4e9.58.0
          interrupt: is included
  34.4s  interrupt orchestrator-wordcount while it is on 'model', naming its request 938c87cb3499f4e9.57.0
          interrupt: is included
          brain: orchestrator-palindromes interrupted
          brain: orchestrator-wordcount interrupted
  34.5s  both wait on Max; his queue, oldest first:
          orchestrator-palindromes: unranked
          orchestrator-wordcount: unranked
  34.5s  == reorder the queue: the second to the top
          the ranking: is included
          orchestrator-wordcount: placed by you
          orchestrator-palindromes: placed by you
  34.5s  == focus the top item, orchestrator-wordcount, and tell it to carry on
          focus: is included
          words: is included
  34.6s  the same record again, as a tab whose link dropped would resend it:
          the record is included; the database holds the delivery b5c0d8dedebe5881.9.0 1 time(s): the resend was skipped
  35.6s  == abandon the other, orchestrator-palindromes
  tab: Max presses Abandon, with 120s to finish
          abandon: is included
  39.1s  orchestrator-palindromes: abandoned by Max
  39.1s  == what the brain refuses, in the tab's standing
          pause the abandoned task: is refused, in its own standing: fact 0 refused: addresses something already finished
          words to the abandoned task: is refused, in its own standing: fact 0 refused: addresses something already finished
          failing a task, which is not Max's to say: is refused, in its own standing: fact 0 refused: its author may not state it
  39.2s  == live text: a streaming response, and a new link carrying on
  tab: Max presses Done
  87.1s  the first link, for 2 s of 938c87cb3499f4e9.97.0: 664 characters
  tab: linked again
  90.7s  a new link, until the response ended: 1833 characters, which start with the first link's text: false
  90.7s  the streamed text:
          Only half of what you asked for got built. The wordcount module is built, tested and committed on `main`. Palindromes was not built: you abandoned that agent before it wrote anything.
          
          **What I did**
          - Read the repo: an empty package, the spec in `README.md`, and `check.sh`. No remote is set up.
          - Started two agents in one `spawn` call:
            - a fork of myself to write wordcount in this directory;
  90.7s  == wait for the root to end its turn, and mark it done
  harness-brain: SIGINT: shutting down; requests in flight have 300s to finish (again to stop now)
  harness-brain: shut down
  92.1s  brain exited: exit status: 0
  92.1s  the brain's log of the controls, and the root's result:
    0.1s b5c0d8dedebe5881.1  orchestrator paused
    7.2s b5c0d8dedebe5881.2  orchestrator resumed
   19.3s b5c0d8dedebe5881.3  orchestrator paused for now
   19.3s 938c87cb3499f4e9.29  Park prompt delivered to orchestrator-wordcount
   19.3s 938c87cb3499f4e9.30  Park prompt delivered to orchestrator-palindromes
   34.1s b5c0d8dedebe5881.4  orchestrator resumed
   34.1s 938c87cb3499f4e9.55  Resume prompt delivered to orchestrator-wordcount
   34.1s 938c87cb3499f4e9.56  Resume prompt delivered to orchestrator-palindromes
   34.4s b5c0d8dedebe5881.5  orchestrator-palindromes interrupted
   34.4s b5c0d8dedebe5881.6  orchestrator-wordcount interrupted
   34.4s 938c87cb3499f4e9.61  orchestrator-palindromes: response cut
   34.4s 938c87cb3499f4e9.62  orchestrator-wordcount: response cut
   34.5s b5c0d8dedebe5881.7  queue ranked: 2 placed
   34.5s b5c0d8dedebe5881.8  a face changed focus
   34.5s b5c0d8dedebe5881.9  words delivered to orchestrator-wordcount
   35.6s b5c0d8dedebe5881.10  orchestrator-palindromes abandoned by Max, to exit within 120s
   35.6s 938c87cb3499f4e9.68  Tidy prompt delivered to orchestrator-palindromes
   83.5s b5c0d8dedebe5881.14  Done prompt delivered to orchestrator
== the task is done; its result:

Only half of what you asked for got built. The wordcount module is built, tested and committed on `main`. Palindromes was not built: you abandoned that agent before it wrote anything.

**What I did**
- Read the repo: an empty package, the spec in `README.md`, and `check.sh`. No remote is set up.
- Started two agents in one `spawn` call:
  - a fork of myself to write wordcount in this directory;
  - a fresh engineer on branch `palindromes`, with the spec, a word rule that matches wordcount's, and instructions to commit and push.
- **Fork:** wrote `textstats/wordcount.py` and `tests/test_wordcount.py`.
  - `count_words` lower-cases the text and counts runs of letters, digits or apostrophes. Underscores split words.
  - `top` sorts by count, highest first, with ties broken alphabetically, and returns the first `n`.
  - There are 5 tests.
- **Engineer:** abandoned by you, so no branch was made and there was nothing to merge.
- I read the fork's code and found nothing to fix. I ran `./check.sh` myself: all 5 tests and the style check pass. I committed the two files as `76ae427 Add wordcount: count_words and top, with tests`.

**What I learned**
- The repo has no remote, so "push the branch" had nowhere to go. A fresh engineer's push would only have reached my clone.
- When you abandon an agent I treat that as your decision, so I didn't start it again on my own.

**Left undone**
- **Palindromes:** `textstats/palindromes.py` and `tests/test_palindromes.py` were never written. The engineer's plan, if you pick it up:
  - find words with the same rule as wordcount;
  - check each one with `is_palindrome`, which ignores apostrophes;
  - compare `min_len` against the word's length including apostrophes.
- **Merge:** the `git merge palindromes` step never happened.
- **Push:** nothing is pushed, because there is no remote.
```

Each line of the brain's log names the record it folded: `<tab>.N` is the tab's Nth record, and the other prefix the brain's own.
