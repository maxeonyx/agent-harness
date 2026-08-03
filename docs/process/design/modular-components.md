# Modular components — construction, config and testability — design scoping

Provisional; built stage by stage per `README.md` (why → what → interactions → summary). **Stages: why, what, interactions, summary (agent-drafted, unreviewed).** User-requested, 2026-07-30, wording preserved: "an experiment which is focused on clean, modular components especially with regard to testing and config". Derives from that request, the `deconfuse` and testing references in `source-notes/reference-codebases.md`, and the engineering discipline section of `REQUIREMENTS.md`.

The thesis: the harness's components are ordinary objects, constructed from composable typed config, with all I/O injected at construction — so the same components compose into an in-process test harness or an out-of-process deployment, and neither is a special case.

## Summary

One idea applied without exception: a component is an ordinary object that is given everything it needs and reads nothing for itself. No globals, no environment variables, no argv, no clock, no working directory. Configuration is then the typed, ordered, recursively merged description of what to give it, with the environment map and argv as *inputs* to that description rather than ambient facts the loader helps itself to.

Three roots force that, and only the first looks like tidiness. A test suite protects you only if you actually run it: the walking skeleton's scenarios spawn binaries, the target here is the whole fast suite well under a second, and a suite slow enough to be annoying gets run less, then only in CI, then stops telling anyone whether they just broke something — which matters more than usual in a project where the thing being broken is being edited by an *agent* that needs to know within seconds. Determinism has to come from construction rather than from patching, because mock and patch achieve it by reaching *inside* the thing under test, which is the opposite of testing it as a black box; injection buys the same determinism by handing the component a clock, an environment and a provider endpoint that happen to be controllable. And in-process composition must not be a test-only path, or the fast suite is confidence pointed at code no user runs. Underneath all three sits the mechanical reason the no-globals rule is not taste: a component that reads ambient state cannot be constructed twice in one process with different configuration, and two-in-one-process is exactly what the other two roots require.

The ports are a short, finite list — a clock, an environment map, argv, a filesystem root, a process spawner, an HTTP client or more cheaply a provider base URL, the terminal the face renders through, a source of randomness and identity, and a handle to the durable record — and what a test supplies through them is *real* implementations that happen to be controllable, not seams cut into components. Two rules keep that from becoming a loophole. Each participant is handed **distinct** port instances, because giving the limb the face's filesystem handle would make an in-process test pass while violating invariant 10, and nothing would notice until the limb was remote; only the builder can enforce distinctness, which is why the rule lives here while topology's deliberately hostile co-located configuration is the check that it held. And the fake provider stays a real HTTP server on a loopback port even in-process, rather than becoming a trait implementation injected inside the brain — the provider wire is a product-public surface, and faking at a trait boundary would bypass request serialisation, headers and wire format, which are the exact things the tests exist to assert.

The config model is a port of the user's `deconfuse`, so the target is specific rather than aspirational: a typed schema declared once with per-field metadata, sources registered in explicit order, precedence kept as a separate axis from load order so a file source can be told its path while CLI arguments still win over the file it points at, recursive field-by-field merge that makes a nested component's config composable rather than a special case, and lists deliberately an *error* when more than one source supplies them, because append-versus-replace has no defensible default. Two of its features are load-bearing here rather than decorative: `give` propagation is what lets a parent hand a child a shared value without a global, and interactive prompting is a *source you install* rather than a behaviour of the loader, which is what makes it structurally impossible for a background brain with no tty to prompt. The one genuine design decision rather than a translation is deconfuse's access-tracked partial config: Rust gives the lazy partial almost for free and makes the ambient tracking awkward, and the alternative — sources declaring which fields they need — is statically checkable but can drift from what a source actually reads. That gets decided by writing both against real components.

