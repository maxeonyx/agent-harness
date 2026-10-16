# Agent Harness — Design Clarifications and Implications

Written by ChatGPT in October 2026 as an architectural review of Max's vision gist. Max shared it as useful critique that may not all be good: it is evidence, not a ruling. It shows the stance he wants from designers, building the right thing in the spirit of what he said rather than what he literally said.

These notes supplement the existing design rather than restating it. They capture clarified intentions, consequences of existing requirements, and questions that remain unresolved.

An important distinction throughout: a desired property of the system does not necessarily imply any particular implementation. Where the implications are uncertain, preserve the question rather than prematurely choosing an architecture.

## 1. Context authority, contributions and provenance

The brain is authoritative over the exact context sent to the model. The brain, limbs, and faces can all contribute information, instructions, and other context elements.

Instruction provenance is first-class. This matters because instructions may change while an agent is operating. Without provenance, an agent may incorrectly interpret an earlier action as violating an instruction introduced only afterward.

The system needs to distinguish what information and instructions were actually present in a given model invocation from those introduced subsequently.

When changes invalidate an existing context, compaction or refresh is necessary. Refreshing a context without compaction is an advanced capability whose exact semantics remain to be developed.

Open questions:

- What precisely constitutes context invalidation?
- How are conflicting or changing contributions reconciled?
- What provenance information is necessary for accurate reconstruction and reasoning?

## 2. Agent hierarchy and graph semantics

Several relationships are involved in agent orchestration, and they are not necessarily equivalent:

- Lifecycle ownership: Responsibility for task lifetime, cancellation, and joining.
- Execution dependencies: Which tasks require results from other tasks.
- Context lineage: How contexts derive from previous contexts through forks, compactions, handovers, and incorporated results.
- Event causality: The causal relationships between events throughout the system.

These relationships may all be expressible within the underlying causal event graph. However, representing them in one graph does not make their semantics interchangeable.

For example, a task depending on another task's result does not, by itself, establish ownership of that task or responsibility for cancelling it.

A strict ownership tree offers a straightforward approach to structured concurrency. It is not yet clear that it is necessary, or whether alternative lifetime-management mechanisms could provide the same guarantees with greater flexibility.

Similarly, suspending a parent until all children complete is a coherent execution model, but structured concurrency does not inherently require that particular scheduling behavior.

Open questions:

- How can structured task lifetimes coexist with shared dependencies?
- What determines the lifetime of a task with multiple dependants?
- Can a parent continue processing partial results while retaining responsibility for its outstanding children?
- To what extent can the general causal graph subsume the more specialized relationships without obscuring their semantics?

The original motivation remains: fork contexts freely, allow concurrent work, and incorporate results without introducing unstructured execution lifetimes.

## 3. Generalized handovers and historical preservation

Compaction, subagent reporting, and initiating a new agent context are all instances of transferring information between contexts.

The existing handover abstraction is deliberately general: it can contain text and attached tool calls. These can be presented to the receiving agent as ordinary tool-call history.

This avoids requiring a special model-facing representation for every kind of information transfer.

There is, however, a distinction between how information is presented to the model and what actually happened in the system.

For example, a receiving agent may see an attached tool result as part of its conversation, even though that result originated elsewhere. The underlying historical record must retain the actual provenance. Otherwise, replay and inspection could misrepresent the execution history.

Compaction never destroys the original transcript. It changes what is actively presented to the model, while previous information remains retrievable.

This is especially important because frequent compaction is an intended mode of operation.

Open question: What provenance and reference semantics are needed for handover attachments, particularly when they refer to resources that may subsequently change?

## 4. Empirical cache-aware scheduling

Cache preservation and proactive compaction have costs and benefits that depend on provider behavior, context structure, and expected future activity.

The executive's decisions can be informed by an empirical expected-value model, rather than fixed assumptions about cache lifetime or the benefits of compaction.

A conceptual starting point:

```
Expected benefit = P(cache miss) × avoided miss cost − compaction cost − expected quality-loss cost
```

This is a decision framework, not necessarily the final formula.

The relevant quantities can be estimated from observed token costs, cache behavior, compaction overhead, and subsequent task performance.

The quality cost of compaction is particularly difficult to estimate. It may sometimes dominate the financial or latency savings.

Provider-specific cache prewarming mechanisms may also offer alternatives to dummy model requests.

Open question: Which measurements provide useful predictive signals, and when is the evidence sufficient to justify proactive compaction?

## 5. Interrupted operations and unknown outcomes

A tool operation interrupted by a process crash or lost connection may have completed successfully without that success being observed.

Therefore, an interrupted operation is not necessarily a failed operation.

This is an unavoidable consequence of interacting with external systems. In general, the harness cannot guarantee exactly-once execution of external side effects.

An explicit unknown outcome is useful because it preserves the distinction between failure and uncertainty.

Recovering safely may involve identifying an earlier operation, inspecting external state, using idempotency mechanisms where available, or involving an agent or human in resolving the uncertainty.

Simply retrying can duplicate side effects.

The same general lifecycle questions apply to graceful shutdown, cancellation, process failure, and disconnection, although their exact semantics may differ.

Open questions:

- What operation information needs to persist to support recovery?
- Which guarantees can the brain and limbs provide internally?
- How is an uncertain external result reconciled before execution continues?

