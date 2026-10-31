#!/usr/bin/env node
// Frame times of the metaballs demo in headless Chromium, over the DevTools
// protocol: node demos/spinner/perf.mjs [QUERY] [--phone] [--throttle N] [--secs S]
// --phone is 412x915 at DPR 2.625, else 1280 x --height (900); --throttle slows the CPU N times, as
// DevTools does. It prints the frame interval (mean, p95), frames, and the
// main thread's busy time (task, script) per frame. --shot FILE saves a
// screenshot, scrolled to the stress grid.
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const args = process.argv.slice(2);
const flag = (name, d) => { const i = args.indexOf(name); return i < 0 ? d : args.splice(i, 2)[1]; };
const phone = args.includes("--phone") && args.splice(args.indexOf("--phone"), 1);
const throttle = +flag("--throttle", 1), secs = +flag("--secs", 5), shot = flag("--shot"), tall = flag("--height");
const query = args[0] ?? "?n=300";
const page = "file://" + path.resolve(path.dirname(new URL(import.meta.url).pathname), "metaballs.html") + query;

const cache = path.join(os.homedir(), ".cache/ms-playwright");
const build = fs.readdirSync(cache).filter((d) => /^chromium-\d+$/.test(d)).sort((a, b) => b.split("-")[1] - a.split("-")[1])[0];
const profile = fs.mkdtempSync(path.join(os.tmpdir(), "perf-"));
const size = phone ? { width: 412, height: 915, dpr: 2.625 } : { width: 1280, height: +(tall ?? 900), dpr: 1 };
const proc = spawn(path.join(cache, build, "chrome-linux64/chrome"), [
  "--headless=new", "--remote-debugging-port=0", `--user-data-dir=${profile}`, "--no-first-run",
  "--enable-unsafe-swiftshader", `--window-size=${size.width},${size.height}`, "about:blank",
], { stdio: ["ignore", "ignore", "pipe"] });
const ws = await new Promise((ok) => {
  let err = "";
  proc.stderr.on("data", (d) => { err += d; const m = err.match(/ws:\/\/\S+/); if (m) ok(m[0]); });
});
const browser = new WebSocket(ws);
await new Promise((ok) => browser.addEventListener("open", ok));
let id = 0;
const waiting = new Map();
browser.addEventListener("message", (e) => {
  const m = JSON.parse(e.data);
  if (m.id && waiting.has(m.id)) { waiting.get(m.id)(m); waiting.delete(m.id); }
});
const send = (method, params = {}, sessionId) => new Promise((ok) => {
  waiting.set(++id, ok);
  browser.send(JSON.stringify({ id, method, params, sessionId }));
});
const { result: { targetId } } = await send("Target.createTarget", { url: "about:blank" });
const { result: { sessionId } } = await send("Target.attachToTarget", { targetId, flatten: true });
const s = (method, params) => send(method, params, sessionId).then((m) => m.result);
await s("Performance.enable");
await s("Emulation.setDeviceMetricsOverride", { width: size.width, height: size.height, deviceScaleFactor: size.dpr, mobile: !!phone });
await s("Emulation.setCPUThrottlingRate", { rate: throttle });
await s("Page.navigate", { url: page });
await new Promise((ok) => setTimeout(ok, 3000));
const metrics = async () => Object.fromEntries((await s("Performance.getMetrics")).metrics.map((m) => [m.name, m.value]));
const before = await metrics();
const r = await s("Runtime.evaluate", { awaitPromise: true, returnByValue: true, expression: `new Promise((ok) => {
  const t = []; let prev = performance.now(); const end = prev + ${secs * 1000};
  const tick = (now) => { t.push(now - prev); prev = now; if (now < end) requestAnimationFrame(tick); else ok(t); };
  requestAnimationFrame(tick);
})` });
const after = await metrics();
const t = r.result.value, sorted = t.slice().sort((a, b) => a - b);
const per = (k) => ((after[k] - before[k]) * 1000 / t.length).toFixed(1);
console.log(`${query} ${phone ? "phone" : "desktop"} x${throttle}: frame ${(t.reduce((a, b) => a + b, 0) / t.length).toFixed(1)} ms, p95 ${sorted[Math.floor(t.length * .95)].toFixed(1)} ms, ${t.length} frames in ${secs} s; main thread per frame: task ${per("TaskDuration")} ms, script ${per("ScriptDuration")} ms`);
if (shot) {
  await s("Runtime.evaluate", { expression: `document.getElementById("stress")?.scrollIntoView()` });
  await new Promise((ok) => setTimeout(ok, 500));
  fs.writeFileSync(shot, Buffer.from((await s("Page.captureScreenshot")).data, "base64"));
}
proc.kill();
fs.rmSync(profile, { recursive: true, force: true, maxRetries: 5 });
process.exit(0);
