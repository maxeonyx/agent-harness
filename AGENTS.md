# agent-harness

A personal agent harness. Max's vision is the gist https://gist.github.com/maxeonyx/014463e0964bebd0add4b914971c492f — his own notes, and the driver. Derive what he wants from it; don't treat a sentence in it as a rule.

The repo was reset on 2026-10-16 (NZT). The earlier attempt is the unconnected branch `v1` (plus `forked-subagents`, `handoffs`, `provider-cache-probe`): evidence of what earlier agents tried and measured, none of it accepted by Max, and none of its process or framing carries over.

## What is settled

- The full vision has many aspects that interact pairwise, so it is built up incrementally: prototype a part against only a few of the other aspects, accept imperfect products, integrate, and repeat until the whole behaves well along every dimension. (High confidence.)
- No permission prompts in the harness. An environment is defined with its permissions; extending them means changing that definition, possibly by another agent. (High confidence; he calls prompts a bad abstraction.)
- No tests in the repo except truly black-box end-to-end scenarios Max can run and watch as a demonstration. Otherwise agents test manually by using the harness. (High confidence.)

## Not yet designed

The development process. Max's direction so far (medium confidence, not designed): an orchestrator agent at low/medium effort; engineer agents that build without running the full design process; design subagents at high/xhigh effort, launched only by the orchestrator — because running heavy design everywhere burns his usage before he gets what he wants. The focus is building, finding the right abstractions, and back-chaining to what he actually wants rather than what he literally says.
