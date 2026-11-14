#!/usr/bin/env node
// The themes' check: every theme's ground painted the way Max sees it, held to the design's bars.
//
//   node dev/theme-check.mjs [THEME...] [--out DIR]
//
// For each theme (default: all of them, from crates/face-web/src/page/theme-*.js), in a headless
// Chromium with shaders and 2D canvas on the CPU (SwiftShader: the slow case):
// - the lead's ink: the cover ink against every pixel of the ground behind the lead's box, at phone
//   and desktop size, for every identity slot: 7:1 or better for 95% of it, never under 4.5:1;
// - deterministic: the same identity at the same size painted twice gives the same pixels;
// - paint time: the script and the raster after it, on a phone (390x844 at 2x, the CPU slowed 4x,
//   as a mid-range phone), under PAINT_MS;
// - the map contract: a chart, and a map.hue outside 35-75 (orange, Max's on the map);
// - the map: the theme's area as a territory beside a neighbour wearing it too, with its chart, drawn
//   as the map draws it (territories.js): each state tone at a live agent's node keeping 90% of the
//   contrast it has on bare paper; the names' ink at 14:1 or better for 95% of each name's box on
//   the wash (the chart's ink under a name is covered by its own halo); the chart's ink near the
//   streams at most NEAR_INK of its weight away from them, as Max asked the background behind the
//   streams to be fainter; and little colour, as an old map has: the land's OKLCh chroma at most
//   MAP_CHROMA at its median and at its 99th percentile;
// - by eye: DIR/<theme>/slots.png, every slot's painting as a thumbnail in order, for telling
//   agents apart at a glance and the theme at thumbnail size; the phone and desktop covers of slot 0
//   and 1; and region.png, the map.
// Prints a row per render and the bars it missed, and exits 1 if any was missed.
// Chromium is $CHROME, or the newest in ~/.cache/ms-playwright. DIR defaults to theme-check.ignore/.
import { spawn } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const PAGE = path.join(ROOT, "crates/face-web/src/page");
// The phone's paint budget in the slow case, in ms.
const PAINT_MS = 500;
const SIZES = {
  desktop: { width: 1440, height: 900, dpr: 1, throttle: 1 },
  phone: { width: 390, height: 844, dpr: 2, throttle: 4, mobile: true },
};
// The state tones (the language's): never a theme's.
const TONES = { yours: "#d4621c", working: "#3b78d4", others: "#8c7fa6", done: "#3c9259", failed: "#c63f36", quiet: "#a39e95" };
const MAP_PAPER = "#f0eee6", NAME_INK = "#141413";
// The map's land's colour, at its median and its 99th percentile (OKLCh chroma): an old map's faded
// wash, under the state tones' colour so they stay the strongest colour where they stand.
const MAP_CHROMA = [0.015, 0.04];
// How much the chart's ink darkens the wash just beyond a stream's gap, as a share of how much it does
// away from the streams.
const NEAR_INK = 0.75;
const SEEDS = ["retry-budget", "harness-features", "art-lead", "nightly-reconcile", "docs-pass", "dependency-sweep", "tool-review", "power-bill"];

const args = process.argv.slice(2);
const outAt = args.indexOf("--out");
const OUT = path.resolve(outAt >= 0 ? args.splice(outAt, 2)[1] : path.join(ROOT, "theme-check.ignore"));
const all = fs.readdirSync(PAGE).map((f) => /^theme-(.+)\.js$/.exec(f)?.[1]).filter(Boolean);
const themes = args.length ? args : all;

