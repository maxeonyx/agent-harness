# OAuth and credential handling — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why, what, interactions, summary (agent-drafted, unreviewed).** A targeted question rather than a broad design; expected to stop at L2 depth. Derives from `source-notes/anthropic-oauth-references.md`.

## Summary

The plainest root in the project: the user wants to drive the harness with the Claude subscription he already pays for, because a harness that only accepts metered API keys is one he will use less. Underneath that sits a second, less obvious requirement — the notes ask for a *history*, a timeline of the tricks that third-party implementations have accumulated, so that whether subscription access is a stable capability or an arms race can be assessed rather than assumed. That answer changes how much of the harness should depend on it. So there are two deliverables of quite different kinds: a small piece of architecture, and research.

The architecture is the boundary between the auth flow and a provider plugin. Provider implementations are meant to live in the soft middle and be edited live by an agent, and the notes' position is that such a plugin should ideally work "without actual access to the auth" — the flow runs outside it, and it is handed back a pre-authenticated fetch. Taken literally that is not a header-injecting wrapper, and noticing why is most of the design: if the plugin can hand the wrapper an arbitrary destination, it can point it at a host it controls and read the `Authorization` header out of its own logs. So the abstraction is a fetch **bound to a destination** — the plugin declares its base URLs, its non-secret headers and which auth flavour it needs, and the shell composes those with the credential it holds. The notes offer "a script or config" and those differ in strength: config, where the shell knows the flows and the plugin only names one, is the stronger boundary and the simpler code, so it is the proposed default with script as a recognised weaker escape hatch. Behind the fetch sit three things, one of them a real footgun. Refresh must be single-flighted, because some providers invalidate the previous refresh token when a new one is issued, so ten concurrent refreshes can lock the user out of his own subscription. Retry is one attempt after a refresh and then a clear error, not a loop. And accounting constrains the interface: the brain owns billing and rate limits, so the fetch must expose enough of the response for it to account while still never revealing the credential — compatible, but only if the fetch is a real boundary with its own return type rather than a passthrough. The honest framing throughout is the user's own: he rates sandboxing lowest of his priorities, so this is a clean interface first and a safety property second, and it is worth care only because it costs nothing extra.

Where the credential lives is the one decision that reaches into another design. Refresh tokens are durable state that must survive restart and must never reach a projection the model sees — that much is invariant 1. The complication only appears once persistence is designed: the session database replicates to every federated brain by default, because the user wants backups by default, so durable credential rows would ship his refresh token to every machine he owns as a side effect of a backup feature. The proposal is therefore that credentials live *outside* that database — the OS keychain where there is one, otherwise a permission-restricted file — which keeps the replication rule simple and true rather than introducing a per-table exception, at the cost of a second store to manage. A pleasant consequence is that the read-only meta limb then has nothing to accidentally expose, because the guarantee comes from absence rather than from a rule someone has to remember.

The research half is a survey, a timeline and a test. The survey covers the eight or so reference implementations named in the notes, and what makes it a method rather than a reading exercise is extracting the *same* facts from each — client id, endpoints, scopes, PKCE, device flow, headers, whether a first-party identity string is required, storage, refresh handling — so they are comparable and the common core is visible. The timeline's instrument is `git log` on the specific auth files at pinned commits, plus the issue discussion that explains why a change landed, producing a dated table and a reading of it: convergent additions across repositories within days of each other strongly imply an enforcement change, while idiosyncratic ones do not. The artefact must carry its own date prominently, because its whole purpose is to inform how much to depend on subscription access and it goes stale. Then the part the user's own reaction demands — he suspects the strictest implementation "comes from a stricter time" — which a catalogue cannot answer: a **subtractive test**, implementing the union of tricks, confirming it works, then removing each one to find the minimum that currently works. The difference between the minimum and the union measures how much of the existing complexity is historical. It has to run against the user's own subscription, more than once over a period, because a trick unnecessary today may not be next month. Two things stay open rather than being assumed: the references say nothing about Claude Team, which changes what "working" means and can only be settled against the user's actual account, and whether subscription-backed access bills differently from an API key is currently nobody's question.

## Why

### 1. The user wants to use the subscription he already pays for — *desire*

The plainest root in the project. The note's title is "Required to get claude sub working with other harnesses". A harness that can only be driven by metered API keys is, for this user, a harness he will use less, because his actual daily usage is subscription-backed.

