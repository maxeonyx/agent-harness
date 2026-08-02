# User turn — design scoping

Provisional. Derives from `source-notes/user-turn.md` (the primary source), `source-notes/context-and-agent-loop.md`, `source-notes/context-updates.md`, and the walking-skeleton rulings in `REQUIREMENTS.md`. Marked per part: **settled** / **open** / **experiment**. Scheduling note from the user kept in PLAN.md: this experiment "requires a lot of hands on from me" — the design can be revised now, the experiment pulled when hands-on time exists. The last section organizes the experiment around that constraint.

## The idea, restated plainly

Today, collaborating with an agent means narrating: you do something out of band, then type a message describing what you did. The user turn removes the narration. The user opens files, runs commands, and searches *inside the session*, through user tools; the agent sees what happened the same way the user sees the agent's tool calls — as activity in the shared history, clearly framed as the other party's. "The point is that the user and the agent should be on the same page about the history, not that they should see exactly the same stuff." **Settled** — this is the thesis.

## The turn: staging and sending

A user turn is not one text message. It accumulates *parts* — typed text, file-tool activity, terminal commands and their output, searches — and is sent as a whole when the user ends the turn. Parts stay distinct on the wire ("I guess it probably sees them differently... Yeah - we keep the distinciton. And user tool calls are multiple message parts"). **Settled.**

The input bindings, now specified by the user (2026-08-03, wording in PLAN.md):

