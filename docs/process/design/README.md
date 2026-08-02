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

## The design matrix

Each design is also indexed as an explicit, sparse matrix — aspects ×
levels of detail, with interactions — so a review pass can see at a
glance what is designed deeply, what shallowly, and what not at all.
Blank cells are honest: they mean "not addressed", and that is
information. Each doc ends with its matrix; `overview.md` carries the
cross-pool matrix.

**Aspects** (shared vocabulary, so matrices are comparable).

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

**Levels of detail** (columns): *Why* → *Behavior* (externally visible
contract decided) → *Mechanics* (the internal how) → *Verified* (proven
by fork experience or a run experiment).

The *Why* column holds the **motivating story** — the concrete situation
that makes the requirement real, not a restatement of the note. Per the
user (2026-08-03): "my vision note might not always be great on the
'why'. better to drill backwards even further to find the motivating
exact story that drives the requirement." A note may state a mechanism;
the story is what makes it a requirement. If the story can't be found,
say so — that's a question for the user, and worth surfacing.

**Cell statuses**: `S` settled (notes or gate ruling decide it) · `F`
fork-proven (validated in the user's OpenCode fork) · `P` proposed here,
needs review · `O` open, no proposal · `E` needs experiment · blank =
not addressed.
