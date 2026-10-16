# fork-framing spike

Does a forked child do only its own part and end its turn, and does it read its parent's prompt cache while doing so? This runs forks on the real model, on Max's subscription, and scores what each fork *did*.

```
cargo run -q --release -- run [--models claude-sonnet-5,claude-opus-5-5] [--framings NAME,...] [--depths 1,2,3] \
  [--trunks K] [--trials N] [--jobs J] [--rounds R] [--out DIR]
cargo run -q --release -- resume RUN_DIR [--models ..] [--framings ..] [--depths ..] [--trials N] [--jobs J] [--rounds R]
cargo run -q --release -- report RUN_DIR...
```

A run writes `trunks.json`, `splits.jsonl` and `trials.jsonl` to `DIR` (default `runs.ignore/<unix time>`), then prints the tables. `resume` runs the trials of a run's recorded trunks that `trials.jsonl` lacks, appending to it, so a run cut off by a usage limit continues without rebuilding its trunks. The token is read from `~/.claude/.credentials.json` at runtime and never refreshed or printed; a billing claim other than a plan window stops the run.

## What runs

The work is a field guide whose outline nests three deep (`src/scenario.rs`). The root owns A and splits it with `spawn` into A.1 (composite), A.2 and A.3. A.1's owner splits it into A.1.1 (composite), A.1.2 and A.1.3; A.1.1's owner splits it into three species pages. Each composite's owner writes its index once its children return. Every fork inherits the whole outline, every sibling's page has the same shape as its own, and the index is right there, so carrying on is always tempting.

A trunk is one real chain of these splits, made by the model, with each composite fork started under the framing being tested. A trial forks the middle sibling at depth 1, 2 or 3 (A.2, A.1.2, A.1.1.2) and runs it on stub tools until it ends its turn or `--rounds` pass.

- **In lane**: it wrote its own page and nothing else, and spawned nothing.
- **Stopped**: its last response ended with `end_turn`.
- **Read parent's cache**: before each trial the parent's spawning request is resent with `max_tokens` 1, which warms its cache entry and gives its cacheable size (the *parent prefix*). The fork read its parent's cache if its first request's `cache_read_input_tokens` is at least that. Every request has one breakpoint, on its last block. Trials recorded before this measurement existed have no prefix and are left out of that column.
- **Split as asked**: for the composite forks while building trunks, how often the response spawned exactly its children and nothing else.

## Framings

Each framing (`src/framing.rs`) is a point on five axes: fork at the parent's response or rewind to its request; what the fork's own spawn returns; what the other spawns return; an explicit identity break; a reminder after each tool round. The mandate wording is shared.

| name | fork at | own spawn returns | other spawns return | identity | reminder |
|---|---|---|---|---|---|
| bare | response | the parent's mandate text | `Spawned A.3.` | | |
| user (design A) | response | `Forked a child for A.2.`, then the mandate as user text | `Forked a child for A.3.` | | |
| user-remind | response | as user | as user | | yes |
| tool | response | the mandate | `Not yours: A.3 is being done by another fork...` | | |
| tool-remind (design B) | response | the mandate | `Not yours: ...` | | yes |
| tool-neutral | response | the mandate | `Forked a child for A.3.` | | yes |
| identity | response | identity break + mandate | `Not yours: ...` | yes | yes |
| rewind | request | (no spawns seen) identity break + mandate appended as user text | | yes | yes |
