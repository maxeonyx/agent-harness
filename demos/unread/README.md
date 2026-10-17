# unread: a brain opens a database a later build wrote to

```sh
demos/unread/run.sh
```

It takes a few seconds and makes no model calls. A brain launches on a fresh database (`target/unread-demo/brain.sqlite`, port 4396, `PORT=` for another) and stops. Then a record is written as a later build would write it: the brain's next record, saying a fact this build has no word for. The brain launches again on the same database.

Every record is an envelope and a payload. The envelope (its time, its clock, whether it is said or a receipt) is the substrate's, and every build reads it. The payload is the schema's, and a later build may say something this one cannot parse. Such a record is `Unread`: held, skipped, reported, never rewritten. So going back to an earlier build always opens what a later one wrote, and going forward again loses nothing. Only the store's table layout is a boundary between builds.

What it printed (17:46 NZT, 2026-10-17):

```
2. A later build writes the brain's record aff729030b654f29.6, with a fact this build has no word for:
  {"at":"aff729030b654f29.6","clock":1792211184000,"what":{"Said":{"values":[],"facts":[{"Note":"a fact only a later build knows"}]}}}

3. This build launches on that database, and stops.
  | harness-brain: skipping the record at aff729030b654f29.6: this build cannot read it: unknown variant `Note`, expected one of `Launched`, `Shutdown`, `Task`, `ModelCall`, `Call`, `Delivery`, `Change`, `Environment`, `Limb`, `Described`, `Reach`, `Face`, `Seen`, `Draft`, `Ranking`, `Focus` at line 1 column 29
  |     0.0s aff729030b654f29.7  brain launched, build 0d0bce6b359f
  ...
4. The database afterwards:
  aff729030b654f29.6  {"at":"aff729030b654f29.6","clock":1792211184000,"what":{"Said":{"values":[],"facts":[{"Note":"a fact only a l
  aff729030b654f29.7  {"at":"aff729030b654f29.7","clock":1792211184741,"what":{"Said":{"values":[],"facts":[{"Launched":{"build":"0d
  ...
OK: the later build's record is held exactly as it was written
```

The brain's next record is `.7`: the unread record's time still counts. `build 0d0bce6b359f` is the start of the digest of the brain's executable, which its `Launched` records.

The direction this protects is a build reading what a later one wrote. A build must read everything an earlier one wrote, so the facts' vocabulary only grows.