There is one function that builds the component tree, and both the shipping monolith and the fast suite call it; a deployment then chooses where to cut the tree and which transport spans the cut, while a test chooses which ports to hand it and reads the same surfaces a user or a provider would. The falsification conditions are unusually crisp and they are the user's own: the thesis fails if in-process tests cannot reach the same assertion surfaces without asserting internals, or if config forces components to know their construction context, or if determinism needs mock/patch seams. Concretely, every assertion in the existing suite must have an in-process equivalent that names no private type — face output through the renderer's output port, the provider wire through the in-process fake's request record and still observable *between* steps rather than only at the end, the durable record through an injected handle — and the exit condition is the walking-skeleton scenario suite running fully in-process while still composing into the two-binary CLI form, plus a written comparison with deconfuse. Two things honestly refuse the speed budget: real-provider use, which is a different suite, and the limb's process-tree scenarios, which must spawn real children because kernel-enforced process-group lifetime is the behaviour under test.

## Why

### 1. A test suite only protects you if you actually run it — *friction*

The walking skeleton's black-box scenarios spawn two binaries. That works and it is honest, but it is slow, and the target here is the whole suite well under a second. The root is not tidiness. A suite that takes long enough to be annoying gets run less, then gets run only in CI, then stops being the thing that tells you whether you just broke something. Speed is what keeps a test suite load-bearing.

This matters more than usual in this project, because self-modification means an *agent* is editing the harness and needs to know within seconds whether it just broke it. Slow tests degrade the self-modification loop directly.

### 2. Determinism must come from construction, not from patching — *correctness*

The user's testing guidelines are explicit: black-box first through public surfaces, injected implementations, never mock or patch. And the standing rule that test flakes are bugs — races structurally excluded, not made unlikely or retried away.

The root is that mock/patch achieves determinism by reaching *inside* the thing under test, which contradicts testing it as a black box. You end up with tests coupled to internals, which break on refactors that changed nothing observable, and which pass when the real composition is broken. Injection gets the same determinism without the coupling: the component takes a clock, an environment, a filesystem, a provider endpoint, and the test supplies real implementations that happen to be controllable.

### 3. If in-process composition is a test-only path, it is a lie — *correctness*

This is topology's why #3 arriving from the other direction. In-process wiring must be *just another composition of the same components*, exercised by real deployments and not only by tests. Otherwise the fast test suite validates a code path that no user ever runs, which is worse than having no fast suite: it is confidence pointed at the wrong thing.

The two experiments therefore share a thesis and a falsification condition, and are strong candidates to merge.

### 4. A component that knows its construction context cannot be built twice — *correctness*

The config model follows the user's `deconfuse` library in Rust terms: a typed schema defined once, explicit ordered sources, recursive merge for nested components, parent→child propagation without globals, and injectable environ and argv.

The no-globals rule looks like taste and is not. If a component reads a global, or an environment variable, or argv directly, then two instances of it cannot coexist in one process with different configuration — which is exactly what roots #1 and #3 require. Ambient state is what makes in-process composition impossible. So propagation-without-globals is *forced* by the goal, not chosen for elegance.

### 5. The user may want this beyond the harness — *desire, hedged*

Recorded with the hedge intact, because it is a live decision rather than a settled one: "Perhaps this would be its own library in the agent-tools ecosystem though, actually - it's useful for all my projects." The experiment should produce evidence for that call rather than presuming it either way.

## Forward: what these roots force

- **Everything ambient becomes a constructor parameter** — clock, environment, argv, filesystem, network, provider base URL, and any source of randomness or identity.
- **The fake provider is a component, not a fixture.** It already exists as a separate HTTP server serving the same OpenAI-compatible API, so real versus fake is just a base URL. In-process composition means it must also be constructible in-process without changing what is being tested.
- **A typed config schema with explicit ordered sources**, and a recursive merge that composes for nested components — the Rust port of deconfuse's model, which is itself a deliverable worth comparing back against the Python original.
- **The assertion surfaces must be reachable in-process without asserting internals.** This is the sharp falsification: if the in-process form can only be tested by reaching inside, the thesis fails.
- **Setup follows the same rule as assertion.** Per the test boundary in `AGENTS.md`, tests may create external conditions a user could bring — files, repos, env vars, fake endpoints — but must drive the harness through its public surfaces, including its own setup. Injection must not become a back door for seeding sessions or context directly.

## What

The design has a natural order: a component, then the things a component is given, then the shape of the configuration that describes it, then the two compositions those pieces have to support, then the surfaces a test observes them through. The falsification conditions come last because by then they are obvious.

