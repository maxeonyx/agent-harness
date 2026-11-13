#!/usr/bin/env node
// Frame times of the metaballs demo in headless Chromium, over the DevTools
// protocol: node demos/spinner/perf.mjs [QUERY] [--phone] [--throttle N] [--secs S]
// --phone is 412x915 at DPR 2.625, else 1280 x --height (900); --throttle slows the CPU N times, as
// DevTools does. It prints the frame interval (mean, p95), frames, and the
// main thread's busy time (task, script) per frame. --shot FILE saves a
// screenshot, scrolled to the stress grid. --lag instead drags the stress
// grid up with a scripted touch gesture and prints, from screenshots taken
// during it (the compositor's frames, as a screencast), how far one node's
// ball trails the box it is drawn for.
import { spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import zlib from "node:zlib";

const args = process.argv.slice(2);
const flag = (name, d) => { const i = args.indexOf(name); return i < 0 ? d : args.splice(i, 2)[1]; };
const phone = args.includes("--phone") && args.splice(args.indexOf("--phone"), 1);
const lag = args.includes("--lag") && args.splice(args.indexOf("--lag"), 1);
const dotsFlag = args.includes("--dots") && args.splice(args.indexOf("--dots"), 1);
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
  if (m.method) onEvent(m);
});
let onEvent = () => {};
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
if (lag) await measureLag();
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
if (dotsFlag) dotJitter((await s("Runtime.evaluate", { returnByValue: true, expression: "window.dotTrace" })).result.value);
if (shot) {
  await s("Runtime.evaluate", { expression: `document.getElementById("stress")?.scrollIntoView()` });
  await new Promise((ok) => setTimeout(ok, 500));
  fs.writeFileSync(shot, Buffer.from((await s("Page.captureScreenshot")).data, "base64"));
}
done();

function done() {
  proc.kill();
  fs.rmSync(profile, { recursive: true, force: true, maxRetries: 5 });
  process.exit(0);
}

// With ?dots, the big spinner's and the map node's compaction dots per frame,
// in spinner units, scaled to the big one's pixels: the jitter is each dot's
// change in velocity from one frame to the next (the motion's own
// acceleration is far below a pixel), and the node against the big one in
// the same frame shows whether it is a scaled copy.
function dotJitter([big, node]) {
  const px = 260 / 2.9, q = (a, f) => a.slice().sort((x, y) => x - y)[Math.floor(f * (a.length - 1))] ?? 0;
  const jitter = (tr) => {
    const j = [];
    for (let f = 2; f < tr.length; f++) {
      const [a, b, c] = [tr[f - 2], tr[f - 1], tr[f]];
      if (a.length !== b.length || b.length !== c.length) continue;
      for (let k = 1; k < c.length; k += 2) j.push(px * Math.hypot(c[k] - 2 * b[k] + a[k], c[k + 1] - 2 * b[k + 1] + a[k + 1]));
    }
    const dots = tr.map((f) => (f.length - 1) / 2);
    return `${tr.length} frames, ${Math.min(...dots)}-${Math.max(...dots)} dots; jitter in big px: mean ${(j.reduce((x, y) => x + y, 0) / j.length).toFixed(3)}, p99 ${q(j, .99).toFixed(2)}, max ${q(j, 1).toFixed(2)}, over 1 px ${j.filter((v) => v > 1).length} of ${j.length}`;
  };
  console.log(`big:  ${jitter(big)}\nnode: ${jitter(node)}`);
  const at = new Map(big.map((f) => [f[0], f])), d = [];
  let unlike = 0;
  for (const f of node) {
    const b = at.get(f[0]);
    if (!b) continue;
    if (b.length !== f.length) { unlike++; continue; }
    for (let k = 1; k < f.length; k++) d.push(px * Math.abs(f[k] - b[k]));
  }
  console.log(`node against big, same frame, big px: mean ${(d.reduce((x, y) => x + y, 0) / d.length).toFixed(3)}, max ${q(d, 1).toFixed(2)}; frames with a different dot count ${unlike}`);
}

