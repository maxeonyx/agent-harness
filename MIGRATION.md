# Moving the crates onto the substrate

On branch `substrate`, `harness-substrate` and `harness-core` compile, and `demos/substrate/run.sh` runs; `brain`, `face-web`, `limb-workspace`, `provider` and `store` do not. This note is for the engineer who moves them. Read the crate docs of `crates/substrate` and `crates/core` first; they hold the model. Delete this file in the PR that makes the workspace compile.

## What changed in core

- `journal` is now `fact`. `Journal`, `Store`, `journal::Error` and `Rejected::OutOfOrder` are gone. The core's old `Record` is now `Stated`: one fact as the fold sees it. The stored unit is `harness_core::schema::Record`, which is `harness_substrate::Record<Harness>`: a place's `Batch`, or the brain's `Included` for one.
- Ids are event times. Every id wraps `EventId { at: Time, i: u32 }`: the record that minted it, and an index in it. There is no `generate()`. Mint inside the closure given to `say(clock, |at| batch)`, as `TaskId(at.event(0))`. The fold refuses a creation whose id is not its own record's (`Rejected::Minted`).
- `InstanceId` and `FaceId` are now `PlaceId`. The brain, each tab and each limb instance are places. `Seq` is now `{ n, i }`: the brain time that included the fact, and its index.
- `Author` is derived, never stated. It is `Brain` for the brain's own place, `Limb(id)` for a place named by an accepted `Reach::Connected { instance }`, and `Max { face, seen }` for a place named by an accepted `Fact::Face(place)`, with `seen` coming from the batch's `Fact::Seen`.
- `ToLimb` and `FromLimb` are gone. A limb's view is `State::limb_rows(limb)`: a `LimbRow::Call { call, stopping }` for each dispatched, unsettled call. A limb's settlements are ordinary batches from its place.
- New facts: `Face(PlaceId)`, `Seen(Time)` and `Draft(DeliveryId, DraftEvent)`. `Focus { task: Option<TaskId> }` is per face (`State::focus(place)`).
- `State::apply`, `with` and `next` are no longer public. Everything folds through `Brain` (`Decider<Harness, _>`).
- `Todo::Dispatch` for a plugin's call is due once, when its limb is connected. `Todo::Interrupt` is due once. Each shows in the limb's rows, and nothing is resent on reconnection.

## store

It becomes `impl harness_substrate::Storage<Harness>` over SQLite:

- one table of records keyed by `(place, n)`, with the clock and the JSON, plus the values table;
- `load()` returns every record;
- `commit(put, drop)` is one transaction: insert `put`, delete `drop`;
- split a batch's `values` out to the values table by `ValueId` and put them back on load. Leave values in place when their record is dropped; value GC is not built;
- keep the brain's own `PlaceId` in the database, so `Brain::open(here, storage)` reads it from there and nowhere else.

`examples/crash.rs` becomes a crash test of this storage, with the same shape as step 9 of the demo.

## brain

- `let (brain, divergences) = Brain::open(here, storage)?`. Log every divergence: it is a record an older build accepted that this build refuses. The operator needs to see it.
- The brain's own facts go through `brain.say(clock, |at| batch)`. A refused batch comes back as `Error::Refused` and is not stored, as before.
- Each link reads `Up` from its follower. `Up::Record(r)` goes to `brain.include(clock, r)`, which returns `None` for a record it already holds. `Up::Live` goes to the tabs, unstored.
- After every change, call `publisher.publish(view)` for each connected follower. The view is `View { as_of: brain.head(), standing: brain.standing(place), rows }`, where `rows` is `limb_rows(limb)` for a limb and the face rows (below) for a tab. Publishers are not stored: a reconnecting follower gets a whole view.
- A link becomes Max's by the brain stating `Fact::Face(place)` before including anything from it. A limb's becomes its own by stating `Reach::Connected { instance: place }`. Who may connect is the link's authentication, decided at the brain.
- `todo()` keeps its shape. Dispatch commits `Dispatched` and the call's row appears; it no longer sends anything.
- A timer firing is `brain.say(clock, |_| Batch::default())`. That record says only the clock (`Key::Clock`), so it is collected.
- The brain reads nothing but its database. `--roles` reading role files at start goes. The role definitions become facts that a co-hosted limb states.

## limb-workspace

- `record.rs` is replaced by a `Log<Harness, _>` on the limb's own storage, under the limb's own `PlaceId`. A settlement is `log.say(...)` before it is shipped, so it survives a brain restart.
- The "started" mark and the process-group file are machine-local and can stay files, until the core has a fact for a call having started.
- It holds a `Replica<LimbKey, LimbRow, Refusal>`. It runs each new `Call` row, stops each row with `stopping: true`, and forgets a row once it is gone.
- On every connect, and after every `say`, it ships `log.unsent(&standing)`. On every step that carries a standing, it calls `log.cover(&standing)`.
- `unsent` returns `Restored` when the brain holds more of this place than its storage does. The limb then continues under a new `PlaceId`.
- `examples/demo.rs` drops `ToLimb`, `FromLimb` and `generate()`.

## provider

`examples/fork.rs` uses `generate()` and `Store`. It mints from a local time instead (`PlaceId(..).origin()`, then `EventId { at, i }`) and uses `Memory` or the new storage.

## face-web (the server)

- Delete `POST /act` and its 409, the SSE whole views on `/view`, and `/live`. One WebSocket per tab carries `Up` and `Down` as JSON.
- The server holds a `Publisher` per tab. `view.rs`'s derived rows become `View<FaceKey, FaceRow, Refusal>`, keyed per object (a task on the map, a page of the open session, a draft beside it), so a step ships only the rows that changed.
- A tab's first message names its `PlaceId`. The brain states `Fact::Face(place)` for it, since on this machine's LAN every tab is Max's. That replaces `FaceId`.
- The face's pre-checks (`State::with`) are gone. Rows say what is allowed now, and anything else comes back refused in the standing.

## The face's JavaScript (`page.html`)

- **Place id.** Keep the tab's place id in `sessionStorage`. A duplicated tab copies `sessionStorage`, so take a Web Lock named by the id (`navigator.locks.request(id, { ifAvailable: true })`). If the lock is held, mint a new place id.
- **Its log.** Keep the tab's own log in IndexedDB as a port of `Log`: its next `n`, the records not yet covered, and the collapse of a superseded `Focus` or draft `Text` before it ships. Those are the two facts `Harness::durability` keeps only as the newest.
- **Seen.** Every act's batch carries `Fact::Seen(view.as_of)`.
- **Drafts.** Opening a draft is `Draft(DeliveryId(EventId { at, i }), Opened { session })` in the record whose time names it. A pause in typing is `Text(value)` and a send is `Send { text }`. Show its fate from the draft rows. Show an act's status from `standing.status(n)`: pending until covered, then included or refused with the reason.
- **Wire format.** Times are strings `"place.n"`, event ids are `"place.n.i"`, and place ids are 16 hex digits.

## Not built

These are not built, so plan around them: SQLite storage, the wire codec and transports, snapshots, value GC, per-kind fact enums, rows for records a build cannot read, an incremental view, and watch rows for limbs.