### What a component is, and what the walking skeleton actually does today

A component is an ordinary Rust struct with a constructor that takes its dependencies and its config, and nothing else. It reads no globals, no environment variables, no argv, no clock, and no working directory. If it is a participant in the topology sense it owns exactly one external world and runs an {inbox + select loop + owned in-flight work} shape; if it is a plain library object like an output truncator it just does its job. Either way it returns `Result`, and whoever constructed it owns joining it.

It helps to be concrete about the distance from here, because the walking skeleton is honest and small and still violates this in five identifiable places. `brain::Config::from_env()` reads `SKELETON_BASE_URL`, `SKELETON_API_KEY`, `SKELETON_MODEL` and `SKELETON_REASONING_EFFORT` from the process environment. `main` reads `SKELETON_RECORD` to decide where the journal goes. The face reads `EDITOR` and calls `std::env::temp_dir()` when it opens the dump. The limb calls `std::env::current_dir()` for its root and reads `HOSTNAME`. The fake provider binary reads `FAKE_PROVIDER_PORT`, `FAKE_PROVIDER_SCRIPT` and `FAKE_PROVIDER_LOG`. Every one of those is a reason two instances cannot coexist in one process with different configuration, and "env-var-only configuration" is already on the walking skeleton's list of things that must not be integrated.

One of the five is not simply a bug to be injected away. Hostname is ruled to be a fact the *limb* contributes — an environment fact that comes from the limb because the limb owns that environment. So the fix is not "inject a hostname string" but "the limb's environment handle answers questions about its environment, and hostname is one of them", which is the difference between a parameter and a port.

### The ports

Everything ambient becomes a constructor parameter, and it is worth naming them because the list is short and finite: a clock; an environment map; argv; a filesystem root, which for a limb is its working directory and for the face is where scratch files go; a process spawner; an HTTP client or, more cheaply, a provider base URL; the terminal in and out that the face's renderer writes through; a source of randomness and identity; and a handle to the durable record.

What the test supplies is the part that distinguishes this design from mocking. Per why #2, a test supplies *real* implementations that happen to be controllable: a real temporary directory, a real HTTP server on a real loopback port, real child processes when the behaviour under test is about child processes, a real environment map that happens to be constructed rather than inherited. The one that deserves a moment's thought is the clock — a controllable clock is a genuinely different implementation of the same port rather than a seam cut into a component, but it is close enough to the line that it wants a ruling rather than an assumption, and it is the only port where determinism and realism actually pull against each other.

The rule that keeps injection from becoming a loophole is that a participant on the far side of a role boundary gets its ports from its own side. Handing the limb the face's filesystem handle would make an in-process test pass while violating invariant 10, and nothing would notice until the limb was remote. In practice this means the in-process composition must give each participant *distinct* port instances — distinct working directory, distinct environment map, distinct clock — which is the same discipline topology arrives at from the deployment side and calls making co-location hostile. The two designs meet exactly here.

### Config: deconfuse in Rust terms

The model to port is one the user already has running in Python, so the target is specific rather than aspirational. A typed schema is declared once as a struct — `#[derive(Config)]` where deconfuse has `@configclass` — and every field carries optional metadata: help text, choices, a secret flag, an explicit CLI flag or environment name, prefix opt-outs, and `give` for parent-to-child propagation. Sources are registered in explicit order and the values they produce become layers; precedence is a separate axis from load order, defaulting to load order but overridable, because a file source cannot load until something tells it the path while CLI arguments should still win over the file it points at.

The merge rules carry over unchanged, and they are more opinionated than they look. Scalars are last-wins. Nested configs merge field by field with the same rule applied recursively, which is what makes a nested component's config composable rather than a special case. Lists provided by more than one source are an *error*, deliberately, because append-versus-replace-versus-interleave has no defensible default — deconfuse's own note is that rather than guess, lists must come from a single source. An optional nested config is enabled implicitly by setting any of its sub-fields and disabled explicitly by its own flag, with explicit disable winning; that asymmetry is ergonomic rather than principled, and it is worth keeping because it has already been validated by use rather than by argument.

