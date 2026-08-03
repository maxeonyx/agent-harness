# User turn — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why (user-involved) · what, interactions, summary (agent-drafted, unreviewed).** Derives from `source-notes/user-turn.md`.

User turn is a rethinking of what it means to collaborate with an agent. Instead of the user sending messages and asking the agent to show things, the user can act *inline* — open files, edit, run commands, search — and those actions attach to the conversation just as an agent's tool calls do.

## Summary

The user should be able to work *in band* — open files, edit, run commands, search — and have the agent stay coordinated without being told about it. The requirement underneath that is narrower and more precise than it first sounds: **mutual observation**, and specifically symmetry of *events*, not of content or depth. The agent should know *that* the user opened a file, looked at something, changed something — enough to ask about it, enough not to panic that somebody is editing the files and it isn't him — even where it never sees exactly what. That is the root everything else depends on. Once it holds, work stops being an XOR between the user doing it and the agent doing it, and most of the "btw I edited X, I ran Y" narration goes away.

One rule delivers all of it. **A user tool owns both projections** — the interactive surface the user acts through, and the compressed view the model sees — and both are projections of a single recorded fact stream, neither derived from the other. That non-derivation is the whole point: a model view derived from the UI would be a screen-scrape, full of chrome and missing whatever the UI chose not to show; a UI derived from the model view would show the user a compressed summary of his own work. This is invariant 3 applied to a new emitter, and it forces the facts to be *recorded* into the session record rather than merely rendered, because everything the model sees must be derivable from that record by any consumer.

A consequence decides a lot of structure: a user tool is not a component in one role, it is a triple spread across all three. Execution authority is the limb's without exception; the *attention* facts — what he expanded, what he searched for, what he is still looking at — are necessarily the face's, since the limb never observes an expansion; the brain owns what reaches the model. What binds them is the fact stream rather than a shared object, which is why the projection is assembled from two emitters rather than owned by one.

What the model's projection should contain is "the same important information, minus the noise": keep what informed the decision, so three files opened and one edited projects as three files; drop intermediate states, but only where the later state supersedes the earlier one entirely; drop visual chrome. The projection is also **live** — that a file is open right now is actionable, and that is what turns mutual observation into something the agent can use mid-task rather than only in retrospect. One test says whether a projection is good enough: could the agent answer "what did the user look at, and what did he change?" without asking?

Two details in the tools are load-bearing rather than cosmetic. Files opening **collapsed** is the mechanism that makes the file projection worth anything: it turns each expansion into an observable act of attention, so the projected signal is the set of regions the user chose to look at rather than the useless fact that he had two thousand lines on screen. And the GitHub tool generalises into the integration policy for every third-party interactive tool this harness might host, which is worth more than getting `gh` right: **a tool we cannot observe cannot own a projection, and a tool without a projection is not a user tool** — it is just a program the user ran.

On the wire, user activity is not assistant tool calls. It is a user message composed of several parts — the user's typed text, and clearly labelled blocks describing his activity, in the order things happened — because providers allow text in a user message rather than arbitrary structured blocks, and because the interleaving is what makes the reasoning trail legible. Activity may piggyback mid-tool-loop; staged-but-unsent text does not, because activity is a fact about the world that is useless if it arrives late while staged text is deliberate composition the user has not chosen to send. At the keyboard end of that same decision, submission has three states rather than two — drafting, staged, sent — which is how the user authors the part structure the model will see.

Conflicts get one asymmetry: the agent's writes are checked against what it read and the user's are not checked at all. Compare-and-swap on the agent's side rejects only genuine lost updates and needs no locking or presence protocol; the asymmetry itself is invariant 7 ("the user wins"), which is a decision rather than something the mechanism produces. Non-file conflicts are deliberately uncovered, and mutual observation is the mitigation — exactly the restraint the notes ask for: don't overprescribe live collaboration.

The concept is not a TUI feature. A UI projection is a **description of state**, not a sequence of terminal escapes, so a renderer realises it — TUI now, web later — and the model's projection is unaffected by which client the user is sitting in front of. That boundary costs almost nothing now and is close to unaffordable to retrofit.

The largest open risk is not mechanical: it is **how generous these projections should be**. Input is cheap compared to output and compared to repeated tool calling, which is what justifies carrying looked-at context at all — but appended material is re-read for the rest of the session, so the answer is a quantity to be measured rather than a position to argue, and this design cannot set it from the inside. The remaining thing only the user can judge is whether the three-state input actually feels right in the hand.

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

The agent should get what *informed* a decision, not just the result — "including what the user saw but didn't use." If the user looked at certain sections of a file to understand what was going on, it makes sense for the agent to see those sections too, even if the user didn't act on them: the agent will, in theory, need that same understanding.

**Input is cheap** — and the comparators matter, because the second one is what makes this a why at all rather than a preference. The user's wording (2026-08-04): input is cheap "compared to output", and "compared to repeated tool calling". The alternative to carrying what he looked at is not saving the tokens; it is an agent making repeated tool calls to rediscover the same material, which costs a round trip and a full cache-read pass each time and arrives at a worse answer. So carrying it is the cheap option, not the indulgent one.