// The page every render runs in: the theme's ground behind a lead, as a page's cover is drawn.
const VIEW = `<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<style>
html, body { margin: 0; background: var(--paper); }
.cover { position: relative; height: 100vh; overflow: hidden; }
canvas { position: absolute; inset: 0; width: 100%; height: 100%; display: block; }
.lead { position: relative; max-width: 640px; margin: 0 auto; padding: clamp(96px, 24vh, 230px) 28px 0; }
h1 { margin: 0 0 18px; font: var(--display-bold) 34px/1.16 var(--display); font-variation-settings: var(--display-var); color: var(--cover-ink); letter-spacing: -0.01em; text-wrap: balance; }
p { margin: 0; font: 17.5px/1.62 Georgia, serif; color: var(--cover-body); }
@media (max-width: 600px) { .lead { padding-left: 20px; padding-right: 20px; } h1 { font-size: 28px; } p { font-size: 17px; } }
.sheet { display: grid; grid-template-columns: repeat(4, 300px); gap: 6px; padding: 6px; background: #888; }
.sheet canvas { position: static; width: 300px; height: 200px; }
</style>
<div class="cover"><canvas></canvas><div class="lead"><h1>Should the art for every theme go to one standing agent?</h1>
<p>Your words about how a page looks go from its agent to the keeper of the design system, which hands them to the art.</p></div></div>
<script type="module">
import { load, wear, paintInto, settle, oklch } from "/page/theme.js";
import { render, territories } from "/page/territories.js";
const q = new URLSearchParams(location.search), theme = await load(q.get("theme"));
await wear(document.documentElement, theme);
window.__check = { theme, paintInto, territories, render, settle, oklch };
window.__ready = true;
</script>`;

const server = http.createServer((req, res) => {
  const url = new URL(req.url, "http://x");
  if (url.pathname === "/") return res.writeHead(200, { "content-type": "text/html" }).end(VIEW);
  const name = path.basename(url.pathname), file = path.join(PAGE, name);
  if (!url.pathname.startsWith("/page/") || !fs.existsSync(file)) return res.writeHead(404).end();
  const type = file.endsWith(".js") ? "text/javascript" : file.endsWith(".woff2") ? "font/woff2" : "application/octet-stream";
  res.writeHead(200, { "content-type": type }).end(fs.readFileSync(file));
});
await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
const BASE = `http://127.0.0.1:${server.address().port}/`;

function chromium() {
  if (process.env.CHROME) return process.env.CHROME;
  const cache = path.join(os.homedir(), ".cache/ms-playwright");
  // The headless shell if there is one: the lighter, and the one the check's numbers were set on.
  const newest = (re) => (fs.existsSync(cache) ? fs.readdirSync(cache) : []).filter((d) => re.test(d))
    .sort((a, b) => Number(b.split("-")[1]) - Number(a.split("-")[1]))[0];
  const shell = newest(/^chromium_headless_shell-\d+$/), full = newest(/^chromium-\d+$/);
  if (shell) return path.join(cache, shell, "chrome-headless-shell-linux64/chrome-headless-shell");
  if (full) return path.join(cache, full, "chrome-linux64/chrome");
  throw new Error(`no Chromium in ${cache}; set CHROME`);
}

// One Chromium on the CPU, driven over the DevTools protocol.
const profile = fs.mkdtempSync(path.join(os.tmpdir(), "theme-check-"));
const chrome = spawn(chromium(), ["--headless", "--remote-debugging-port=0", `--user-data-dir=${profile}`, "--no-first-run",
  "--password-store=basic", "--hide-scrollbars", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist",
  "--disable-accelerated-2d-canvas", "about:blank"], { stdio: ["ignore", "ignore", "pipe"] });
const wsUrl = await new Promise((ok, no) => {
  let err = "";
  chrome.stderr.on("data", (d) => { err += d; const m = /DevTools listening on (ws:\S+)/.exec(err); if (m) ok(m[1]); });
  chrome.on("exit", () => no(new Error(`Chromium exited: ${err.slice(-400)}`)));
});
const ws = new WebSocket(wsUrl);
await new Promise((ok) => ws.addEventListener("open", ok, { once: true }));
let nextId = 0;
const waiting = new Map(), thrown = new Map();
ws.addEventListener("message", (e) => {
  const m = JSON.parse(e.data);
  if (m.method === "Runtime.exceptionThrown") thrown.set(m.sessionId, m.params.exceptionDetails.exception?.description ?? m.params.exceptionDetails.text);
  if (m.id === undefined) return;
  const w = waiting.get(m.id);
  waiting.delete(m.id);
  if (m.error) w.no(new Error(`${w.method}: ${m.error.message}`)); else w.ok(m.result);
});
const send = (method, params = {}, sessionId) => new Promise((ok, no) => {
  const id = ++nextId;
  waiting.set(id, { ok, no, method });
  ws.send(JSON.stringify({ id, method, params, sessionId }));
});

