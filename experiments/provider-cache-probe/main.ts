// provider-cache-probe — measures what the Anthropic Messages API and the
// OpenAI Responses API actually charge for a cached prefix, one question per
// request pair. See ../../docs/process/experiments/provider-cache-probe-brief.md.

import { type Measurement, type Price } from "./src/types.ts";
import { ANTHROPIC_PRICES, ANTHROPIC_URL, anthropicMeasurements } from "./src/anthropic.ts";
import { OPENAI_PRICES, OPENAI_URL, openaiMeasurements } from "./src/openai.ts";
import {
  anthropicAdapter,
  AuthError,
  fileSink,
  MissingKeysError,
  openaiAdapter,
  preflight,
  readKeys,
  runMeasurement,
} from "./src/runner.ts";
import { consoleSummary, estimateUpperBoundCost, markdownReport } from "./src/report.ts";

const DOCS_READ = [
  "https://platform.claude.com/docs/en/build-with-claude/prompt-caching — read 2026-09-08",
  "https://platform.openai.com/docs/guides/prompt-caching — read 2026-09-08",
  "https://developers.openai.com/api/docs/pricing — read 2026-09-08",
];

const DEFAULTS = {
  provider: "both",
  out: "results.ignore",
  keys: "keys.ignore.env",
  "anthropic-model": "claude-sonnet-5",
  "anthropic-utility-model": "claude-haiku-4-5",
  "openai-model": "gpt-5.6-luna",
  "openai-large-model": "gpt-5.6-terra",
  "run-id": "",
  only: "",
  "anthropic-url": ANTHROPIC_URL,
  "openai-url": OPENAI_URL,
};

const FLAGS = ["slow", "dry-run", "help"];

function parseArgs(argv: string[]): { options: Record<string, string>; flags: Set<string> } {
  const options: Record<string, string> = { ...DEFAULTS };
  const flags = new Set<string>();
  const problems: string[] = [];

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (!arg.startsWith("--")) {
      problems.push(`unexpected argument: ${arg}`);
      continue;
    }
    const name = arg.slice(2);
    if (FLAGS.includes(name)) {
      flags.add(name);
    } else if (name in options) {
      const value = argv[++i];
      if (value === undefined) problems.push(`--${name} needs a value`);
      else options[name] = value;
    } else {
      problems.push(`unknown option: ${arg}`);
    }
  }

  if (!["anthropic", "openai", "both"].includes(options.provider)) {
    problems.push(`--provider must be anthropic, openai or both (got ${options.provider})`);
  }
  if (problems.length > 0) {
    throw new Error(`${problems.join("\n")}\n\n${usage()}`);
  }
  return { options, flags };
}

function usage(): string {
  return [
    "usage: deno task probe [options]",
    "",
    "  --provider anthropic|openai|both   default both",
    "  --only a,b,c                       run only these measurement ids",
    "  --slow                             include the measurements that wait out a cache TTL",
    "  --dry-run                          build and price every request, send nothing",
    "  --out <dir>                        default results.ignore",
    "  --keys <path>                      default keys.ignore.env",
    "  --anthropic-model <id>             default claude-sonnet-5",
    "  --anthropic-utility-model <id>     default claude-haiku-4-5",
    "  --openai-model <id>                default gpt-5.6-luna",
    "  --openai-large-model <id>          default gpt-5.6-terra",
    "  --run-id <id>                      default is the local timestamp",
    "  --anthropic-url <url>              endpoint override, for the local fake",
    "  --openai-url <url>                 endpoint override, for the local fake",
  ].join("\n");
}