That also fixes how the tension with permanence should be read. Appended material is re-read on every subsequent request for the rest of the session (context-updates' why #3), which looks like a counter-position and is not one. He ruled it a sizing question: "this is not about a versus b. It's about how much a versus how much b." He also observed that user activity piggybacks on turns that would have happened anyway rather than creating new ones — while noting he had not fully settled the point; his working-through is preserved verbatim in `INTERACTIONS.md` and should be read there rather than paraphrased here. So the design consequence is a *quantity*: how generous these projections should be, defaulted from measurement rather than argued.

## What

### One tool, two projections

Everything in this design comes out of one rule, so it is worth stating precisely before the tools that follow from it. The user's own phrasing: "The tools own both projections - so the user tool owns both the UI and the context compression / projection."

A user tool is therefore **not** a piece of UI that happens to log something. It is three things bound together: an interactive surface the user acts through, a stream of recorded facts about what the user did, and a projection from those facts into text the model sees. The UI and the model's view are siblings — two projections of one fact stream — and neither is derived from the other. That is what invariant 3 already says about all activity ("an event is about its emitter, not *for* anyone; consumers project it"), applied to a new emitter.

Getting this the wrong way round is the failure mode worth naming. If the model's view were derived from the UI, it would be a screen-scrape: full of visual noise, missing everything the UI chose not to show, and impossible to render differently for a web client. If the UI were derived from the model's view, the user would be looking at a compressed summary of his own work. Both projections must come from the facts.

Two consequences follow immediately, and both are load-bearing.

First, **the facts have to be recorded, not just rendered**, and they have to be recorded into the session record. Invariant 10 is blunt: "Everything the model sees must be derivable from the session record by any consumer." So a user tool cannot keep the interesting part in its own head and hand the model a string; it records what happened and the projection is a pure function of the record. This is also what makes the projection reviewable, testable at the wire surface, and re-renderable when a second client attaches.

Second, and less obvious: **a user tool is not a component in one role — it is a triple spread across all three.** The face owns the interactive surface, because the face owns the TUI as its external world. The limb owns execution, because the limb owns the filesystem and processes. The brain owns what reaches the model. So "the file tool" is a face part, a limb part, and a projection, and the thing that binds them is the fact stream rather than a shared object.

That has one sharp practical edge. Invariant 10's by-design exception says a face and a limb commonly *do* share an environment — the user's machine — which makes it tempting to have the face's editor read and write files directly. Two questions get fused there and they have different answers.

**Authority is the limb's, without exception.** Every write and every command execution goes through the limb, and every *execution* fact is recorded by it. Three reasons: a remote limb otherwise cannot work at all; the limb is authoritative over truncation, diagnostics, and what context reaches the model from its domain; and the walking-skeleton ruling on split tool-fact recording puts execution facts on the limb side.

**Attention is the face's, necessarily.** "The user expanded this region", "he ran a find for that", "he is still looking at it" are things only the face observes — the limb never sees an expansion — so those facts are face-emitted whatever fetches the bytes. That matters because they are the highest-value content in the whole design (see the collapsed-by-default argument below), and it means the projection is assembled from two emitters rather than owned by one.

Which leaves fetching bytes for the screen as a third, smaller question: it can go through the limb, and for a remote limb it must. Locally it could be a face read — but then the limb's truncation and diagnostics do not apply and the model's view of a file diverges from what an agent read would have produced, so the default here is through the limb, and the cost is that an interactive editor over a remote limb is a materially harder problem. What stays face-local either way is the *editing buffer* — the unsaved state, the cursor, the scroll position — which is face-local UI state and, where it needs to be shared between clients, multi-client-ui's problem rather than this design's. This split is mine, not the notes'; see Questions for review.

The note draws a corollary that is worth keeping visible even though it points outward: if user tools own their UI projection, then **agent tools should too** — "and do already - but why not make this more explicit!". Which means the two-projection rule is not a special accommodation for the user. It is the general tool contract, and user tools are the case that forces it to be stated.

#### What "the same information, minus the noise" actually means

The note is specific about what the model's projection should contain: "all of the same important information, including what the user saw but *didn't* use, but can exclude any purely visual or irrelevant info, or can exclude intermediate states that the user doesn't need."

That splits into three rules, and the middle one is the interesting one.

**Keep what informed the decision.** This is why #4, and it is the rule that makes these projections different from a diff. If the user opened three files and edited one, the projection carries all three, because the agent will in theory need the same understanding the user built. Why #4's justification is that input is cheap *compared to repeated tool calling* — the agent would otherwise re-read these files itself, at a round trip each — which is a licence to be generous with *content* and none at all to be generous with noise. And the generosity is a dial, not a principle: whatever is appended is re-read on every subsequent request for the rest of the session, so how much to carry is the sizing question why #4 records, to be defaulted from measurement.

**Drop intermediate states.** Fifteen saves of the same file are one diff. A command run, corrected, and re-run is arguably one command — though this is exactly where the rule needs care, because the *correction* is sometimes the informative part, and a failed command that taught the user something is not noise. My reading is that intermediate states collapse when the later state supersedes the earlier one entirely (successive saves) and survive when the earlier one carries information the later one does not (a command that failed). The notes do not draw that line; I am proposing it.

**Drop visual chrome.** Frames, colours, cursor positions, scroll offsets, the layout of the explorer pane. Nothing the model can act on.

The projection is also **live**, which is a real requirement and not a nicety: "written live, eg. when the user opens a file, and is still looking at it, the agent might get to know this to aid collaboration". So the fact stream includes *open* and not just *closed*, and the projection can say "the user currently has this file open at this function". That is what turns mutual observation (why #1) into something the agent can act on mid-task rather than only in retrospect.

A single test tells you whether a projection is good enough, and it is worth writing down because it is testable: **could the agent answer "what did the user look at, and what did he change?" without asking?** If it has to ask, the projection is too lossy. If it asks about something in a wall of noise, the projection is too big.

### The tools

#### The file tool

An explorer; selecting a file opens an interactive editor; files "open in fully collapsed view to start with". Tracked: the diff of what changed, what the user looked at, the explorer navigation itself, and any `find` within the file.

The collapsed-by-default detail looks like a UI preference and is actually the mechanism that makes the whole thing work. If a file opens fully expanded, "the user viewed this file" is a single useless fact — technically he saw two thousand lines, informatively he saw nothing. Opening collapsed makes **expanding an observable act of attention**: each expansion is the user deciding *this* part matters. So the projection's most valuable content is the set of regions the user expanded, and the collapse default is what generates that signal. That inference is mine, joining two clauses the note states separately, but it is the reason the two clauses belong together.

So the projection for a file, concretely: the path; the regions the user expanded, with their content, deduplicated and in the order first opened; any find queries and what they matched; a unified diff for anything edited; and, while it is open, the fact that it is open and roughly where the user is. Explorer navigation projects more coarsely — which directories were browsed is a weak but real signal of what the user was looking for.

Size is the obvious risk: a user who expands most of a large file produces a large append. Truncation is the limb's job (`source-notes/agent-harness-design.md` names truncating large tool outputs as limb-owned execution context), so the file tool's projection is subject to the same truncation policy as an agent's read, rather than inventing its own. `P`.

#### The terminal

"terminal tool. run command in terminal. terminal should be persistent rather than ephemeral, probably." The hedge is the user's and stays: persistence is the lean, not a ruling.

Then the ambitious part, which the note itself marks: "quite keen on it being not an *actual* bash / fish terminal, as it would be ideal to be able to fork / undo it with the message history. This is totally a stretch goal though. Likewise, REPL tool too that works the same way." **This stays a stretch goal.** It is worth understanding *why* it is attractive, because the reason is a genuine consequence of the rest of the design: if a session can be forked, and the terminal is a real shell process with real accumulated state, then a forked session inherits a conversation but not a shell — so the two halves of the session's state fork differently. A terminal that is really a replayable command history forks and undoes with the conversation and keeps the whole session one coherent thing. That is a real prize, and it is also a large piece of work with a hard core (any command with side effects outside the harness cannot be undone by replaying anything), which is presumably why the user parked it. So: near term, a persistent shell process owned by the limb; the fork/undo semantics remain a stretch goal, and the design should avoid assuming shell state is forkable.

The projection: commands, their exit status, and their output. Two things the notes do not settle. **What to do with very long output** — the user may have scrolled through two hundred lines of a ten-thousand-line dump, and by the why-#4 rule what he *looked at* is the interesting part, which suggests projecting a truncated head and tail plus any region he actually scrolled to; that is a proposal. And **whether the user's terminal shares state with the agent's command tool** — in particular whether a `cd` in the user's terminal affects where the agent's commands run. My lean is no, they are separate: the agent's calls are one-shot and the user's terminal is his own persistent session, with its current directory projected as a fact so the agent knows where the user is working. Both are in Questions for review.

#### Search

"search for stuff. show the history of what the user searched for and show what they looked for and found." The projection is the query, the result set (paths and matched lines), and — the part worth adding — **which results the user then opened**, because that is where search hands off to the file tool and it is the difference between "he searched for `handover`" and "he searched for `handover` and went to these two of the nine hits". Preserving that chain is why #4 in its purest form: the reasoning is visible in the path taken through the results, not in the outcome. `P`.

#### GitHub, and the general problem of hosting somebody else's tool

The note wants "a github tool for the user - an interactive gh terminal to view PR desc, comments, reviews, diff etc." and then names the real constraint in parentheses: "ideally we just integrate (BUT need to track what's going on inside, so may need to fork - this goes for other tools too)".

That parenthetical generalises into the rule that governs every third-party interactive tool we host: **a tool we cannot observe cannot own a projection, and a tool without a projection is not a user tool** — it is just a program the user ran. Three integration strategies follow, in decreasing order of preference:

Drive the tool's **machine-readable surface** under our own UI. We own both projections outright and there is nothing to observe, because we are the one making the calls. For `gh` specifically this is unusually attractive, because `gh` has a clean non-interactive JSON API surface and the interactive parts are the least valuable bits. So the likely shape is our own PR viewer over `gh --json`, not a hosted `gh` TUI. `P`, and a departure from the note's "interactive gh terminal" framing in favour of what the note itself asks for underneath it.

**Host the tool's own interface and observe it** — scraping a TUI. Fragile, and it produces a projection made of rendered output, which is exactly the screen-scrape failure the two-projection rule exists to avoid.

**Fork it** so it emits facts. The note's own fallback. Real cost, real ongoing maintenance, and worth it only where the tool's interactive experience is the thing being bought.

The same three-way choice applies to a browser, an editor, a database client — anything the user might want in band. Which means it is not a GitHub question; it is the integration policy for the whole class, and getting it stated once is worth more than getting `gh` right.

#### A subagent, as a user tool

The user is clearly pleased with this one: "one obvious user tool is a subagent tool!! e.g 'find me that nix issue where XYZ' and then the user's prompt + the subagent's response is included. Yeah, that's really great."

The mechanics are mostly inherited. Both forked and fresh are supported. Forked warns if the cache is likely expired, and then **the user judges** — with the note's reasoning preserved because it is a good argument that cuts against the obvious one: "note that forked can still be cheaper even if cache expired, if the model would have to do many sequential tool calls to get back up to speed. User can judge." So the warning is information, not a block. It does need cache-state prediction, which is the same machinery compaction-handover and context-updates need.

The projection is unusually easy, and the note says why: "In this case we don't have to attach what the subagent saw - only what the user saw." So it is the user's prompt plus the subagent's response, and nothing else — which also happens to match how subagent results already work in this harness (a parent sees only the child's final result, invariant 6).

The interesting part is that this is the **only user tool that spends model tokens**, and it brushes invariant 2 hard enough to need an explicit resolution. Invariant 2 says passive user activity never triggers a request. Launching a subagent plainly causes requests. The resolution is that the invariant's word is *passive*: the user explicitly launching a subagent is not passive activity, and in any case the requests belong to the **child** session, not the parent. What lands in the parent is the prompt and the result, appended — and *that* is passive with respect to the parent and piggybacks like everything else. No exception to invariant 2 is needed, but the distinction should be tested rather than assumed, because it is the one place in this design where a user keystroke costs money.

Two things left open. Whether the user's subagent participates in the parent's structured-concurrency scope or sits outside it entirely: the parent agent is not blocked on it in any meaningful sense (the *user* launched it), which suggests outside, but that makes it the one unstructured piece of concurrency in a design that is otherwise proud of structure. And whether the user's subagent can itself be user-facing, which forked-subagents' why #7 (no manufactured obligation without consent) has views about. Both flagged.

### How user activity reaches the model

Three things fix the shape here, and they combine into one answer.

The note rules that **user tool calls are multiple message parts**, having talked itself through the alternative: "ultimately we're sending one message part, I think? Although maybe we send multiple user message parts to the model. I guess it probably sees them differently, so it probably makes sense to keep the distinction. Yeah - we keep the distinciton."

Invariant 3 and the note both require the activity to be framed as **user activity, not agent tool calls**. So this does not go on the wire as assistant `tool_calls` with `tool` results. It goes as content within the user's turn.

And providers, in practice, allow a user message to contain text (and images), not arbitrary structured blocks. So the realistic wire form is **a user message composed of several parts: the user's typed text, and clearly-labelled blocks describing his tool activity, in the order things happened.** Ordering matters more than it looks: interleaving typed text with activity in real order is what lets the model see "he said this, then looked at that, then said this", which is the reasoning trail of why #4 rather than a pile of attachments.

One structural consequence, from behaviour the walking skeleton already proved: user activity can **piggyback mid-tool-loop**, arriving as a user message after a tool result rather than at turn end. So a single episode of user work may be split across several user messages on the wire — some piggybacked, the rest flushed at submit. That is fine and already tested, but it raises a question the notes do not answer, and it is a good one: if the agent is mid-loop and the user has *staged* text (below) but not submitted, does the staged text piggyback too?

My proposal is no, and the split is principled. **Activity piggybacks; staged text does not.** Activity is a fact about the world, and why #1's whole point is that the agent should not be left wondering who is editing the files — so telling it late defeats the design. Staged text is deliberate composition that the user has not chosen to send; flushing it early would send half a thought, and would make staging unusable for its actual purpose. See Questions for review.

### Getting in and out: sigils, and the three-state submission scheme

"The main harness view should stay a chat UI, but then a hotkey would put the user into the tool." Named: `$` for the terminal, opening it "ready to type a command"; `@` for the file tool; and the search hotkey is explicitly undecided ("Not sure what for search") — so it stays undecided here, with the constraint recorded that it must not collide with the command prefix or with a character that appears often at the start of ordinary prose.

The sigils have an obvious problem, which the note notices and solves in a way I cannot unambiguously read: "esc to return to the harness view would leave the $ in the terminal so that the user can actually type $ normally". The requirement is clear — **`$` must remain typeable in a chat message** — but two mechanisms fit the words, and I am not going to guess between them:

- The sigil only fires when the message buffer is **empty**, so it behaves like a leading sigil and `$` mid-message is an ordinary character. Simple, and the "leave the `$` in the terminal" clause then describes the terminal retaining its state across an escape.
- The sigil fires always, inserts the `$` into the terminal's input line, and escaping back **carries it into the chat buffer**, so `$` followed by escape yields a literal `$` in the message.

The first is simpler and likelier; the second is closer to the literal wording. Question for review, with the user's wording preserved above.

#### The three states

The submission scheme is the user's fresher spec (2026-08-03, wording preserved): "I want shift enter (kitty escape seq, configured in my win terminal) to be newline, enter to stage, enter with *no* content to submit, and control enter (if possible) to be submit too."

This **supersedes** the older source note's "enter would not automatically send the message, it would be ctrl+enter", and the difference is not cosmetic: the new scheme introduces a third state the source note does not have. So the input has three states rather than two:

**Drafting** — there is content in the buffer being typed. Shift+enter inserts a newline within it, which depends on the kitty keyboard protocol; the user has this configured in his Windows terminal, and that dependency is real and worth carrying forward as a requirement rather than an incidental detail (a terminal that cannot deliver shift+enter distinctly loses multi-line drafting).

**Staged** — enter with content commits the buffer as a part and empties it. Staged parts accumulate. Nothing has been sent.

**Sent** — enter with an empty buffer submits everything staged; ctrl+enter submits from any state, if the terminal can deliver it (the user's "if possible" hedge is his, and it is a real terminal capability question).

Two observations that make this scheme better than it first appears, both of which are mine rather than the note's.

Enter stays the send key. That is the half of chat muscle memory worth keeping — the alternative the older note reached for was ctrl+enter, which retrains the primary gesture — while requiring a *second* press makes it structurally impossible to fire off a half-typed line by reflex, which is the actual complaint behind "enter would not automatically send the message". It is a deliberate departure from one-press-sends, not a match for it, and the departure is the point.

And staging is not merely a UX convenience: it is **how the user authors the part structure**. Since a user turn is multiple message parts, and the model "probably sees them differently", staging is the user's control over where those boundaries fall — which means the three-state scheme and the message-part decision are the same decision seen from the two ends, one on the wire and one at the keyboard. That is why the newer scheme is an improvement rather than a complication.

What is not settled: whether staged parts can be edited or unstaged before submit (they have not been sent, so there is no protocol reason they cannot, and it is obviously desirable), and whether the tool-activity blocks interleave with staged text by timestamp or are grouped. Flagged rather than decided.

### Concurrency: one asymmetry, enforced by compare-and-swap

The note is deliberately restrained, and the restraint is the design: "we might reject an agent's updates to a file if the user currently has it open, or has edited it since the user last did so. I don't think we should be too eager about that. Maybe only if the updates actually conflict. We don't want to overprescribe live collaboration. The fact that the agent gets to observe the user is already a massive win."

`REQUIREMENTS.md` sets the floor from the other side — invariant 7 and the worker's needs: "the user wins on conflicting edits, and stale agent output never silently overwrites newer user work." So the design has to satisfy a hard rule with a light touch.

There is one rule, and it is an **asymmetry**: the agent's writes are checked against what it read, and the user's writes are not checked at all. Invariant 7 is what makes it asymmetric, and it is worth being explicit that the asymmetry does not fall out of any mechanism — a symmetric check produces "whoever is stale loses", which sometimes means the *user* loses, and that is the outcome the invariant forbids. So the asymmetry is the design decision, and compare-and-swap is only how the checked half of it is enforced.

**Agent writes carry the version they were based on.** The agent read the file at some content hash; when it writes, the write is accepted only if the file still has that hash. That is a compare-and-swap, and on this side it has exactly the properties both the invariant and the note want. It rejects only *real* conflicts — a genuine lost update, where the agent is about to overwrite a change it never saw. It permits everything else, including the agent writing to a file the user merely has open but has not changed, which is the case an eager rule would block for no benefit. And it needs no locking, no presence protocol, no live-collaboration model.

When an agent write is rejected, the tool result should say the file changed underneath it and show what changed. That turns a conflict into a small, well-framed context update rather than an error — and it is the same content a change notice would carry, arriving on the tool-result channel because that is where the agent asked the question.

**User writes are accepted unconditionally**, including where the agent has changed the file since the user last read it. That is invariant 7 applied directly. But "the user wins" and "the user is never told" are different things, and an editor showing him a file the agent has since modified is a real situation — the honest behaviour is to accept his write and *tell* him what he overwrote, so the losing side is visible rather than silent. Where that notification lives is multi-client-ui's problem more than this design's, but it should not be assumed away.

One case is not covered at all, and that is deliberate. **Non-file conflicts** — two writers on the same terminal, or the agent running a command that undoes what the user just did — are not addressed by content hashing and are not addressed here. Mutual observation is the mitigation, which is precisely the "don't overprescribe" position the note asks for.

### Two renderers from the start

"This concept is NOT just for a TUI - it's for a harness which could later also be a GUI. We build GUI/web support in from the start, even if unimplemented."

Read structurally, this is not a feature commitment, it is a constraint on where UI logic may live, and it is the same constraint the walking skeleton already ruled on for the face: "rendering != face innards", the TUI is an output port, not loop logic. Applied here: **a user tool's UI projection is a description of state, not a sequence of terminal escapes.** The tool says what should be on screen; a renderer — TUI today, web later — realises it. The fact stream is identical either way, so the model's projection is entirely unaffected by which client the user happens to be sitting in front of.

That is what "built in from the start even if unimplemented" buys, concretely: the abstraction boundary exists and has exactly one implementation. It costs almost nothing now and is close to unaffordable to retrofit, because retrofitting means unpicking escape sequences from tool logic.

The note also observes that this is where the concept gets *more* natural rather than merely portable: "This would be more natural in fact as the user may commonly want to use a web browser." A browser is a plausible user tool that is barely conceivable in a TUI and obvious in a GUI — and, by the integration policy above, an unusually hard one to observe. Keep the user's hedge: this is a "may commonly want", not a requirement.

### Voice, alongside everything else

"Ideally we might also record/transcribe the user's voice at the same time, so they can talk while they work and have that attached too." An *ideally*, and it stays one.

Structurally it is another fact stream with a projection, which is a good sign for the contract: timestamped transcript, interleaved with the rest of the activity in real order, so "he said this while looking at that" survives. It is passive by construction and therefore triggers nothing (invariant 2). Whether it lands as staged text or as piggybacking activity is genuinely ambiguous — it is *speech*, which is content, but it is produced *while working*, which is activity — and my lean is activity, since the phrase is "talk while they work". Flagged.

### The agent must know these tools are not its own

Short, and easy to get wrong: "The user tools are NOT the same as the agent tools, and we should make sure that the context is clear to the agent that it has a different tool set to the user (we don't want the agent trying to use the user's tools)."

Two mechanisms, both cheap. The activity blocks must be **self-describing** — legible as a report of what the user did, never as something callable. And the system prompt should **say so explicitly**: the user has his own tools, here is roughly what they are, you cannot call them, and what you *can* do is ask him.

That last clause is the useful half rather than a courtesy. Why #1 says the point of mutual observation is that it lets the agent **ask** — so the correct framing is not merely "these are not yours" but "these are his, and you may ask him to use them". An agent that knows the user has a terminal open can say "could you run that and show me" instead of guessing, which is the collaboration of why #2 in its smallest possible form.

The failure is directly observable and makes a good test: an agent that tries to call a user tool, or that narrates as though it can see the user's screen when it cannot.

### What this makes falsifiable

The thesis: if every user tool owns both a UI projection and a context projection over one recorded fact stream, the user can work in band and the agent stays genuinely coordinated — knowing what happened and what the user looked at — without any of that activity triggering a model request.

It is falsified if: the agent gets confused or alarmed by user activity rather than absorbing it, which is why #1's own failure story surviving the fix; the agent asks for something it was already shown, meaning the projection is too lossy; passive activity triggers a request, contradicting invariant 2; the conflict rule either blocks the agent routinely or lets a user edit be lost silently; the agent tries to use the user's tools; or the three-state input scheme turns out to be confusing in the hand, which only the user can determine and which is why `PLAN.md` records that this experiment "requires a lot of hands on from me tho".

One thing that looks like a falsifier and is not: the projections turning out large. Per why #4 that is a *sizing* result, not a refutation — the experiment's job there is to measure how generous is right and set the default, and only a projection so large it derails attention at *any* useful generosity would falsify anything.

Invariants touched: **2** (activity never triggers; piggyback and turn-end flush are the legal paths), **3** (two projections of one fact stream, and user activity framed as user activity), **6** (the subagent-as-user-tool and whether it sits inside a structured scope), **7** (the user wins, delivered by compare-and-swap on writes rather than by a special rule), and **10** (facts travel in the message; the face does not touch limb files even when it shares the machine).

## Interactions

### What this design owns, and what it borrows

This design owns the two-projection contract and everything downstream of it: the fact stream, the tools that produce it, the projections into text the model sees, the sigils and the three-state input scheme, compare-and-swap on writes, and the rule that a UI projection is a description of state rather than a sequence of escape sequences. Those are what the experiment has to demonstrate, and most of them are only demonstrable with the user's hands on them.

The things it borrows are worth being blunt about, because otherwise this experiment quietly grows to include half the portfolio.

Execution belongs to the limb. Reads, writes, commands, truncation of large output, LSP annotation, and the persistent shell process are all limb-owned, and this design consumes them rather than designing them — which is exactly the ruling in the "one tool, two projections" section, arriving here as a scope boundary rather than as a principle. One consequence should be stated as a scoping decision rather than left implicit: an interactive editor driven over a *remote* limb is a materially harder problem than one over a local limb, and this experiment should be scoped to the local case with the remote case recorded as untested. That is a call, not a ruling; it is in Questions for review.

Shared-live state belongs to multi-client-ui. Draft buffers, staged-but-unsent text, cursor position, which regions are expanded on *this* screen, and how any of that converges between two clients are all its deliverables. This design owns only what has finished being true.

The subagent primitive belongs to forked-subagents: Task and Resume, fork versus fresh, the launch rules, what a result is, and the outcome class. The durable event log, per-emitter sequencing, and the project and working-session notions that make timesheets possible belong to persistence-analytics. The transport and the boundary discipline belong to topology. Cache-state prediction, which the forked-subagent warning needs, is shared machinery — see `INTERACTIONS.md`.

### Multi-client-ui draws the line this design's projections stop at

The two designs meet at a single question — what the user *did* versus what he is *in the middle of doing* — and multi-client-ui's proposal answers it in a way this design can adopt directly: the same subject produces two events in two classes, distinguished by whether the thing has finished changing. The file the user opened, the regions he expanded, the edit he saved: finished, durable, projected. The cursor as it moves and the buffer as it fills: live, shared between his clients, never projected.

Applying that line exposes one thing this doc currently gets slightly wrong, or at least states too loosely. The file tool's projection is described above as including, while the file is open, "the fact that it is open and roughly where the user is". The first half survives the line — that a file is open is a fact the agent can act on, and it is what makes mutual observation live rather than retrospective. The second half does not, because a cursor position is precisely the live state multi-client-ui classifies as never projected. The reading that keeps both designs coherent is that the *regions the user expanded* are the projected signal — they are finished acts of attention, which is exactly the argument the collapsed-by-default section already makes — and the moving cursor is not. That is a narrowing of this doc's own claim and it goes to Questions for review rather than being edited in above.

The staged-text rule gets stronger from this pairing rather than weaker. This doc argues that activity piggybacks and staged text does not, on the grounds that flushing half a thought defeats what staging is for. Multi-client-ui arrives at the same place from an unrelated root: an unsent draft has no durable counterpart because it is not something the user did. One rule, two derivations, which is better evidence than the composition argument alone.

The second front-end is not this experiment's work. This design owes the abstraction boundary — a UI projection that a renderer realises — and multi-client-ui owes the proof that a second renderer can attach to it. That split is what keeps "we build GUI/web support in from the start, even if unimplemented" affordable.

### Forked-subagents: the user's subagent is the launch button, not a new primitive

The subagent-as-user-tool looked like it introduced the one unstructured piece of concurrency in a design otherwise proud of structure. Reading forked-subagents' stage 2 closely, it does not.

That design already has the user launching a user-facing session into a scope by pressing a button, with no model involvement at all, and it treats dynamic sibling sets — children added to a scope after the parent blocked — as a first-class case rather than an oddity, precisely because the user can do this. The subagent-as-user-tool is the same primitive exercised through a tool surface instead of a button. So it joins the scope the user is currently looking at, the parent resumes with a result it did not ask for, and the resume framing has to tell it what it is getting — which forked-subagents already requires for exactly this reason. Nothing is unstructured except the model's expectation.

Consent works out the same way. Why #7 exists so that an autonomous agent cannot manufacture a blocking obligation on an absent human; a user launching his own child is the person being obliged, so the rule is satisfied trivially and a user-launched child may be user-facing. Both of those are proposed answers to the open question above rather than rulings, and the question stands.

The cache-expiry warning on a forked user subagent is the one place this design touches cache-state prediction, and it needs the weakest possible form of it: a hint, shown to a human who then decides, with the note's own argument preserved — forked can still be cheaper even with a cold cache if the alternative is many sequential tool calls. A wrong prediction here costs a slightly misleading warning, not a wrong decision, which makes this the cheapest consumer of that machinery in the portfolio.

### Persistence-analytics: "in the order things happened" is the face's order, and it is recorded

An earlier version of this section derived a problem here — that because typed text is face-emitted and terminal output is limb-emitted, and invariant 10 forbids a shared clock across role boundaries, cross-emitter ordering was only causally approximate and the reasoning trail would "scramble the first time a limb was remote." The user rejected that as an invented constraint (2026-08-04): "the face has its own total order... the brain is in charge of recording... it stores the order that the face saw things. And, in fact, that order is recordable... because every new event that the face emits comes with... a back end time and a front end time. So it's after this time on the front end, after that time on the back end. It's a partial order. Sure. But that's not to say that the face doesn't have its own total order and that we can't remember that. Now primarily, this is about representation."

The error was treating the trail as something reconstructed *across* emitters when it is a fact *about one* emitter. The reasoning trail is what the user said and looked at, and the user says and looks at things through the face — so the face's own total order over "he typed this, this result rendered, he opened that" is the trail, observed directly by the thing that emitted it. No clock comparison across a role boundary is involved, remote limbs change nothing, and invariant 10 is untouched. What persistence needs from this is representational: a face event carries its emitter sequence (the total order), its own front-end timestamp, and its anchoring against what it had observed from the brain (the back-end time it was after) — a recorded partial order across emitters alongside an exact total order within each.

The rest of the relationship is ordinary consumption. User tool facts are events with the face or the limb as emitter, they carry the lifecycle classification, and the project and working-session notions that turn this activity into a timesheet are that design's to define.

### Context-updates: two facts about one edit

The user editing an AGENTS.md or a loaded skill mid-session produces an activity projection here and, potentially, a change notice there. That looks like duplication and is not, because they are different facts: this design reports that the user did something, and context-updates reports that something the agent's understanding depends on is now different.

Context-updates' diff-at-flush formulation is what makes the two compose with no coordination — if this design's projection already carried the new content into the context, the comparison against the world is empty and no notice fires. The development of that is in that doc; what belongs here is the scope boundary, which is that this design never emits change notices and never needs to know whether one was emitted.

### Compaction-handover, and the quantity that only measurement settles

User activity is what fills a context fastest, so in practice it is what makes a handover due. The relationship is one-directional: compaction decides what survives, and it asks for the user's own wording to be carried verbatim because paraphrasing an instruction loses intent in a way paraphrasing a tool result does not.

The other half — why #4's "input is cheap" against context-updates' arithmetic that appended material is paid on every subsequent request rather than once — was recorded as a conflict and is not one. The user reframed it (2026-08-04): "this is not about a versus b. It's about how much a versus how much b." So it is a sizing question, and what stage 3 adds is that neither of these designs owns the measurement: persistence-analytics' request-attempt table does, since cache-read share and cost per unit of work are queries over it. The generosity of these projections is a *tunable* whose default comes from that measurement rather than from argument here. (`INTERACTIONS.md` carries the reframing and the user's full working-through, including his hedge that he had not entirely settled whether the two arithmetics meet.)

### The thin and the empty

Self-modification puts almost all UI content in the soft middle, which means user tools are rapidly iterable — a good property for a design whose surface only the user can evaluate. By self-modification's own classification test the two-projection contract is shell, because other things depend on its shape, and an individual user tool is a soft-middle contribution to it. That is a pleasant confirmation rather than a constraint.

Topology matters in one narrow way. A user tool is a triple spread across face, brain and limb, so terminal output is precisely the traffic the optional direct face↔limb stream exists for — and since model-token streaming is deferred by explicit ruling, tool output is the only thing that fast path can currently be tested with. Modular-components matters only for testing: the face's output port is where a UI projection is asserted, and two faces in one process is what makes the multi-client half testable at all.

Oauth-credentials, cancellation-economics and layered-shutdown have no relationship with this design. Operator-lifecycle has one edge and it is not this design's to solve: a face refused on a version mismatch is told to reload, and the user's staged text must survive that, which is multi-client-ui's durability question about shared-live state rather than anything about user tools.

## Questions for review

- **The `$` sigil.** Your note's escape behaviour has two readings and I would not pick between them: sigil-fires-only-on-empty-buffer, or sigil-fires-always-and-escape-carries-the-character-back. The requirement (`$` stays typeable in a message) is clear either way. Which did you mean?
- **Execution authority is the limb's; attention facts are the face's; byte-fetching-for-the-screen is the open one.** Writes and commands go through the limb without exception. What the user expanded or searched for can only be observed by the face, so those facts are face-emitted whatever fetches the bytes. That leaves reads-for-rendering: I have put them through the limb so the model's view of a file matches what an agent read would have produced, at the cost of an interactive editor over a remote limb being a materially harder problem. Is that the right trade, or should a local face read directly and accept the divergence?
- **Staged text does not piggyback; activity does.** If the agent is mid-loop, I would send your file/terminal/search activity straight away but hold staged text until you submit. That preserves what staging is *for*, at the cost of the agent seeing your activity before your commentary on it.
- **The GitHub tool is probably not a hosted interactive `gh`.** Your note says "interactive gh terminal ... ideally we just integrate (BUT need to track what's going on inside, so may need to fork)". I have proposed our own PR view driven by `gh --json`, because then we own both projections outright and have nothing to observe. That is a real departure from the framing even though it satisfies the constraint you named.
- **Does the user's subagent sit inside the parent's structured-concurrency scope?** The parent is not blocked on it — *you* launched it — which argues for outside, but that makes it the only unstructured concurrency in the design. Related: may a user-launched subagent itself be user-facing, given forked-subagents' no-manufactured-obligation rule?
- **Terminal state and the agent's command tool.** I have assumed they are separate — your terminal is your own persistent session, and its current directory is projected as a fact rather than shared. Also unsettled: what a very long output projects as, where my proposal is head + tail + whatever you actually scrolled to.
- **Collapsed-by-default is the mechanism, not a preference.** I have read your two clauses (opens collapsed; tracks what the user looked at) as one idea: collapsing is what makes expanding an observable act of attention. If that is not what you meant, the file projection's most valuable content changes.
- **Intermediate states: where is the line?** I propose that a later state supersedes an earlier one only when it carries all its information — so successive saves collapse, but a command that failed survives, because the failure taught you something.
- **The conflict rule is an asymmetry, not just a compare-and-swap.** Agent writes are checked against the version they read; your writes are accepted unconditionally, because invariant 7 says you win — a symmetric check would sometimes reject *you*, which is exactly what the invariant forbids. So the asymmetry is a decision rather than something the mechanism delivers on its own, and it wants your eye as such. Two riders: I have proposed that when your write overwrites something the agent changed, you are *told* what you overwrote rather than it being silent; and non-file conflicts stay uncovered, which I think is right given "don't overprescribe", but worth confirming.
- **Voice attaches as activity rather than as staged text.** Leaning on "talk while they work". It is genuinely ambiguous — it is speech, which is content.
- **Search should preserve which results you opened**, not just the query and the hits, because the path through the results is the reasoning. That is my addition to the note.
- **The two-projection rule is being stated as the general tool contract**, not a user-tool accommodation — your own corollary about agent tools owning their UI projections points that way. If you agree, that has consequences beyond this experiment for how every tool is written.
- **I have scoped this experiment to a local limb.** User tools over a *remote* limb — an interactive editor driven across an SSH tunnel — is a materially harder problem, and I would record it as untested rather than carry it here. The design still forbids the face touching files directly, so nothing precludes it later; the experiment just does not prove it.
- **The live half of the file projection is too loose as written.** The what says the projection includes, while a file is open, "roughly where the user is". Multi-client-ui classifies cursor position as shared-live state that is never projected. My proposed narrowing is that the *regions you expanded* are the projected signal, being finished acts of attention, and the moving cursor is not. That is a reduction of the doc's own claim, so it wants your ruling rather than a silent edit.
- ~~Cross-emitter ordering.~~ Withdrawn 2026-08-04 as an invented constraint. The trail is the face's own total order — the user sees everything through the face — recorded with front-end and back-end time anchors. See the persistence-analytics interaction.
- **Why #4 now carries your 2026-08-04 wording, and it changes what the experiment measures.** The earlier draft had why #4 as a bare "input is cheap" and treated the tension with permanence as a conflict this design might lose. Both comparators are now in the why — cheap "compared to output", and "compared to repeated tool calling", the second being what justifies carrying looked-at context at all — and the tension is recorded as your sizing question rather than a conflict. Consequence: projection size is no longer in the falsification list, because a large projection is a measurement result and not a refutation. Confirm that is the reading you intended, since a why you were involved in has moved.
- **The user's subagent joins the scope, by the same primitive as your launch button.** Forked-subagents already lets you add a child to an open scope with no model involvement and treats dynamic sibling sets as first-class, so I have read the subagent-as-user-tool as that primitive through a tool surface. That dissolves the unstructured-concurrency worry and makes a user-launched user-facing child fine under the consent rule. It is a proposed answer to the open question above, not a replacement for it.

## Index

| Aspect | L1 | L2 | L3 |
|---|---|---|---|
| Model framing | P | §How user activity reaches the model, §The agent must know these tools are not its own | §What "the same information, minus the noise" actually means |
| Wire & cache | P | §How user activity reaches the model | |
| Tool surface | P | §The tools | §The file tool, §GitHub, and the general problem of hosting somebody else's tool |
| UX & input | S | §Getting in and out: sigils, and the three-state submission scheme | §The three states |
| Ownership & placement | P | §One tool, two projections | |
| Lifecycle | O | | §A subagent, as a user tool |
| Storage | S | §One tool, two projections | |
| Economics | E | | §What "the same information, minus the noise" actually means |
| Security | P | §Concurrency: one asymmetry, enforced by compare-and-swap | |
| Testing & verification | P | §What this makes falsifiable | |
| Code shape | P | §Two renderers from the start | |
| Dev workflow & references | | | |
| Core migration | | | |
