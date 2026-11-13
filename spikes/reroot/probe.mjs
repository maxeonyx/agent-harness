// T9 spike: does the provider accept a context's old messages, signed
// thinking blocks included, under a new root (system prompt, tool list,
// model, params)? And what does each re-root cost in cache terms?
//
//   node spikes/reroot/probe.mjs [MODEL]     # default claude-sonnet-5-5
//
// Each request goes through `harness-api` on Max's login, so it spends his
// usage (a few cents in all). Bodies and responses land in
// spikes/reroot/out.ignore/ for inspection.

import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";

const MODEL = process.argv[2] ?? "claude-sonnet-5-5";
const OTHER = MODEL.includes("opus") ? "claude-sonnet-5-5" : "claude-opus-5-5";
const API = new URL("../../target/debug/harness-api", import.meta.url).pathname;
const OUT = new URL("./out.ignore/", import.meta.url).pathname;
mkdirSync(OUT, { recursive: true });

const IDENTITY = "You are Claude Code, Anthropic's official CLI for Claude.";
const ttl = { type: "ephemeral", ttl: "1h" };
// Past every model's minimum cacheable prefix (4096 tokens on the largest).
const padding = Array.from(
  { length: 400 },
  (_, i) => `Rule ${i}: keep file ${i} tidy and name things for what they hold.`,
).join("\n");

const system = (role) => [
  { type: "text", text: IDENTITY },
  { type: "text", text: `${role}\n\n${padding}`, cache_control: ttl },
];
const tool = (name, description) => ({
  name,
  description,
  input_schema: { type: "object", properties: { path: { type: "string" } }, required: ["path"] },
});
const READ = tool("read", "Read a file's whole text.");
const tools1 = [READ, tool("list", "List a directory.")];
const tools2 = [tool("read", "Read a file as numbered lines."), tool("list", "List a directory."), tool("grep", "Search files.")];
const tools3 = [tool("list", "List a directory."), tool("grep", "Search files.")];

const params = (effort) => ({
  max_tokens: 8000,
  thinking: { type: "adaptive", display: "summarized" },
  output_config: { effort },
});

// The last block of the last message carries the moving breakpoint, as the
// harness's own requests do.
const marked = (messages) =>
  messages.map((m, i) => {
    if (i !== messages.length - 1) return m;
    const content = typeof m.content === "string" ? [{ type: "text", text: m.content }] : m.content;
    return { ...m, content: content.map((b, j) => (j === content.length - 1 ? { ...b, cache_control: ttl } : b)) };
  });

let n = 0;
function call(label, { model = MODEL, sys, tools, effort = "high", messages }) {
  n += 1;
  const body = { model, ...params(effort), system: sys, tools, messages: marked(messages), stream: false };
  const file = `${OUT}${n}-${label}.json`;
  writeFileSync(file, JSON.stringify(body, null, 1));
  let stdout;
  let status;
  try {
    stdout = execFileSync(API, ["POST", "/v1/messages", file], { stdio: ["ignore", "pipe", "pipe"] }).toString();
    status = 200;
  } catch (e) {
    stdout = e.stdout.toString();
    status = Number(e.stderr.toString().split("\n")[0]) || e.status;
  }
  writeFileSync(`${OUT}${n}-${label}.response.json`, stdout);
  const r = JSON.parse(stdout);
  const u = r.usage ?? {};
  const blocks = (r.content ?? []).map((b) => b.type).join(",");
  console.log(
    [
      `${n} ${label}`.padEnd(34),
      `${status}`,
      `model=${model}`,
      `input=${u.input_tokens ?? "-"}`,
      `cache_write=${u.cache_creation_input_tokens ?? "-"}`,
      `cache_read=${u.cache_read_input_tokens ?? "-"}`,
      r.error ? `error=${JSON.stringify(r.error.message)}` : `blocks=${blocks}`,
    ].join("  "),
  );
  return r;
}

const ask = {
  role: "user",
  content:
    "Before answering, think carefully: is 391 prime? Then read notes.txt with the read tool. Do both, in that order.",
};
const r1 = call("start", { sys: system("You are a careful engineer."), tools: tools1, messages: [ask] });
const asst = { role: "assistant", content: r1.content };
const use = r1.content.find((b) => b.type === "tool_use");
if (!r1.content.some((b) => b.type === "thinking")) throw new Error("no thinking block to re-root; rerun");
if (!use) throw new Error("no tool_use; rerun");
const result = {
  role: "user",
  content: [{ type: "tool_result", tool_use_id: use.id, content: "notes.txt: ship on Friday." }],
};
const history = [ask, asst, result];
const S1 = system("You are a careful engineer.");
const S2 = system("You are a careful engineer. Your role file changed: answer in one line.");

call("continue (same root)", { sys: S1, tools: tools1, messages: history });
call("re-root: system+tools", { sys: S2, tools: tools2, messages: history });
call("continue on new root", { sys: S2, tools: tools2, messages: history });
call("re-root: used tool gone", { sys: S2, tools: tools3, messages: history });
call("re-root: effort low", { sys: S1, tools: tools1, effort: "low", messages: history });
call(`re-root: model ${OTHER}`, { model: OTHER, sys: S2, tools: tools2, messages: history });
// Controls: is the signature checked at all? The displayed thinking is a
// summary, so editing it may change nothing; the signature carries the rest.
const edit = (f) => [ask, { role: "assistant", content: asst.content.flatMap((b) => (b.type === "thinking" ? f(b) : [b])) }, result];
call("control: edited summary", { sys: S2, tools: tools2, messages: edit((b) => [{ ...b, thinking: `${b.thinking} (edited)` }]) });
const sig = (s) => s.slice(0, 40) + (s[40] === "A" ? "B" : "A") + s.slice(41);
call("control: edited signature", { sys: S2, tools: tools2, messages: edit((b) => [{ ...b, signature: sig(b.signature) }]) });
call("control: thinking dropped", { sys: S2, tools: tools2, messages: edit(() => []) });
