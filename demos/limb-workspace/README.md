# The workspace limb

The coding tools an engineer agent works with (`bash`, `read`, `write`, `edit`, `grep`) running as calls of the core model, in a scratch directory. The demo plays the brain: two agents' contexts, each with the file versions it has seen.

```sh
cargo run -q -p harness-limb-workspace --example demo
```

Run it from the repo root. It works in `demo-scratch.ignore/` there, and shows, in order:

- the tool definitions exactly as a context's root carries them;
- agent B editing a file agent A had read, so A's edit is refused with no effect until A reads again;
- a `sed` run through `bash` changing a watched file, reported after the call, so B's next edit is refused too;
- 2000 lines of test output cut to head, tail and the one line matching the agent's `grep` term `FAILED`, with the full output kept by digest;
- a command that started a tree of `sleep`s, interrupted, with every process in the tree gone afterwards;
- the limb process killed mid-command: the next instance kills the tree it left behind and answers `Recover` from its record, saying the cut-off call's outcome is unknown, not failed.
