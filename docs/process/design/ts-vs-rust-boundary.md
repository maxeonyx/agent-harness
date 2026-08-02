# The TS/Rust boundary — design scoping

Provisional and cross-cutting: this doc doesn't belong to one experiment — it scopes the self-modification experiment and quietly constrains all the others, because every experiment's logic eventually lands on one side of this line. Derives from `source-notes/tech.md` (the primary source), `source-notes/reference-codebases.md`, `source-notes/context-updates.md`, and the walking-skeleton rulings. Marked per part: **settled** / **open** / **experiment**.

## The organizing principle

The user's framing (2026-08-03): "the more's in TS, the more is boosted by self-modification." That single sentence decides most placements, because it gives a test to apply to any component: *would we want an agent to iterate on this live, in-session, with reload and rollback?*

- If yes, it belongs in TypeScript, running in embedded Deno, delivered as a plugin. Edit → reload → keep going; brick → quarantine → roll back. Cheap iteration is the point.
- If no — because it's the machinery that makes reload/rollback *safe*, or because getting it wrong corrupts sessions, money, or credentials — it belongs in Rust. Rust changes are also self-modifiable in the vision (the agent edits the harness, rebuilds, relaunches onto new code, and sessions continue), but that's the heavyweight path: rebuild plus relaunch-with-state, not an in-place reload.

Put shortly: **Rust is the part that must not brick; TS is the part that's allowed to be wrong for a minute.** tech.md's own list of what goes in TS: "almost all of the 'business logic' of our harness, via plugins: tools, providers, user tools, limbs?" — the question mark on limbs is the user's and stays.

There's a second, less obvious reason the split works: the Rust substrate is what *enforces the invariants*, so TS code doesn't have to be trusted to uphold them. That thought recurs below.

## Component by component

**Provider adapters — TS, with auth outside. Settled** (it's explicit in tech.md). A provider plugin shapes requests and parses responses for one wire dialect. It never sees credentials: the OAuth/device/token flows are implemented in Rust, and the plugin receives "a pre-authenticated fetch wrapper or something". This keeps invariant 1 (credentials never reach plugins, schemas, logs, or context) structural rather than disciplinary. New provider quirks, new dialects, new response fields: all live-iterable.

**Agent tools — TS. Settled.** The classic plugin surface. A tool's definition, its schema, its execution logic, and (per the user-turn doc's symmetric-projection rule) both of its projections — model transcript and UI form — are one TS-side package. The *effects* of tools go through capabilities the substrate hands them (see the sandbox contract), so a tool can be wrong without being dangerous.

**User tools — TS logic, with a real open question about UI. Open.** A user tool's tracking and compression logic is exactly the taste-heavy, often-revised code the principle sends to TS. But a user tool is also an interactive surface — a file explorer, an editor, a terminal pane — and the face's rendering machinery might be Rust (see TUI below). So the boundary runs *through* a user tool: TS decides what the tool shows and records; something face-side turns that into pixels/cells. The clean version is a declarative UI description crossing the boundary (TS emits structure, the face renders it), which is also what keeps web GUI and TUI as two renderers of one tool. Whether opentui_rust, OpenTUI (TS), or Deno's Tauri-like GUI hosts which part is genuinely unresolved in the notes — reference-codebases wants opentui_rust tried, tech.md is excited about Deno webview. **Experiment** (probably the self-modification and multi-client-ui experiments jointly).

