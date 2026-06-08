# Limb Context Layers — Design Notes

> Status: **placeholder / stretch goal** — very incomplete

---

## Concept

The limb aggregates context from multiple sources and presents a unified environment to the agent. This is core to its role as the context and execution environment (see agent-harness-design.md).

Provisional context layer types (not exhaustive):

- **User-specific** — user preferences, prompts, agent personas, skills
- **User-and-machine-specific** — credentials, SSH config, shell config, per-machine user preferences
- **Machine-specific** — installed toolchains, available formatters/linters, environment variables. (Machine = user on system, not whole system. No use case for system-wide yet.)
- **Project-specific** — AGENTS.md, project config, ignored paths, git root, package manager, etc.
- **User-and-project-specific** — personal project preferences, custom prompts, per-project user settings

---

## Where does state live, and how does an agent edit it?

**Project config** — straightforward. Lives in the project. Agent edits it via the project limb (files in the repo).

**User config** — brain-local limb? The brain itself could expose a limb-like context environment for user-level configuration that agents can read and edit.

**Machine config** — another limb, maybe in the user's home directory on that machine. This is a natural fit since machine context is per-user-on-system and the home directory is where that state already lives.

---

## Open questions

- How are layers discovered and composed?
- What is the precedence/merge order when layers conflict?
- How does the brain know what context the limb has injected?
- Should context layers be declared to the brain (so it can reason about them), or opaque?
- How does this interact with skills/extensions?
- Does "brain-local limb" for user config blur the brain/limb boundary in a problematic way, or is it a natural special case?

---

> This area needs significant design work. Deferred.
