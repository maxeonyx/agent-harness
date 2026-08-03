# OAuth and credential handling — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why, what (agent-drafted, unreviewed) · interactions, summary — not yet done.** A targeted question rather than a broad design; expected to stop at L2 depth. Derives from `source-notes/anthropic-oauth-references.md`.

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

## Parked for later stages

**Interactions flagged for stage 3:** self-modification (provider implementations as soft-middle plugins with auth outside the plugin is the load-bearing link); topology (credentials stay brain-owned, so limbs never carry them — already invariant); persistence-analytics (token storage as durable state that is never projected to the model); operator-lifecycle (a provider plugin update must not invalidate a live session's auth).

## Questions for review

- Do you want the timeline artefact before or alongside a working implementation? It is the part that informs how much to depend on this, so arguably first.
- Is Claude Team the subscription that matters here? The references are silent on it and it changes what "working" means.
- How much should the harness degrade gracefully if subscription access breaks — fall back to API keys transparently, or fail loudly?
- **Credentials are proposed to live outside the session database** (OS keychain, else a permission-restricted file), because that database replicates to every federated brain by default and durable credential rows would ship your refresh token to every machine you own as a side effect of a backup feature. It keeps the replication rule simple at the cost of a second store. Same question appears in the persistence design.
- The "pre-authenticated fetch" is designed above as **bound to a destination**, not just carrying a header — otherwise a plugin can point it at its own server and read the credential out of its logs. That is cheap, but it does mean the plugin declares its endpoints up front rather than constructing URLs freely. Any provider you know of where that would be awkward?
- The notes say a plugin "contributes a script or config". Those are different strength boundaries — script means the plugin drives the flow but never keeps the result; config means the shell knows the flows and the plugin names one. Config is proposed as the default with script as a recognised weaker escape hatch. Agreed?
- The **subtractive test** — implement the union of tricks, then remove them one by one to find the current minimum — is proposed as the core of the survey rather than an extra, because it is the only thing that can answer "is loosening ok". It costs real requests against your own subscription. Worth it?
