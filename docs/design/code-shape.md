# Code shape

The Code shape aspect of the harness design: the box an implementer writes inside, and the tests that hold it.

## Max's statements

### The box

> an implementer could write the code without leeway for major wrong decisions on important behavioural aspects. It obviously still needs to invent stuff. But we should have broad guidance about how the data should be modeled, how the code should look & feel, and especially _defining the box that the implementer should work within_ - if the implementer uses too many capabilities, they go outside the box. The more constrained the experiment code, the more useful it is for the final system. Eg. did they write it as a pure dataflow? or did they write it as imperative logic with I/O? Does write data? or does it leave that to be done by some other kind of layer? Does it read the current time? or does it take it as a parameter?... Truly good code is written for the minimal capabilities & runtime.

— decision 2026-08-12

> takes a snapshot + a batch of events over each input dimension and produces its own snapshot + events. It is pure, has no I/O, cannot see anything except its declared inputs, and does nothing except produce its declared outputs.

— decision 2026-08-12 (his ideal module)

> there's stuff inside the box, then there's building that box. But we should put as much as possible in the former.

— decision 2026-08-12

> 'Well behaved' code is the goal, and it may be highly counter-intuitive.

— decision 2026-08-12

### The convex hull, not only the inner box

> the point is to nail down the convex hull of the feature across many, many dimensions. Some of those are the 'inner box', but there's more.

— decision 2026-08-12

> We have to build some I/O stuff! We have to read the time, read the context files for a 'ordinary fs limb' & emit the new versions, etc.

— decision 2026-08-12 (on `context-updates.md`, whose pure logic he called "mostly 'logic inside a box'")

> what's stored & what's ephemeral (if anything), what the data lifecycle is (when do we drop the data, if ever), and more

— decision 2026-08-12 (also to be defined by a "what" stage)

### Experiments have latitude

> experiments do not have to behave to all the constraints - that's probably too hard, and is what core integration is for. Experiments should still try to behave where doing so would be _hard_ - up-front the difficult innards while validating the important surface. Though it could be argued those could be different experiments - the important surface, then doing it in a constrained way, then integrating it into core. The scope of each experiment can be negotiated later.

— decision 2026-08-12

### Every boundary has an in-memory implementation

> the box-building layer still gets in-memory e2e tests! The point is to remove system call overhead & blocking calls. The box-building layer should still accept I/O interfaces with in-memory implementations. We should never _have_ to read a file from an _actual_ filesystem, or send a packet across a TCP socket. We should be able to use some kind of hash map backed tree structure, and a lightweight channel (with maybe some delayer implementation for network robustness tests).

— decision 2026-08-12

> requires we never do any ad-hoc boundary breaking when programming the harness code (note this is a strict requirement for core, but not necessarily for experiments)

— decision 2026-08-12

### The two kinds of test

> every feature should have a 'scenario test' - a real-as-possible, possibly even containerized, end-to-end 'outer' black box test.

— decision 2026-08-12

> a fully black box end-to-end test including setup, probably UI eventually. Uses real I/O as the boundary. Proves real usage, forces that the harness is itself harnessable for tests (eg. forces observability features useful for testing), can be slow (but should never be flaky) and is a happy path test mostly.

— decision 2026-08-12 (scenario test)

> uses in-memory I/O, a whole distributed system in one process, relying on solid abstractions and requires we never do any ad-hoc boundary breaking when programming the harness code (note this is a strict requirement for core, but not necessarily for experiments)

— decision 2026-08-12 (fast end-to-end test)

> a few black-box 'real' end-to-end tests, and more in-memory-only fast end-to-end tests (still don't have a good catchy name for those)

— decision 2026-08-12

> virtual time. good. although the scenario test can take real time, but should not have dumb waiting/polling still - everything should have an event it waits for.

— decision 2026-08-12

> re-ordered data should NOT be a problem, a data source should NOT do this. the _delayer channel_ is for _fake network conditions_ for in-process distributed systems stuff.

— decision 2026-08-12

### Language split

> the data source / data flow framework is rust I think. but everything within it should be deno but TS not JS.

— decision 2026-08-12

### Where this doctrine belongs

> yes, excellent!

— decision 2026-08-12, agreeing that the two test definitions are shared by every tool rather than specific to the harness

> these things will be shared constraints with shared reasoning. Some of it belongs not even in the agent-harness docs but in the agent-tools workspace docs like the testing approach definitions.

— decision 2026-08-12

## Open questions

1. **Is the dataflow graph explicit?** The previous doc carried `"that need not be explicitly a dataflow graph, but also, maybe it should be"` as your words and built its constraints so they would hold either way. That sentence has no hit in `source-notes/` or in `REQUIREMENTS.md`; it survives only in agent-written text. Is it yours, and does it still hold — or is the pure-dataflow module of your "ideal module" quote meant literally?

2. **Does a data source present a view at a point in time?** Two more quotes with no source behind them: a data source `"presents a view of various data at a given time, that can then be used in downstream computation"`, and it `"may or may not always listen to changes"` — from which the doc derived the rule that no downstream logic may branch on which. The pull the other way is your own vector-clock cut, which needs a source to answer "as of when", not to replay a log. Confirm, deny, or refine.

3. **How fast does this have to be, and may we spend code on it?** No source: the doc asserted `"This should be unnoticeably fast, so don't build machinery to hide latency"` and that pre-warming the graph while the user drafts a message `"would work and is a fine idea, but is not needed"`. Your standing-demand quote says the notice set is already current while a turn runs, which covers the mid-turn case but not the first request of a turn. Is pre-warming out, or just not now?

4. **Are policy and tunables data, or code?** The doc asserted `"The per-element decisions and the thresholds are values the code reads, not branches the code hard-codes"`. Toward data: you want a background meta-agent to tune settings by A/B test, which needs them to be values. Toward code: your utility-model classifier consults a model, which is an effect rather than a value. Data with the classifier as a declared effect, or code?

5. **Are notices structured values with one shared projection?** The doc recorded as settled that `"Notices are data, not text"` and that rendering is `"a separate projection, shared with /dump so the two cannot diverge"`. That rests on walking-skeleton evidence — the experiment found the shared projection works — not on anything you said about notices. Does the `/dump` rule extend to every piggybacked contribution?

6. **Is "no flakes" a stronger claim than yours?** You said a scenario test `"can be slow (but should never be flaky)"` and asked for virtual time. The doc hardened that into races being `"structurally excluded rather than made unlikely"`. Is structural exclusion the bar for core, or is your bar "no dumb waiting and it must not flake"?

7. **What moves up to the workspace, and when?** You said some of this belongs in the `agent-tools` workspace docs, naming the testing definitions. Is the box doctrine — minimal capabilities, declared inputs and outputs, in-memory implementations, no ad-hoc boundary breaking — in that set too, or is it harness-specific?
