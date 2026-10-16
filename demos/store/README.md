# Store: crash mid-commit, reopen, query

```sh
demos/store/run.sh
```

A writer acting as a brain commits batches through the core's `Journal` to a fresh store at `target/store-demo/journal.sqlite`. Each batch is 500 one-turn tasks, 2000 records, with the bytes they name (each question and answer) travelling in the batch; the system prompt, tools and request parameters are stored once, by the first batch, and named by every later one. The journal folds each batch first and then hands it to the store. After three batches commit, the demo `kill -9`s the writer halfway through the store's append of the fourth.

It then reopens the journal and folds it. The folded `State` must equal the writer's at a batch boundary, which the demo rebuilds from the same start time on a store in memory: batch 4 is wholly absent (or, if the kill landed after the commit, wholly present). Anything else prints `FAIL`.

Next it commits a fact naming a value that was never stored, and the journal refuses it.

Then a face that had seen seq 2 catches up one page, reading a response's bytes from the value store, and one SQL query over the journal reports the five tasks with the most output tokens. The store is a plain SQLite file, so the same query works from the shell while a brain is writing:

```sh
sqlite3 target/store-demo/journal.sqlite
```

Ids are hex strings in the JSON, so `->>` and `json_extract` read them exactly, and they equal the `subject` column.
