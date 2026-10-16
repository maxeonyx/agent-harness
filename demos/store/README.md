# Store: crash mid-batch, reopen, query

```sh
demos/store/run.sh
```

A writer acting as a brain appends batches to a fresh store at `target/store-demo/journal.sqlite`. Each batch is one task's whole conversation with Max: 2001 records, every record folded into the writer's `State` first, and every context entry stored before the fact that names it. After three batches commit, the demo `kill -9`s the writer halfway through appending the fourth.

It then reopens the store and folds it. The fold must end exactly at a batch boundary, with the same state digest the writer had there: batch 4 is wholly absent (or, if the kill landed after the commit, wholly present). Anything else prints `FAIL`.

Then a face that had seen seq 2 catches up one page, and one SQL query over the journal reports tokens per task. The store is a plain SQLite file, so the same query works from the shell while a brain is writing:

```sh
sqlite3 target/store-demo/journal.sqlite
```

Read ids out of a fact with `->` (exact text), not `json_extract` or `->>`: ids are 128-bit, and SQLite reads them as lossy reals.