Nothing subtle sits underneath this. It is a direct desire, and it gates whether the harness is usable as a daily driver rather than a project.

### 2. Whether this stays possible is a live risk, and the user wants it assessed — *risk*

The notes do something unusual and worth honouring: they ask for history, not just a working implementation. "I also ideally want a timeline of tricks added to these implementations to see how active anthropic has been on their oauth / third party harness restrictions."

That is a risk-assessment request. The question behind it is whether third-party subscription access is a stable capability or an arms race — because the answer changes how much of the harness should depend on it, and whether it belongs in the soft middle where it can be updated quickly rather than in the compiled shell.

The user also flags that some existing implementations look over-engineered for present conditions — "this seems solid but also I feel this comes from a stricter time and we'd do well to somehow figure out if loosening it is ok" — and reacts against the most convoluted approach found ("*shudder*"). So the deliverable includes a judgement about the *minimum* that currently works, not just something that works.

### 3. Credential handling should be principled because provider auth is the brain's job — *correctness*

Invariant 1 keeps provider credentials brain-owned — never reaching limbs, faces, plugins, tool schemas, logs or model context. That is already settled, and it is not motivated by security posture; it follows from the brain being the only role that talks to providers.

Self-modification adds a sharper requirement. Providers are intended to be *plugins* in the soft middle, edited live by an agent. The notes' position is that a provider plugin should ideally operate "without actual access to the auth" — the OAuth, device or token flow runs outside the plugin, and the plugin is handed back a pre-authenticated fetch wrapper. This is a real design constraint rather than a nicety, because agent-edited plugin code holding long-lived subscription credentials is the one place where the project's general indifference to security stops being comfortable.

Even so: the user rates sandboxing lowest of his priorities. The honest framing is that a pre-authenticated wrapper is a *clean interface* first and a safety property second.

## Forward: what this forces

- **The auth flow lives in the shell; provider implementations live in the soft middle.** The boundary self-modification defines already places the provider *API* in the shell and provider *implementations* in the soft middle — this design is what makes that boundary real for authenticated providers.
- **A pre-authenticated fetch abstraction** as the thing handed to a provider plugin, with token refresh handled behind it.
- **A survey with a recommendation, not just a copy.** Many reference implementations exist; per `AGENTS.md` and the reference-codebases note, the practice is to study and re-implement cleanly rather than copy.
- **A recorded timeline** of restriction-and-workaround history, as its own artefact — that is the risk assessment, and it dates quickly, so it should be dated.
- **Token storage with a lifecycle class**, since refresh tokens are durable state that must survive restart but must never reach a projection the model sees.

## What

Two deliverables, and they are different kinds of thing. One is a small piece of architecture — the boundary between the auth flow and a provider plugin. The other is research: a dated survey of the reference implementations and a timeline of how the restrictions have moved. Neither needs to go past L2, and the second is mostly a question of doing it properly rather than designing it.

### What makes a fetch "pre-authenticated"

The notes' position is that a provider plugin should ideally operate "without actual access to the auth" — the OAuth, device or token flow runs outside the plugin, "it contributes a script or config, then we hand it back a pre-authenticated fetch wrapper or something."

Taken literally, a wrapper that injects a header is not enough, and noticing why is most of the design. If the plugin can hand the wrapper an arbitrary destination, it can point it at a host it controls, read the injected `Authorization` header out of its own server logs, and the boundary has bought nothing. So the abstraction is not "a fetch with auth added"; it is **a fetch bound to a destination**. The plugin gets an object that will only talk to the provider's own base URL (or an allowlist of them, since some providers use more than one host), with auth injected and refresh handled behind it, and no way to read the credential back out of anything it can observe.

The plugin's side of the boundary is then declaration rather than execution: it declares its base URL or URLs, the non-secret headers the provider requires (versions, beta flags, whatever the survey finds), and which auth flavour it needs. The shell composes those with the credential it holds.

The notes offer two readings and it is worth being explicit that they differ in strength. "Contributes a script" would mean the flow itself is plugin-provided while the credentials stay outside — the plugin gets to drive the dance but not keep the result. "Contributes config" means the shell knows how to run each flow and the plugin only names one. Config is the stronger boundary and the simpler code, and it is proposed as the default. Script is the escape hatch for a provider whose flow is strange enough that hard-coding it in the shell is worse, and it should be recognised as a weaker boundary when it is used, not silently equivalent.

