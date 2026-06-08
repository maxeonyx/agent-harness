# Open Questions & Known Risks

> Remaining after initial design review. Resolved items removed.

---

## Still open

### Shared mutable state under concurrency
Fork-by-default + parallel siblings + same limb = multiple agents on the same filesystem. No solution yet. Known to be error-prone in practice. Possible directions: scoped ownership via instructions, or some borrow-checker-style rule on workspace regions.

### Stuck child / abandoned user-facing session
A forgotten `/done` or hung child blocks the parent scope indefinitely. Intentional that the user is responsible, but the permission prompt that precedes it is a known UX pain point. Expiry as escape hatch is noted but not decided.

### Limb-local context opacity
Limb owns what reaches the model from its domain. Brain owns cost/reliability/debugging. Can the brain manage those without visibility into what the limb injected or filtered? Not yet resolved.

### Extension compatibility
Stock Pi extensions may break behaviourally even with an identical API surface — tools now execute remotely, state lives elsewhere, context is shaped by the limb. Risk acknowledged, not mitigated.

### Session storage blast radius
Claimed to not be deeply entangled with the agent loop, but the hierarchy model makes storage semantics central to concurrency, blocking, resume, and compaction. May be larger work than assumed.

---

## Intentional / resolved

- **1. Fork vs build-on-top** — resolved: build on top, only fork session storage. Implementation details not decided at design level.
- **2. Brain permissions vs limb context** — resolved: brain owns permissions, limb owns context construction. Exact enforcement boundary still open but not contradictory.
- **3. Limb identity** — resolved by example: local subprocess, remote SSH, in-process meta, limb-as-a-service.
- **4. What is a result** — resolved: last message part in a turn.
- **6. Storage needs underplayed** — acknowledged and listed in harness-design.md.
- **7. Transient connection loss** — resolved: limb shuts down, reconnect with timeout.
- **8. Stuck scope** — intentional, user is in charge.
- **9. Brain crash/restart** — resolved: resume from SQLite, agent reasons about interrupted tool calls.
- **10. Duplicate side effects** — resolved: agent reasons about state itself on resume.
- **11. Ephemeral limbs vs resumability** — resolved: brain stores session content, limb rebuilds context on fresh/compacted start.
- **12. Fork-for-cache vs shared workspace** — open but acknowledged (see shared mutable state above).
- **13. Structured concurrency purity** — resolved: intentional design, remains textbook structured concurrency.
- **17. Centralized brain scheduling** — not a concern, considered tractable.
