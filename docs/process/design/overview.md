# Design scoping — overview and interaction map

Written 2026-08-03 as preparation for the user's design review pass. Four experiments from the pool are scoped in detail; this doc is the map: what's covered at what depth, and how the experiments pull on each other. All provisional.

## The documents

| Doc | Pool entry | Why scoped now |
|---|---|---|
| `forked-subagents.md` | unique soul | the user would "quite like to see forked subagents designed" |
| `user-turn.md` | unique soul | leading experiment candidate; new UX requirements captured; hands-on constraint shapes its ordering |
| `ts-vs-rust-boundary.md` | cross-cutting | "the more's in TS, the more is boosted by self-modification" — constrains every experiment |
| `compaction-handover.md` | unique soul | freshest source-note thinking (2026-08-03); partly fork-proven already |

Not yet scoped: limb-context (the fourth unique-soul pillar — deliberately waiting, because forked-subagents and ts-vs-rust both flag questions into it), topology, persistence-analytics, context-updates, self-modification (partly covered by the boundary doc), modular-components, multi-client-ui, operator-lifecycle, and the targeted questions.

## The matrix convention

`README.md` defines the shared vocabulary: thirteen aspects (nine product, four implementation/process), four levels of detail (Why → Behavior → Mechanics → Verified), and cell statuses (`S` settled, `F` fork-proven, `P` proposed-needs-review, `O` open, `E` needs-experiment). The Why column holds the motivating story, drilled back past the vision notes to the concrete situation that drives the requirement. Each doc ends with its full matrix; blank cells are honest.

## The cross-pool frontier

One cell per experiment × aspect: the dominant status at that experiment's design frontier (see the doc matrices for the full Why→Verified rows). `w` = only the motivating story exists so far. Blank rows are unscoped — the blankness is the point.

| Experiment | MF | WC | TS | UX | OP | LC | ST | EC | SE | TV | CS | DW | CM |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| forked-subagents | E | P | S | S | O | P | O | E | S | O | P | S | S |
| user-turn | S | S | S | S | P | S | w | w | S | S | P | S | |
| ts-vs-rust (self-modification) | | S | S | E | S | S | O | E | S | P | S | S | S |
| compaction-handover | F | F | F | O | O | P | S | E | | E | P | S | O |
| limb-context | | | | | | | | | | | | | |
| topology | | | | | | | | | | | | | |
| persistence-analytics | | | | | | | | | | | | | |
| context-updates | | | | | | | | | | | | | |
| modular-components | | | | | | | | | | | | | |
| multi-client-ui | | | | | | | | | | | | | |
| operator-lifecycle | | | | | | | | | | | | | |
| cancellation-economics | | | | | | | | | | | | | |
| oauth-credentials | | | | | | | | | | | | | |
| layered-shutdown | | | | | | | | | | | | | |

Aspect abbreviations: MF model framing · WC wire & cache · TS tool surface · UX ux & input · OP ownership & placement · LC lifecycle · ST storage · EC economics · SE security · TV testing & verification · CS code shape · DW dev workflow & references · CM core migration.

(The unscoped rows are not quite as blank as shown — their PLAN.md entries carry key tests and design inputs — but no design doc has drilled their stories or proposed mechanics yet.)

## The proposals that most want the user's eyes

Collected from the four docs — each is an **open, proposed** item that goes beyond the notes:

1. *Forks copy the wire-visible context, not the pending piggyback queue.* (forked-subagents)
2. *Scope cancellation composed from invariant 9*: cancel-scope drains every child, no new siblings during drain, mixed per-child outcomes, completed work kept, cancelled parent resumable. (forked-subagents)
3. *The agent_type/cache tension and its options* — forks default to the parent's persona, or persona-as-appended-instruction. (forked-subagents)
4. *"Live" transcript means the record is current, not that the model is poked.* (user-turn)
5. *User-tool execution ownership*: face owns interaction, limb owns effects, brain records. (user-turn)
6. *Rust enforces, TS decides within* — the invariant-gate vs judgement split, clearest on request triggering. (ts-vs-rust, flagged as a pattern worth blessing)
7. *The agent loop is a Rust skeleton calling TS steps; projection is a Rust pipeline running TS policies.* (ts-vs-rust)
8. *The handover's stage-two instructions include the rebuild-plan diff* — what changes / stays / leaves, derived from the same honesty machinery as `/dump`. (compaction-handover)
9. *A handover is launching a fresh continuation of yourself* — same context/attachments/task machinery as fresh-task launching, differing only in identity. (compaction-handover)

## Interaction map

The edges that showed up while designing, each direction noted from the doc that raised it:

- **forked-subagents ↔ compaction-handover** — one mechanism, three doors: forked task, fresh task, and handover share context+attachments+task; the two-part launch doubles as a fork cache point. Whichever experiment runs first designs the shared shape.
- **forked-subagents → limb-context** — "fresh across limbs" and "seed per limb" both defer to limb-context for what building a fresh context in a limb actually assembles.
- **forked-subagents → persistence-analytics** — scope state (blocked parents, dynamic siblings, resume targets, per-child cost) is the storage stress case.
- **user-turn → context-updates** — user edits to AGENTS.md/skills in-band are the main producer of change notices; append-notice now, canonical at rebuild.
- **user-turn → multi-client-ui** — open-file/draft/pane state is shared UI state; user-turn keeps it local and must only avoid leaking it into model context.
- **user-turn ↔ forked-subagents** — the user-side subagent tool; shared fork-staleness warning.
- **compaction-handover ↔ context-updates** — the same boundary from opposite sides: context-updates is change *without* rebuild; handover is rebuild done *well*; obsolete notices must not replay.
- **compaction-handover → persistence-analytics** — cache ids/expiry as durable state; handover continuity across restarts; cost measurements.
- **ts-vs-rust → everything** — each experiment's logic lands on one side; the boundary doc's table is the running answer. Sharpest cases: user tools (projection code placement), triggering (gate vs judgement), self-modification (it IS the experiment for this doc).
- **ts-vs-rust ↔ modular-components** — construction-time capability injection is one idea appearing twice: injectable I/O for tests (Rust) and sandbox capabilities (TS).
- **ts-vs-rust ↔ operator-lifecycle** — relaunch-onto-new-code with state continuity is one mechanism serving self-modification and updates; build it once.
- **cache correctness (no single owner)** — forked-subagents (fork/seed prefixes), compaction-handover (append-only two-stage, expiry timing), context-updates (append vs rebuild) all depend on being "*very* correct" with OpenAI Responses and Anthropic Messages caching. Whichever runs first builds the observation tooling (wire-level cache assertions against the fake provider; measured hit rates against real ones).

## Reading order for the review pass

Any order works, but the least backtracking is: `ts-vs-rust-boundary.md` first (it sets the placement vocabulary the others lean on), then `forked-subagents.md`, then `compaction-handover.md` (reads as a sibling of forked-subagents), then `user-turn.md` (mostly independent). The nine proposals above can also be reviewed straight from this page and the docs consulted only where a proposal needs its surrounding argument.
