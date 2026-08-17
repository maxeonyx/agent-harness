# Event-streaming experiment notes

These are design inputs for later event-streaming / replication experiment(s), not a experiment brief or an accepted architecture. The walking skeleton now uses shared mutable state and typed channels instead. The stream "needs to be principled, it needs to be structured well in order to be worth it."

## Sequencing follows the substrate

"there is no sequencer unless it's between, say, A and B, and A and B share a process. If they don't share a process, then they're too far away to sequence. We treat them as asynchronous, and we accept there's no total order for the events across the two. We don't synchronize."

"for the single process model that we're testing right now, we do synchronize, and therefore we do have a sequencer. But the brain is not the sequencer. The brain is another participant." Sequencing is a property of the deployment substrate, not a participant's role. Whether IPC on one machine is close enough to sequence remains explicitly unclear.

The corresponding append question is synchronous versus asynchronous append: "the question is whether to synchronously append or asynchronously append. ie. do we wait, or not? If we can synchronously append, we don't need to worry." In-process appends are synchronous method calls under a lock. Async append is only a question once a process boundary exists, and is deferred here.

## Candidate replicated shape

The experiment should give each peer a replica of session state. In particular, the face maintains its own replica rather than treating the brain's memory as its query surface. A resumable, sequence-based stream lets a peer catch up from the last sequence it has seen. The experiment needs to cover lag, reconnect, catch-up, and the point at which ordinary catch-up becomes a full resync.

Keep proposals distinct from sequenced facts. A peer proposes something; the deployment's sequencing mechanism, where one exists, establishes the fact that replicas consume. Across substrates that are too far apart to sequence, do not manufacture a total order.

Pending assistant tool calls are one concrete proposal/fact boundary. A completed response can retain a proposed call without claiming it ran. Its wire projection stays absent until execution; no replica should invent an outcome to make the exchange look complete.

A joining peer should declare its contributions, catch up, then signal ready. The brief needs to define that handshake and what existing peers may do before readiness. If two writers race to resolve the same operation, the outcome should be idempotent and first-wins, rather than producing two effective outcomes.

## Peers and topology

"definitely, the three should be symmetric, and their roles should be defined before we go later on into the other experiments." Every component owns exactly one external world: face = terminal/UI, brain = provider connection, limb = the environment (filesystem, processes, tools). "Any provider state is part of the brain conceptually, just like ephemeral UI state is part of the face."

For the future design: "the limb is definitely an event peer!! tool calls can be complicated, can stream their results. the limb watches files for changes, etc. it has a select loop too, basically." Cancellation then needs to be a replicated fact, not an in-memory token passed across the role boundary.

The brain has "input events from face, multiple faces, limb, multiple limbs maybe. I don't think that's gonna happen, though. One limb per session." The protocol therefore needs multiple faces; multiple limbs remain possible but not expected. "it's gonna be very common for the face and limb to be same process" on the user's machine, with the brain possibly remote.

Deferred aside: "the brain is in charge of configuration changes, I believe." Leave this for later. One candidate interpretation is configuration changes as brain-sequenced facts, but that has not been ruled in as architecture.

## Where to start

The current experiment's JSONL append journal is already latent event sourcing. The experiment can grow from replaying that journal into replicas rather than inventing storage and replication at the same time.

Graceful shutdown is an adjacent design input: "every layer should think about how it's shutting down gracefully in response to a cancellation." Each layer gracefully shuts down what it owns; the layer above holds a timeout backstop and kills it on expiry. One idea is to "give a global timeout budget... and then basically hand down a slightly shorter budget at each level. I'm not sure exactly how that should work. I don't know what asupersync does here. We should look." Check what asupersync does here before briefing this part.
