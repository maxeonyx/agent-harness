# face-controls: Max's controls, his queue and live text, from the face

```sh
demos/face-controls/run.sh
```

It starts a real brain run on the brain demo's scratch repo and mandate, then drives it only through the face's HTTP endpoints, the way the page does. Watch it at https://mx-hv-mj.local:4323 while it runs (another port: `PORT=4400`). It takes a few minutes and ends when the root has its result.

What it does, in order:

1. **Hold.** It holds the root. Nothing under it sends a request, and the children show "held". Then it releases the root.
2. **Interrupt.** It interrupts both children. The one that was streaming has its response abandoned, and both wait on Max: they are in his queue.
3. **The queue.** It reorders the queue, focuses the new top item, and sends it words. It then posts the same words again, as a page would after a dropped connection, and the core refuses the copy as `Exists`.
4. **Stop.** It stops the other child, which ends cancelled by Max.
5. **Refusals.** Holding the stopped task, or sending it words, is refused with the core's reason, `Finished`. Failing a task isn't Max's to author, and the core refuses it as `Author`.
6. **Live text.** It reads the root's report's visible text from `/live?model=<id>` for 2 seconds, cuts the connection, and reconnects with `Last-Event-ID` the way a browser does. The second connection carries on from the next event to the end of the request. Then it presses **Done** on the root, as Max, and its last turn writes the result.

Every control is a batch of Max's facts in the journal's own encoding, posted to `/act` as `{face, seen, batch: {values, facts}}`. Max's words are a value: the page posts their bytes to `/digest` for their id, and sends the bytes in `values` beside the fact that names them:

```json
{"Task": ["<task>", {"Drain": "ToMax"}]}
{"Task": ["<task>", {"Drain": {"Stop": "Max"}}]}
{"Task": ["<task>", {"Held": true}]}
{"Ranking": {"order": [{"task": "<task>", "why": "placed by you"}]}}
{"Focus": {"face": "<face>", "task": "<task>"}}
{"Delivery": {"id": "<new id>", "to": "<task>", "content": {"Words": "<digest of the words' bytes>"}}}
```

The brain commits the batch as Max's: the core decides what he may author, and a 409 carries its `Rejected` by name.

## On the page

- **Open a task's node** and you focus on it: that's a `Focus` fact for this page's face.
- **Interrupt**, **Hold/Release**, **Done** and **Stop** sit in the open node's header. The answer appears beside them: done, or `refused: <reason>`.
- **While the task's request runs**, its visible text streams in at the foot of its log.
- **The needs-you pill** opens the queue in ranked order. The ↑ ↓ buttons reorder it, and **Next** opens the top item.

## Real output

A run at 12:05 NZT on 2026-10-17. The demo's own lines:

```
   0.0s  the root task is orchestrator
   0.0s  == hold the root: nothing under it sends a request until it is released
          answer:  -> 200
   0.1s  held now: orchestrator
   5.1s  5 s later: 0 requests sent meanwhile; release it
          answer:  -> 200
   5.1s  == interrupt both children as soon as they exist
  17.4s  interrupt orchestrator-palindromes while it is on 'harness'
          answer:  -> 200
  17.4s  interrupt orchestrator-wordcount while it is on 'model'
          answer:  -> 200
  19.0s  both wait on Max; his queue, oldest first:
          orchestrator-palindromes: unranked
          orchestrator-wordcount: unranked
  19.0s  == reorder the queue: the second to the top
          answer:  -> 200
          orchestrator-wordcount: placed by you
          orchestrator-palindromes: placed by you
  19.1s  == focus the top item, orchestrator-wordcount, and tell it to carry on
          answer:  -> 200
          answer:  -> 200
  19.1s  the same post again, as a page whose connection dropped would retry it:
          answer: creates something that already exists -> 409
  19.2s  == stop the other, orchestrator-palindromes
          answer:  -> 200
  19.2s  orchestrator-palindromes: cancelled by Max
  19.2s  == what the core refuses
  19.2s  hold the stopped task:
          answer: addresses something already finished -> 409
  19.2s  words to the stopped task:
          answer: addresses something already finished -> 409
  19.3s  a fact that is not Max's to author (failing a task):
          answer: its author may not state it -> 409
  19.3s  == live text: the root's report as it streams, from /live
  26.0s  first connection to /live?model=e599e3419aa6c17fe40cb9ed9bc2d128, cut after 2 s: events 1 to 3
  28.9s  reconnected with Last-Event-ID: 3: then: event: end
  28.9s  the streamed text, from both connections:
          Oops, a leftover line; remove it.
  28.9s  == wait for the root to end its turn, and mark it done
  82.5s  done; the brain's own log:
```

The brain's log, trimmed to the controls:

```
    0.0s    18  orchestrator held
    5.1s    19  orchestrator released
   17.4s    38  orchestrator-palindromes interrupted
   17.4s    39  orchestrator-wordcount interrupted
   18.7s    41  orchestrator-wordcount: response abandoned
   19.0s    42  queue ranked: 2 placed
   19.1s    43  a face changed focus
   19.1s    44  words delivered to orchestrator-wordcount
   19.2s    46  orchestrator-palindromes stopped by Max
   75.9s    92  Done prompt delivered to orchestrator
```
