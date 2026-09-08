// The edges: credentials, HTTP, the clock, and the filesystem. Everything that
// decides anything lives in the provider modules and `report.ts`.

import {
  type Json,
  type Measurement,
  type MeasurementResult,
  type Observation,
  type ProviderName,
  type Usage,
  ZERO_USAGE,
} from "./types.ts";
import {
  ANTHROPIC_URL,
  anthropicCost,
  anthropicHeaders,
  anthropicPreflight,
  parseAnthropicUsage,
} from "./anthropic.ts";
import { OPENAI_URL, openaiCost, openaiHeaders, openaiPreflight, parseOpenaiUsage } from "./openai.ts";

export class AuthError extends Error {}
export class MissingKeysError extends Error {}

export type Keys = { anthropic?: string; openai?: string };

/**
 * Reads `KEY=value` lines. The file is `.ignore`-named so it cannot be
 * committed, and its contents never enter a log, a result file or an error
 * message: only the names of the keys that were found are ever reported.
 */
export async function readKeys(path: string): Promise<Keys> {
  let contents: string;
  try {
    contents = await Deno.readTextFile(path);
  } catch (cause) {
    if (cause instanceof Deno.errors.NotFound) {
      throw new MissingKeysError(
        [
          `No credentials file at ${path}.`,
          "",
          "Create it with one line per provider you want to probe:",
          "",
          "    ANTHROPIC_API_KEY=sk-ant-...",
          "    OPENAI_API_KEY=sk-...",
          "",
          "The `.ignore` in the filename keeps it out of git. Nothing in this",
          "probe prints, logs or stores a key.",
        ].join("\n"),
      );
    }
    throw cause;
  }

  const keys: Keys = {};
  for (const line of contents.split("\n")) {
    const trimmed = line.trim();
    if (trimmed === "" || trimmed.startsWith("#")) continue;
    const split = trimmed.indexOf("=");
    if (split === -1) continue;
    const name = trimmed.slice(0, split).trim();
    const value = trimmed.slice(split + 1).trim().replace(/^["']|["']$/g, "");
    if (name === "ANTHROPIC_API_KEY") keys.anthropic = value;
    if (name === "OPENAI_API_KEY") keys.openai = value;
  }
  return keys;
}

export type Adapter = {
  name: ProviderName;
  url: string;
  headers: HeadersInit;
  parseUsage: (response: unknown) => Usage;
  cost: (model: string, usage: Usage) => number;
  /** Minimum spacing between sequential requests, in milliseconds. */
  gapMs: number;
  /** Mutable: when the last request went out, for enforcing gapMs. */
  pacing: { lastSentAt: number };
  preflight: (model: string) => Json;
};

export function anthropicAdapter(apiKey: string, url = ANTHROPIC_URL): Adapter {
  return {
    name: "anthropic",
    url,
    headers: anthropicHeaders(apiKey),
    parseUsage: parseAnthropicUsage,
    cost: anthropicCost,
    gapMs: 500,
    pacing: { lastSentAt: 0 },
    preflight: anthropicPreflight,
  };
}

export function openaiAdapter(apiKey: string, url = OPENAI_URL): Adapter {
  return {
    name: "openai",
    url,
    headers: openaiHeaders(apiKey),
    parseUsage: parseOpenaiUsage,
    cost: openaiCost,
    // Cached states live on individual machines and traffic above 15 requests
    // per minute can overflow to a machine without the entry, which would show
    // up as a cache miss that means nothing. 4.2s keeps the run under that.
    gapMs: 4200,
    pacing: { lastSentAt: 0 },
    preflight: openaiPreflight,
  };
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

async function send(
  adapter: Adapter,
  label: string,
  model: string,
  body: Json,
  options: { paced: boolean } = { paced: true },
): Promise<Observation> {
  // The pacing gate is global to the adapter, not per measurement: on OpenAI,
  // exceeding 15 requests per minute can overflow to a machine without the
  // cache entry, which would look like a miss and mean nothing.
  if (options.paced) {
    const waitMs = adapter.pacing.lastSentAt + adapter.gapMs - Date.now();
    if (waitMs > 0) await sleep(waitMs);
    adapter.pacing.lastSentAt = Date.now();
  }

  const startedAt = performance.now();
  const response = await fetch(adapter.url, {
    method: "POST",
    headers: adapter.headers,
    body: JSON.stringify(body),
  });
  const text = await response.text();
  const elapsedMs = Math.round(performance.now() - startedAt);

  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    parsed = { non_json_body: text };
  }

  if (response.status === 401 || response.status === 403) {
    throw new AuthError(
      `${adapter.name} rejected the credential (HTTP ${response.status}). Check the key in keys.ignore.env.`,
    );
  }

  if (!response.ok) {
    return {
      label,
      model,
      httpStatus: response.status,
      request: body,
      response: parsed,
      usage: { ...ZERO_USAGE },
      costUsd: 0,
      elapsedMs,
      error: JSON.stringify(parsed).slice(0, 500),
    };
  }

  const usage = adapter.parseUsage(parsed);
  return {
    label,
    model,
    httpStatus: response.status,
    request: body,
    response: parsed,
    usage,
    costUsd: adapter.cost(model, usage),
    elapsedMs,
  };
}

export type Sink = (relativePath: string, contents: unknown) => Promise<void>;

export function fileSink(root: string): Sink {
  return async (relativePath, contents) => {
    const path = `${root}/${relativePath}`;
    await Deno.mkdir(path.slice(0, path.lastIndexOf("/")), { recursive: true });
    await Deno.writeTextFile(path, JSON.stringify(contents, null, 2) + "\n");
  };
}

export async function runMeasurement(
  adapter: Adapter,
  measurement: Measurement,
  sink: Sink,
  log: (line: string) => void,
): Promise<MeasurementResult> {
  const observations: Observation[] = [];

  if (measurement.concurrent) {
    // Simultaneity is the thing being measured, so these bypass the pacing gate.
    const waitMs = adapter.pacing.lastSentAt + adapter.gapMs - Date.now();
    if (waitMs > 0) await sleep(waitMs);
    adapter.pacing.lastSentAt = Date.now();
    const results = await Promise.all(
      measurement.requests.map((request) =>
        send(adapter, request.label, request.model, request.build([]), { paced: false })
      ),
    );
    observations.push(...results);
  } else {
    for (const request of measurement.requests) {
      if (request.waitSeconds) {
        log(`    waiting ${request.waitSeconds}s before ${request.label}`);
        await sleep(request.waitSeconds * 1000);
      }
      observations.push(
        await send(adapter, request.label, request.model, request.build(observations)),
      );
    }
  }

  for (const [index, observation] of observations.entries()) {
    const stem = `${adapter.name}/${measurement.id}/${index + 1}-${observation.label}`;
    await sink(`${stem}.request.json`, observation.request);
    await sink(`${stem}.response.json`, observation.response);
  }

  return {
    id: measurement.id,
    provider: measurement.provider,
    question: measurement.question,
    documented: measurement.documented,
    verdict: measurement.verdict(observations),
    costUsd: observations.reduce((total, o) => total + o.costUsd, 0),
    observations,
  };
}

/** Confirms auth, model id and parameter shape before spending the suite. */
export async function preflight(adapter: Adapter, model: string): Promise<void> {
  const observation = await send(adapter, "preflight", model, adapter.preflight(model));
  if (observation.error) {
    throw new Error(
      `${adapter.name} preflight failed for model ${model} (HTTP ${observation.httpStatus}): ${observation.error}`,
    );
  }
}
