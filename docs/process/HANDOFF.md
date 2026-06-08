# Agent Harness Process Handoff

Status: initial setup committed and pushed
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
- Subrepo created at `https://github.com/maxeonyx/agent-harness`.
- Subrepo commits:
  - `235358d` - initial process scaffold
  - `a2a40b1` - public site references
  - `e7a9bb3` - onboarding baseline
  - `f2f523d` - CI path-dependency setup
- Workspace integration commit: `7486961`.
- Workspace pointer update commit: `2bea403`.
- Local checks passed under `devenv`:
  - `cargo fmt --check`
  - `cargo test`
  - `cargo clippy -- -D warnings`
- GitHub CI passed on `main` for `f2f523d`:
  - Check
  - six release build targets
  - Release `v0.1.0`
  - Pages deployment
- Commit `b644c63` recorded the green CI baseline but intentionally triggered
  the release version guard because `v0.1.0` already belonged to `f2f523d`.
- Version `0.1.1` is the next release baseline after the handoff update.
- Version `0.1.2` adds the core package TDD ratchet gatekeeper and CNAME file.
- GitHub Pages was enabled for workflow deployment.
- Repository homepage metadata points to
  `https://agent-harness.maxeonyx.com`.
- Standards baseline was run from the workspace through `devenv`.

## Standards Backlog

The first standards run passed these onboarding-relevant checks for
`agent-harness`:

- workspace routing
- vision and process
- OpenCode skill docs
- tests present
- code standards
- fast/slow checks

Known remaining failures:

- TDD ratchet is not initialized (`.test-status.json` missing).
- Auto-update integration is not implemented.
- Manual attestations are missing for agentic concerns.
- Release, CI-green, Pages, version artifact, and install-link checks need the
  public DNS/custom-domain cycle. Release and GitHub Actions Pages deployment
  now exist.
- Standalone publishability still needs the shared workspace path dependency
  story to be resolved.
- Devenv was fixed after the first run by adding the standard `.gitignore`
  entries; rerun standards to confirm.

## Open Questions

- The eventual user-facing command name is still undecided.
- The first scenario DSL shape is not designed yet.
- Imported source notes are preserved as raw source material; curated canonical
  requirements have not yet been extracted.