The user rates sandboxing lowest of his priorities, so the honest framing stays as the why has it: this is a clean interface first and a safety property second. It happens to cost nothing extra, which is the only reason it is worth being careful about.

### What lives behind the fetch

Three things, and one of them is a real footgun.

**Refresh** must be single-flighted. If ten requests are in flight when a token expires, ten refreshes fire, and some providers invalidate the previous refresh token when a new one is issued — so the naive version can lock the user out of his own subscription by being concurrent. One refresh at a time, with the others waiting on it, is a small amount of care that avoids a bad failure.

**Retry** is one retry after a refresh on an auth failure, and then a clear error. Not a loop.

**Accounting** is the one that constrains the interface. The brain owns billing and rate limits, so it needs the response metadata that carries usage and rate-limit signals — which means the fetch surface has to expose enough of the response for the brain to account, while still not letting the plugin see the credential. Those are compatible, but only if the fetch is a real boundary with its own return type rather than a thin passthrough of whatever the HTTP library produced.

### Where the credential actually lives

Refresh tokens are durable state: they must survive restart, and they must never reach a projection the model sees, a log, or a tool schema. That much is invariant 1.

But there is a complication that only becomes visible once persistence is designed: the session database replicates to every federated brain by default, because the user wants backups by default. Durable credential rows in that database would ship his Claude refresh token to every machine he owns, as a side effect of a backup feature.

So the proposal is that **credentials live outside the session database** — the OS keychain where there is one, otherwise a separate file with restrictive permissions. That keeps the replication rule simple and true (everything durable in the session database syncs) instead of introducing a per-table exception to it, at the cost of a second store to manage. This is a change of home rather than a change of class, and it is in the questions because it lands on the persistence design too.

### The survey

Eight or so reference implementations exist, and per `AGENTS.md` and the reference-codebases note the practice is to study and re-implement cleanly rather than copy. The subjects, from the notes:

`nitishxyz/otto` (its `anthropic-oauth-client`), `leohenon/pi-anthropic-oauth`, `griffinmartin/opencode-claude-auth`, `cortexkit/anthropic-auth`, `leohenon/op-anthropic-auth` (and the note suggests checking that author for others), and an extensive set of files in `can1357/oh-my-pi` covering registry, oauth, auth utils, client, wire, messages-server, schema, an OpenAI→Anthropic shim, API-key validation, error classes, tool-choice, compat and identity classification, plus its provider docs and research scripts. `@zgltyq/pi-provider-claude` is not open source but is flagged as possibly worth downloading and inspecting. The `CLIProxyAPI`-based approach is explicitly disliked — "*shudder*" — and is in the survey only so the timeline is complete, not as a candidate.

What makes this a method rather than a reading exercise is extracting the *same* facts from each, so they are comparable: which client id, which endpoints, which scopes, whether PKCE, whether a device flow, which headers are set and which of those are imitating a first-party client, whether a system-prompt preamble or identity string is required, how the token is stored, and how refresh is handled. That comparison is what produces the answer to "what do these actually have in common", which is the thing worth re-implementing.

### The timeline, and the instrument for it

The notes ask for something the other references do not provide: "I also ideally want a timeline of tricks added to these implementations to see how active anthropic has been on their oauth / third party harness restrictions."

The instrument is `git log` on the specific auth files across those repositories, at pinned commits, plus whatever issue and PR discussion explains why a change landed. Each trick gets a date and, where the discussion says so, an apparent trigger. The output is a dated table — repository, commit, date, what was added, what it appears to have been reacting to — and a short reading of it: whether the additions cluster around particular months, whether they are convergent (everyone adding the same header within a week of each other, which strongly implies an enforcement change) or idiosyncratic.

The artefact must carry its own date prominently, because it goes stale, and its whole purpose is to inform how much of the harness should depend on subscription access. A conclusion of "very active, still moving" argues for keeping providers firmly in the soft middle where they can be updated in minutes. A conclusion of "one flurry two years ago and quiet since" argues for relaxing.

### The subtractive test

