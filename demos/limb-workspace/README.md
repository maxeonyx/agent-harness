# The workspace limb

The coding tools an engineer agent works with (`bash`, `read`, `write`, `edit`, `grep`) running as calls of the core model, in a scratch directory. The demo plays the brain: two agents' contexts, each with the file versions it has seen.

```sh
cargo run -q -p harness-limb-workspace --example demo
```

Run it from the repo root. It works in `demo-scratch.ignore/` there, and shows, in order:

- the tool definitions exactly as a context's root carries them, and the instructions beside them, which name the directory;
- every write arriving as one batch of journal facts: the settlement, its output's bytes, and a `Change` naming the call that wrote the file;
- agent B editing a file agent A had read, so A's edit is refused with no effect until A reads again;
- a `sed` run through `bash` changing a watched file, reported in the command's settlement as changed by it, so B's next edit is refused too;
- 2000 lines of test output cut to head, tail and the one line matching the agent's `grep` term `FAILED`, with the full output sent as a value beside the settlement;
- a command that started a tree of `sleep`s, interrupted, with every process in the tree gone afterwards;
- the limb process killed mid-command: the next instance kills the tree it left behind, then answers the calls the brain sends again after reconnecting from its record. A call that had settled gets the same batch again (which the journal refuses as a repeat). The cut-off command settles as `Unknown`, not failed, because `bash` may not be rerun. A `Stop` for a call it has no record of settles as `Unknown` too.
