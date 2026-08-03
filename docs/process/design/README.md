# Design scoping documents

Pre-experiment design docs: deeper than a brief, earlier than an
implementation. All provisional — the user goes through and revises them;
experiments then test what the revised design claims.

Writing rules (user, 2026-08-03, wording preserved): "Write the docs so
that they're a gentle, boringly readable logical flow, not a 'polished
jargon dump'. The logic of the design should dictate the structure, not
anything else. This means that the different design docs will look
different." Derive from the vision docs (`docs/source-notes/`), and do
thinking to catch inconsistencies and tradeoffs rather than restating the
notes.

## How we develop a design doc

The point is not the method — it's the docs. Better design docs are more
correct, more grounded, more insightful, and so survive the user's review
with less change, which gets us onto the actual experiments faster. The
method below just serves that.

We work **top-down-bottom-up** — each stage drills down (decompose,
analyse) then builds back up (synthesise). The stages run in this order
(user, 2026-08-03, wording preserved):

> why (drill back then forward) -> what (drill deep into the details then
> piece back together) -> interactions (examine breadth of interaction
> matrix then develop or discard) -> summary (think about the right order
> to explain, then actually write)

1. **Why** — drill *back* to the motivating stor(y/ies): the concrete
   situation(s) that make the requirement real, not a restatement of the
   note. User, 2026-08-03, wording preserved: "my vision note might not
   always be great on the 'why'. better to drill backwards even further to
   find the motivating exact story that drives the requirement." A note
   may state a mechanism; the story is what makes it a requirement. Then
   drill *forward* to what those stories force. If a story can't be found,
   say so — that's a question for the user.

   Drill to the *root*, and don't stop early. Keep asking "why does that
   matter?" until you bottom out in one of: a **human desire of the
   user's** (often the why is for the user, not just the agent — e.g. "I
   want to see the structure of my own work and work in a way that follows
   structured concurrency"); a **correctness or safety property** (e.g.
   clean teardown of side-effecting work when it's cancelled); or an
   **irreducible resource pressure** (cost, cache, context bloat). Do
   *not* reflexively reach for cost/tokens — it is the easiest root to
   name and often the wrong one. If the honest root is correctness or
   desire, cost framing actively misleads (e.g. cancellation deliberately
   *spends* tokens so agents can clean up). Two more traps: stopping at a
   *mechanism* (the note's how), and mistaking a *consequence* for a why.
   A consequence is something that follows *from* the design choice and
   must be managed (e.g. "a forked child must reliably stop after its
   slice") — it is not a reason the thing exists, and it belongs in the
   *what*, not the *why*.
2. **What** — drill deep into the details of each aspect, then piece them
   back together into a coherent design.
3. **Interactions** — examine the breadth of the interaction matrix (this
   design's aspects × other experiments' aspects), then develop the real
   connections or discard the empty ones. Interactions live in their own
   **section**, not an index column.
4. **Summary** — only now, once the design is understood: think about the
   right order to explain it, then write the L1 summary.

The summary and the index are **products of the work, written last** —
never the starting point.

## The three levels of detail

The logic of each design dictates its structure, so the docs look
different from one another. But they share three readable **levels of
detail** (user correction, 2026-08-03, wording preserved: "those are not
'levels of detail'!!! those are more aspects!! levels of detail is like
summary of whole aspect, then details about that aspect, and then specific
details that needs more extensive explanation"):

- *L1 summary* — a section you can read alone and understand the design.
  User, 2026-08-03, wording preserved: "summary should not just be 'one
  line'. summary is a whole thing which is a 'zoomed out picture' - only
  the *key* details, but none of the extraneous ones. only the key *why*.
  etc. it shouldn't be an 'executive summary' - it's still aimed at
  *understanding* and not *convincing*."
- *L2 details* — the logic-driven body: the details of each aspect that
  matter, structured however the design's own logic wants.
- *L3 extensive* — dedicated deep-dive sections for the specific things
  that need extended treatment. Not mandated and not bounded: a doc may
  have no L3 at all, or many, whatever the design's logic calls for.

## The index

Each doc ends with a sparse matrix that **indexes** aspects against the
three levels — it is a table of contents, not where content lives, and it
is written last. "The matrix (sparse at greater LOD) is an index" (user,
2026-08-03). Reading a row tells you: this aspect's status, and where in
the doc it's treated at each depth. It tends to get sparser at greater
LOD, but that's an observation, not a rule.

Columns: `Aspect | L1 | L2 | L3`.

- **L1** holds the aspect's status letter (below). Blank L1 = the aspect
  is not addressed at all, and that blank is honest information.
- **L2** / **L3** hold a pointer to the section that treats the aspect at
  that depth (e.g. `§The two-stage flow`), or blank if it doesn't go that
  deep.

Interactions are **not** an index column — they're a section (stage 3
above). A column only earns its place if it would be a genuinely large
list, which is not assumed in advance.

**Status letters**: `S` settled (notes or gate ruling decide it) · `F`
fork-proven (validated in the user's OpenCode fork) · `P` proposed here,
needs review · `O` open, no proposal · `E` needs experiment. These carry
design maturity, so there is no separate maturity axis.

## Aspects

Shared vocabulary, so the indexes are comparable across docs.

Product aspects:

- *Model framing* — what the model sees and how it is prompted
- *Wire & cache* — requests, prefixes, caching, triggers
- *Tool surface* — tools, parameters, APIs
- *UX & input* — user-visible behavior
- *Ownership & placement* — face/brain/limb split, TS/Rust side
- *Lifecycle* — cancellation, failure, drain, resume
- *Storage* — what persists and how
- *Economics* — cost mechanics and measurements
- *Security* — authority boundaries

Implementation and process aspects:

- *Testing & verification* — how this gets tested: surfaces, fakes,
  what needs a real model vs the fake provider
- *Code shape* — modularity, construction, injection; how the code wants
  to be structured
- *Dev workflow & references* — how we iterate on this, and which
  reference codebases to consult before inventing
- *Core migration* — what promotion into core looks like; what must not
  calcify in the meantime
