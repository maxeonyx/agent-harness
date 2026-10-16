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

## Results (run `full1`, 2026-10-16/17 NZT, 912 trials)

Rows missing for Sonnet at depth 2 and 3 under `tool`, `tool-remind` and `tool-neutral` are missing because no A.1 fork under those framings ever split A.1 as asked (0/18 each): that is the same failure one level up. "Read parent's cache" counts only trials with the warm-request measurement.

| framing | model | depth | n | in lane | stopped | both | read parent's cache | parent prefix (mean) | uncached input (mean) |
|---|---|---|---|---|---|---|---|---|---|
| bare | claude-opus-5-5 | 1 | 24 | 0/24 (0%) | 24/24 (100%) | 0/24 (0%) | 20/20 | 2827 | 575 |
| bare | claude-sonnet-5 | 1 | 24 | 0/24 (0%) | 24/24 (100%) | 0/24 (0%) | 12/12 | 2895 | 1768 |
| user | claude-opus-5-5 | 1 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 20/20 | 2827 | 598 |
| user | claude-opus-5-5 | 2 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 20/20 | 5481 | 673 |
| user | claude-opus-5-5 | 3 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 20/20 | 7150 | 391 |
| user | claude-sonnet-5 | 1 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 12/12 | 2895 | 1266 |
| user | claude-sonnet-5 | 2 | 24 | 23/24 (96%) | 24/24 (100%) | 23/24 (96%) | 12/12 | 5889 | 2430 |
| user | claude-sonnet-5 | 3 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 12/12 | 8903 | 3836 |
| user-remind | claude-opus-5-5 | 1 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 20/20 | 2827 | 4 |
| user-remind | claude-opus-5-5 | 2 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 20/20 | 5481 | 605 |
| user-remind | claude-opus-5-5 | 3 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 20/20 | 7190 | 408 |
| user-remind | claude-sonnet-5 | 1 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 12/12 | 2895 | 2 |
| user-remind | claude-sonnet-5 | 2 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 12/12 | 5889 | 1331 |
| user-remind | claude-sonnet-5 | 3 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 12/12 | 8649 | 2785 |
| tool | claude-opus-5-5 | 1 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 20/20 | 2827 | 702 |
| tool | claude-opus-5-5 | 2 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 20/20 | 5507 | 554 |
| tool | claude-opus-5-5 | 3 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 20/20 | 7394 | 547 |
| tool | claude-sonnet-5 | 1 | 24 | 0/24 (0%) | 19/24 (79%) | 0/24 (0%) | 12/12 | 2895 | 1067 |
| tool-remind | claude-opus-5-5 | 1 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 20/20 | 2827 | 2 |
| tool-remind | claude-opus-5-5 | 2 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 21/21 | 5511 | 379 |
| tool-remind | claude-opus-5-5 | 3 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 20/20 | 7271 | 432 |
| tool-remind | claude-sonnet-5 | 1 | 24 | 0/24 (0%) | 22/24 (92%) | 0/24 (0%) | 12/12 | 2895 | 2 |
| tool-neutral | claude-opus-5-5 | 1 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 21/21 | 2827 | 495 |
| tool-neutral | claude-opus-5-5 | 2 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 24/24 | 5481 | 278 |
| tool-neutral | claude-opus-5-5 | 3 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 24/24 | 7262 | 186 |
| tool-neutral | claude-sonnet-5 | 1 | 24 | 0/24 (0%) | 24/24 (100%) | 0/24 (0%) | 12/12 | 2895 | 831 |
| identity | claude-opus-5-5 | 1 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 24/24 | 2827 | 714 |
| identity | claude-opus-5-5 | 2 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 24/24 | 5575 | 215 |
| identity | claude-opus-5-5 | 3 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 24/24 | 7382 | 188 |
| identity | claude-sonnet-5 | 1 | 24 | 14/24 (58%) | 24/24 (100%) | 14/24 (58%) | 12/12 | 2895 | 741 |
| identity | claude-sonnet-5 | 2 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 12/12 | 5971 | 1481 |
| identity | claude-sonnet-5 | 3 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 12/12 | 10504 | 1534 |
| rewind | claude-opus-5-5 | 1 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 24/24 | 2827 | 102 |
| rewind | claude-opus-5-5 | 2 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 24/24 | 3584 | 121 |
| rewind | claude-opus-5-5 | 3 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 24/24 | 4263 | 135 |
| rewind | claude-sonnet-5 | 1 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 12/12 | 2895 | 329 |
| rewind | claude-sonnet-5 | 2 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 12/12 | 3757 | 308 |
| rewind | claude-sonnet-5 | 3 | 24 | 24/24 (100%) | 24/24 (100%) | 24/24 (100%) | 12/12 | 4782 | 536 |

| composite fork | framing | model | split as asked |
|---|---|---|---|
| A.1 | bare | claude-opus-5-5 | 0/18 |
| A.1 | bare | claude-sonnet-5 | 0/18 |
| A.1 | user | claude-opus-5-5 | 3/3 |
| A.1.1 | user | claude-opus-5-5 | 3/3 |
| A.1 | user | claude-sonnet-5 | 3/3 |
| A.1.1 | user | claude-sonnet-5 | 3/3 |
| A.1 | user-remind | claude-opus-5-5 | 3/3 |
| A.1.1 | user-remind | claude-opus-5-5 | 3/3 |
| A.1 | user-remind | claude-sonnet-5 | 3/3 |
| A.1.1 | user-remind | claude-sonnet-5 | 3/3 |
| A.1 | tool | claude-opus-5-5 | 3/3 |
| A.1.1 | tool | claude-opus-5-5 | 3/3 |
| A.1 | tool | claude-sonnet-5 | 0/18 |
| A.1 | tool-remind | claude-opus-5-5 | 3/3 |
| A.1.1 | tool-remind | claude-opus-5-5 | 3/3 |
| A.1 | tool-remind | claude-sonnet-5 | 0/18 |
| A.1 | tool-neutral | claude-opus-5-5 | 3/3 |
| A.1.1 | tool-neutral | claude-opus-5-5 | 3/3 |
| A.1 | tool-neutral | claude-sonnet-5 | 0/18 |
| A.1 | identity | claude-opus-5-5 | 3/3 |
| A.1.1 | identity | claude-opus-5-5 | 3/3 |
| A.1 | identity | claude-sonnet-5 | 3/11 |
| A.1.1 | identity | claude-sonnet-5 | 3/5 |
| A.1 | rewind | claude-opus-5-5 | 3/3 |
| A.1.1 | rewind | claude-opus-5-5 | 3/3 |
| A.1 | rewind | claude-sonnet-5 | 3/3 |
| A.1.1 | rewind | claude-sonnet-5 | 3/3 |
| A | root | claude-opus-5-5 | 3/3 |
| A | root | claude-sonnet-5 | 3/3 |