The user's reaction to the most solid-looking implementation is the part that shapes the deliverable most: "this seems solid but also I feel this comes from a stricter time and we'd do well to somehow figure out if loosening it is ok." A catalogue of tricks cannot answer that. Only removing them one at a time can.

So the survey's final step is subtractive: implement the union of tricks, confirm it works, then remove each one and see whether it still works. What remains is the minimum that currently works, and the difference between that and the union is the measure of how much of the existing complexity is historical. This has to run against the user's own subscription, since that is the only account whose behaviour matters, and it should be run more than once over a period, because a trick that is unnecessary today may not be next month — which is the same risk the timeline is assessing, seen from the other side.

### The gap: which subscription

The references are silent on Claude Team — the note says only "nothing about Claude Team sub mentioned. should work the same hopefully" — so the survey cannot answer it from source. The only way to know is to try it with the user's own account, which requires knowing which subscription he actually has. It is left as an explicit unknown rather than assumed, because it changes what "working" means.

### Thesis, falsification, and invariants

The thesis: **provider auth can live entirely outside provider plugins — the plugin declaring its endpoint and non-secret headers, the shell running the flow and handing back a destination-bound, refresh-handling fetch — and subscription-backed Anthropic access can be made to work through that boundary with materially fewer tricks than the strictest reference implementation uses.**

It is falsified if: some provider's auth genuinely requires the plugin to see the credential, so the boundary has to be broken rather than bent (a plugin needing to sign a request body itself would be the realistic case); a destination-bound fetch cannot expose enough response metadata for the brain to do rate-limit and billing accounting; single-flighted refresh is not sufficient and something about the provider's token rotation defeats it; or the subtractive test shows the tricks are all load-bearing, which would mean the strict implementations are strict because they have to be, and the risk assessment should be read pessimistically.

Invariants touched: 1 primarily and almost exclusively — this is the design that makes brain-owned credentials real for authenticated providers rather than merely asserted; and 5, because credential storage is durable state whose class and home are decided here.

## Interactions

This design is deliberately peripheral, and its sparseness is worth stating plainly because it is the reason it can run whenever convenient. It has one load-bearing relationship, one that resolves by removing something rather than adding it, and one that is already settled by an invariant and needs no coordination at all. Everything else in the portfolio is genuinely unconnected to it.

**What this experiment owns**: the destination-bound fetch abstraction and the boundary it draws; what lives behind that boundary, which is single-flighted refresh, one retry after a refresh, and a return type rich enough for the brain to do rate-limit and billing accounting; the decision about where the credential actually lives; the dated survey of the reference implementations; the timeline of restrictions and workarounds; and the subtractive test that finds the current minimum.

### Self-modification is the load-bearing link

Provider implementations are soft-middle plugins and the provider *API* is shell — that classification is self-modification's, and this design is what makes it real for authenticated providers. The mapping is exact and worth stating that way rather than as a general affinity: the destination-bound fetch is a **framework the shell provides**, and a plugin's declaration of its base URLs, its non-secret headers and which auth flavour it needs is a **contribution written within that framework**. That is the same shape as the replication protocol and the CLI-argument contributions in self-modification's sketch, which is some reassurance that the boundary is not being invented here.

The reason this matters rather than merely fitting is self-modification's own why #5, arriving with a different justification. Agent-edited plugin code holding a long-lived subscription credential is the one place where this project's general indifference to security stops being comfortable — but the argument for the boundary is not security, it is that the plugin's job is to describe a provider and not to hold a secret, which makes it a clean interface first. Self-modification reaches a structurally similar conclusion about its sandbox, where the value turns out to be fault attribution rather than isolation.

What this design assumes from self-modification and does not test: plugin loading, load-time validation, quarantine, rollback and the version pinning. A provider plugin is an ordinary plugin, and nothing about authentication needs a special reload path.

### Persistence interacts by subtraction

Credentials are proposed to live *outside* the session database — the OS keychain where there is one, otherwise a permission-restricted file — because that database replicates to every federated brain by default, and durable credential rows would ship a refresh token to every machine the user owns as a side effect of a backup feature. So the interaction is a removal rather than a contribution: persistence's replication rule stays simple and true, with no per-table exception, and this design gains a second store to manage.

There is a pleasant consequence worth recording. Persistence exposes the same query set through a read-only meta limb, and a limb must not project credential rows. If credentials are not in that database at all, there is nothing to withhold, and the guarantee comes from absence rather than from a rule someone has to remember. The same move multi-client-ui argues for with shared-live state.

