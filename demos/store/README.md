# Store: kill a brain mid-commit, reopen it

```sh
demos/store/run.sh
```

A writer opens a brain on a fresh store at `target/store-demo/brain.sqlite`; the file mints the brain's replica id on first open. The brain launches, accepts Max's face and defines an environment, then includes one record of the face after another, each creating a task and carrying its mandate's bytes. Each include is one commit of two records: the face's record and the brain's receipt for it. Once the writer reports 300 tasks, the demo `kill -9`s it while it keeps committing.

It then reopens the brain from the file and checks, printing `OK` or `FAIL` for each:

- the replica id is the one the file minted;
- every record folds as it did when written, and none of the face's was refused;
- every task the writer reported is there, plus at most the one it was committing;
- no commit is half there: the file holds exactly the brain's two records and two per task.

It reads the newest task's mandate back by its value id through a second connection to the same file, and relaunches the brain, whose next record continues the count. The store is a plain SQLite file; the bytes of each batch are kept apart from its record, in `value`:

```sh
sqlite3 target/store-demo/brain.sqlite "SELECT replica, n, record FROM record LIMIT 4"
```
