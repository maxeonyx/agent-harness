# Curated Requirements

This is the live behavioral target. The detailed stakeholder requirements and
test-case lists remain in `docs/source-notes/requirements.md` §2 (verbatim
source material); this file curates the invariants and tracks validation
status. When a spike outcome changes a requirement, record it here — source
notes are never edited locally.

Last reconciled with source notes: gist revision of 2026-06-13.

## Invariants

The non-negotiables every gate checks against. A change that violates one of
these stops and goes to the user.

1. The brain is the only role that drives provider API requests. Provider
   credentials never reach limbs, faces, plugins, tool schemas, logs, or model
   context.
2. Recording context, appending context, rebuilding context, and triggering
   inference are distinct operations. Passive user activity never triggers a
   model request.
3. User-tool activity is framed as user activity, never as agent tool calls.
   Each user tool keeps two surfaces: rich interactive UI for the user,
   compressed context for the model.
4. Face, brain, and limb are logical roles. Co-location versus splitting is a
   deployment choice over the same logical model.
5. Durable session data is analytics-grade and queryable. Durable,
   cache-supporting-transient, shared-UI, and disposable-stream data are
   explicitly distinguished.
6. Subagent concurrency is structured: parents block on children; sibling
   results stay hidden until the parent resumes.
7. Multi-client UI state is explicitly modeled. Stale clients cannot silently
   overwrite newer state; the user wins on conflicting edits.
8. Spike code never becomes core by copying. Core integration is a fresh
   design from evidence.

## Requirement areas and validation status

| Area | Source | Validated by | Status |
|------|--------|--------------|--------|
| In-band user work, user-tool compression, user-wins conflicts | source-notes `requirements.md` §2.1 | Spike 1 | not started |
| Process/context edits first-class, rapid tool iteration, context lifecycle | §2.2 | Spike 5 | not started |
| Disposable spikes, safe reload, self-modification | §2.3 | Spikes 0/5 | not started |
| Topology, lifecycle, direct streams, updates/migrations | §2.4 | Spikes 2/7 | not started |
| Analytics-grade storage, data lifecycle | §2.5 | Spike 3 | not started |
| Authority boundaries, credential ownership, tool framing | §2.6 | Spikes 1/2 | not started |
| Structured subagent concurrency, scope legibility | §2.7 | Spike 4 | not started |
| Multi-client state, shared UI state, TUI + web GUI | §2.8 | Spike 6 | not started |

Spike 0 (walking skeleton) validates no requirement area by itself; it is the
shared substrate the others run on.

## Requirement changes from spike evidence

None yet.
