// Anthropic Messages API measurements.
//
// Documentation read 2026-09-08 from
// https://platform.claude.com/docs/en/build-with-claude/prompt-caching
// (the docs.anthropic.com URL redirects there). Every `documented` string below
// is that page's claim, so a measurement that disagrees is visible as a
// difference rather than a surprise.
//
// Prices read 2026-09-08 from the same page's pricing table.

import {
  costOf,
  describe,
  type Json,
  type Measurement,
  type Observation,
  type Price,
  totalInput,
  type Usage,
  ZERO_USAGE,
} from "./types.ts";
import { filler, LARGE_WORDS, STANDARD_WORDS, TINY_WORDS } from "./filler.ts";

export const ANTHROPIC_URL = "https://api.anthropic.com/v1/messages";
export const ANTHROPIC_VERSION = "2023-06-01";

/** USD per million tokens, per the pricing table read 2026-09-08. */
export const ANTHROPIC_PRICES: Record<string, Price> = {
  "claude-sonnet-5": { input: 2, write5m: 2.5, write1h: 4, read: 0.2, output: 10 },
  "claude-haiku-4-5": { input: 1, write5m: 1.25, write1h: 2, read: 0.1, output: 5 },
  "claude-opus-5": { input: 5, write5m: 6.25, write1h: 10, read: 0.5, output: 25 },
  "claude-sonnet-4-5": { input: 3, write5m: 3.75, write1h: 6, read: 0.3, output: 15 },
};

/** Minimum cacheable prefix in tokens, per the docs read 2026-09-08. */
export const ANTHROPIC_MINIMUM_CACHEABLE: Record<string, number> = {
  "claude-sonnet-5": 1024,
  "claude-haiku-4-5": 4096,
  "claude-opus-5": 512,
  "claude-sonnet-4-5": 1024,
};

export function anthropicHeaders(apiKey: string): HeadersInit {
  return {
    "content-type": "application/json",
    "x-api-key": apiKey,
    "anthropic-version": ANTHROPIC_VERSION,
  };
}

export function parseAnthropicUsage(response: unknown): Usage {
  const usage = (response as { usage?: Record<string, unknown> } | null)?.usage;
  if (!usage) return { ...ZERO_USAGE };

  const num = (value: unknown) => (typeof value === "number" ? value : 0);
  const breakdown = usage.cache_creation as Record<string, unknown> | undefined;

  // `cache_creation` splits writes by TTL when present. Without it, a write is
  // a 5-minute write, which is the only TTL the requests here use by default.
  const write5m = breakdown
    ? num(breakdown.ephemeral_5m_input_tokens)
    : num(usage.cache_creation_input_tokens);
  const write1h = breakdown ? num(breakdown.ephemeral_1h_input_tokens) : 0;

  return {
    // On this API `input_tokens` counts only tokens after the last breakpoint.
    uncached: num(usage.input_tokens),
    write5m,
    write1h,
    read: num(usage.cache_read_input_tokens),
    output: num(usage.output_tokens),
  };
}

export function anthropicCost(model: string, usage: Usage): number {
  const price = ANTHROPIC_PRICES[model];
  if (!price) throw new Error(`no price entry for Anthropic model ${model}`);
  return costOf(usage, price);
}

// --- request pieces -------------------------------------------------------

const BREAKPOINT = { type: "ephemeral" } as const;
const ASK = "Reply with the single word: ok";

type TextBlock = { type: "text"; text: string; cache_control?: { type: string; ttl?: string } };

function text(body: string, breakpoint?: { type: string; ttl?: string }): TextBlock {
  return breakpoint ? { type: "text", text: body, cache_control: breakpoint } : { type: "text", text: body };
}

function tool(name: string, description: string): Json {
  return {
    name,
    description,
    input_schema: {
      type: "object",
      properties: { path: { type: "string", description: "A filesystem path." } },
      required: ["path"],
    },
  };
}

function baseTools(): Json[] {
  return [
    tool("read_file", "Read a file from the workspace and return its contents."),
    tool("write_file", "Write the given contents to a file in the workspace."),
    tool("list_dir", "List the entries of a directory in the workspace."),
  ];
}