Both docs carry this as a question because the ruling lands on both. And it has one consequence neither covers, raised in operator-lifecycle: a separate credential store is a durable thing with no version, snapshot or migration story attached to it.

### Topology needs nothing from this design

Credentials stay brain-owned because the brain is the only role that talks to providers, and invariant 1 already settles it. Topology's why #4 supplies the reason — rate limits, billing, session management and provider connection state should exist exactly once — and topology's own falsification list already includes provider credentials being observable anywhere but the brain, asserted in its `face+limb ↔ brain` configuration where the far side must demonstrably not hold them. **That assertion is topology's to run, and this design does not duplicate it.** The whole of the coupling is that neither design may quietly weaken the invariant.

### Operator-lifecycle: resolved rather than open

The interaction flagged from the other side was that a provider plugin update must not invalidate a live session's auth. It resolves cleanly: refresh state lives outside both the plugin and the binary, so neither a plugin reload nor a binary relaunch can invalidate it. The residual case is a change to the credential store's own format, which is the versioning gap above and is operator-lifecycle's rather than this design's.

### What turned out to be empty

Forked-subagents, user-turn, multi-client-ui, context-updates, compaction-handover, limb-model, layered-shutdown and modular-components have nothing to say to this design beyond the trivial. A limb never carries a credential, which is an invariant rather than an interaction. A subagent inherits its parent's provider access, which requires no mechanism. There is one line worth carrying to cancellation-economics from the other direction: that measurement is proposed on API keys only, holding subscription-backed billing constant rather than testing it, because whether a subscription bills differently is unknown and entangled with this work. Closing that gap is this design's if it matters, and it is not currently in scope.

## Questions for review

- Do you want the timeline artefact before or alongside a working implementation? It is the part that informs how much to depend on this, so arguably first.
- Is Claude Team the subscription that matters here? The references are silent on it and it changes what "working" means.
- How much should the harness degrade gracefully if subscription access breaks — fall back to API keys transparently, or fail loudly?
- **Credentials are proposed to live outside the session database** (OS keychain, else a permission-restricted file), because that database replicates to every federated brain by default and durable credential rows would ship your refresh token to every machine you own as a side effect of a backup feature. It keeps the replication rule simple at the cost of a second store. Same question appears in the persistence design.
- The "pre-authenticated fetch" is designed above as **bound to a destination**, not just carrying a header — otherwise a plugin can point it at its own server and read the credential out of its logs. That is cheap, but it does mean the plugin declares its endpoints up front rather than constructing URLs freely. Any provider you know of where that would be awkward?
- The notes say a plugin "contributes a script or config". Those are different strength boundaries — script means the plugin drives the flow but never keeps the result; config means the shell knows the flows and the plugin names one. Config is proposed as the default with script as a recognised weaker escape hatch. Agreed?
- The **subtractive test** — implement the union of tricks, then remove them one by one to find the current minimum — is proposed as the core of the survey rather than an extra, because it is the only thing that can answer "is loosening ok". It costs real requests against your own subscription. Worth it?
- **Should this design close the subscription-versus-API-key billing gap?** Cancellation-economics deliberately holds subscription billing constant because it is unknown here. Nobody currently owns finding out, and a subscription is what you actually use, so the gap is in the one configuration that matters most.
- Should this experiment also establish **whether subscription-backed access must degrade to API keys transparently or fail loudly**, given that decision affects the fetch abstraction's return type? It is listed above as an open question about behaviour; making it a deliverable would widen this experiment slightly.

## Index

| Aspect | L1 | L2 | L3 |
|---|---|---|---|
| Model framing | | | |
| Wire & cache | P | §What lives behind the fetch | |
| Tool surface | | | |
| UX & input | O | | |
| Ownership & placement | S | §Where the credential actually lives | |
| Lifecycle | | | |
| Storage | P | §Where the credential actually lives | |
| Economics | O | §The gap: which subscription | |
| Security | P | §What makes a fetch "pre-authenticated" | |
| Testing & verification | P | §The subtractive test | |
| Code shape | P | §What makes a fetch "pre-authenticated" | |
| Dev workflow & references | P | §The survey | |
| Core migration | | | |