Nested naming falls out mechanically: `storage.s3.bucket` becomes `--storage-s3-bucket` and `PREFIX_STORAGE_S3_BUCKET`. And the two pieces this project specifically needs are that the environment map and argv are *inputs to the loader* rather than things the loader reads for itself, and that `give` propagation is what lets a parent hand a child a shared value without a global. Those two together are the whole of "a component that knows its construction context cannot be built twice", stated as machinery.

Two things deconfuse spends real effort on should not be dropped as decoration. The generated help — the three-column layout showing every field's flag, environment variable, type, requiredness and where else it could come from — is most of the library's practical value, and it reappears verbatim in the error path when required configuration is missing. And interactive prompting for missing values is a *source you install*, not a behaviour of the loader, which matters here more than it does in Python: a brain running in the background with a tray icon must never prompt, so the interactive resolver can only be installed by a face that owns a tty. That is a real constraint this design inherits from topology's ownership rule, and it is cheap to honour if prompting is a source from the start.

### Deep dive: the part of the port that is genuinely hard

Most of deconfuse translates to Rust without argument. One piece does not, and since a written comparison with deconfuse is an exit condition, it is worth being precise about where the languages diverge.

Deconfuse's multi-stage loading works because a source's factory receives a `PartialConfig[T]` — a lazy wrapper where every attribute access either returns a value, returns another `PartialConfig` for a nested config, or raises `MissingConfigError` — and because those accesses are *tracked* through a `ContextVar` while the factory runs. The tracking is what produces the automatic constraint inference: if the file source's factory read `config_file`, then `config_file` cannot be supplied by the file source or anything loading after it, and the help text can say so without anyone writing that down. A missing required field during factory construction is caught and the source is skipped optimistically, failing only at final validation.

Rust gives the first half of that for nearly free and makes the second half awkward. The lazy partial has an obvious shape: a generated companion type with `Option` at every level, nested partials all the way down, and a `check()` that validates and materialises the real type — deconfuse's own insistence that `PartialConfig[T]` is *not* substitutable for `T` is exactly what Rust's type system would enforce anyway. Ambient access tracking through a task-local is the awkward part: it is possible, but it is the sort of dynamic magic that reads as un-Rust-like and interacts badly with async. The alternative is to make the dependency explicit — a source declares which fields it needs, and the loader both provides them and derives the same constraint information from the declaration. That costs more typing per source and buys back static checkability, but it gives up the one thing tracking guarantees for free: that the declared dependencies are exactly the ones the source actually read.

This is the one place where the port is a design decision rather than a translation, so it is the one place where the comparison document will have something to say. It should be decided by writing both against two or three real harness components rather than by argument.

### The fake provider is the external world, not a fixture

The fake provider already exists as a separate HTTP server speaking the same OpenAI-compatible API, which is what makes "real versus fake is just a base URL" true. The requirement for in-process composition is that it stays an HTTP server, constructed in-process, listening on an ephemeral loopback port — not that it becomes a `Provider` trait implementation injected in place of the real one.

That distinction is the crux of the whole design, so it is worth spelling out why the cheaper option is wrong. Injecting a fake at a trait boundary inside the brain would bypass request serialisation, headers, and the wire format — the exact things the tests exist to assert. The provider wire *is* a product-public surface, and a test that stops short of it is asserting an internal. Keeping HTTP even in-process costs a socket and buys the property that the fast in-process suite and the two-binary suite are testing the same thing.

It also resolves a question that otherwise looks like a violation. When the in-process fake provider's recorded requests are read through a handle rather than by parsing a JSONL file, is that asserting an internal? No — the fake provider is not the thing under test, it is the external world standing in for one. Its record of what arrived is the provider wire boundary, whichever way the test reads it. The thing that would be a violation is reaching into the *harness* for the same information.

### The assertion surfaces, one by one

The thesis lives or dies on this list, so each of the walking skeleton's existing surfaces needs a named in-process equivalent.

Face output, today lines on stdout matched by `wait_for` and `drain_seen`, becomes the face's output port with the test owning the sink. This is legitimate rather than a compromise because the TUI is ruled to be the face's external world and not its innards — a test holding the far end of the renderer's output port is in the same position as a terminal.

