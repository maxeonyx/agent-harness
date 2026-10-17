# unread: a brain opens a database a later build wrote to

```sh
demos/unread/run.sh
```

It takes a few seconds and makes no model calls. A brain launches on a fresh database (the test brain `unread`'s, `.drive.ignore/unread/brain.sqlite`, started and stopped with `dev/drive.mjs`) and stops. Then a record is written as a later build would write it: the brain's next record, saying a fact this build has no word for. The brain launches again on the same database.

Every record is an envelope and a payload. The envelope (its time, its clock, whether it is said or a receipt) is the substrate's, and every build reads it. The payload is the schema's, and a later build may say something this one cannot parse. Such a record is `Unread`: held, skipped, reported, never rewritten. So going back to an earlier build always opens what a later one wrote, and going forward again loses nothing. Only the store's table layout is a boundary between builds.

What it printed (04:35 NZDT, 2026-10-18):

```
2. A later build writes the brain's record 9b7455a31a67b526.5, with a fact this build has no word for:
  {"at":"9b7455a31a67b526.5","clock":1792251268000,"what":{"Said":{"values":[],"facts":[{"Note":"a fact only a later build knows"}]}}}

3. This build launches on that database, and stops.
  | harness-brain e2619f6d4c85+dirty
  | harness-brain: skipping the record at 9b7455a31a67b526.5: this build cannot read it: unknown variant `Note`, expected one of `Launched`, `Shutdown`, `Task`, ...
  |     0.0s 9b7455a31a67b526.6  brain launched, build 1d0f8bd80eb6
  | face: https://mx-hv-mj.local:4485
  | harness-brain: SIGTERM: shutting down; requests in flight have 300s to finish (again to stop now)
  |     0.5s 9b7455a31a67b526.7  brain shutting down, within 300s
  | harness-brain: shut down

4. The database afterwards:
  9b7455a31a67b526.5  {"at":"9b7455a31a67b526.5","clock":1792251268000,"what":{"Said":{"values":[],"facts":[{"Note":"a fact only a l
  9b7455a31a67b526.6  {"at":"9b7455a31a67b526.6","clock":1792251268756,"what":{"Said":{"values":[],"facts":[{"Launched":{"build":"1d
  9b7455a31a67b526.7  {"at":"9b7455a31a67b526.7","clock":1792251269264,"what":{"Said":{"values":[],"facts":[{"Shutdown":{"within":30
OK: the later build's record is held exactly as it was written
```

The brain's next record is `.6`: the unread record's time still counts. `build 1d0f8bd80eb6` is the start of the digest of the brain's executable, which its `Launched` records.

The direction this protects is a build reading what a later one wrote. A build must read everything an earlier one wrote, so the facts' vocabulary only grows.