function localTimestamp(date: Date): string {
  const parts = new Intl.DateTimeFormat("en-NZ", {
    timeZone: "Pacific/Auckland",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).formatToParts(date);
  const get = (type: string) => parts.find((part) => part.type === type)?.value ?? "";
  return `${get("year")}${get("month")}${get("day")}-${get("hour")}${get("minute")}${get("second")}`;
}

function priceFor(model: string): Price {
  const price = ANTHROPIC_PRICES[model] ?? OPENAI_PRICES[model];
  if (!price) {
    throw new Error(
      `no price entry for model ${model}. Add it to ANTHROPIC_PRICES or OPENAI_PRICES in src/, with the date the price was read.`,
    );
  }
  return price;
}

function selectMeasurements(
  options: Record<string, string>,
  flags: Set<string>,
  runId: string,
): Measurement[] {
  const wanted = options.only === "" ? null : new Set(options.only.split(",").map((id) => id.trim()));
  const all: Measurement[] = [];

  if (options.provider === "anthropic" || options.provider === "both") {
    all.push(...anthropicMeasurements({
      runId,
      model: options["anthropic-model"],
      utilityModel: options["anthropic-utility-model"],
    }));
  }
  if (options.provider === "openai" || options.provider === "both") {
    all.push(...openaiMeasurements({
      runId,
      model: options["openai-model"],
      largeModel: options["openai-large-model"],
    }));
  }

  const selected = all
    .filter((measurement) => flags.has("slow") || !measurement.slow)
    .filter((measurement) => wanted === null || wanted.has(measurement.id));

  if (wanted !== null) {
    const found = new Set(selected.map((measurement) => measurement.id));
    const missing = [...wanted].filter((id) => !found.has(id));
    if (missing.length > 0) {
      throw new Error(
        `--only named measurements that do not exist or are excluded (slow measurements need --slow): ${
          missing.join(", ")
        }\n\navailable: ${all.map((m) => m.id).join(", ")}`,
      );
    }
  }
  return selected;
}

async function main(): Promise<number> {
  const { options, flags } = parseArgs(Deno.args);
  if (flags.has("help")) {
    console.log(usage());
    return 0;
  }

  const startedAtDate = new Date();
  const runId = options["run-id"] || localTimestamp(startedAtDate);
  const measurements = selectMeasurements(options, flags, runId);

  // Fails here rather than mid-run if a configured model has no price entry.
  for (const measurement of measurements) {
    for (const request of measurement.requests) priceFor(request.model);
  }

  const estimate = estimateUpperBoundCost(measurements, priceFor);
  console.log(`run ${runId}: ${measurements.length} measurements`);
  for (const [provider, cost] of Object.entries(estimate.byProvider)) {
    console.log(`  ${provider}: upper-bound cost $${cost.toFixed(4)}`);
  }
  console.log(`  upper-bound total: $${estimate.total.toFixed(4)}`);

  const root = `${options.out}/${runId}`;
  const sink = fileSink(root);

  if (flags.has("dry-run")) {
    for (const measurement of measurements) {
      console.log(`\n${measurement.provider}/${measurement.id}`);
      console.log(`  question:   ${measurement.question}`);
      console.log(`  documented: ${measurement.documented}`);
      for (const [index, request] of measurement.requests.entries()) {
        await sink(
          `${measurement.provider}/${measurement.id}/${index + 1}-${request.label}.request.json`,
          request.build([]),
        );
        console.log(`  step ${index + 1}: ${request.label} (${request.model})`);
      }
    }
    console.log(`\ndry run — nothing was sent. Request bodies written to ${root}/`);
    return 0;
  }

  const keys = await readKeys(options.keys);
  const adapters = [];
  if (options.provider === "anthropic" || options.provider === "both") {
    if (!keys.anthropic) throw new MissingKeysError(`ANTHROPIC_API_KEY is not set in ${options.keys}`);
    adapters.push({
      adapter: anthropicAdapter(keys.anthropic, options["anthropic-url"]),
      models: [options["anthropic-model"]],
    });
  }
  if (options.provider === "openai" || options.provider === "both") {
    if (!keys.openai) throw new MissingKeysError(`OPENAI_API_KEY is not set in ${options.keys}`);
    adapters.push({
      adapter: openaiAdapter(keys.openai, options["openai-url"]),
      models: [options["openai-model"]],
    });
  }

  for (const { adapter, models } of adapters) {
    console.log(`\npreflight ${adapter.name} (${models[0]})`);
    await preflight(adapter, models[0]);
  }

  const results = [];
  for (const { adapter } of adapters) {
    for (const measurement of measurements.filter((m) => m.provider === adapter.name)) {
      console.log(`\n${adapter.name}/${measurement.id}`);
      const result = await runMeasurement(adapter, measurement, sink, (line) => console.log(line));
      console.log(`  ${result.verdict}`);
      results.push(result);
    }
  }

  const meta = {
    runId,
    startedAt: startedAtDate.toLocaleString("en-NZ", { timeZone: "Pacific/Auckland" }),
    models: Object.fromEntries(
      adapters.map(({ adapter }) =>
        adapter.name === "anthropic"
          ? ["anthropic", [options["anthropic-model"], options["anthropic-utility-model"]]]
          : ["openai", [options["openai-model"], options["openai-large-model"]]]
      ),
    ),
    docsRead: DOCS_READ,
    slow: flags.has("slow"),
  };

  await sink("summary.json", { meta, results });
  await Deno.writeTextFile(`${root}/summary.md`, markdownReport(results, meta));

  console.log(`\n${consoleSummary(results)}`);
  console.log(`\nevidence: ${root}/  (summary.md is the readable one)`);
  return 0;
}

if (import.meta.main) {
  try {
    Deno.exit(await main());
  } catch (error) {
    if (error instanceof MissingKeysError) {
      console.error(error.message);
      Deno.exit(2);
    }
    if (error instanceof AuthError) {
      console.error(error.message);
      Deno.exit(3);
    }
    console.error(error instanceof Error ? error.message : String(error));
    Deno.exit(1);
  }
}
