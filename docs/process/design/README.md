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
   *spends* tokens so agents can clean up). Two known **false roots** in
   this project specifically: **cost/tokens** (as above) and
   **security/isolation** — the user treats security as a low-priority
   nice-to-have, never a top driver, so proposing it as a why is almost
   always wrong.

   But "false root" is about the *role* the thing plays, not about the word
   being banned, and collapsing the two produces its own errors — a doc that
   dodges naming cost when cost is the honest answer, or one that argues a
   low-priority concern must really be high-priority because an invariant
   depends on it. So name which role it plays. Cost and security are false
   **drivers**. They remain legitimate as **constraints** (invariant 1 keeps
   provider credentials brain-owned, non-negotiably, without security being
   a driver) and as **subjects** (a doc whose question *is* "what does this
   cost" is rooted in cost, correctly).

   Two more traps: stopping at a *mechanism* (the note's how), and
   mistaking a *consequence* for a why.
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

   Two things come out of this stage and they have different homes. Each
   doc gets an **Interactions section** covering what *that* design owns,
   what it defers to a sibling, and the connections that survived scrutiny.
   But the matrix itself is inherently cross-cutting, and the most valuable
   findings — machinery several experiments need and none owns, conflicts
   between two designs' whys, and the **dependencies** those imply — belong
   to no single doc. Those go in `INTERACTIONS.md`, which is the portfolio
   view. Do not duplicate: a doc points at `INTERACTIONS.md` for shared
   machinery rather than restating it, and a conflict between two designs
   is written up once *there* rather than twice in both docs.

   The division of labour on ordering is exact, because both files sound
   like they own it and only one does. `INTERACTIONS.md` records
   **dependencies**: X's result is uninterpretable before Y's. `PLAN.md`
   turns dependencies into an **order**. So experiment *sequencing*
   consequences do not live here at all.
4. **Summary** — only now, once the design is understood: think about the
   right order to explain it, then write the L1 summary.

The summary and the index are **products of the work, written last** —
never the starting point.

## Review provenance — not all content here is equally settled

These docs are provisional by construction, but they are *unevenly*
provisional, and that must be visible. The user, 2026-08-03 (wording
preserved): "I've reviewed or been involved in the above so they can be
considered more settled than the stuff that you produce here, which I won't
have reviewed yet."

So every doc carries a **Stages** line in its header naming, per stage,
whether the content is user-reviewed or agent-drafted:

```
Stages: why (user-involved) · what, interactions, summary (agent-drafted,
unreviewed).
```

Rules:

- A stage drilled *with* the user is `user-involved`. Do not silently
  rewrite user-involved content while working a later stage. If a later
  stage genuinely contradicts a user-involved why, that is a **finding** —
  record it under "Questions for review" in the doc, don't just fix it.
- A stage the agent produced alone is `agent-drafted, unreviewed` until the
  user says otherwise.
- Every **design doc** ends with a **Questions for review** section listing
  the calls the agent made that most want the user's ruling. Empty is a
  smell — an honest design pass at this depth generates real questions.
  `INTERACTIONS.md` carries one too, for the portfolio-level calls that
  belong to no single design; this file and `PLAN.md` do not, being method
  and pool rather than design.
- **Audit the premise under a question, not just the answer above it.**
  Escalating something to the user reads as diligence, which is exactly why
  it is a good hiding place for an invented constraint: a question phrased
  as "which of these two do you want?" quietly asserts that there are two,
  and nobody re-examines it because it is already flagged. So a question
  earns the same scrutiny as an assertion — check that the trade-off it
  offers is real before asking the user to resolve it. Several of the
  questions in these docs dissolved on that check, and dissolving one is
  worth more to the user than answering it.

## Where a finished doc hands off

The point of the doc is to make an experiment startable. `PROCESS.md` step
1 needs a brief with: **thesis**, **what evidence would falsify it**, and
**which invariants it touches** (`REQUIREMENTS.md`). A doc is done when
those three fall out of it *without further design thinking* — the
interactions section having already decided what this experiment owns
versus what it defers to a sibling. The doc is not the brief and does not
replace it; it is what makes writing the brief mechanical.

## Stages advance across the portfolio, not one doc at a time

A consequence of stage 3 that is easy to get wrong: interactions are
`this design's aspects × other experiments' aspects`, so a doc cannot
honestly reach stage 3 until its siblings have a stage-2 *what* to interact
with. Taking one doc all the way 1→4 in isolation guarantees its
interactions section is guesswork, and guarantees rework when the siblings
land.

So the whole set advances a stage at a time: why for every doc, then what
for every doc, then interactions across the set (the matrix is only real
now), then summaries and indexes. A doc's own stages still run in order —
it is the *cohort* that moves together.

This also means **summaries are written last across the board**, which is
the same rule as before, just at portfolio scale.

Docs need not all reach the same depth. The soul experiments
(`REQUIREMENTS.md`) earn L3 treatment; good-taste and targeted-question
docs may honestly stop at L2. Depth is a judgement recorded in the index,
not a quota.

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

## Style: boring, explicit, baby steps

From the user's first full read of a finished doc (compaction-handover, 2026-08-04). His verdict on the content was good ("I do not actually see anything wrong in this compaction handover document") and his verdict on the writing was not, so this section exists.

### The governing principle: decompress

This is the rule the others serve, and it is counter-intuitive enough to state first. Wording preserved, 2026-08-04:

> "the principle been writing for me is that word count is not expensive because I have a very fast reading speed, but word depth is expensive because I actually have quite a slow mental speed. So decompressed is much better than compressed, much, much better than compressed. Priest language is really difficult for me. Decompressed language is very easy. So go through the examples. Go through the story. Go through the the entire logic chain, and I'll read that much faster than I'll read one sentence of compressed language."

("Priest language" is a transcription artifact — read it as *terse* or *compressed* language.)

So the cost model is not the obvious one. **Length is cheap. Depth per sentence is expensive.** A dense sentence that packs three inferences costs him more than four plain sentences that each carry one, even though the four are longer. Optimising for brevity is therefore optimising the wrong variable, and can make a doc *more* expensive to read.

What that means in practice:

- Walk the whole logic chain. Do not skip a step because it follows obviously — following obviously is exactly the work being pushed onto the reader.
- Use examples and worked stories. "Go through the examples. Go through the story." These are not padding; they are the cheap form of the same content.
- Never compress two claims into one clause to save a line. Split them.
- Do not target a line count. Completeness of the chain beats shortness, always.

His earlier feedback in the same session is compatible with this and still stands — it was aimed at *baroque* prose (words that add no content), not at length:

> "It may repeat itself in places, use too many words, and not enough diagrams... explain the details first, then really go hard on the technical detail."

> "re-write... into clearer, more explicit, more basic and boring technical language, with the logic laid out in baby steps instead of in baroque prose"

### The rules that follow

- **Boring technical language.** No rhetorical build-ups, no elegant-turn-of-phrase sentences whose content is one clause. Write the clause plainly, then write the next one.
- **Baby steps.** Each paragraph advances the logic by one checkable step. A reader should never have to hold unstated premises to parse a sentence.
- **Concrete statements only.** Every sentence should be checkable. His example of failure: "modular components touches this design only through testing" — "a lot of words for a not quite concrete statement." If the concrete version is unknown, say what is unknown instead.
- **His vocabulary, or define it on first use.** He bounced off "context-lifetime collection" and "epoch-keyed rows": "These are not terms I use, so the statement is unclear to me." Coined terms get defined in plain words where they first appear, and are used sparingly.
- **Details first, then depth.** Plain overview of a mechanism before its extended technical treatment — not interleaved.
- **Diagrams.** Tables, ASCII diagrams, and worked sequences wherever they replace prose. "Not enough diagrams" is his direct feedback.
- **Repetition is not the enemy; density is.** Saying a thing once at its home and pointing at it is still right for *decisions* (two prose copies drift). But re-stating a premise where it is used, rather than making the reader recall it from ten paragraphs earlier, is decompression and is wanted.

A calibration datum, for measuring whether a rewrite worked: the compaction doc took him "around 20min to half an hour to read, understand and respond" at 327 lines, and he still expected he "likely still forgot some things." The target is that the same reader clears a doc faster with less forgotten — and a longer doc that achieves that is a better doc.

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
