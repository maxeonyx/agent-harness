# The workspace limb

The coding tools an engineer agent works with (`bash`, `read`, `write`, `edit`, `grep`) running as calls of the core model, in a scratch directory. The demo plays the brain: it keeps the limb's rows (a call row and an input value row per call, and a watch row), ships them to the limb as views, and takes the limb's records by advancing its standing, dropping a call's rows once it has the call's settlement. Two agents' contexts each carry the file versions they have seen.

```sh
cargo run -q -p harness-limb-workspace --example demo
```

Run it from the repo root. It works in `demo-scratch.ignore/` there, and shows, in order:

- the tool definitions exactly as a context's root carries them, and the instructions beside them, which name the directory;
- every write arriving as one record: the settlement, its output's bytes, and a `Change` naming the call that wrote the file;
- agent B editing a file agent A had read, so A's edit is refused with no effect until A reads again;
- a `sed` run through `bash` changing a file the watch row names, reported in the command's settlement record as changed by it, so B's next edit is refused too;
- 2000 lines of test output cut to head, tail and the one line matching the agent's `grep` term `FAILED`, with the full output as a value in the same record;
- a command that started a tree of `sleep`s, stopped by giving its call row a `stopping` deadline, with every process in the tree gone afterwards;
- a limb instance in a child process killed mid-command, before the brain took what it said. The next instance ends the tree it left behind, then is shipped the brain's whole view: the quick call that had settled before the kill arrives as the record already in the limb's log, not run again (`once.log` has one line), and the cut-off command settles as `Unknown`, not failed, because `bash` may not be rerun.
