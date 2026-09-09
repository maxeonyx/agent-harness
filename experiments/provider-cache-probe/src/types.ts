// Shared vocabulary for the probe. Everything here is data or pure functions;
// the network and the filesystem live in `runner.ts`.

export type Json = Record<string, unknown>;

export type ProviderName = "anthropic" | "openai";

/** Normalised billing view of one response, so both providers compare directly. */
export type Usage = {
  /** Input tokens billed at the ordinary (uncached) input rate. */
  uncached: number;
  /** Input tokens billed as a cache write at the 5-minute rate. */
  write5m: number;
  /** Input tokens billed as a cache write at the 1-hour rate. */
  write1h: number;
  /** Input tokens billed at the cache-read rate. */
  read: number;
  /** Output tokens (includes reasoning tokens where the provider bills them as output). */
  output: number;
};

export type Price = {
  /** USD per million tokens. */
  input: number;
  write5m: number;
  /** Absent where the provider has no 1-hour tier. */
  write1h?: number;
  read: number;
  output: number;
};

export type RequestSpec = {
  /** Short name for this step, e.g. "write" or "read-after-tool-added". */
  label: string;
  model: string;
  /**
   * Builds the request body. Takes the observations already made in this
   * measurement, so a step can feed a real assistant turn back into the next
   * request. Pure.
   */
  build: (prior: Observation[]) => Json;
  /** Seconds to sleep before sending. Only the TTL measurements use this. */
  waitSeconds?: number;
};

export type Measurement = {
  id: string;
  provider: ProviderName;
  /** The design question this measurement exists to answer, in one sentence. */
  question: string;
  /** What the provider documentation claims, so a difference is visible. */
  documented: string;
  requests: RequestSpec[];
  /** Fire all requests at once instead of one after another. */
  concurrent?: boolean;
  /** Excluded unless --slow: these wait out a cache TTL. */
  slow?: boolean;
  /** States the measured fact. Pure over the observations. */
  verdict: (observations: Observation[]) => string;
};

export type Observation = {
  label: string;
  model: string;
  httpStatus: number;
  /** Request body as sent. Never contains credentials — headers are not recorded. */
  request: Json;
  /** Response body as received, verbatim. */
  response: unknown;
  usage: Usage;
  costUsd: number;
  elapsedMs: number;
  /** Set when the provider returned a non-2xx status. */
  error?: string;
};

export type MeasurementResult = {
  id: string;
  provider: ProviderName;
  question: string;
  documented: string;
  verdict: string;
  costUsd: number;
  observations: Observation[];
};

export const ZERO_USAGE: Usage = {
  uncached: 0,
  write5m: 0,
  write1h: 0,
  read: 0,
  output: 0,
};

export function costOf(usage: Usage, price: Price): number {
  const write1hRate = price.write1h ?? price.write5m;
  return (
    (usage.uncached * price.input +
      usage.write5m * price.write5m +
      usage.write1h * write1hRate +
      usage.read * price.read +
      usage.output * price.output) /
    1_000_000
  );
}

export function totalInput(usage: Usage): number {
  return usage.uncached + usage.write5m + usage.write1h + usage.read;
}

/** One-line billing summary of a step, used in verdicts and the report. */
export function describe(observation: Observation): string {
  if (observation.error) return `HTTP ${observation.httpStatus}: ${observation.error}`;
  const u = observation.usage;
  const write = u.write5m + u.write1h;
  return `read=${u.read} write=${write} uncached=${u.uncached} output=${u.output}`;
}