// A fresh tab at a size, on the view for a theme; its evaluate and screenshot.
async function tab(theme, size) {
  const { targetId } = await send("Target.createTarget", { url: "about:blank" });
  const { sessionId } = await send("Target.attachToTarget", { targetId, flatten: true });
  const s = (method, params) => send(method, params, sessionId);
  await s("Emulation.setDeviceMetricsOverride", { width: size.width, height: size.height, deviceScaleFactor: size.dpr, mobile: !!size.mobile });
  await s("Page.enable");
  await s("Runtime.enable");
  await s("Page.navigate", { url: `${BASE}?theme=${theme}` });
  const evaluate = async (expression) => {
    const r = await s("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
    return r.result.value;
  };
  for (let i = 0; !(await evaluate("!!window.__ready").catch(() => false)); i++) {
    if (i > 300) throw new Error(`${theme}: the view never loaded: ${thrown.get(sessionId) ?? "no error"}`);
    await new Promise((ok) => setTimeout(ok, 100));
  }
  if (size.throttle > 1) await s("Emulation.setCPUThrottlingRate", { rate: size.throttle });
  const shot = async (file) => fs.writeFileSync(file, Buffer.from((await s("Page.captureScreenshot", { format: "png" })).data, "base64"));
  return { evaluate, shot, close: () => send("Target.closeTarget", { targetId }) };
}

// In the page: paint one identity behind the lead, time it, measure the lead's ink against the
// ground in its box, and paint it again to compare.
const COVER = (who) => `(() => {
  const { theme, paintInto, settle } = window.__check;
  const canvas = document.querySelector(".cover canvas"), lead = document.querySelector(".lead");
  const b = [...lead.children].reduce((r, el) => { const e = el.getBoundingClientRect(); return { l: Math.min(r.l, e.left), t: Math.min(r.t, e.top), r: Math.max(r.r, e.right), b: Math.max(r.b, e.bottom) }; }, { l: 1e9, t: 1e9, r: 0, b: 0 });
  const quiet = { x: b.l - 32, y: b.t - 28, w: b.r - b.l + 64, h: b.b - b.t + 56 };
  const who = ${JSON.stringify(who)};
  const script = paintInto(canvas, theme, who, quiet);
  const ms = script + settle(canvas);
  const dpr = canvas.width / canvas.clientWidth, ctx = canvas.getContext("2d");
  const img = ctx.getImageData(Math.round(b.l * dpr), Math.round(b.t * dpr), Math.round((b.r - b.l) * dpr), Math.round((b.b - b.t) * dpr)).data;
  const lum = (r, g, bl) => { const f = (c) => (c /= 255) <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(bl); };
  const n = parseInt(theme.palette.cover_ink.slice(1), 16), ink = lum(n >> 16, (n >> 8) & 255, n & 255);
  const cs = [];
  for (let i = 0; i < img.length; i += 16) { const L = lum(img[i], img[i + 1], img[i + 2]); cs.push((Math.max(L, ink) + 0.05) / (Math.min(L, ink) + 0.05)); }
  cs.sort((x, y) => x - y);
  const digest = (c) => { const d = c.getContext("2d").getImageData(0, 0, c.width, c.height).data; let h = 2166136261; for (let i = 0; i < d.length; i++) h = Math.imul(h ^ d[i], 16777619); return h >>> 0; };
  const first = digest(canvas), again = document.createElement("canvas");
  again.style.cssText = "position:absolute;left:0;top:0;width:" + canvas.clientWidth + "px;height:" + canvas.clientHeight + "px;visibility:hidden";
  document.body.append(again);
  paintInto(again, theme, who, quiet);
  const same = first === digest(again);
  again.remove();
  return { script, ms, min: cs[0], p5: cs[Math.floor(cs.length * 0.05)], same };
})()`;

// In the page: every slot's painting as a thumbnail, in order.
const SHEET = (seeds) => `(() => {
  const { theme, paintInto } = window.__check;
  document.querySelector(".cover").remove();
  const sheet = document.createElement("div"); sheet.className = "sheet"; document.body.append(sheet);
  theme.slots.forEach((_, slot) => { const c = document.createElement("canvas"); sheet.append(c); paintInto(c, theme, { slot, seed: ${JSON.stringify(seeds)}[slot % ${seeds.length}] }, null); });
  return theme.slots.length;
})()`;

// In the page: the theme's area on the map beside a neighbour wearing it too, each a few live agents
// on the now line with their history above them, drawn as the map draws it, with the state tones at
// the live agents' nodes and their names' boxes.
const REGION = `(async () => {
  const { theme, territories, render, oklch } = window.__check;
  document.querySelector(".cover").remove();
  const X = 0, Y = -1000, w = 1500, h = 1300, live = [], names = [], lines = [];
  const area = (id, cols) => {
    const marks = [], labels = [];
    for (const x of cols) {
      const chain = [];
      for (let y = 0; y > -900; y -= 120) { marks.push([x - 32, y - 32, x + 32, y + 32]); chain.push([x, y, 1.1]); }
      lines.push(chain);
      live.push({ x, y: 0 });
      const name = [x + 24, -9, x + 150, 9];
      labels.push(name); names.push(name);
    }
    lines.push(cols.map((x) => [x, -840, 1.1]));
    return { id, theme, seed: id, name: id, marks, labels };
  };
  const t0 = performance.now();
  const drawn = territories([area("harness-features", [200, 470, 740]), area("retry-budget", [1110, 1330])], lines, "${MAP_PAPER}");
  const t1 = performance.now(), rect = { x: X, y: Y, w, h }, full = render(drawn, rect, w, h), ms = [t1 - t0, performance.now() - t1];
  const ground = (pic) => { const c = new OffscreenCanvas(w, h), x = c.getContext("2d"); x.fillStyle = "${MAP_PAPER}"; x.fillRect(0, 0, w, h); x.drawImage(pic, 0, 0); return x.getImageData(0, 0, w, h).data; };
  const view = document.createElement("canvas"); view.width = w; view.height = h;
  const v = view.getContext("2d"); v.fillStyle = "${MAP_PAPER}"; v.fillRect(0, 0, w, h); v.drawImage(full, 0, 0);
  view.style.cssText = "display:block;width:" + w + "px;height:" + h + "px"; document.body.append(view);
  const all = v.getImageData(0, 0, w, h).data, wash = ground(render(drawn, rect, w, h, { washOnly: true }));
  const hex = (d, i) => "#" + [d[i], d[i + 1], d[i + 2]].map((x) => x.toString(16).padStart(2, "0")).join("");
  const lum = (c) => { const n = parseInt(c.slice(1), 16), f = (x) => (x /= 255) <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4; return 0.2126 * f(n >> 16) + 0.7152 * f((n >> 8) & 255) + 0.0722 * f(n & 255); };
  const contrast = (a, b) => { const A = lum(a), B = lum(b); return (Math.max(A, B) + 0.05) / (Math.min(A, B) + 0.05); };
  const pixels = (d, x0, y0, x1, y1) => { const out = []; for (let y = y0 - Y; y < y1 - Y; y++) for (let x = x0 - X; x < x1 - X; x++) out.push(hex(d, (y * w + x) * 4)); return out; };
  const R = 8, at = live.map((p) => { const cs = pixels(all, p.x - 3 * R, p.y - 3 * R, p.x + 3 * R, p.y + 3 * R).sort((a, b) => lum(a) - lum(b)); return cs[cs.length >> 1]; });
  // A name stands on the wash: the ink under it is covered by its own halo (map.css).
  const nameP5 = Math.min(...names.map((b) => { const cs = pixels(wash, ...b).map((c) => contrast("${NAME_INK}", c)).sort((a, b) => a - b); return cs[Math.floor(cs.length * 0.05)]; }));
  const chroma = [];
  for (let i = 0; i < all.length; i += 4 * 3) { const c = hex(all, i); if (c !== "${MAP_PAPER}") chroma.push(oklch(c)[1]); }
  chroma.sort((a, b) => a - b);
  // The chart's ink, as how much it darkens the wash, within 12 units beyond the streams' gap and
  // more than 60 units from any stream, in the land round the present row.
  const segs = lines.flatMap((P) => P.slice(1).map((q, i) => [P[i][0], P[i][1], q[0], q[1]]));
  const far = (x, y) => Math.min(...segs.map(([ax, ay, bx, by]) => { const vx = bx - ax, vy = by - ay, L = vx * vx + vy * vy, t = L ? Math.max(0, Math.min(1, ((x - ax) * vx + (y - ay) * vy) / L)) : 0; return Math.hypot(x - ax - t * vx, y - ay - t * vy); }));
  let near = [0, 0], away = [0, 0];
  for (let y = Y + 300; y < Y + h; y += 3) for (let x = X; x < X + w; x += 3) {
    const i = ((y - Y) * w + (x - X)) * 4, inked = Math.max(0, lum(hex(wash, i)) - lum(hex(all, i)));
    if (all[i + 3] === 0 || hex(wash, i) === "${MAP_PAPER}") continue;
    const d = far(x, y);
    if (d > 1.1 + 4 && d < 1.1 + 16) { near[0] += inked; near[1]++; } else if (d > 60) { away[0] += inked; away[1]++; }
  }
  return { ms, ops: drawn.areas.reduce((n, a) => n + a.ink.length + a.texts.length, 0), nameP5, unknown: drawn.unknown, chroma: [chroma[chroma.length >> 1] ?? 0, chroma[Math.floor(chroma.length * 0.99)] ?? 0],
    nearInk: near[1] && away[1] && away[0] ? (near[0] / near[1]) / (away[0] / away[1]) : 0,
    tones: Object.fromEntries(Object.entries(${JSON.stringify(TONES)}).map(([k, c]) => [k, { paper: contrast(c, "${MAP_PAPER}"), at: at.map((m) => contrast(c, m)) }])) };
})()`;

const missed = [];
const miss = (what) => { missed.push(what); return "  MISSED"; };
try {
  for (const name of themes) {
    const dir = path.join(OUT, name);
    fs.mkdirSync(dir, { recursive: true });
    console.log(`${name}: paint ms (script / settled), the lead ink's contrast in its box (min / 5th pct), painted twice`);
    const probe = await tab(name, SIZES.desktop);
    const slots = await probe.evaluate("window.__check.theme.slots.length");
    const { chart, hue } = await probe.evaluate("({ chart: typeof window.__check.theme.chart, hue: window.__check.theme.map?.hue })");
    await probe.close();
    if (chart !== "function") console.log(`  no chart(ctx, scene): the map would draw its territory as a bare wash${miss(`${name}: no chart`)}`);
    if (typeof hue !== "number" || (hue >= 35 && hue <= 75)) console.log(`  map.hue ${hue}: not an OKLCH hue outside 35-75, which is orange, Max's on the map${miss(`${name}: map.hue ${hue}`)}`);
    for (const [size, spec] of Object.entries(SIZES)) {
      for (let slot = 0; slot < slots; slot++) {
        const seed = SEEDS[slot % SEEDS.length], t = await tab(name, spec);
        const r = await t.evaluate(COVER({ slot, seed }));
        if (slot < 2) await t.shot(path.join(dir, `${size}-${slot}.png`));
        await t.close();
        let bars = "";
        if (r.p5 < 7 || r.min < 4.5) bars += miss(`${name} ${size} slot ${slot}: ink ${r.min.toFixed(1)} / ${r.p5.toFixed(1)}`);
        if (!r.same) bars += miss(`${name} ${size} slot ${slot}: painted twice, different pixels`);
        if (size === "phone" && r.ms > PAINT_MS) bars += miss(`${name} phone slot ${slot}: ${Math.round(r.ms)} ms, over ${PAINT_MS}`);
        console.log(`  ${size.padEnd(7)} slot ${slot} ${seed.padEnd(18)} ${String(Math.round(r.script)).padStart(4)} / ${String(Math.round(r.ms)).padStart(4)} ms   contrast ${r.min.toFixed(1).padStart(5)} / ${r.p5.toFixed(1).padStart(5)}   ${r.same ? "same" : "DIFFERENT"}${bars}`);
      }
    }
    const sheet = await tab(name, { width: 1236, height: 6 + Math.ceil(slots / 4) * 206, dpr: 1, throttle: 1 });
    await sheet.evaluate(SHEET(SEEDS));
    await sheet.shot(path.join(dir, "slots.png"));
    await sheet.close();
    const region = await tab(name, { width: 1500, height: 1300, dpr: 1, throttle: 1 });
    const m = await region.evaluate(REGION);
    await region.shot(path.join(dir, "region.png"));
    await region.close();
    let mapBars = "";
    if (m.nameP5 < 14) mapBars += miss(`${name} map: names' ink ${m.nameP5.toFixed(1)}:1 in 5% of a name's box`);
    if (!(m.nearInk <= NEAR_INK)) mapBars += miss(`${name} map: the ink near the streams is ${Math.round(m.nearInk * 100)}% of its weight away from them, over ${NEAR_INK * 100}%`);
    if (m.chroma[0] > MAP_CHROMA[0] || m.chroma[1] > MAP_CHROMA[1]) mapBars += miss(`${name} map: land chroma ${m.chroma.map((c) => c.toFixed(3)).join(" / ")} over ${MAP_CHROMA.join(" / ")}`);
    if (m.unknown.length) mapBars += miss(`${name} map: its chart used ${m.unknown.join(", ")}, which the map's context lacks`);
    console.log(`  map: worked out in ${Math.round(m.ms[0])} ms and drawn in ${Math.round(m.ms[1])} ms, ${m.ops} paths and words; ink near the streams ${Math.round(m.nearInk * 100)}% of away; names' ink ${m.nameP5.toFixed(1)}:1 (5th pct); land chroma ${m.chroma.map((c) => c.toFixed(3)).join(" / ")} (median / 99th)${mapBars}`);
    for (const [tone, t] of Object.entries(m.tones)) {
      const keep = Math.min(...t.at) / t.paper;
      const bars = keep < 0.9 ? miss(`${name} map: ${tone} keeps ${Math.round(keep * 100)}% of its contrast`) : "";
      console.log(`    ${tone.padEnd(8)} keeps ${Math.round(keep * 100)}% of its contrast at a live node (${Math.min(...t.at).toFixed(2)} / ${t.paper.toFixed(2)})${bars}`);
    }
    console.log(`  by eye: ${dir}/slots.png, region.png, {phone,desktop}-{0,1}.png`);
  }
} finally {
  const gone = new Promise((ok) => chrome.on("exit", ok));
  await send("Browser.close").catch(() => chrome.kill());
  await gone;
  ws.close();
  server.close();
  fs.rmSync(profile, { recursive: true, force: true });
}
console.log(missed.length ? `missed ${missed.length} bar(s):\n  ${missed.join("\n  ")}` : "every bar met");
process.exit(missed.length ? 1 : 0);
