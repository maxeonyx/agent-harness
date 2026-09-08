// OpenAI Responses API measurements.
//
// Documentation read 2026-09-08 from
// https://platform.openai.com/docs/guides/prompt-caching (served as
// https://developers.openai.com/api/docs/guides/prompt-caching) and the pricing
// table at https://developers.openai.com/api/docs/pricing.
//
// The two APIs differ in a way that matters for every number below: Anthropic's
// `input_tokens` counts only the uncached suffix, while OpenAI's `input_tokens`
// is the total and the cached and written portions are subtracted out of it.

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

export const OPENAI_URL = "https://api.openai.com/v1/responses";

/** USD per million tokens, short-context standard tier, read 2026-09-08. */
export const OPENAI_PRICES: Record<string, Price> = {
  "gpt-5.6-luna": { input: 0.2, write5m: 0.25, read: 0.02, output: 1.2 },
  "gpt-5.6-terra": { input: 2, write5m: 2.5, read: 0.2, output: 12 },
  "gpt-5.6-sol": { input: 4, write5m: 5, read: 0.4, output: 20 },
  "gpt-6-astra": { input: 10, write5m: 12.5, read: 1, output: 50 },
};

/** Minimum cacheable prefix in visible input tokens, per the docs read 2026-09-08. */
export const OPENAI_MINIMUM_CACHEABLE = 1024;

export function openaiHeaders(apiKey: string): HeadersInit {
  return {
    "content-type": "application/json",
    authorization: `Bearer ${apiKey}`,
  };
}

export function parseOpenaiUsage(response: unknown): Usage {
  const usage = (response as { usage?: Record<string, unknown> } | null)?.usage;
  if (!usage) return { ...ZERO_USAGE };

  const num = (value: unknown) => (typeof value === "number" ? value : 0);
  const details = usage.input_tokens_details as Record<string, unknown> | undefined;
  const read = num(details?.cached_tokens);
  const write = num(details?.cache_write_tokens);

  return {
    uncached: Math.max(0, num(usage.input_tokens) - read - write),
    write5m: write,
    write1h: 0,
    read,
    output: num(usage.output_tokens),
  };
}

export function openaiCost(model: string, usage: Usage): number {
  const price = OPENAI_PRICES[model];
  if (!price) throw new Error(`no price entry for OpenAI model ${model}`);
  return costOf(usage, price);
}

// --- request pieces -------------------------------------------------------

const EXPLICIT = { mode: "explicit" } as const;
const ASK = "Reply with the single word: ok";

type InputText = { type: "input_text"; text: string; prompt_cache_breakpoint?: { mode: string } };

function inputText(body: string, breakpoint = false): InputText {
  return breakpoint
    ? { type: "input_text", text: body, prompt_cache_breakpoint: EXPLICIT }
    : { type: "input_text", text: body };
}