The provider wire becomes the in-process fake provider's request record, and it has to keep the property the existing suite depends on most: observing what the provider saw *between* steps, not only at the end. `requests_between` is what proves that appends never trigger requests, and a design that could only inspect the wire after the scenario finished would lose the experiment's most important assertion.

The durable record — today a JSONL journal, later whatever persistence-analytics decides — becomes an injected handle, which is also what makes storage tests fast. Process facts stay as they are: PIDs recorded by fixtures that the harness spawned, never a process-table scan, because process ownership is the limb's and the tests exercise it rather than reimplement it. And the transport protocol becomes an assertion surface once it is public, which is topology's business to make public.

Input follows the same rule as output: stdin lines become writes to the face's input port, and setup goes through public surfaces only. Injection must not become a back door for seeding sessions, context, queues or provider state directly — a test may create conditions a user could create, and must drive the harness through its own setup.

The sharp version of all of this, and the thing to actually check: every assertion in the existing suite must have an in-process equivalent that names no private type. If a test wants to reach into the brain's message list rather than going through `/dump`, the wire, or the durable record, the thesis has failed and the honest response is to say so.

### One builder, two compositions

Why #3 insists that in-process composition must not be a test-only path. The mechanism for that is narrow: there is one function that builds the component tree, and both the shipping monolith and the fast test suite call it. They differ only in which ports they pass and which config sources they register. If the test suite has its own wiring function, the fast suite is validating a path no user runs, and per why #3 that is worse than having no fast suite.

The out-of-process form is then the same tree with a boundary drawn through it and topology's transport substituted at the cut, which is why these two designs keep arriving at each other's conclusions.

### trunc as the first citizen

The notes ask for `trunc` to gain a library mode and be used by default for command output, with the agent required to supply a grep term — and, preserved because it is the load-bearing detail, "in trunc, grep terms only *add* to the results in addition to the head + tail".

It makes an unusually good first citizen of this model. It is pure, so nothing about it needs injecting except its configuration; its configuration is exactly the nested-and-propagated shape the config model has to handle, since a limb will want defaults that individual tool invocations override; and it is a real cross-repository dependency in the agent-tools ecosystem, which means it exercises the standalone-library question in miniature before that question has to be answered for the config library itself.

### The standalone-library question, and what evidence would settle it

Whether the config work becomes its own agent-tools library or stays harness infrastructure is the user's decision and is deliberately not presumed here — "Perhaps this would be its own library in the agent-tools ecosystem though, actually - it's useful for all my projects." What the experiment can contribute is evidence rather than an opinion: whether the schema and loader end up with harness-specific concepts leaking into them, and whether its API has to change for harness reasons during the experiment. If neither happens, it is already a library that happens to have one consumer, and extracting it is cheap. If either happens, extracting it at the outset would have meant maintaining an immature library and a moving target at the same time.

### What stays slow, honestly

The target is the whole suite well under a second, and it is worth being clear about what makes that achievable and what refuses to cooperate. No process spawning, no real sleeps, loopback HTTP only, and no filesystem barriers gets most of the way there.

Two categories resist. Real-provider use is in scope for experiments — "I want to actually use it" — and cannot be fast, but it is also not the same suite. Less obviously, the limb's process-tree scenarios *must* spawn real processes, because kernel-enforced process-group lifetime is the behaviour under test and faking it would be testing nothing. So the suite honestly has two tiers: in-process with no external processes, which is where the under-a-second budget applies, and scenarios that spawn real children, which are inherently slower and still fast enough. Pretending otherwise would mean either a dishonest budget or a dishonest test.

### Putting it back together

The whole design is one idea applied consistently: a component is given everything it needs and reads nothing for itself. Configuration is the typed, ordered, recursively merged description of what to give it, with the environment and argv as inputs to that description rather than ambient facts. The external worlds are injected as ports, and the fakes are real implementations of those ports rather than seams cut into components. One builder assembles the tree; a deployment chooses where to cut it and which transport spans the cut; a test chooses which ports to hand it and reads the same surfaces a user or a provider would. Nothing in that sentence is test-specific, which is the point.