/** The assistant text from a prior response, for feeding a real turn back. */
function assistantText(observation: Observation | undefined): string {
  const content = (observation?.response as { content?: unknown[] } | undefined)?.content;
  if (!Array.isArray(content)) return "";
  return content
    .filter((block): block is { type: string; text: string } =>
      typeof block === "object" && block !== null && (block as { type?: string }).type === "text"
    )
    .map((block) => block.text)
    .join("");
}

// --- verdict helpers ------------------------------------------------------

function reused(observation: Observation): boolean {
  return observation.usage.read > 0;
}

function steps(observations: Observation[]): string {
  return observations.map((o) => `${o.label}: ${describe(o)}`).join("; ");
}

/** States whether the named step reused a prefix, with the numbers behind it. */
function reuseVerdict(observations: Observation[], stepIndex: number, meaning: {
  yes: string;
  no: string;
}): string {
  const errored = observations.find((o) => o.error);
  if (errored) return `INCONCLUSIVE — ${errored.label} failed. ${steps(observations)}`;
  const step = observations[stepIndex];
  const answer = reused(step) ? meaning.yes : meaning.no;
  return `${answer} [${steps(observations)}]`;
}

// --- measurements ---------------------------------------------------------

export type AnthropicOptions = {
  runId: string;
  /** The model under test, e.g. claude-sonnet-5. */
  model: string;
  /** A cheaper model, for the cross-model cache-key question. */
  utilityModel: string;
};

