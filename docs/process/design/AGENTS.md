# Writing design docs

These docs exist to communicate the harness design — the specific, exact technical details, use cases, and vision — to the agents who will build it. Max reviews them so he can trust what gets built. The docs are a means; the harness is the point. Spend effort on design content, not on documentation apparatus.

**Sources.** Everything in a doc must trace to one of: `docs/source-notes/` (Max's notes, verbatim, the only user-written content in this repo); `docs/process/REQUIREMENTS.md` (current truth: invariants, experiment evidence, decisions from his reviews); experiment evidence. A design doc is agent output and never a source. Anything not traceable was introduced without cause — hunt it and remove it. (The first doc generation was deleted 2026-08-12 for this; git history has it.)

## The document model

A document is a normalized database. Four kinds of content, labeled so the reader knows which they are reading:

- **Core** — the claims and decisions. Compressed: few words, one claim per sentence.
- **Why** — the concrete stories and use cases the core derives from.
- **Implications** — consequences; interactions with sibling designs.
- **Examples** — expansion of any of the above, so the reader can compress. Compression is understanding — the reader's.

Three tools, three jobs: compression is for brevity (the core). Expansion is for understanding. Clever prose is for reader motivation — the readers here need none, so use almost none.

**Refinement shortens.** Each revision says everything important in fewer words; expansion is cut once it has done its job. Revision passes are mandatory — single-pass output is a draft.

The cost model (Max): every word costs time; every opaque phrase costs a question; every extraneous wrong fact costs a correction or a decision to ignore it. And sentence *depth* costs separately from word count — one claim per sentence, no unstated premises. Less is more.

## Rules

- Drab, functional, boring. Content first — no scene-setting, no narrative hooks.
- Concrete before abstract; core facts before derived facts.
- Every abstract claim gets a "for example". A wrong concrete commitment is visible and correctable — this is how the wrong "prefix" definition was caught.
- Vocabulary section up front, written last, only terms the doc uses: one line + one example each.
- Sections stand alone; the doc is scannable.
- Write what the reader needs to read. This goes against the model's natural style — fight it.
- No READMEs for agent consumption. AGENTS.md always.

## Structure

Provenance line (what Max has reviewed, dates) · intro (a few sentences, written last) · vocabulary · **why** (first: the stories and use cases, drilled to roots) · **what** (the core and details) · **interactions** · questions for review (strike answered items with the date) · index.

## Method

Stages: why → what → interactions → summary; each drills down then builds back up; stages advance across the portfolio, not per doc.

A why bottoms out in a human desire of Max's, a correctness/safety property, or an irreducible resource pressure. Cost/tokens and security/isolation are false *drivers* here — legitimate as constraints or as a doc's explicit subject; name the role. A consequence (something the design must manage) is not a why (a reason it exists).

Watch for invented constraints: a priced statement read as a prohibition, or two decouplable concerns fused into a forced trade-off. For example: "credentials must live outside the database" came from an over-broad replication premise. These hide under escalated questions — check the premise under a question before asking Max to answer it.

Max's review decisions get folded directly into the design content, plainly. `REQUIREMENTS.md` carries current truth; docs point at it rather than embedding transcripts.

## Index

Sparse matrix `Aspect | L1 | L2 | L3`, written last — a table of contents. L1 holds the status letter; L2/L3 point at sections. Status: `S` settled (notes or review) · `F` fork-proven · `P` proposed · `O` open · `E` needs experiment.

Aspects: Model framing · Wire & cache · Tool surface · UX & input · Ownership & placement · Lifecycle · Storage · Economics · Security · Testing & verification · Code shape · Dev workflow & references · Core migration.