The falsification conditions are the user's own and stand as he framed them: the thesis fails if in-process tests cannot reach the same assertion surfaces without asserting internals, or if config forces components to know their construction context, or if determinism needs mock/patch seams. The exit condition is the walking-skeleton scenario suite running fully in-process *and* still composable into the two-binary CLI form, plus a written comparison with deconfuse. Touches invariants 4, 8 and 10.

## Interactions

This design is unusual in the portfolio: `INTERACTIONS.md` records it as upstream of every other experiment's *testing* rather than of their designs, with topology the one exception where it is upstream of the design too. That shape decides how this section is organised. Most of what follows is about what other experiments may assume from here, and the small number of places where their requirements land back on this one.

### What this experiment owns, stated so others can lean on it

The construction-and-injection model in full: a component takes its dependencies and its config and reads nothing for itself; the ports are the finite list in the what; configuration is a typed schema with explicitly ordered sources, precedence as a separate axis from load order, and recursive merge; one builder assembles the tree for both the shipping monolith and the fast suite; and the fake provider stays a real HTTP server even in-process. Every other experiment's fast tests are a consequence of that, and none of them should re-derive any of it.

Correspondingly, this experiment tests none of their subject matter. It does not settle what the durable record *is*, what the transport protocol *is*, or what a context layer *contains*. It only requires that each of those is reachable through a port.

### Being upstream of everyone's tests, concretely

Three of the assertion surfaces have identifiable dependents, and naming them changes what this experiment must not simplify away.

The most load-bearing is mid-scenario wire observation. The existing suite's `requests_between` proves that appends never trigger requests, and that assertion is not local to this design — it is the shared instrument for invariant 2. Context-updates needs it to show that saving a file produces zero requests and that the notice then rides on a request that was happening anyway. Compaction-handover needs it to show that the harness telling an agent the state of play costs nothing extra. User-turn needs it to show that passive activity triggers nothing at all. So a design that could only inspect the wire after a scenario finished would not merely lose a convenience; it would remove the only instrument three other experiments have for their central invariant. **In-process wire observation between steps is a hard requirement of this design, not an optimisation of it.**

The durable-record handle wants one decision that this doc has not previously made. Persistence-analytics has already ruled that the *queries* are the compatibility surface rather than the table shapes, with a set of named queries checked into the repository and exercised by black-box tests. Taking that seriously, **the injected durable-record handle should expose the named query set rather than the schema**, so this design's tests survive persistence's migrations by construction rather than by luck. That also keeps the boundary honest: reading the record through a handle is reading a product-public surface, exactly as reading the fake provider's request log is, whereas a handle that exposed tables would be an internal wearing a port's clothing.

Two faces in one test process is multi-client-ui's dependency on this design, and it is a sharper test of the injection model than it looks. Two faces need two terminals, two viewport sizes and two resume points, and multi-client-ui's face-local class is precisely the state that must *not* be shared between them. So the distinct-port-instances rule is what makes in-process multi-face testing legitimate rather than a fiction, and if any face-local state turns out to need a process global, multi-client-ui has to run two processes and this design has failed at a real case. The division is clean: **this experiment assumes multi-client-ui's three-class taxonomy rather than inventing a state model; multi-client-ui assumes it can construct two faces in one process rather than proving it.**

One dependent is honestly outside the budget rather than inside it. The limb's process-tree scenarios must spawn real children because kernel-enforced group lifetime is the behaviour under test, and layered-shutdown depends on the same property from the other end. That is the second tier the what already admits, and it is a shared tier rather than this design's exception.

### Config is upstream of two other designs, and one of them wants a feature kept

`PLAN.md` already flags that limb-model's context-layer composition and this design's config composition are the same recursive-merge problem, and names them merge candidates. Stage 2 makes the boundary clearer than "merge candidates" suggests. Limb-model bounded its own problem and declined to design it: composition must be a deterministic function evaluated at a small number of known rebuild boundaries, and the *precedence order itself* — whether project beats user, whether project config may override a machine's toolchain facts, what happens when two layers contribute conflicting AGENTS.md-shaped instructions — is genuinely open and is not this design's to invent.

