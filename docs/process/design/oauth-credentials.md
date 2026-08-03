# OAuth and credential handling — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why (agent-drafted, unreviewed) · what, interactions, summary — not yet done.** A targeted question rather than a broad design; expected to stop at L2 depth. Derives from `source-notes/anthropic-oauth-references.md`.

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

## Parked for later stages

**Reference implementations named in the notes**, to be studied rather than copied: `nitishxyz/otto` (anthropic-oauth-client), `leohenon/pi-anthropic-oauth`, `griffinmartin/opencode-claude-auth`, `cortexkit/anthropic-auth`, `leohenon/op-anthropic-auth`, and an extensive set of files in `can1357/oh-my-pi` covering registry, oauth, auth utils, client, wire, messages-server, schema, an OpenAI→Anthropic shim, API-key validation, error classes, tool-choice, compat and identity classification, plus its provider docs and research scripts. One non-open-source package is flagged as possibly worth inspecting (`@zgltyq/pi-provider-claude`). One approach relying on `CLIProxyAPI` is explicitly disliked.

**Open unknown:** nothing in the references mentions Claude Team subscriptions — "should work the same hopefully." Worth confirming, since it may be what the user actually has.

**Interactions flagged for stage 3:** self-modification (provider implementations as soft-middle plugins with auth outside the plugin is the load-bearing link); topology (credentials stay brain-owned, so limbs never carry them — already invariant); persistence-analytics (token storage as durable state that is never projected to the model); operator-lifecycle (a provider plugin update must not invalidate a live session's auth).

## Questions for review

- Do you want the timeline artefact before or alongside a working implementation? It is the part that informs how much to depend on this, so arguably first.
- Is Claude Team the subscription that matters here? The references are silent on it and it changes what "working" means.
- How much should the harness degrade gracefully if subscription access breaks — fall back to API keys transparently, or fail loudly?