- **shift+enter** — newline (delivered via kitty escape sequence, which the user has configured in his Windows Terminal — so the harness must speak the kitty keyboard protocol to distinguish it from plain enter)
- **enter** — stage the current text as a part
- **enter with no content** — submit the turn
- **ctrl+enter** — submit too, "if possible" (same kitty-protocol dependency; terminals without it can't send this distinctly, hence the hedge)

So a natural typing flow is: type, enter (staged), type, enter (staged), enter (sent). Staging is also what user tools do implicitly — running a terminal command stages a part. **Settled as requirement; experiment for terminal-protocol edge cases.**

It's worth noticing that staging isn't a UI affordance bolted on — it's the visible face of the request-trigger discipline. The context accumulates freely; requests happen only at defined moments. The keybinding design and invariant 2 are the same idea at different layers.

Hotkeys drop the user into tools from the chat-shaped main view: `$` opens the terminal ready to type (esc returns, leaving the `$` typed so `$` is still typable normally), `@` opens the file tool. The search binding is unresolved. **Settled/cosmetic-open.**

## What never happens: requests from passive activity

Invariant 2 applies with full force here. Opening a file, editing, searching, terminal output arriving — none of it triggers an API request. It piggybacks: if a request is in flight or the tool loop continues, accumulated user activity rides along; otherwise it waits for turn end. Only the four triggers (turn end, tool-loop continuation, cache-nearly-expired handover, explicit resume) cause requests. **Settled**, proven in principle by the walking skeleton, and the core wire assertion of this experiment.

The notes also want the transcript "written live, eg. when the user opens a file, and is still looking at it, the agent might get to know this to aid collaboration". These two statements can look contradictory; they aren't, but the reconciliation deserves one clear sentence: **live means the session record is current, not that the model is poked.** The transcript accumulates in real time in the durable session state, and whenever the *next legitimate request* happens — for any of the four reasons — it carries the freshest state, including "user currently has X open". The agent working through a tool loop while the user pokes around genuinely does learn what the user is doing, one tool-step later. An idle agent learns it at turn end. Nothing needs to be pushed. **Settled once stated.**

## Every tool owns two projections

Each user tool produces two things, and the tool owns both:

1. **The UI** — rich and interactive: the file explorer, the rendered file, the live terminal.
2. **The transcript** — the compressed context form: what the user did and saw, "framed clearly as 'useful context on what the user is doing'".

The compression rules from the notes, kept with their emphasis: all the important information survives, including what the user looked at but *didn't* use ("NOT just the decision outcome" — the agent should know what the user considered); purely visual detail, irrelevant info, and intermediate states the agent doesn't need can go. **Settled as principle; experiment for the actual compression quality per tool.**

The notes draw the corollary explicitly: agent tools own both projections too — they already have a context form, and they should own their UI rendering as well. One symmetric rule: *a tool owns how it appears to the user and how it appears to the model.* This is invariant 3 (every event projected per consumer) applied at the tool level. **Settled.**

Where projection code lives and runs — a tool is code, and it renders into a face UI while compressing into brain-held context — is a real placement question that belongs to the ts-vs-rust boundary doc. Flagged there; not resolved here.

## Who executes what

The walking-skeleton rulings give the ownership frame: each participant owns one external world — the face owns the TUI, the limb owns the environment (filesystem, processes), and a synchronous tty takeover such as an editor is face-owned in-flight work (the face keeps selecting, never blocked blind).

Applied to user tools, the natural split, **open, proposed for review**:

- The **face** owns the interaction: panes, keybindings, the editor takeover, rendering both user-tool and agent-tool UI projections.
- The **limb** owns the effects: the actual file read/write, the actual command execution, the search over the actual repo. A user edit lands in the same environment, under the same ownership-and-cleanup rules, as an agent edit would.
- The **brain** owns neither; it records. User activity facts enter the session record like all facts, and projection into model context happens where context is assembled.

Invariant 10's deliberate exception matters here: a face and limb commonly *do* share the user's machine, and the common case (you, at your desk, working in a repo) is exactly that co-location. The design must still work split — a remote limb means the file tool edits remote files, which is a feature, not an accident — but nothing should make the co-located case slower or weirder. **Open.**

The terminal tool has an extra note kept intact: persistent rather than ephemeral, probably; and "quite keen on it being not an *actual* bash / fish terminal" so that it could be forked and undone along with message history — "totally a stretch goal though," same for a REPL tool.

## The tools themselves

**Settled as a wishlist, each tool its own design-and-UX effort:**

- **File tool** — file explorer; selecting a file opens an interactive editor; files open fully collapsed; tracks changes as diffs; tracks what the user *looked at*, including explorer navigation and find-in-file commands.
- **Terminal tool** — run commands; persistent (probably); the forkable non-bash idea above.
- **Search tool** — search the workspace; the history of what was searched and what was found is part of the transcript.
- **GitHub tool** — an interactive `gh`-style surface for PR descriptions, comments, reviews, diffs — "ideally we just integrate... an existing tool for this," but it must be trackable inside, "so may need to fork - this goes for other tools too".
- **Subagent tool** — the user launches subagents directly ("find me that nix issue where XYZ"); prompt and result join the user's turn; forked and fresh both supported; forked warns if the cache is likely expired (with the note that forked can still be cheaper even then — "User can judge"); only what the *user* saw needs attaching, not what the subagent saw.
- **Voice** — "Ideally we might also record/transcribe the user's voice at the same time" — kept as the aspiration it is.

The general rule for how many tools: "as many tools as needed to allow the user to fully make decisions 'in band'".

## Conflicts

The user wins. An agent update to a file the user has open or has edited since the agent last saw it *may* be rejected — but the notes' restraint is part of the design and is kept verbatim: "I don't think we should be too eager about that. Maybe only if the updates actually conflict. We don't want to overprescribe live collaboration. The fact that the agent gets to observe the user is already a massive win." So: detect genuine conflicts, prefer the user's version, and otherwise stay out of the way. Invariant 7's multi-client machinery (stale sends representable, never silently overwriting) is the deeper mechanism, but this experiment only needs the smoke-test version. **Settled posture; experiment for the detection boundary.**

## Framing, and the tool-set boundary

User tools are not agent tools. The model's context must make its own tool set unmistakable, so the agent never tries to call the user's tools; user activity arrives framed as user activity, never as agent tool calls (one projection of invariant 3). **Settled.**

A quiet but important case: the user edits AGENTS.md (or a skill, or process config) in-band. The harness sees the edit like any file edit — but it also *knows what that file is*, records that process context changed, and handles it honestly against the warm cache: an append-mode change notice now, canonical new content at the next rebuild/new session ("the harness knows & can update it & know that next handover / next new session it should use the new content"). The mechanics belong to context-updates; this experiment establishes that user activity feeds that machinery. **Settled intent.**

## Interactions with other experiments

- **context-updates** — the AGENTS.md case above; user activity is a main producer of the change notices that experiment designs.
- **forked-subagents** — the user-side subagent tool is that model surfaced to the user; the fork-staleness warning is shared logic.
- **multi-client-ui** — which file the user has open, draft turn state, pane layout: exactly the shared-UI-state class that experiment owns. This experiment can keep them single-client and local; it must only avoid designing them *into* model context by accident.
- **limb-context** — user tools act within the session's limb environment (the proposed ownership split above assumes so). Whether user-tool availability varies per limb is a limb-context question. Flagged.
- **compaction-handover** — does accumulated user-activity context survive a handover, and in what form? Probably as part of what the handover's what-stays/what-leaves contract covers. Flagged as open there.
- **ts-vs-rust** — user tools are the sharpest test of the projection-code placement question.

## Experiment shape under the hands-on constraint

Two layers, deliberately separable:

**Agent-verifiable (front-load; wire assertions against the fake provider):** activity piggybacks and never triggers requests; turn end carries accumulated parts as distinct message parts; user-tool output framed as user activity; the agent's tool set excludes user tools; large outputs compressed within bounds; looked-at-but-unused context present in minimal form; stale-edit user-wins smoke test. All of this can be driven programmatically through the public surfaces without Max present.

**Hands-on (batch for when the user has time):** the feel of staging and the keybindings (kitty protocol across his terminals), the editor takeover, collapsed-file navigation, terminal persistence, whether the compression actually reads as "what I was doing" to the person who did it. These sessions should be scheduled deliberately, with the agent-verifiable layer already green so the hands-on time is spent on UX, not plumbing.

Exit (from PLAN.md): the user-tool contract is expressive and disciplined enough to build around.

## The matrix

Levels, statuses, and aspect definitions per `README.md`. The Why column is the motivating story. Blank = not addressed.

| Aspect | Why (the story) | Behavior | Mechanics | Verified | Interacts with |
|---|---|---|---|---|---|
| Model framing | the user checks three issues and picks one; the agent should know what was rejected, not just the outcome | S framed as user activity; S looked-but-unused kept minimal | P "live" = record current, not model poked | E compression quality per tool | context-updates |
| Wire & cache | user activity is constant; if it triggered requests, cost explodes and the cache never stays warm | S four triggers only; S parts distinct on wire | | S triggers (walking skeleton) | compaction-handover |
| Tool surface | every out-of-band action today becomes a narration message: "btw I edited X and re-ran the tests" | S tool wishlist (file/terminal/search/gh/subagent/voice) | O each tool's own design | | forked-subagents |
| UX & input | enter-sends-too-early is a standing papercut; the user already configured kitty seqs in his terminal to escape it | S staging model; S keybindings (user-specified, 2026-08-03) | E kitty protocol edges; O search hotkey; O forkable terminal (stretch) | | |
| Ownership & placement | | P face owns interaction, limb owns effects, brain records | O projection code placement | | ts-vs-rust, limb-context |
| Lifecycle | the agent overwrites the file the user is mid-edit in | S user wins; S not-too-eager posture | E conflict detection boundary | | multi-client-ui |
| Storage | the transcript IS the collaboration record; lose it and the same-page thesis dies | | | | persistence-analytics |
| Economics | raw terminal/file dumps would swamp the very context the user is trying to enrich | | | | |
| Security | a model calling user tools impersonates the user in the shared record | S model cannot call user tools | | | |
| Testing & verification | UX needs the user's hands; wire discipline doesn't — split them so hands-on time goes to UX only | S agent-verifiable layer front-loaded; S hands-on batched | | | |
| Code shape | | P a tool owns both projections as one package | | | ts-vs-rust |
| Dev workflow & references | | S OpenCode fork is the UX source of truth; gh tool integrates an existing tool ("may need to fork") | | | |
| Core migration | | | | | |
