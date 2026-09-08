// A local stand-in for both APIs, used to verify the probe itself.
//
// It is NOT evidence and it is NOT a model of provider behaviour: it implements
// the caching rules the documentation describes, so running against it can only
// show that the probe builds the requests it means to, parses each provider's
// usage block correctly, prices the tokens correctly, reaches every verdict
// branch, and writes its evidence files. Any claim about how a provider
// actually behaves has to come from a run against the real endpoint.
//
//   deno task fake                       # listens on 127.0.0.1:8477
//   deno run --allow-net --allow-read=. --allow-write=. main.ts \
//     --anthropic-url http://127.0.0.1:8477/v1/messages \
//     --openai-url http://127.0.0.1:8477/v1/responses

const PORT = Number(Deno.env.get("FAKE_PROVIDER_PORT") ?? 8477);

/** Prefixes that have been written, and the moment each became readable. */
const written = new Map<string, number>();

/** Emulates "a cache entry only becomes available after the first response begins". */
const VISIBILITY_DELAY_MS = 200;

const MINIMUM_CACHEABLE: Record<string, number> = {
  "claude-sonnet-5": 1024,
  "claude-haiku-4-5": 4096,
  "gpt-5.6-luna": 1024,
  "gpt-5.6-terra": 1024,
};

const estimateTokens = (text: string) => Math.ceil(text.length / 4);

type Block = { text: string; breakpoint: boolean };
type Billing = { read: number; write: number; uncached: number };

/**
 * Writes an entry at every breakpoint, reads the longest entry a previous
 * request already wrote, and charges the gap between the two as a write — the
 * rules as the Anthropic docs state them, which the OpenAI docs restate almost
 * word for word.
 */
function bill(model: string, blocks: Block[]): Billing {
  const positions = blocks.flatMap((block, index) => (block.breakpoint ? [index] : []));
  const joined = (end: number) => blocks.slice(0, end + 1).map((block) => block.text).join("\u0000");
  const allTokens = estimateTokens(joined(blocks.length - 1));
  if (positions.length === 0) return { read: 0, write: 0, uncached: allTokens };

  const last = positions[positions.length - 1];
  const throughLast = estimateTokens(joined(last));
  const suffix = allTokens - throughLast;
  const minimum = MINIMUM_CACHEABLE[model] ?? 1024;
  if (throughLast < minimum) return { read: 0, write: 0, uncached: allTokens };

  // The cache key includes the model, so a different model cannot read it.
  const keyAt = (end: number) => `${model}\u0000${joined(end)}`;

  let read = 0;
  for (let index = positions.length - 1; index >= 0; index--) {
    const visibleAt = written.get(keyAt(positions[index]));
    if (visibleAt !== undefined && Date.now() >= visibleAt) {
      read = estimateTokens(joined(positions[index]));
      break;
    }
  }
  for (const position of positions) {
    const key = keyAt(position);
    if (!written.has(key)) written.set(key, Date.now() + VISIBILITY_DELAY_MS);
  }

  return { read, write: throughLast - read, uncached: suffix };
}

// --- Anthropic shape ------------------------------------------------------

type AnthropicBody = {
  model: string;
  tools?: unknown[];
  system?: { text: string; cache_control?: unknown }[];
  messages: { role: string; content: unknown }[];
};

function anthropicBlocks(body: AnthropicBody): Block[] {
  const blocks: Block[] = [];
  if (body.tools) blocks.push({ text: JSON.stringify(body.tools), breakpoint: false });
  for (const block of body.system ?? []) {
    blocks.push({ text: block.text, breakpoint: block.cache_control !== undefined });
  }
  for (const message of body.messages) {
    const content = message.content;
    if (typeof content === "string") {
      blocks.push({ text: `${message.role}:${content}`, breakpoint: false });
      continue;
    }
    for (const block of content as { text?: string; cache_control?: unknown }[]) {
      blocks.push({
        text: `${message.role}:${block.text ?? JSON.stringify(block)}`,
        breakpoint: block.cache_control !== undefined,
      });
    }
  }
  return blocks;
}

