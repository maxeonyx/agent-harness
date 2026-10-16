# Store: crash mid-batch, reopen, query

```sh
demos/store/run.sh
```

A writer acting as a brain appends batches to a fresh store at `target/store-demo/journal.sqlite`. Each batch is one task's whole conversation with Max: 2001 records, every record folded into the writer's `State` first, and every context entry stored before the fact that names it. After three batches commit, the demo `kill -9`s the writer halfway through appending the fourth.

It then reopens the store and folds it. The folded `State` must equal the writer's at a batch boundary, which the demo rebuilds from the same start time: batch 4 is wholly absent (or, if the kill landed after the commit, wholly present). Anything else prints `FAIL`.

Next it appends a fact naming a context entry that was never stored, and the store refuses it.

Then a face that had seen seq 2 catches up one page, and one SQL query over the journal reports tokens per task. The store is a plain SQLite file, so the same query works from the shell while a brain is writing:

```sh
sqlite3 target/store-demo/journal.sqlite
```

Ids are hex strings in the JSON, so `->>` and `json_extract` read them exactly, and they equal the `subject` column.