// The ball of a finished node (one green ball, still) against a magenta
// outline on its box: in each screenshot, the ball's centre less the box's,
// less the same at rest, is how far the ball trails.
async function measureLag() {
  await s("Runtime.evaluate", { expression: `(() => {
    document.getElementById("stress").scrollIntoView();
    const i = sessions.findIndex((ses, i) => ses.state === "done" && slots[i].closest(".cell")
      && slots[i].getBoundingClientRect().top > innerHeight * .7
      && slots[i].getBoundingClientRect().top < innerHeight * .85);
    slots[i].style.outline = "2px solid #f0f"; sessions[i].pips = 0;
  })()` });
  await new Promise((ok) => setTimeout(ok, 1000));
  const moved = [];
  const offset = (data) => {
    const img = png(Buffer.from(data, "base64"));
    let top = Infinity, bottom = -1, left = Infinity, right = -1;
    for (let y = 0; y < img.h; y++) for (let x = 0; x < img.w; x++) {
      const [r, g, b] = img.at(x, y);
      if (r > 220 && g < 60 && b > 220) { top = Math.min(top, y); bottom = y; left = Math.min(left, x); right = Math.max(right, x); }
    }
    if (bottom < 0) return null;
    let sum = 0, n = 0;
    for (let y = Math.max(0, top - 80); y < Math.min(img.h, bottom + 80); y++) for (let x = left; x <= right; x++) {
      const [r, g, b] = img.at(x, y);
      if (Math.abs(r - 79) + Math.abs(g - 138) + Math.abs(b - 85) < 40) { sum += y; n++; }
    }
    if (n) moved.push((top + bottom) / 2 / size.dpr);
    return n ? ((sum / n) - (top + bottom) / 2) / size.dpr : null;
  };
  await s("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 1 });
  const where = async () => (await s("Runtime.evaluate", { expression: "scrollY" })).result.value;
  const y0 = await where();
  const rest = offset((await s("Page.captureScreenshot")).data);
  if (rest === null) throw new Error("no marked ball on screen");
  // A finger dragging half the screen up over a second, a move every 16 ms.
  const touch = (type, y) => s("Input.dispatchTouchEvent", { type, touchPoints: y === undefined ? [] : [{ x: size.width / 2, y }] });
  const gesture = (async () => {
    const from = size.height * .8, steps = 60;
    await touch("touchStart", from);
    for (let k = 1; k <= steps; k++) {
      await new Promise((ok) => setTimeout(ok, 16));
      await touch("touchMove", from - size.height / 2 * k / steps);
    }
    await touch("touchEnd");
  })();
  const frames = [];
  onEvent = (m) => {
    if (m.method !== "Page.screencastFrame") return;
    frames.push(m.params.data);
    s("Page.screencastFrameAck", { sessionId: m.params.sessionId });
  };
  await s("Page.startScreencast", { format: "png" });
  await gesture;
  await s("Page.stopScreencast");
  console.log(`scrollY ${y0} -> ${await where()}`);
  const trail = frames.map(offset).filter((o) => o !== null).map((o) => o - rest);
  const abs = trail.map(Math.abs), mean = abs.reduce((a, b) => a + b, 0) / abs.length;
  console.log(`${query} ${phone ? "phone" : "desktop"} x${throttle} lag: ball behind its box, css px, over ${trail.length} of ${frames.length} screencast frames `
    + `during the drag (its box moving from ${moved[1]?.toFixed(0)} to ${moved.at(-1)?.toFixed(0)}): mean ${mean.toFixed(1)}, max ${Math.max(...abs).toFixed(1)}, `
    + `${abs.filter((a) => a > 1).length} frames off by more than 1 px`);
  done();
}

// An 8-bit RGB or RGBA PNG, as Chromium writes its screenshots.
function png(buf) {
  let w, h, ch, idat = [];
  for (let o = 8; o < buf.length;) {
    const len = buf.readUInt32BE(o), type = buf.toString("ascii", o + 4, o + 8), d = buf.subarray(o + 8, o + 8 + len);
    if (type === "IHDR") { w = d.readUInt32BE(0); h = d.readUInt32BE(4); ch = d[9] === 6 ? 4 : 3; }
    if (type === "IDAT") idat.push(d);
    o += 12 + len;
  }
  const raw = zlib.inflateSync(Buffer.concat(idat)), stride = w * ch, px = Buffer.alloc(h * stride);
  for (let y = 0; y < h; y++) {
    const f = raw[y * (stride + 1)], src = y * (stride + 1) + 1, row = y * stride;
    for (let x = 0; x < stride; x++) {
      const a = x >= ch ? px[row + x - ch] : 0, b = y ? px[row - stride + x] : 0, c = x >= ch && y ? px[row - stride + x - ch] : 0;
      const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
      const pred = [0, a, b, (a + b) >> 1, pa <= pb && pa <= pc ? a : pb <= pc ? b : c][f];
      px[row + x] = raw[src + x] + pred;
    }
  }
  return { w, h, at: (x, y) => px.subarray(y * stride + x * ch, y * stride + x * ch + 3) };
}
