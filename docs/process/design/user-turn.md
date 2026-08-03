# User turn — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Currently at stage 1 (why).** Derives from `source-notes/user-turn.md`.

User turn is a rethinking of what it means to collaborate with an agent. Instead of the user sending messages and asking the agent to show things, the user can act *inline* — open files, edit, run commands, search — and those actions attach to the conversation just as an agent's tool calls do.

## Why

The whys form a dependency, not a flat list: one root enables the rest.

### 1. Mutual observation — the same page on history — *foundational*

This is the root the others depend on. The user and the agent should be **on the same page about the history** — "not that they should see exactly the same stuff." The failure it prevents: the user starts working in-band and the agent panics — "someone's editing the files and it's not me, what's going on?" — when it's just the user, but the agent has no way to know without being told. Being on the same page is what makes everything else possible.

The precise shape: it is **symmetry of events/existence, not symmetry of content or depth**. The agent should know *that* something happened — the user opened a file, looked at something, is working on a thing — even if it does not see exactly what. That is enough to let it **ask**, or stay coordinated. The point is not symmetry; it is **mutual observation**. (The user already has a clear view of the agent's tool calls; this is the mirror.)

### 2. Collaboration instead of XOR — *desire (enabled by #1)*

Today the user effectively has a choice: *he* does the work, or *the agent* does the work — one or the other. He wants it collaborative: load up the files, look himself, have the agent see what he's seeing and see his changes; then they're on the same page, the agent can continue, the user can interject. It flows a lot better. This is only possible once #1 holds.

### 3. Remove *some* of the out-of-band narration overhead — *desire/friction (enabled by #1)*

Today, changing something yourself means telling the agent "btw I edited X, I ran Y." With mutual observation you just do it and the agent sees it. Example: you edit AGENTS.md and the harness knows to use the new content next handover/session — no announcement. Note the hedge: this removes *some* overhead, not all.

### 4. Share the reasoning, not just the outcome — *quality*

The agent should get what *informed* a decision, not just the result — "including what the user saw but didn't use." If the user looked at certain sections of a file to understand what was going on, it makes sense for the agent to see those sections too, even if the user didn't act on them: the agent will, in theory, need that same understanding. And **input is cheap**, so carrying the looked-at context is affordable.

## Parked as "what" for later stages

- The tool set: file (explorer + editor, tracks diffs *and* what was viewed, incl. finds; opens collapsed), terminal (persistent; ideally forkable/undoable with history rather than a real shell — stretch), search (shows what was searched and found), a github tool (interactive `gh`; likely integrate/fork an existing tool), and a **subagent-as-user-tool** ("find me that nix issue where XYZ" — attaches the user's prompt + the subagent's response; only what the *user* saw needs attaching; forked or fresh, warn if cache likely expired).
- Each tool owns **both projections**: the UI/actions for the user, and the live transcript/context projection for the agent (all the important info incl. looked-at-but-unused, minus purely visual/intermediate noise). Corollary: agent tools should own their UI projections too (TUI + web).
- Two UIs per state (what the user sees vs what gets attached).
- Keybindings: `$` → terminal ready to type (esc leaves the `$` so it can be typed normally), `@` → file, search hotkey TBD; enter vs ctrl+enter and shift+enter — keep the distinction, and user tool calls are multiple message parts.
- Voice transcription while working (attach it too).
- GUI/web support built in from the start even if unimplemented (the user may commonly want a web browser).
- Concurrency: light touch — maybe reject an agent's edit only on a real conflict with a file the user has open/edited; "don't overprescribe live collaboration." That the agent observes the user at all is already the big win.
- The agent must know its tool set differs from the user's, so it doesn't try to use the user's tools.

**Interactions flagged for stage 3:** forked-subagents (the subagent-as-user-tool; forked vs fresh + cache-expiry judgement); compaction-handover (accumulated user-activity context is part of what a handover must decide to carry forward); context-updates (the user editing AGENTS.md/skills mid-session is exactly a context change the agent must be told about).