function handleAnthropic(body: AnthropicBody): Response {
  if (body.messages.some((message) => message.role === "system")) {
    return Response.json({
      type: "error",
      error: {
        type: "invalid_request_error",
        message: `messages: role "system" is not supported by ${body.model}; use the top-level system field`,
      },
    }, { status: 400 });
  }

  const billing = bill(body.model, anthropicBlocks(body));
  const oneHour = body.system?.some((block) =>
    (block.cache_control as { ttl?: string } | undefined)?.ttl === "1h"
  );

  return Response.json({
    id: `msg_fake_${crypto.randomUUID()}`,
    type: "message",
    role: "assistant",
    model: body.model,
    content: [{ type: "text", text: "ok" }],
    stop_reason: "end_turn",
    usage: {
      input_tokens: billing.uncached,
      cache_creation_input_tokens: billing.write,
      cache_read_input_tokens: billing.read,
      cache_creation: {
        ephemeral_5m_input_tokens: oneHour ? 0 : billing.write,
        ephemeral_1h_input_tokens: oneHour ? billing.write : 0,
      },
      output_tokens: 3,
    },
  });
}

// --- OpenAI Responses shape ----------------------------------------------

type OpenaiBody = {
  model: string;
  tools?: unknown[];
  prompt_cache_options?: { mode?: string };
  input: { role?: string; content: unknown }[];
};

function openaiBlocks(body: OpenaiBody, implicit: boolean): Block[] {
  const blocks: Block[] = [];
  if (body.tools) blocks.push({ text: JSON.stringify(body.tools), breakpoint: false });
  for (const item of body.input) {
    const content = item.content;
    if (typeof content === "string") {
      blocks.push({ text: `${item.role}:${content}`, breakpoint: false });
      continue;
    }
    for (const block of content as { text?: string; prompt_cache_breakpoint?: unknown }[]) {
      blocks.push({
        text: `${item.role}:${block.text ?? JSON.stringify(block)}`,
        breakpoint: !implicit && block.prompt_cache_breakpoint !== undefined,
      });
    }
  }
  // Implicit mode puts a breakpoint at the end of the latest eligible message.
  if (implicit && blocks.length > 0) blocks[blocks.length - 1].breakpoint = true;
  return blocks;
}

function handleOpenai(body: OpenaiBody): Response {
  const implicit = (body.prompt_cache_options?.mode ?? "implicit") === "implicit";
  const billing = bill(body.model, openaiBlocks(body, implicit));

  return Response.json({
    id: `resp_fake_${crypto.randomUUID()}`,
    object: "response",
    model: body.model,
    status: "completed",
    output: [{
      type: "message",
      role: "assistant",
      content: [{ type: "output_text", text: "ok" }],
    }],
    usage: {
      input_tokens: billing.read + billing.write + billing.uncached,
      input_tokens_details: {
        cached_tokens: billing.read,
        cache_write_tokens: billing.write,
      },
      output_tokens: 3,
    },
  });
}

// --- server ---------------------------------------------------------------

Deno.serve({ port: PORT, hostname: "127.0.0.1" }, async (request) => {
  const url = new URL(request.url);
  const authorized = request.headers.has("x-api-key") || request.headers.has("authorization");
  if (!authorized) {
    return Response.json({ error: { message: "missing credential" } }, { status: 401 });
  }

  const body = await request.json();
  if (url.pathname === "/v1/messages") return handleAnthropic(body);
  if (url.pathname === "/v1/responses") return handleOpenai(body);
  return Response.json({ error: { message: `no route ${url.pathname}` } }, { status: 404 });
});

console.log(`fake provider on http://127.0.0.1:${PORT} (/v1/messages, /v1/responses)`);
