# face-controls: Max's controls, his queue and live text, from a tab

```sh
demos/face-controls/run.sh
```

It starts a real brain run (every role on Sonnet at low effort, `--model sonnet/low`; `MODEL=haiku` for another) on the brain demo's scratch repo and mandate (`demos/brain/scratch.sh`, `demos/brain/mandate.md`), then drives it as Max, only from one tab: a replica on the face's link, `wss://…/link?replica=<id>`, played by the same `Tab` the page runs as wasm. Watch it at https://mx-hv-mj.local:4400 while it runs (another port: `PORT=…`). It takes about two minutes and ends when the root has its result. The script is `crates/brain/examples/face_controls.rs`; its files are in `target/face-controls-demo`.

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
- **Sending** takes two presses: Enter stages the words above a divider, and Enter in the empty box sends what is staged. Words sent to a task Max paused write `Pause::Off` in the same record.
- **While the task's request runs**, its visible text streams in at the foot of its log.
- **What needs Max** off screen is a mark at the screen's edge, the next in the queue haloed; a click opens it. The page does not rank the queue: ranking is the example's own `Ranking` fact.

## Real output

A run on Sonnet at low effort at 04:05 NZDT on 2026-10-18, on port 4377:

```
  brain launched (pid 2328588), log target/face-controls-demo/brain.log
  watch it at https://mx-hv-mj.local:4377
  tab: tab 91f6d8960a1afee1 linked
  tab: Max starts a chat in /home/maxeonyx/agent-harness-workspace/agent-harness-r2-retry-same/target/face-controls-demo/repo: "Build textstats with two agents in parallel.\n\nThis repository is a tiny Python package with no code yet; `README.md` says what it should become.\n\nBuild it with two agents working in parallel, in one `spawn` call:\n- a fork of yourself writes `textstats/wordcount.py` and `tests/test_wordcount.py`;\n- a fresh engineer, on the branch `palindromes` in its own clone, writes `textstats/palindromes.py` and `tests/test_palindromes.py`, commits them and pushes the branch.\n\nRead the repository first, so the fork knows what you know; give the fresh engineer everything it needs in its mandate. Each agent runs `./check.sh` before it ends its turn.\n\nWhen both have returned, merge the engineer's branch (`git merge palindromes`), run `./check.sh` yourself, fix anything small, commit everything with git, and end your turn with a short report of what each agent did.\n"
   0.9s  the root task is Build textstats with two agents in
   0.9s  == pause the root: nothing under it sends a request until it is resumed
          pause: is included
   1.0s  paused now: Build textstats with two agents in
   6.0s  5 s later: 0 requests sent meanwhile; resume it
          resume: is included
   6.0s  == park the root once both children are under way: each brings its work to a safe point and waits
          park: is included
  34.5s  Build textstats with two agents in-palindromes: parked
  34.5s  Build textstats with two agents in-wordcount: parked
          resume: is included
  34.8s  Build textstats with two agents in-palindromes: carrying on, on 'model'
  34.8s  Build textstats with two agents in-wordcount: carrying on, on 'model'
  34.8s  == interrupt both children in their next request
  34.8s  interrupt Build textstats with two agents in-palindromes while it is on 'model', naming its request 894c25c4f506913a.48.0
          interrupt: is included
  34.8s  interrupt Build textstats with two agents in-wordcount while it is on 'model', naming its request 894c25c4f506913a.47.0
          interrupt: is included
          brain: Build textstats with two agents in-palindromes interrupted
          brain: Build textstats with two agents in-wordcount interrupted
  34.8s  both wait on Max; his queue, oldest first:
          Build textstats with two agents in-wordcount: unranked
          Build textstats with two agents in-palindromes: unranked
  34.8s  == reorder the queue: the second to the top
          the ranking: is included
          Build textstats with two agents in-palindromes: placed by you
          Build textstats with two agents in-wordcount: placed by you
  34.9s  == focus the top item, Build textstats with two agents in-palindromes, and tell it to carry on
          focus: is included
          words: is included
  34.9s  the same record again, as a tab whose link dropped would resend it:
          the record is included; the database holds the delivery 91f6d8960a1afee1.10.0 1 time(s): the resend was skipped
  35.9s  == abandon the other, Build textstats with two agents in-wordcount
  tab: Max presses Abandon, with 120s to finish
          abandon: is included
  38.5s  Build textstats with two agents in-wordcount: abandoned by Max
  38.5s  == what the brain refuses, in the tab's standing
          pause the abandoned task: is refused, in its own standing: fact 0 refused: addresses something already finished
          words to the abandoned task: is refused, in its own standing: fact 0 refused: addresses something already finished
          failing a task, which is not Max's to say: is refused, in its own standing: fact 0 refused: its author may not state it
  38.6s  == live text: a streaming response, and a new link carrying on
  tab: Max presses Done
  82.3s  the first link, for 2 s of 894c25c4f506913a.87.0: 830 characters
  tab: linked again
  82.8s  a new link, until the response ended: 1067 characters, which start with the first link's text: true
  82.8s  the streamed text:
          **Result**
          
          **Did:**
          - I read the repo, then ran two agents in parallel. A fork wrote `textstats/wordcount.py` and `tests/test_wordcount.py`. A fresh engineer, on branch `palindromes`, wrote `textstats/palindromes.py` and `tests/test_palindromes.py` and pushed the branch.
          - I merged `palindromes` into `main` with no conflicts. `./check.sh` passes: 8 tests OK, style OK.
          - I committed the wordcount files as "Add wordcount".
  82.8s  == wait for the root to end its turn, and mark it done
  harness-brain: SIGINT: shutting down; requests in flight have 300s to finish (again to stop now)
  harness-brain: shut down
  84.1s  brain exited: exit status: 0
  84.1s  the brain's log of the controls, and the root's result:
    0.2s 91f6d8960a1afee1.2  Build textstats with two agents in paused
    5.3s 91f6d8960a1afee1.3  Build textstats with two agents in resumed
   12.3s 91f6d8960a1afee1.4  Build textstats with two agents in paused for now
   12.3s 894c25c4f506913a.28  Park prompt delivered to Build textstats with two agents in-wordcount
   12.3s 894c25c4f506913a.29  Park prompt delivered to Build textstats with two agents in-palindromes
   33.8s 91f6d8960a1afee1.5  Build textstats with two agents in resumed
   33.8s 894c25c4f506913a.45  Resume prompt delivered to Build textstats with two agents in-wordcount
   33.8s 894c25c4f506913a.46  Resume prompt delivered to Build textstats with two agents in-palindromes
   34.1s 91f6d8960a1afee1.6  Build textstats with two agents in-palindromes interrupted
   34.1s 91f6d8960a1afee1.7  Build textstats with two agents in-wordcount interrupted
   34.1s 894c25c4f506913a.51  Build textstats with two agents in-wordcount: response cut
   34.1s 894c25c4f506913a.52  Build textstats with two agents in-palindromes: response cut
   34.1s 91f6d8960a1afee1.8  queue ranked: 2 placed
   34.2s 91f6d8960a1afee1.9  a face changed focus
   34.2s 91f6d8960a1afee1.10  words delivered to Build textstats with two agents in-palindromes
   35.2s 91f6d8960a1afee1.11  Build textstats with two agents in-wordcount abandoned by Max, to exit within 120s
   35.2s 894c25c4f506913a.59  Tidy prompt delivered to Build textstats with two agents in-wordcount
   78.6s 91f6d8960a1afee1.15  Done prompt delivered to Build textstats with two agents in
== the task is done; its result:

**Result**

**Did:**
- I read the repo, then ran two agents in parallel. A fork wrote `textstats/wordcount.py` and `tests/test_wordcount.py`. A fresh engineer, on branch `palindromes`, wrote `textstats/palindromes.py` and `tests/test_palindromes.py` and pushed the branch.
- I merged `palindromes` into `main` with no conflicts. `./check.sh` passes: 8 tests OK, style OK.
- I committed the wordcount files as "Add wordcount".

**Learned:**
- The fork's report said Max had abandoned the task, but its files were complete and uncommitted. I kept them and did not check why it said that.
- The engineer's `cd repo-palindromes` failed because it was already in that directory. This did no harm.
- For palindromes, apostrophes are ignored when checking a word, but they stay in the returned word. The README doesn't say which way this should go.
- I never confirmed that the `palindromes` push reached the remote.

**Left undone:**
- `git push` of `main` failed because the repo has no remote or upstream configured. `main` is local only.
- The apostrophe behaviour needs Max's decision.


The database: target/face-controls-demo/brain.sqlite
```