## 6. Versioned and evolving execution

The system is intended to support different versions of the harness simultaneously, potentially across multiple machines and sessions.

Relevant versions may include the executable implementation, event/data model, session/context model, and plugins or tools.

This is particularly relevant to a system that can modify and upgrade its own implementation.

Restart and compaction provide natural opportunities for adopting newer versions, but they address different aspects of the problem.

Compaction may enable reconstruction of model context under newer instructions or tool definitions. It does not, by itself, solve compatibility between versions of persistent event or session data.

Historical information therefore needs enough interpretation context to remain usable across supported upgrades.

Open questions:

- What compatibility guarantees are required between versions?
- Which changes can be adopted within a running session?
- What makes a version transition safe?
- How can an unsuccessful self-upgrade be recovered from?

No particular migration or hot-reload mechanism is implied yet.

## 7. Distributed event state and causal consistency

The design allows different components—faces, brains, sessions, limbs, and potentially shared services—to maintain their own local databases or event-derived state.

Some information is shared; some remains local. Replication follows the need for information across these components rather than requiring every component to possess the complete global history.

The fundamental ordering requirement is causal, not globally total.

A component deriving state from events needs the causal information on which that state depends.

An additional issue arises when multiple components can concurrently modify the same logical entity.

Causal ordering alone does not resolve conflicting concurrent modifications. It establishes relationships between events, but not necessarily which competing modification prevails.

This is a separate design problem, potentially requiring different solutions for different kinds of state.

Likewise, event sourcing does not inherently provide CRDT merge semantics. CRDTs are one possible mechanism for particular shared-state requirements, rather than an automatic consequence of event-based design.

Open question: Which state actually requires concurrent multi-writer semantics, and what conflict guarantees does each kind of state require?

## 8. Context and workspace independence

Forking a model context and creating an isolated filesystem/workspace are independent operations.

An agent may fork its context while continuing to operate on the same workspace. Alternatively, an execution policy may provide a separate workspace.

The harness can support or automate workspace isolation without making it intrinsic to context forking.

This matters because workspace organization is itself a separate concern, and different development workflows have different isolation requirements.

Concurrent writes to shared workspaces create potential conflicts, but a specific solution—such as worktrees, locking, or write ownership—has not yet been established as necessary.

## 9. Remote limb lifetimes

Remote limbs are intended to be native, lightweight, and ephemeral.

The desired deployment model includes the ability to bootstrap a compatible harness executable onto a remote machine over SSH, run it without permanent installation, and remove it when no longer needed.

Three lifetimes are distinct:

- The remote workspace and its persistent files.
- The running limb process.
- The connection between the brain and limb.

Separating these matters because losing a connection does not necessarily mean the workspace or its ongoing operations cease to exist.

Open questions:

- What behavior is desired when connections disappear unexpectedly?
- Which remote operations survive a disconnection?
- How are abandoned processes detected and cleaned up?
- What guarantees are necessary when reconnecting to a workspace?

These questions follow from ephemeral remote execution without implying a particular lease or supervision implementation.

## 10. Reloadable implementation and plugin runtime

Hot reloadability and the ability to evolve the harness during operation are important requirements.

Embedded Deno is one possible implementation, but it introduces another runtime and associated integration costs. WASM or separate processes may offer different tradeoffs.

The relevant comparison is not only execution speed, but also startup overhead, implementation complexity, state continuity, and the behavior of running sessions during changes.

There is not yet sufficient justification to treat any one plugin runtime as architecturally necessary.

The requirement is the desired dynamic behavior, not a particular language or execution mechanism.

## 11. Observable execution and waiting state

An important goal is that the system exposes what it is doing and, particularly, what it is waiting for.

Because agents, model calls, tools, and remote operations are asynchronous, a generic running/idle indicator loses significant information.

Representing waiting relationships explicitly makes it possible to derive more informative UI state: what is blocked, what it depends on, and whether the dependency is progressing.

The proposed throbber is one small visualization of this underlying information.

This reinforces the broader architectural intention that faces display projections of actual execution state, rather than maintaining independent approximations of it.

## 12. Scheduling human attention

The human-attention scheduler is intended to help enforce reflectively chosen priorities, not merely respond to immediate preferences.

Consequently, an interface that allows effortless deviation from the selected task may undermine the purpose of the feature.

The scheduler's reasoning matters: the user needs to understand why a particular task was selected and have a way to provide feedback that informs future decisions.

The desired level of enforcement and flexibility remains an open design question. It is not necessarily equivalent to ordinary task recommendations or an easily dismissible focus mode.

## Cross-cutting architectural questions

Several questions recur across these areas:

- Identity and lifetime: What constitutes a context, task, session, process, or workspace, and which lifetimes are genuinely coupled?
- Causality and authority: What can be derived from the causal event history, and what additional ownership or conflict semantics are necessary?
- Provenance: Can the system accurately explain what an agent knew, where that information came from, and what actually occurred?
- Recovery and evolution: What information must survive failures and implementation upgrades to preserve the intended behavior?
- Minimal sufficient mechanisms: Which guarantees are intrinsic to the goals, and which proposed mechanisms can remain optional policies or implementation details?

The objective is not to answer these questions prematurely. It is to prevent an attractive implementation approach from silently becoming an architectural constraint without a demonstrated need.
