# Self-modification — the shell / soft-middle boundary — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Currently at stage 1 (why).** Derives from `source-notes/tech.md`.

**Terminology guard (the user corrected an error here):** *"core"* in this project means **the stuff finalized/incorporated from experiments** — a *maturity* axis, not a layer, and not "the Rust implementation". A plugin can be core; you don't know whether something is core until its experiment is done. The layering axis below (**shell** vs **soft middle**) is *orthogonal* to core-ness.

## Why

### 1. Rapid iteration and self-hosting — *desire (the north star)*

The point is rapid iteration and development *of the harness* — on whatever machine it happens to be running, no matter which one. Without bothering with source control, without deployment, without even files. Anything that is a plugin gets edited **directly in memory and immediately shared, over the same mechanism the harness already uses to share its data.** "The more's in TS, the more is boosted by self-modification." Forward: bootstrap self-modification as fast as possible (basic loop → agent edits code → relaunch onto the new code) so the harness starts building itself early.

### 2. But bounded — a thin shell must be compiled separately — *correctness/capability*

We can't do that for everything. The outer skeleton — the hosting binary, the web package / wasm module, the platform-specific "how data gets from A to B" — needs to be compiled separately. We should make that shell as **thin as we can**, and use Rust and/or JavaScript for it.

### 3. Never live-brick — *safety (top priority)*

The scar: the very first thing hit with Pi's self-modification was an agent self-edit that *immediately bricked the harness*. Really dumb. So: live editing, but **never live bricking** — exercise plugin code on load as much as possible, and revert to the old version if the new one crashes. You must be able to change yourself without dying.

### 4. Stable APIs the soft middle conforms to — *correctness (this is what makes self-mod safe *and* cache-stable)*

The shell provides stable **APIs / frameworks**; the soft middle provides **implementations that fit them**. The load-bearing example: the **context-provider API** — a plugin reload must not cause a cache break, which *implies* a stable API that context providers conform to (the system prompt, including tool defs, stays put across a reload; old tool calls remain usable until a deliberate handover/compaction or an explicit cache break). Same pattern for the replication protocol and CLI-argument contributions: the *framework* is shell, the *contributions* are soft middle.

### 5. Sandbox / secrets — *nice to have, lowest priority*

Sandboxing agent-edited plugins away from credentials (auth flow outside the plugin; hand back a pre-authenticated fetch wrapper; no arbitrary node modules) is nice, but the user does not personally prioritise it. Lowest on this list.

## The boundary (stage-2 "what" material — the user sketched it)

**Shell** (thin, compiled separately; provides frameworks, APIs, transport, rendering engines):

- website package, wasm module, hosting binary
- the CLI — though CLI *arguments* may have contributions from the soft middle
- getting data from place to place; the **replication protocol** (enforced by the shell); the framework one must write replication-protocol and CLI-arg contributions within
- the **provider API** — the interface provider implementations must fit
- the **rendering framework** for the TUI and for the web
- stable APIs generally (e.g. the context-provider API in #4)

**Soft middle** (plugins, edited live, rapid iteration):

- provider *implementations*
- tools, user tools, business logic
- **pretty much all UI content — even the TUI content** (not the rendering framework), because UI especially wants rapid iteration
- UI elements for a provider's usage/cost/etc — given a place in the UI, kept as flexible as possible
- exact data *formats* may be provided or extended by the soft middle (the transport and protocol are shell)

Principle: make the shell **as thin as possible**; push everything you want to iterate rapidly into the soft middle.

**How the editing happens: the meta limb.** Self-editing happens via the **"meta limb"** — the agent operates on the harness itself from that limb, rather than on a project. This is the concrete home of #1: the plugin-editing-in-memory that gets shared over the harness's own data mechanism is work done in the meta limb.

**Interactions flagged for stage 3:** the **limb model** (the meta limb is where self-modification is performed — self-modification is, in effect, an application of the limb model to the harness itself); compaction-handover & context-updates (the context-provider stable API and cache stability across reloads); persistence-analytics (plugins stored/versioned — "in the DB, perhaps" — so an old schema stays addressable while the cache is valid); and *every* experiment (whether its result lands in the shell or the soft middle, and separately whether it becomes core).