export function anthropicMeasurements(options: AnthropicOptions): Measurement[] {
  const { runId, model, utilityModel } = options;
  const seed = (id: string) => `${runId}/anthropic/${id}`;
  const prefix = (id: string, words = STANDARD_WORDS) => filler(seed(id), words);

  /** A request whose whole cached prefix is one system block. */
  const simple = (
    id: string,
    overrides: {
      words?: number;
      /** Omit for the default 5-minute breakpoint; null places no breakpoint. */
      breakpoint?: { type: string; ttl?: string } | null;
      tools?: Json[];
      toolChoice?: Json;
      ask?: string;
      maxTokens?: number;
      systemExtra?: string;
      systemHeader?: string;
      model?: string;
    } = {},
  ): Json => {
    const blocks: TextBlock[] = [];
    if (overrides.systemHeader !== undefined) blocks.push(text(overrides.systemHeader));
    blocks.push(text(
      prefix(id, overrides.words ?? STANDARD_WORDS),
      overrides.breakpoint === undefined ? BREAKPOINT : overrides.breakpoint ?? undefined,
    ));
    if (overrides.systemExtra !== undefined) blocks.push(text(overrides.systemExtra));

    const body: Json = {
      model: overrides.model ?? model,
      max_tokens: overrides.maxTokens ?? 64,
      system: blocks,
      messages: [{ role: "user", content: overrides.ask ?? ASK }],
    };
    if (overrides.tools) body.tools = overrides.tools;
    if (overrides.toolChoice) body.tool_choice = overrides.toolChoice;
    return body;
  };

  /** Two requests that differ only in the mutation applied to the second. */
  const pair = (
    id: string,
    question: string,
    documented: string,
    second: () => Json,
    meaning: { yes: string; no: string },
    first: () => Json = () => simple(id),
    labels: [string, string] = ["write", "read-attempt"],
  ): Measurement => ({
    id,
    provider: "anthropic",
    question,
    documented,
    requests: [
      { label: labels[0], model, build: first },
      { label: labels[1], model, build: second },
    ],
    verdict: (observations) => reuseVerdict(observations, 1, meaning),
  });

  const toolsPair = (
    id: string,
    question: string,
    documented: string,
    mutate: (tools: Json[]) => Json[],
    meaning: { yes: string; no: string },
  ): Measurement =>
    pair(
      id,
      question,
      documented,
      () => simple(id, { tools: mutate(baseTools()) }),
      meaning,
      () => simple(id, { tools: baseTools() }),
    );

  return [
    {
      id: "write-then-read",
      provider: "anthropic",
      question: "What does the usage block report for a cache write, a cache read, and the uncached suffix?",
      documented:
        "cache_creation_input_tokens counts the write, cache_read_input_tokens the read, and input_tokens only the tokens after the last breakpoint. Writes cost 1.25x base input, reads 0.1x, breakpoints nothing.",
      requests: [
        { label: "write", model, build: () => simple("write-then-read") },
        { label: "read", model, build: () => simple("write-then-read") },
      ],
      verdict: (observations) => {
        const [first, second] = observations;
        if (first.error || second.error) {
          return `INCONCLUSIVE — ${steps(observations)}`;
        }
        const minimum = ANTHROPIC_MINIMUM_CACHEABLE[model];
        const short = minimum !== undefined && totalInput(first.usage) < minimum
          ? ` NOTE: prefix was ${
            totalInput(first.usage)
          } tokens, under this model's documented ${minimum}-token minimum, so every result in this run is suspect.`
          : "";
        return `write charged ${first.usage.write5m} tokens as a 5m cache write with ${first.usage.uncached} uncached; second identical request read ${second.usage.read} and paid ${second.usage.uncached} uncached. Second request cost $${
          second.costUsd.toFixed(6)
        } against $${first.costUsd.toFixed(6)} for the first.${short} [${steps(observations)}]`;
      },
    },

    pair(
      "no-breakpoint",
      "Is a repeated prefix cached when no breakpoint is placed at all?",
      "Caching happens only at a breakpoint: either cache_control on a block or the top-level automatic field. Without one, nothing is written.",
      () => simple("no-breakpoint", { breakpoint: null }),
      {
        yes: "PREFIX REUSED without any breakpoint — caching is not opt-in as documented",
        no: "NO REUSE — a breakpoint is required for any caching to happen",
      },
      () => simple("no-breakpoint", { breakpoint: null }),
    ),

    pair(
      "below-minimum",
      "Is a prefix shorter than the model's minimum cacheable length silently uncached?",
      "Prompts under the minimum (1,024 tokens for Sonnet 5) are processed without caching and no error is returned; both cache usage fields come back 0.",
      () => simple("below-minimum", { words: TINY_WORDS }),
      {
        yes: "REUSED — the documented minimum did not apply here",
        no: "NO REUSE and no error — a short prefix is silently uncached, as documented",
      },
      () => simple("below-minimum", { words: TINY_WORDS }),
    ),

    {
      id: "assistant-turn-write",
      provider: "anthropic",
      question:
        "Is the previous assistant turn charged as a cache write on the next request, rather than having been cached when it was generated?",
      documented:
        "Yes: 'cache creation input tokens account for new assistant and user turns'. Model output is not cached at generation time.",
      requests: [
        {
          label: "turn-1",
          model,
          build: () =>
            simple("assistant-turn-write", {
              ask: "In one short sentence, state that you are ready.",
              maxTokens: 256,
            }),
        },
        {
          label: "turn-2",
          model,
          build: (prior) => ({
            model,
            max_tokens: 64,
            system: [text(prefix("assistant-turn-write"), BREAKPOINT)],
            messages: [
              { role: "user", content: "In one short sentence, state that you are ready." },
              { role: "assistant", content: assistantText(prior[0]) || "I am ready." },
              { role: "user", content: [text(ASK, BREAKPOINT)] },
            ],
          }),
        },
      ],
      verdict: (observations) => {
        const [, second] = observations;
        if (observations.some((o) => o.error)) return `INCONCLUSIVE — ${steps(observations)}`;
        const write = second.usage.write5m + second.usage.write1h;
        return write > 0
          ? `CHARGED AS A WRITE — turn 2 read ${second.usage.read} tokens of the system prefix and paid a ${write}-token cache write covering the assistant turn plus the new user message, as documented [${
            steps(observations)
          }]`
          : `NOT CHARGED AS A WRITE — turn 2 reported no cache write, which contradicts the documentation [${
            steps(observations)
          }]`;
      },
    },

    toolsPair(
      "tool-append",
      "Can a tool be added without invalidating the cached prefix? (Max: 'is it ever possible to change anything about tools without involving the cached prefix')",
      "No: 'Modifying tool definitions (names, descriptions, parameters) invalidates the entire cache'. Tools sit ahead of system and messages in the hashed prefix.",
      (tools) => [...tools, tool("search", "Search the workspace for a pattern.")],
      {
        yes: "REUSED — a tool can be appended without breaking the prefix",
        no:
          "NO REUSE — appending a tool invalidates the whole prefix, so mid-session tool addition cannot be done by append",
      },
    ),

    toolsPair(
      "tool-remove",
      "Does removing a tool invalidate the cached prefix?",
      "Yes: any modification of tool definitions invalidates the entire cache.",
      (tools) => tools.slice(0, -1),
      {
        yes: "REUSED — removing a tool left the prefix intact",
        no: "NO REUSE — tool removal invalidates the whole prefix",
      },
    ),

    toolsPair(
      "tool-description-change",
      "Does editing one tool's description invalidate the cached prefix?",
      "Yes: descriptions are part of the tool definition, and modifying them invalidates the entire cache.",
      (tools) => [
        tools[0],
        { ...(tools[1] as Json), description: "Write the given contents to a file, creating parents." },
        tools[2],
      ],
      {
        yes: "REUSED — a description edit left the prefix intact",
        no: "NO REUSE — editing a tool description invalidates the whole prefix",
      },
    ),

    toolsPair(
      "tool-reorder",
      "Does reordering the tools array invalidate the cached prefix, even with identical definitions?",
      "Not stated directly. Cache hits require 100% identical prompt segments, and the tools array is rendered in order, so reordering should invalidate.",
      (tools) => [tools[1], tools[0], tools[2]],
      {
        yes: "REUSED — tool order is not part of the hashed prefix",
        no:
          "NO REUSE — tool order is part of the hashed prefix, so the tools array must be built deterministically",
      },
    ),

    pair(
      "tool-choice-change",
      "Can tool_choice change without invalidating a breakpoint in the system section?",
      "Yes for tools and system, no for messages: 'Changes to tool_choice parameter only affect message blocks'.",
      () => simple("tool-choice-change", { tools: baseTools(), toolChoice: { type: "none" } }),
      {
        yes: "REUSED — a tool_choice change leaves a system-section breakpoint valid, as documented",
        no: "NO REUSE — a tool_choice change invalidated the system-section prefix too",
      },
      () => simple("tool-choice-change", { tools: baseTools(), toolChoice: { type: "auto" } }),
    ),

    pair(
      "fork-with-breakpoint",
      "Does a fork read its parent's cache when a breakpoint sits at the fork point? (two requests share a long prefix, then diverge)",
      "Yes: the write happened at that breakpoint, and the sibling's lookback finds it.",
      () => simple("fork-with-breakpoint", { ask: "Reply with the single word: two" }),
      {
        yes: "FORK INHERITS — a sibling sharing the prefix reads the cache written at the shared breakpoint",
        no: "FORK INHERITS NOTHING even with a breakpoint at the fork point",
      },
      () => simple("fork-with-breakpoint", { ask: "Reply with the single word: one" }),
      ["parent", "fork"],
    ),

    {
      id: "fork-without-breakpoint",
      provider: "anthropic",
      question:
        "Does a fork read anything when the only breakpoint sat after the fork point, so no write ever happened at the shared boundary?",
      documented:
        "No: 'The lookback does not find stable content behind your breakpoint and cache it. It finds entries that prior requests already wrote.' A fork point must be a breakpoint before the fork, not after.",
      requests: [
        {
          label: "parent",
          model,
          build: () => ({
            model,
            max_tokens: 64,
            system: [text(prefix("fork-without-breakpoint"))],
            messages: [{ role: "user", content: [text("Reply with the single word: one", BREAKPOINT)] }],
          }),
        },
        {
          label: "fork",
          model,
          build: () => ({
            model,
            max_tokens: 64,
            system: [text(prefix("fork-without-breakpoint"))],
            messages: [{ role: "user", content: [text("Reply with the single word: two", BREAKPOINT)] }],
          }),
        },
      ],
      verdict: (observations) =>
        reuseVerdict(observations, 1, {
          yes: "FORK INHERITS — the lookback found a shared prefix nobody wrote a breakpoint at",
          no:
            "FORK INHERITS NOTHING — a fork point must carry its own breakpoint before the fork, or the sibling pays full price",
        }),
    },

    pair(
      "late-system-append",
      "Can a late system section be appended after the breakpoint without invalidating the cached prefix?",
      "Yes: content after the last breakpoint is not part of the hash. It is charged as ordinary input.",
      () =>
        simple("late-system-append", {
          systemExtra: "Late addition: prefer short answers. This block sits after the breakpoint.",
        }),
      {
        yes:
          "REUSED — a system block appended after the breakpoint keeps the prefix valid and is billed as ordinary input",
        no: "NO REUSE — appending after the breakpoint still invalidated the prefix",
      },
    ),

    pair(
      "early-system-change",
      "Does editing a system block that sits before the breakpoint invalidate the cached prefix?",
      "Yes: the hash is cumulative over everything up to and including the breakpoint.",
      () => simple("early-system-change", { systemHeader: "Header revision two." }),
      {
        yes: "REUSED — an edit before the breakpoint did not invalidate",
        no:
          "NO REUSE — any edit before the breakpoint invalidates, so the system section can only change on a fresh prefix",
      },
      () => simple("early-system-change", { systemHeader: "Header revision one." }),
    ),

    {
      id: "mid-conversation-system-message",
      provider: "anthropic",
      question:
        "Can a late instruction be delivered as a role:system message inside messages, leaving the cached prefix intact?",
      documented:
        "Supported on Opus 4.8, Opus 5 and the Fable/Mythos models, and explicitly NOT available on Claude Sonnet 5 — the docs say to use the top-level system field instead. Expect a rejection.",
      requests: [
        { label: "write", model, build: () => simple("mid-conversation-system-message") },
        {
          label: "system-message",
          model,
          build: () => ({
            model,
            max_tokens: 64,
            system: [text(prefix("mid-conversation-system-message"), BREAKPOINT)],
            messages: [
              { role: "user", content: "Reply with the single word: one" },
              { role: "system", content: "Late instruction: prefer short answers." },
              { role: "user", content: ASK },
            ],
          }),
        },
      ],
      verdict: (observations) => {
        const second = observations[1];
        if (second.error) {
          return `REJECTED — this model does not accept a role:system message inside messages, so late instructions must go through a fresh prefix. ${
            steps(observations)
          }`;
        }
        return reuseVerdict(observations, 1, {
          yes: "ACCEPTED AND REUSED — a mid-conversation system message works on this model despite the docs",
          no: "ACCEPTED BUT NOT REUSED — the message was allowed but invalidated the prefix",
        });
      },
    },

    {
      id: "ttl-1h-write-fields",
      provider: "anthropic",
      question:
        "What does a 1-hour-TTL write report, and does the usage block distinguish the two write tiers?",
      documented:
        "A 1-hour write costs 2x base input against 1.25x for 5 minutes; the usage block splits writes into ephemeral_5m_input_tokens and ephemeral_1h_input_tokens.",
      requests: [
        {
          label: "write-1h",
          model,
          build: () => simple("ttl-1h-write-fields", { breakpoint: { type: "ephemeral", ttl: "1h" } }),
        },
        {
          label: "read",
          model,
          build: () => simple("ttl-1h-write-fields", { breakpoint: { type: "ephemeral", ttl: "1h" } }),
        },
      ],
      verdict: (observations) => {
        const [first, second] = observations;
        if (observations.some((o) => o.error)) return `INCONCLUSIVE — ${steps(observations)}`;
        const split = first.usage.write1h > 0
          ? `the write was reported in the 1h tier (${first.usage.write1h} tokens)`
          : `the write was NOT reported in a 1h tier (5m tier: ${first.usage.write5m} tokens) — the TTL split is not observable this way`;
        return `${split}; the following read returned ${second.usage.read} tokens. [${steps(observations)}]`;
      },
    },

    {
      id: "parallel-fork",
      provider: "anthropic",
      question:
        "If two forks sharing a prefix are launched at the same moment, does either read the other's cache?",
      documented:
        "No: 'a cache entry only becomes available after the first response begins. If you need cache hits for parallel requests, wait for the first response before sending subsequent requests.'",
      concurrent: true,
      requests: [
        { label: "parallel-a", model, build: () => simple("parallel-fork") },
        { label: "parallel-b", model, build: () => simple("parallel-fork") },
      ],
      verdict: (observations) => {
        if (observations.some((o) => o.error)) return `INCONCLUSIVE — ${steps(observations)}`;
        const reads = observations.filter(reused).length;
        return reads === 0
          ? `BOTH PAID FULL PRICE — simultaneous forks cannot share a cache write, so a fan-out must serialise its first request to be cache-cheap [${
            steps(observations)
          }]`
          : `${reads} of ${observations.length} simultaneous requests read the cache — the entry became visible before the other request was processed [${
            steps(observations)
          }]`;
      },
    },

    {
      id: "cross-model-read",
      provider: "anthropic",
      question:
        "Can a cheaper utility model read a prefix cached by the larger model at the read rate? (Max: 'it's potentially possible that a utility model (eg. claude haiku) can re-use the same prefix at 0.1x cost. This should be confirmed empirically')",
      documented:
        "Not stated. The cache key is expected to include the model. This prefix is sized above the utility model's own minimum cacheable length so a miss cannot be blamed on prefix length.",
      requests: [
        {
          label: `write-${model}`,
          model,
          build: () => simple("cross-model-read", { words: LARGE_WORDS }),
        },
        {
          label: `read-${utilityModel}`,
          model: utilityModel,
          build: () => simple("cross-model-read", { words: LARGE_WORDS, model: utilityModel }),
        },
      ],
      verdict: (observations) =>
        reuseVerdict(observations, 1, {
          yes:
            `CROSS-MODEL READ WORKS — ${utilityModel} read a prefix written by ${model}, so a utility model can run against the live context cheaply`,
          no:
            `CROSS-MODEL READ FAILS — ${utilityModel} could not read ${model}'s prefix, so a utility model must be given its own one-shot context`,
        }),
    },

    {
      id: "ttl-refresh-5m",
      provider: "anthropic",
      question: "Does reading a cached prefix refresh its 5-minute lifetime?",
      documented:
        "Yes: 'The cache is refreshed for no additional cost each time the cached content is used.' Lifetime is measured from the start of the request that writes or reads it.",
      slow: true,
      requests: [
        { label: "write", model, build: () => simple("ttl-refresh-5m") },
        { label: "read-at-4m30s", model, waitSeconds: 270, build: () => simple("ttl-refresh-5m") },
        { label: "read-at-9m00s", model, waitSeconds: 270, build: () => simple("ttl-refresh-5m") },
      ],
      verdict: (observations) => {
        if (observations.some((o) => o.error)) return `INCONCLUSIVE — ${steps(observations)}`;
        const [, second, third] = observations;
        if (!reused(second)) {
          return `INCONCLUSIVE — the read at 4m30s already missed, so nothing was left to refresh [${
            steps(observations)
          }]`;
        }
        return reused(third)
          ? `REFRESH CONFIRMED — a prefix written 9 minutes earlier was still readable because the read at 4m30s renewed it [${
            steps(observations)
          }]`
          : `NO REFRESH — the entry expired 5 minutes after the write regardless of the intervening read [${
            steps(observations)
          }]`;
      },
    },

    {
      id: "ttl-expiry-5m",
      provider: "anthropic",
      question: "Is the 5-minute expiry observable, and is expiry silent?",
      documented:
        "The default lifetime is 5 minutes from the start of the writing request. Expiry is not signalled; it shows up only as a read of 0 tokens.",
      slow: true,
      requests: [
        { label: "write", model, build: () => simple("ttl-expiry-5m") },
        { label: "read-at-6m30s", model, waitSeconds: 390, build: () => simple("ttl-expiry-5m") },
      ],
      verdict: (observations) =>
        reuseVerdict(observations, 1, {
          yes:
            "STILL WARM AT 6m30s — the 5-minute lifetime is a floor, not a bound, so expiry timing cannot be assumed",
          no:
            "EXPIRED BY 6m30s, silently — the only signal is a read of 0 tokens, so a harness must track cache age itself",
        }),
    },
  ];
}

/** Cheapest possible request: confirms auth, model id and parameter shape. */
export function anthropicPreflight(model: string): Json {
  return {
    model,
    max_tokens: 8,
    messages: [{ role: "user", content: "Reply with the single word: ok" }],
  };
}
