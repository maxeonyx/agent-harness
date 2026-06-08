# Agent Harness Process Handoff

Status: initial setup
Active loop: onboarding / process stewardship
Source notes version: gist `014463e0964bebd0add4b914971c492f` cloned 2026-06-08

## Current Position

The external design/process notes have been imported into `docs/source-notes/`.
No product harness behavior has been implemented.

The repo now has explicit boundaries:

- `experiments/` is for disposable spikes.
- `src/` is not a dumping ground for spike architecture.
- `tests/` starts by enforcing the process scaffold.

## Next Recommended Loop

Pre-spike A: test harness primitives.

Immediate target:

1. Scope the first tiny scenario language for fake face/brain/limb actors.
2. Define black-box tests before implementation.
3. Prove at least one passive user-tool context event does not trigger a model
   request.
4. Prove a user turn end does trigger a model request with accumulated context.

## User-Gated Decisions

Ask the user before exiting Pre-spike A scope:

- Whether the first primitive set is adequate for Spike 1.
- Whether the fake actor names and scenario language are clear enough to build
  on.

Ask the user before any core integration:

- Whether the spike behavior proves the intended requirement.
- Whether the spike outcome document correctly separates evidence from
  architecture.

## Do Not Integrate Yet

- Do not build a real provider adapter.
- Do not build a real TUI or GUI.
- Do not choose the final face/brain/limb transport.
- Do not turn `experiments/` code into `src/` code without a spike outcome and
  promoted black-box tests.

## Evidence So Far

- Source notes imported.
- Process handoff created.
- Initial process contract tests define the setup bar.

## Open Questions

- The eventual user-facing command name is still undecided.
- The first scenario DSL shape is not designed yet.
- The standards compliance backlog has not been run after submodule integration.
- Imported source notes are preserved as raw source material; curated canonical
  requirements have not yet been extracted.