That splits cleanly, and the split has a design consequence worth acting on. **This experiment owns the merge machinery; limb-model owns the precedence policy.** Which means the machinery must take precedence as an explicit input rather than baking config's own answer into the merge. Deconfuse already works this way — precedence is a separate axis from load order, defaulting to load order but overridable — so the requirement is to *preserve* that separation in the port rather than simplify it away as an unused generality. It is not unused; it is what lets limb-model reuse the machinery once someone rules on layer precedence.

That also feeds the standalone-library question with evidence rather than opinion. Stage 3 has now named consumers beyond the harness's own components: limb-model's layer composition, and `trunc`, which is already a cross-repository dependency. The "one consumer, extract later" argument is weaker than it was, though the decision is still the user's and the experiment should still report whether harness-specific concepts leaked in.

### The standing merge question with topology

Both docs ask it, and the answer proposed in topology's interactions section and repeated here is: **do not merge; make this experiment a precondition of topology.**

Their falsification surfaces differ. This design fails if an in-process test cannot reach an assertion surface without naming a private type, or if config forces a component to know its construction context. Topology fails if a scenario behaves differently co-located and split, or if a durable record's facts differ between configurations. Those are measured in different places, and a merged experiment that failed would not say which claim failed — which matters especially here, because "in-process composition is not a test-only path" is a claim about honesty and the temptation to fudge it is real.

The genuine overlap is one mechanism rather than one design, and it should live here. **This experiment owns the builder and the rule that each participant is handed distinct port instances — distinct working directory, distinct environment map, distinct clock; topology's deliberately hostile co-located configuration is the test that the rule held.** Only the builder can enforce distinctness, and only a hostile deployment can prove it was not quietly bypassed. That answers the last question in the list below: the rule is this design's standard, and topology's enforcement is a check on it rather than a duplicate of it. What merging would cost is that nothing about the deconfuse port, its generated help or its precedence model bears on transports, and nothing about the IPC sequencing verdict bears on config schemas.

### Where injection turns out to be load-bearing for somebody else's safety property

Three connections that look like plumbing and are not.

Self-modification's highest-priority why is never live-bricking, and its auto-rollback rung has a prerequisite it names honestly: you have to know what to blame. Fault attribution is what the Deno isolate and the hard sandbox buy, and self-modification already reframes sandboxing as being about blast radius and attribution rather than security. The construction model is the other half of that property, and it is the cheaper half. **A plugin that reads globals, the environment or the clock can damage state it was never handed, which makes a fault unattributable no matter how well isolated the plugin is.** So the no-ambient-access rule is not only what makes two instances coexist in one process; it is what makes "this plugin faulted, roll back this plugin" a sound inference. The division is that this design owns how a plugin is *constructed*, and self-modification owns validation, quarantine, versioning and retention.

The same rule is what gives oauth-credentials' boundary any strength. Oauth proposes handing a provider plugin a fetch *bound to a destination*, because a wrapper that merely injects a header can be pointed at a host the plugin controls. That boundary is worth exactly as much as the rule that a plugin cannot construct network access for itself — which is this design's rule, not oauth's. Conversely, the plugin's side of oauth's boundary is declaration: its base URLs, its non-secret headers, and which auth flavour it needs. That is a config contribution in this design's terms, so oauth's interface is this model's ports and schema applied to one case rather than a new mechanism.

Operator-lifecycle adds a small requirement that is cheap now and awkward later. Its pre-activation self-check asks a staged binary to prove it can start: open the database read-only, confirm the schema version is in range, **validate its configuration**, and exit with a known code — all without touching the running system. That means config validation has to be a pure, side-effect-free operation over a constructed loader, available as a mode rather than as a consequence of starting up. It also sharpens the interactive-prompting design from a preference into a requirement: operator-lifecycle wants the brain running as a background server under a supervisor, where there is no tty and nobody to answer, so prompting *must* be a source that only a face owning a tty installs. The what already proposes that; the interaction is what makes it non-negotiable.

### The cells that stayed empty