function tool(name: string, description: string): Json {
  return {
    type: "function",
    name,
    description,
    strict: false,
    parameters: {
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
  const output = (observation?.response as { output?: unknown[] } | undefined)?.output;
  if (!Array.isArray(output)) return "";
  return output
    .flatMap((item) => {
      const content = (item as { content?: unknown[] }).content;
      return Array.isArray(content) ? content : [];
    })
    .filter((block): block is { type: string; text: string } =>
      typeof block === "object" && block !== null &&
      (block as { type?: string }).type === "output_text"
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

function reuseVerdict(observations: Observation[], stepIndex: number, meaning: {
  yes: string;
  no: string;
}): string {
  const errored = observations.find((o) => o.error);
  if (errored) return `INCONCLUSIVE — ${errored.label} failed. ${steps(observations)}`;
  const answer = reused(observations[stepIndex]) ? meaning.yes : meaning.no;
  return `${answer} [${steps(observations)}]`;
}

// --- measurements ---------------------------------------------------------

export type OpenaiOptions = {
  runId: string;
  /** The model under test, e.g. gpt-5.6-luna. */
  model: string;
  /** A larger model, for the cross-model cache-key question. */
  largeModel: string;
};

export function openaiMeasurements(options: OpenaiOptions): Measurement[] {
  const { runId, model, largeModel } = options;
  const seed = (id: string) => `${runId}/openai/${id}`;
  const prefix = (id: string, words = STANDARD_WORDS) => filler(seed(id), words);

  const simple = (
    id: string,
    overrides: {
      words?: number;
      /** Defaults to explicit mode with a breakpoint on the developer block. */
      mode?: "explicit" | "implicit";
      breakpoint?: boolean;
      tools?: Json[];
      /** "auto" / "none", or the object form such as allowed_tools. */
      toolChoice?: Json | string;
      ask?: string;
      maxTokens?: number;
      developerExtra?: string;
      cacheKey?: string;
      model?: string;
      reasoningEffort?: string;
      include?: string[];
    } = {},
  ): Json => {
    const input: Json[] = [{
      role: "developer",
      content: [inputText(prefix(id, overrides.words ?? STANDARD_WORDS), overrides.breakpoint ?? true)],
    }];
    if (overrides.developerExtra !== undefined) {
      input.push({ role: "developer", content: overrides.developerExtra });
    }
    input.push({ role: "user", content: overrides.ask ?? ASK });

    const body: Json = {
      model: overrides.model ?? model,
      max_output_tokens: overrides.maxTokens ?? 64,
      store: false,
      prompt_cache_key: overrides.cacheKey ?? seed(id),
      prompt_cache_options: { mode: overrides.mode ?? "explicit" },
      input,
    };
    if (overrides.tools) body.tools = overrides.tools;
    if (overrides.toolChoice) body.tool_choice = overrides.toolChoice;
    if (overrides.reasoningEffort) body.reasoning = { effort: overrides.reasoningEffort };
    if (overrides.include) body.include = overrides.include;
    return body;
  };

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
    provider: "openai",
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
      provider: "openai",
      question: "What does the usage block report for a cache write, a cache read, and the uncached suffix?",
      documented:
        "input_tokens_details.cache_write_tokens counts the write and .cached_tokens the read, both inside the total input_tokens. Writes cost 1.25x uncached input, reads 0.1x.",
      requests: [
        { label: "write", model, build: () => simple("write-then-read") },
        { label: "read", model, build: () => simple("write-then-read") },
      ],
      verdict: (observations) => {
        const [first, second] = observations;
        if (observations.some((o) => o.error)) return `INCONCLUSIVE — ${steps(observations)}`;
        const short = totalInput(first.usage) < OPENAI_MINIMUM_CACHEABLE
          ? ` NOTE: prefix was ${
            totalInput(first.usage)
          } tokens, under the documented ${OPENAI_MINIMUM_CACHEABLE}-token minimum, so every result in this run is suspect.`
          : "";
        return `write charged ${first.usage.write5m} tokens as a cache write with ${first.usage.uncached} uncached; the identical second request read ${second.usage.read} and paid ${second.usage.uncached} uncached. Second request cost $${
          second.costUsd.toFixed(6)
        } against $${first.costUsd.toFixed(6)} for the first.${short} [${steps(observations)}]`;
      },
    },

    pair(
      "implicit-mode",
      "In implicit mode, is an identical prompt cached without any breakpoint being placed by the caller?",
      "Yes: implicit mode places a breakpoint at the end of the latest eligible message and writes through it.",
      () => simple("implicit-mode", { mode: "implicit", breakpoint: false }),
      {
        yes: "REUSED — implicit mode caches without caller-placed breakpoints",
        no: "NO REUSE — implicit mode did not produce a reusable entry here",
      },
      () => simple("implicit-mode", { mode: "implicit", breakpoint: false }),
    ),

    pair(
      "implicit-mode-varying-suffix",
      "In implicit mode with a stable developer message and a changing user message, is the stable part reusable?",
      "No — this is the documented gotcha: 'a shared prefix is not always a cached prefix'. The implicit breakpoint sits after the changing content, so no entry exists at the stable boundary.",
      () =>
        simple("implicit-mode-varying-suffix", {
          mode: "implicit",
          breakpoint: false,
          ask: "Reply with the single word: two",
        }),
      {
        yes: "REUSED — the stable prefix was reusable despite the varying suffix",
        no:
          "NO REUSE — a stable prefix behind a varying suffix needs an explicit breakpoint to be reusable at all",
      },
      () =>
        simple("implicit-mode-varying-suffix", {
          mode: "implicit",
          breakpoint: false,
          ask: "Reply with the single word: one",
        }),
    ),

    pair(
      "explicit-mode-no-breakpoint",
      "In explicit mode with no breakpoints placed, does any caching happen?",
      "No: 'When no explicit breakpoints are placed, the request does not use prompt caching or create cache writes.'",
      () => simple("explicit-mode-no-breakpoint", { breakpoint: false }),
      {
        yes: "REUSED — explicit mode cached something without a breakpoint",
        no: "NO CACHING — explicit mode with no breakpoint neither writes nor reads, as documented",
      },
      () => simple("explicit-mode-no-breakpoint", { breakpoint: false }),
    ),

    pair(
      "below-minimum",
      "Is a prefix shorter than the minimum cacheable length silently uncached?",
      "Yes: a prefix must reach 1,024 visible input tokens on GPT-5.6 and later before it can be cached.",
      () => simple("below-minimum", { words: TINY_WORDS }),
      {
        yes: "REUSED — the documented minimum did not apply here",
        no: "NO REUSE and no error — a short prefix is silently uncached, as documented",
      },
      () => simple("below-minimum", { words: TINY_WORDS }),
    ),

    {
      id: "assistant-turn-write",
      provider: "openai",
      question:
        "Is the previous assistant turn charged as a cache write on the next request, as it is on the Anthropic API?",
      documented:
        "Not stated for the assistant turn specifically. A write covers everything up to the breakpoint that is not already cached, so an assistant turn ahead of the new breakpoint should be charged as a write.",
      requests: [
        {
          label: "turn-1",
          model,
          build: () =>
            simple("assistant-turn-write", {
              ask: "In one short sentence, state that you are ready.",
              maxTokens: 400,
            }),
        },
        {
          label: "turn-2",
          model,
          build: (prior) => {
            const reply = assistantText(prior[0]);
            return {
              model,
              max_output_tokens: 64,
              store: false,
              prompt_cache_key: seed("assistant-turn-write"),
              prompt_cache_options: EXPLICIT,
              input: [
                {
                  role: "developer",
                  content: [inputText(prefix("assistant-turn-write"), true)],
                },
                { role: "user", content: "In one short sentence, state that you are ready." },
                { role: "assistant", content: reply || "I am ready." },
                { role: "user", content: [inputText(ASK, true)] },
              ],
            };
          },
        },
      ],
      verdict: (observations) => {
        const [first, second] = observations;
        if (observations.some((o) => o.error)) return `INCONCLUSIVE — ${steps(observations)}`;
        const synthesised = assistantText(first) === ""
          ? " NOTE: turn 1 returned no assistant text (output budget went to reasoning), so turn 2 fed back a synthesised assistant message."
          : "";
        return second.usage.write5m > 0
          ? `CHARGED AS A WRITE — turn 2 read ${second.usage.read} tokens of the developer prefix and paid a ${second.usage.write5m}-token write covering the assistant turn plus the new user message.${synthesised} [${
            steps(observations)
          }]`
          : `NOT CHARGED AS A WRITE — turn 2 reported no cache write.${synthesised} [${steps(observations)}]`;
      },
    },

    toolsPair(
      "tool-append",
      "Can a tool be added without invalidating the cached prefix? (Max: 'is it ever possible to change anything about tools without involving the cached prefix')",
      "No for the tools array itself: changing 'tool names, descriptions, schemas, ordering, or tool-specific instructions' changes the rendered prefix. The documented way to add a tool mid-thread is an `additional_tools` input item appended at the end of context, which this probe does not exercise.",
      (tools) => [...tools, tool("search", "Search the workspace for a pattern.")],
      {
        yes: "REUSED — a tool can be appended to the tools array without breaking the prefix",
        no:
          "NO REUSE — appending to the tools array invalidates the prefix, so mid-session tool addition needs the append-only mechanisms instead",
      },
    ),

    toolsPair(
      "tool-remove",
      "Does removing a tool invalidate the cached prefix?",
      "Yes: the tools array is part of the rendered prefix.",
      (tools) => tools.slice(0, -1),
      {
        yes: "REUSED — removing a tool left the prefix intact",
        no: "NO REUSE — tool removal invalidates the prefix",
      },
    ),

    toolsPair(
      "tool-description-change",
      "Does editing one tool's description invalidate the cached prefix?",
      "Yes: descriptions are named explicitly as prefix-affecting.",
      (tools) => [
        tools[0],
        { ...(tools[1] as Json), description: "Write the given contents to a file, creating parents." },
        tools[2],
      ],
      {
        yes: "REUSED — a description edit left the prefix intact",
        no: "NO REUSE — editing a tool description invalidates the prefix",
      },
    ),

    toolsPair(
      "tool-reorder",
      "Does reordering the tools array invalidate the cached prefix, even with identical definitions?",
      "Yes: ordering is named explicitly as prefix-affecting.",
      (tools) => [tools[1], tools[0], tools[2]],
      {
        yes: "REUSED — tool order is not part of the rendered prefix",
        no:
          "NO REUSE — tool order is part of the rendered prefix, so the tools array must be built deterministically",
      },
    ),

    pair(
      "tool-choice-none",
      "Can tool use be switched off for one request without invalidating the prefix?",
      "Yes: 'Set tool_choice to \"none\" instead of removing the tool definitions.'",
      () => simple("tool-choice-none", { tools: baseTools(), toolChoice: "none" }),
      {
        yes: "REUSED — tool_choice:none disables tools while keeping the prefix valid, as documented",
        no: "NO REUSE — tool_choice:none invalidated the prefix",
      },
      () => simple("tool-choice-none", { tools: baseTools(), toolChoice: "auto" }),
    ),

    pair(
      "allowed-tools",
      "Can the callable subset of tools change without invalidating the prefix?",
      "Yes: 'Use allowed_tools to restrict which tools are callable while keeping the supplied tools list stable.' This is the documented answer to changing tool availability mid-session.",
      () =>
        simple("allowed-tools", {
          tools: baseTools(),
          toolChoice: {
            type: "allowed_tools",
            mode: "auto",
            tools: [{ type: "function", name: "read_file" }],
          },
        }),
      {
        yes:
          "REUSED — restricting the callable set with allowed_tools keeps the prefix valid, so tool availability can change without a fresh prefix",
        no: "NO REUSE — allowed_tools invalidated the prefix",
      },
      () => simple("allowed-tools", { tools: baseTools(), toolChoice: "auto" }),
    ),

    pair(
      "fork-with-breakpoint",
      "Does a fork read its parent's cache when a breakpoint sits at the fork point?",
      "Yes: the write happened at that breakpoint, and reads consider up to the latest 50 breakpoints in the conversation, reusing the longest match.",
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
      provider: "openai",
      question:
        "Does a fork read anything when the only breakpoint sat after the fork point, so no write ever happened at the shared boundary?",
      documented:
        "No: a later request 'looks for the longest matching cached prefix available, working backward through eligible breakpoints'. With the only breakpoint on the diverging block, no entry exists at the shared boundary.",
      requests: [
        {
          label: "parent",
          model,
          build: () => ({
            model,
            max_output_tokens: 64,
            store: false,
            prompt_cache_key: seed("fork-without-breakpoint"),
            prompt_cache_options: EXPLICIT,
            input: [
              { role: "developer", content: [inputText(prefix("fork-without-breakpoint"))] },
              { role: "user", content: [inputText("Reply with the single word: one", true)] },
            ],
          }),
        },
        {
          label: "fork",
          model,
          build: () => ({
            model,
            max_output_tokens: 64,
            store: false,
            prompt_cache_key: seed("fork-without-breakpoint"),
            prompt_cache_options: EXPLICIT,
            input: [
              { role: "developer", content: [inputText(prefix("fork-without-breakpoint"))] },
              { role: "user", content: [inputText("Reply with the single word: two", true)] },
            ],
          }),
        },
      ],
      verdict: (observations) =>
        reuseVerdict(observations, 1, {
          yes:
            "FORK INHERITS — a shared prefix was reused although no breakpoint was placed at the fork point",
          no:
            "FORK INHERITS NOTHING — a fork point must carry its own breakpoint before the fork, or the sibling pays full price",
        }),
    },

    pair(
      "late-developer-message-change",
      "Can a late developer instruction change without invalidating the cached prefix ahead of it?",
      "Yes: 'If developer instructions contain timestamps, user-specific content, or other dynamic content, place those at the end.' Content after the last breakpoint is charged at the uncached rate with no write.",
      () =>
        simple("late-developer-message-change", {
          developerExtra: "Late addition, revision two: prefer short answers.",
        }),
      {
        yes:
          "REUSED — a developer message after the breakpoint can change freely and is billed as ordinary input",
        no: "NO REUSE — changing a developer message after the breakpoint still invalidated the prefix",
      },
      () =>
        simple("late-developer-message-change", {
          developerExtra: "Late addition, revision one: prefer short answers.",
        }),
    ),

    pair(
      "cache-key-differs",
      "Does a different prompt_cache_key prevent reuse of an identical prefix?",
      "It should not prevent it: 'Keys influence routing; they do not pin requests to a machine or guarantee a cache read hit.' A miss here would mean the key participates in cache identity.",
      () => simple("cache-key-differs", { cacheKey: `${seed("cache-key-differs")}/other` }),
      {
        yes: "REUSED ACROSS KEYS — prompt_cache_key affects routing only, not cache identity",
        no: "NOT REUSED — a different prompt_cache_key cost the hit, so the key must be stable per prefix",
      },
    ),

    pair(
      "reasoning-effort-change",
      "Does changing reasoning.effort between requests invalidate the cached prefix?",
      "Expected yes on this model: 'Request-level changes can alter reasoning instructions.' A per-thread `configuration_update` input item avoids this on GPT-6 Astra only.",
      () => simple("reasoning-effort-change", { reasoningEffort: "medium" }),
      {
        yes: "REUSED — an effort change left the prefix valid",
        no: "NO REUSE — reasoning effort is rendered into the prefix, so changing it costs the whole cache",
      },
      () => simple("reasoning-effort-change", { reasoningEffort: "low" }),
    ),

    {
      id: "parallel-fork",
      provider: "openai",
      question:
        "If two forks sharing a prefix are launched at the same moment, does either read the other's cache?",
      documented:
        "Not stated directly. 'Cached states live on individual machines' and the first request must write before another can read, so simultaneous requests are expected to both miss.",
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
          : `${reads} of ${observations.length} simultaneous requests read the cache [${
            steps(observations)
          }]`;
      },
    },

    {
      id: "cross-model-read",
      provider: "openai",
      question: "Can the cheap model read a prefix cached by a larger model of the same generation?",
      documented:
        "Expected no: 'A different model can use different weights and caching behaviour.' Both models here share the same 1,024-token minimum, so a miss cannot be blamed on prefix length.",
      requests: [
        {
          label: `write-${largeModel}`,
          model: largeModel,
          build: () => simple("cross-model-read", { words: LARGE_WORDS, model: largeModel }),
        },
        {
          label: `read-${model}`,
          model,
          build: () => simple("cross-model-read", { words: LARGE_WORDS }),
        },
      ],
      verdict: (observations) =>
        reuseVerdict(observations, 1, {
          yes:
            `CROSS-MODEL READ WORKS — ${model} read a prefix written by ${largeModel}, so a utility model can run against the live context cheaply`,
          no:
            `CROSS-MODEL READ FAILS — ${model} could not read ${largeModel}'s prefix, so a utility model must be given its own one-shot context`,
        }),
    },

    {
      id: "ttl-hold-30m",
      provider: "openai",
      question: "Is the documented 30-minute minimum cache lifetime real?",
      documented:
        "'A cached prefix remains eligible for reuse for 30 minutes after its most recent write or reuse, though OpenAI may retain it longer.' Only the floor is falsifiable: a hit at 31 minutes confirms the floor, and a longer wait proves nothing because retention may exceed it.",
      slow: true,
      requests: [
        { label: "write", model, build: () => simple("ttl-hold-30m") },
        { label: "read-at-31m", model, waitSeconds: 1860, build: () => simple("ttl-hold-30m") },
      ],
      verdict: (observations) =>
        reuseVerdict(observations, 1, {
          yes: "FLOOR HOLDS — a prefix written 31 minutes earlier was still readable",
          no: "FLOOR BROKEN — the entry was gone before the documented 30-minute minimum",
        }),
    },
  ];
}

/** Cheapest possible request: confirms auth, model id and parameter shape. */
export function openaiPreflight(model: string): Json {
  return {
    model,
    max_output_tokens: 16,
    store: false,
    input: [{ role: "user", content: "Reply with the single word: ok" }],
  };
}
