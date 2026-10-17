# Substrate: two devices, one brain, one data flow

```sh
demos/substrate/run.sh
```

The brain, Max's PC and his phone are three replicas, each with its own storage (in memory, cloned to simulate a crash). Each device writes its own records first and ships them up; the brain decides them in its own order and ships each device the rows it shows, with its standing: which of the device's records the brain has included, and which it refused and why. Nothing else crosses a link: no request, no reply, no acknowledgement.

What it shows, in order:

1. The brain accepts the two tabs as Max's faces. Who said a record is derived from where it came from.
2. The PC creates a task with an id it did not mint. The brain refuses it in band: the refusal sits in the PC's standing, beside the record, and the PC reads it off the view it shows. Then it creates the task properly.
3. The PC types a draft offline. Every pause is a record, but only the newest text waits to ship. The phone, once both sync, shows the PC's draft.
4. Both devices press Send on the same draft at once, the phone with extra words. The brain takes the PC's first: it delivers. The phone's Send opens a new draft holding its words, named by that Send. Both devices show exactly the same thing. The delivery says it was by Max at the PC, who had seen the brain's view as of a given time.
5. Both press Stop at once. One stops the session; the other is refused in band (already finished), and the brain still holds it as a fact.
6. The phone sends its new draft to the finished session. Nothing is refused and nothing is lost: the draft holds its words and says why it was not delivered.
7. A hundred clicks between sessions add two records to the brain, since only the newest focus per face is kept.
8. Streamed tokens ride the same link, take no time and are never stored.
9. The brain crashes and restarts from what it stored. Its replayed state equals its state before the crash. A record the PC sends again is skipped, and the PC reconnects to a whole view.
10. The phone's storage is wiped and it opens with its old replica id. The brain holds more of the phone than the phone does, so the phone continues as a new replica instead of reusing times the brain already holds.

Anything wrong fails an assertion.