Cancellation-economics has nothing to say to this design, and `INTERACTIONS.md` records that as one of its several empty cells. The reason is worth a line, because it is not laziness: that experiment measures billed tokens against an external account-level instrument on real providers, so the fast in-process suite is irrelevant to it in both directions — it cannot use a fake provider, and its results place no requirement on how components are constructed.

Forked-subagents, compaction-handover, context-updates and user-turn interact with this design only as consumers of the fast suite and of the wire-observation instrument above. There is no design-level connection: none of them changes what a component is, and this design does not change what any of them decides. The same is true of persistence-analytics beyond the named-query handle. That is the useful finding — the construction model is broadly depended on and narrowly coupled, which is exactly the property that lets it be built early and in parallel with either cluster.

## Questions for review

- Should this merge with topology? They share a thesis, and proving one without the other seems to leave the claim half-tested.
- Should the deconfuse port be a separate agent-tools library from the outset, given you already suspect it wants to be one — accepting that the harness then depends on an immature library?
- Does "whole suite well under a second" survive contact with a real provider requirement, or does it apply only to the fake-provider suite?
- A second pressure on the same budget, found while drilling: the limb's process-tree scenarios must spawn real child processes, because kernel-enforced group lifetime is the behaviour under test. Do those live inside the under-a-second budget, or is a second tier accepted?
- Is a controllable clock an injected implementation or a mock in your terms? It is the one port where determinism and realism actually conflict, and why #2 says never mock or patch.
- The Rust port of deconfuse's access-tracked `PartialConfig` is the one genuine design decision rather than a translation. Ambient tracking via a task-local reproduces the automatic constraint inference but reads as un-Rust-like; explicit dependency declaration per source is statically checkable but can drift from what the source actually reads. Which do you want tried first?
- Interactive prompting: keep it, as a source only a face that owns a tty may install, so a background brain can never prompt? Or leave it out of the harness entirely?
- Injection makes it easy to hand a component another component's world, which would satisfy an in-process test while violating invariant 10. Should "each participant gets distinct port instances" be a standard of this experiment, or is it topology's to enforce? (The interactions section answers this: the rule is this experiment's, because only the builder can enforce it, and topology's hostile co-located configuration is the check that it held. Confirm.)
- **The merge question, answered: don't merge with topology, be its precondition.** Different falsification surfaces mean a merged failure would be unattributable, and the real overlap is the builder plus the distinct-port rule rather than a whole design. Ruling wanted, since both docs raised it.
- **The injected durable-record handle is proposed to expose persistence's named query set rather than its schema**, on the grounds that persistence has already made the named queries the compatibility surface. That keeps these tests immune to its migrations, but it means this experiment cannot start until that query set exists in at least a provisional form.
- **Config precedence is proposed to stay an explicit input to the merge machinery**, rather than being fixed to config's own answer, because limb-model wants to reuse the machinery for context layers and its precedence order is genuinely undesigned. Deconfuse already separates precedence from load order, so this is a case of preserving a feature rather than adding one — but it means the port keeps a generality with no in-harness consumer yet.
- **Operator-lifecycle needs config validation as a pure, side-effect-free mode** so a staged binary can prove it can start before activation. Cheap to build now and awkward to retrofit. Is that a requirement of this experiment or of that one?

## Index

| Aspect | L1 | L2 | L3 |
|---|---|---|---|
| Model framing | | | |
| Wire & cache | P | §The fake provider is the external world, not a fixture | |
| Tool surface | S | §trunc as the first citizen | |
| UX & input | P | §Config: deconfuse in Rust terms | |
| Ownership & placement | S | §The ports, §One builder, two compositions | |
| Lifecycle | S | §What a component is, and what the walking skeleton actually does today | |
| Storage | P | §Being upstream of everyone's tests, concretely | |
| Economics | | | |
| Security | P | §Where injection turns out to be load-bearing for somebody else's safety property | |
| Testing & verification | E | §The assertion surfaces, one by one, §What stays slow, honestly | |
| Code shape | P | §The ports, §Config: deconfuse in Rust terms | §Deep dive: the part of the port that is genuinely hard |
| Dev workflow & references | O | §The standalone-library question, and what evidence would settle it | |
| Core migration | S | §What a component is, and what the walking skeleton actually does today | |