**Context projection and compression — TS-leaning. Open.** How a tool call is truncated, how user activity is summarized, what a per-model view keeps: taste-heavy, endlessly revisable, exactly what self-modification should boost. The reason for hesitation is that projection is also where "everything the model sees must be derivable from the session record" (invariant 10's tail) gets enforced, and the walking-skeleton introspection ruling requires the request builder and `/dump` to share one projection. A workable split: the substrate owns the projection *pipeline* and its guarantees (single shared projection, derivability, dump honesty); TS owns the projection *policies* plugged into it. **Open, proposed.**

**Request triggering — Rust enforces, TS decides within. Open, proposed.** The four-trigger whitelist (turn end, tool-loop continuation, cache-nearly-expired handover, explicit resume) is invariant 2 — substrate territory; no plugin should be able to add a fifth reason to spend money. But *within* a legitimate trigger there is judgement (is the cache near enough to expiry to bother? is this handover worth it now?) that wants iteration. Rust owns the gate, TS owns the judgement behind it. **Needs the user's review** — it's the clearest case of the enforce/decide split and worth blessing as a pattern.

**Prompts, personas, skills, tool descriptions — data, not code. Settled.** Whichever side reads them, they're content: versioned, cache-relevant (their bytes are in the prefix), and edited constantly. The context-updates machinery governs how their changes reach warm sessions. No placement controversy; noted because it removes a lot of things from this debate.

**The agent loop — Rust skeleton, TS steps. Open, proposed.** The walking-skeleton shape (inbox + select loop + owned in-flight work, structured lifecycle, cancellation with four-valued outcomes) is precisely the code that must not brick and must not leak tasks: Rust. What happens *inside* a step — how a response's tool calls map to executions, retry shaping, the router judgement of forked-vs-fresh — is business logic. The loop calls TS; TS never owns the loop. This placement keeps invariant 9 (cancellation baked in) enforceable in one language.

**Limb execution — Rust substrate, TS tool definitions; the notes' "limbs?" stays a question. Open.** The cleanup-by-ownership ruling (process trees owned and reaped on every resolution path, no process-table scans) and filesystem/process lifecycle are substrate. Tools that *use* that substrate are TS, same as agent tools. Whether a whole limb — its context-assembly behavior, its layer composition — is "a plugin" is the question mark tech.md wrote; limb-context's design will bear on it. Flagged, not resolved.

**Config — Rust schema, plugins contribute. Open.** The deconfuse-in-Rust idea (typed schema, ordered sources, recursive merge, injectable environ/argv) is the modular-components experiment and is substrate-flavored: construction happens before any plugin runs. But plugins contribute config *schema* (a provider plugin has settings), so the schema boundary crosses the language boundary — schema contributions must be declarable from TS in a form the Rust composition understands. This is a real design problem for modular-components, noted there via PLAN; it doesn't change the placement.

**Storage, session journal, analytics — Rust and SQL. Settled by principle.** Durable session data, the event journal, schema migrations, and the analytics query surface (product behavior, per the test-boundary rules) are the ground truth everything else derives from. Plugins get read access through capabilities (the read-only meta surface from analytics.md), not their own writes to the journal.

**TUI core — Rust-leaning, undecided. Open.** Rendering performance, the ruled rendering discipline ("rendering != face innards", tty takeover as owned in-flight work), and opentui_rust point to Rust; the declarative-UI idea above would keep TS tools expressive anyway; Deno's webview option exists for GUI. This is the placement with the least evidence either way. **Experiment.**

### The table

| Component | Side | Status |
|---|---|---|
| Provider adapters | TS (auth stays Rust) | settled |
| Agent tools | TS | settled |
| User tools | TS logic; UI rendering split open | open |
| Projection/compression | TS policies in a Rust pipeline | open, proposed |
| Request triggering | Rust gate, TS judgement | open, proposed |
| Prompts/personas/skills | data | settled |
| Agent loop | Rust skeleton, TS steps | open, proposed |
| Limb execution | Rust substrate, TS tools; "limbs?" open | open |
| Config composition | Rust schema; TS contributes | open |
| Storage/journal/analytics | Rust + SQL | settled |
| TUI core | Rust-leaning | experiment |
| Credentials/auth flows | Rust only | settled (invariant 1) |
| Cancellation/lifecycle | Rust only | settled (invariant 9) |

## The sandbox contract

**Settled in intent** from tech.md: plugins run in a hard sandbox — "no installing node modules", no ambient authority. Everything a plugin can touch arrives as an injected capability: the pre-authenticated fetch, filesystem handles scoped to what the tool legitimately operates on, the read-only analytics surface. A provider plugin "can operate without actual access to the auth".

This is the same shape as modular-components' thesis — components constructed from explicit config with all I/O injectable — applied across a language boundary. One design idea serving both: *construction-time capability injection is the only way anything gets anything*, in Rust for testability, in TS for safety. The rhyme is worth preserving deliberately.

## Schema stability across reloads

Reloading a plugin must not corrupt warm sessions. The system prompt contains tool definitions; a warm cache means those bytes are fixed. So after a reload, sessions that are still warm keep *speaking the old schema* — "we still want the old tool calls to be usable by the agent until the next handover/compaction or the next cache break", with an explicit cache-break option that is "not normal path". tech.md's suggestion that plugins live in the DB ("perhaps") exists exactly to make old versions addressable: a session's context references plugin-version, not plugin-latest. Whether the DB is the right store is hedged; the versioned-addressing requirement is not. Schema-*changed* tools are also the one case context-updates injects full content for, not a bare notice — the two experiments meet here. **Settled requirement, open storage mechanism.**

## Failure containment and rollback

Live editing without live bricking, both sides:

- **TS:** a failed reload quarantines; the old version keeps serving; roll-forward is explicit. "If the new version of the plugin crashes we can also revert to the old version." Exercise plugin code as much as possible at load time to catch bricks early.
- **Rust:** the binary relaunches onto new code with a state scheme that resumes sessions ("launch back into the same session"); a failing new binary reverts. This is the same machinery operator-lifecycle needs for *updates* — self-modification and operator-lifecycle should share it rather than build it twice. Graceful shutdown rules from tech.md apply (finish in-flight API requests, remember pending tool calls, continue after relaunch).

**Settled in intent; experiment in nearly every mechanism.**

## The costs, honestly

The split is not free, and the self-modification experiment should measure rather than assume:

- **The boundary tax.** Every Rust↔TS crossing is serialization plus an async hop. If projection policies or tool execution cross per-event, that's the hot path. The experiment needs a real measurement of crossing cost at realistic event rates.
- **Two toolchains**, two test styles, two failure vocabularies — carried forever.
- **deno_core maturity risk.** Embedding Deno is the bet the user is "excited to try" — an outcome, not a commitment. The experiment should establish embedding feasibility *early*: sandboxing as specced (no modules, capability-only), snapshot/startup cost, memory per isolate (the 1-brain-N-limbs memory target in agent-harness-design.md is part of why the harness exists).
- **Gravity toward Rust.** Things placed in Rust "for now" never move; every such placement silently shrinks what self-modification can touch. The placement table above is the ratchet against that — moving something Rust-ward later should be a reviewed decision, not drift.

Fallback if Deno embedding disappoints: the placement *principle* survives even if the mechanism changes — out-of-process TS (a Deno subprocess speaking the same capability protocol) preserves the iterate-live property at a higher latency cost; giving up TS entirely would gut self-modification down to the heavyweight rebuild path and should be treated as a major vision change, not an engineering fallback. **Open.**

## What the self-modification experiment must produce

1. Embedding feasibility: deno_core hosting the sandbox contract as specced, with measured startup, memory, and crossing costs.
2. A tool edited live: schema-stable path (warm session keeps old schema, new sessions get new) and cache-break path (explicit), both observed on the wire.
3. A deliberately broken reload: quarantine, old version serving, rollback — no bricked session.
4. The heavyweight path once: harness edits itself, rebuilds, relaunches, session continues.
5. Evidence for the open placements above (projection pipeline/policy split, trigger gate/judgement split) — even if only as "we built it this way and it held".

Exit (from PLAN.md): an agent can edit a plugin or the harness, rebuild, reload or relaunch, and continue — with rollback when the new version is broken.

## The matrix

Levels, statuses, and aspect definitions per `README.md`. The Why column is the motivating story. Blank = not addressed. This doc is one deep aspect (placement) plus its consequences, so several rows are thin by design.

| Aspect | Why (the story) | Behavior | Mechanics | Verified | Interacts with |
|---|---|---|---|---|---|
| Model framing | | | | | |
| Wire & cache | a plugin reload that shifts one byte of the system prompt silently costs the whole warm cache | S old schema until rebuild/handover; explicit cache-break path | O versioned plugin addressing (DB "perhaps") | | context-updates, compaction-handover |
| Tool surface | Pi agents "sometimes brick themselves" editing plugins — no edit safety | S plugins provide tools/providers/user tools/"limbs?" | O declarative UI crossing the boundary | | user-turn |
| UX & input | one tool needs two renderers (TUI and web) or the GUI forever lags the TUI | | E TUI host: opentui_rust vs OpenTUI vs Deno webview | | multi-client-ui |
| Ownership & placement | "the more's in TS, the more is boosted by self-modification" — the fork experience: TS iteration is why its features exist | S placement table per component | P gate/judgement; P skeleton/steps; P pipeline/policies | | every experiment |
| Lifecycle | live editing without live bricking | S quarantine, old-version serving, rollback | E reload mechanics; E relaunch-with-state | | operator-lifecycle, self-modification |
| Storage | old tool schemas must stay addressable or warm sessions break on reload | | O the versioned store itself | | persistence-analytics |
| Economics | N projects x ~400-500 MB of harness each, today | | E boundary tax; E isolate memory; E startup cost | | |
| Security | provider keys reachable from a plugin end up in logs and context eventually | S capability-only sandbox; no node modules; credentials Rust-only | S pre-authenticated fetch shape | | (invariant 1) |
| Testing & verification | the boundary is where mocks creep in; injected capabilities keep tests black-box | P in-process black-box suite via capabilities | E exercising plugin code at load | | modular-components |
| Code shape | | S capability injection at construction, both sides of the boundary | | | modular-components |
| Dev workflow & references | | S steal designs, not code: Pi extensions, oh-my-pi, deno_core | E embedding feasibility spike early | | |
| Core migration | things placed in Rust "for now" never move, silently shrinking self-modification | S the placement table is the ratchet; Rust-ward moves are reviewed decisions | | | |
