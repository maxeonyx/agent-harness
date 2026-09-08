// Pure rendering and pure cost arithmetic. No I/O.

import {
  costOf,
  describe,
  type Json,
  type Measurement,
  type MeasurementResult,
  type Price,
  totalInput,
} from "./types.ts";

/** Every string in a request body, for a character-based token estimate. */
function textLength(value: unknown): number {
  if (typeof value === "string") return value.length;
  if (Array.isArray(value)) return value.reduce((total, item) => total + textLength(item), 0);
  if (typeof value === "object" && value !== null) {
    return Object.values(value).reduce((total: number, item) => total + textLength(item), 0);
  }
  return 0;
}

/**
 * An upper bound, for saying what a run will cost before spending anything.
 * Assumes every request pays a full cache write over its whole body and burns
 * its entire output budget — both false in practice, since reads are the point
 * and 64 output tokens rarely all get used. Four characters per token is the
 * usual English rule of thumb.
 */
export function estimateUpperBoundCost(
  measurements: Measurement[],
  priceFor: (model: string) => Price,
): { byProvider: Record<string, number>; total: number } {
  const byProvider: Record<string, number> = {};
  for (const measurement of measurements) {
    for (const request of measurement.requests) {
      const body = request.build([]) as Json;
      const inputTokens = Math.ceil(textLength(body) / 4);
      const outputBudget = Number(body.max_tokens ?? body.max_output_tokens ?? 64);
      const price = priceFor(request.model);
      const cost = costOf(
        { uncached: 0, write5m: inputTokens, write1h: 0, read: 0, output: outputBudget },
        price,
      );
      byProvider[measurement.provider] = (byProvider[measurement.provider] ?? 0) + cost;
    }
  }
  const total = Object.values(byProvider).reduce((sum, value) => sum + value, 0);
  return { byProvider, total };
}

export function consoleSummary(results: MeasurementResult[]): string {
  const lines: string[] = [];
  for (const result of results) {
    lines.push(`${result.provider}/${result.id}  ($${result.costUsd.toFixed(6)})`);
    lines.push(`  ${result.verdict}`);
  }
  const total = results.reduce((sum, result) => sum + result.costUsd, 0);
  lines.push("");
  lines.push(`total spent: $${total.toFixed(4)} over ${results.length} measurements`);
  return lines.join("\n");
}

export type ReportMeta = {
  runId: string;
  startedAt: string;
  models: Record<string, string[]>;
  docsRead: string[];
  slow: boolean;
};

export function markdownReport(results: MeasurementResult[], meta: ReportMeta): string {
  const lines: string[] = [];
  const total = results.reduce((sum, result) => sum + result.costUsd, 0);

  lines.push(`# provider-cache-probe run ${meta.runId}`);
  lines.push("");
  lines.push(`Started ${meta.startedAt}. Slow (TTL) measurements ${meta.slow ? "included" : "skipped"}.`);
  lines.push(`Total billed: $${total.toFixed(4)} across ${results.length} measurements.`);
  lines.push("");
  lines.push("Models exercised:");
  lines.push("");
  for (const [provider, models] of Object.entries(meta.models)) {
    lines.push(`- ${provider}: ${models.join(", ")}`);
  }
  lines.push("");
  lines.push("Documentation the expectations came from:");
  lines.push("");
  for (const source of meta.docsRead) lines.push(`- ${source}`);
  lines.push("");
  lines.push(
    "`read`/`write`/`uncached` below are token counts taken from the provider's own usage block, normalised so the two APIs compare directly: Anthropic reports the uncached suffix in `input_tokens`, while OpenAI reports a total from which the cached and written portions are subtracted.",
  );

  for (const provider of ["anthropic", "openai"] as const) {
    const forProvider = results.filter((result) => result.provider === provider);
    if (forProvider.length === 0) continue;

    lines.push("");
    lines.push(`## ${provider}`);

    for (const result of forProvider) {
      lines.push("");
      lines.push(`### ${result.id}`);
      lines.push("");
      lines.push(`**Question.** ${result.question}`);
      lines.push("");
      lines.push(`**Documented.** ${result.documented}`);
      lines.push("");
      lines.push(`**Measured.** ${result.verdict}`);
      lines.push("");
      lines.push("| step | model | http | read | write | uncached | output | cost |");
      lines.push("| --- | --- | --- | --- | --- | --- | --- | --- |");
      for (const observation of result.observations) {
        const u = observation.usage;
        lines.push(
          `| ${observation.label} | ${observation.model} | ${observation.httpStatus} | ${u.read} | ${
            u.write5m + u.write1h
          } | ${u.uncached} | ${u.output} | $${observation.costUsd.toFixed(6)} |`,
        );
      }
      const totalPrefix = result.observations[0] ? totalInput(result.observations[0].usage) : 0;
      lines.push("");
      lines.push(
        `First step's total input: ${totalPrefix} tokens. Raw request and response bodies: \`${provider}/${result.id}/\`.`,
      );
      const failures = result.observations.filter((observation) => observation.error);
      for (const failure of failures) {
        lines.push("");
        lines.push(`Step \`${failure.label}\` failed — ${describe(failure)}`);
      }
    }
  }

  return lines.join("\n") + "\n";
}
