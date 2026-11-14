#!/usr/bin/env node
// The themes' check: every theme's ground painted the way Max sees it, held to the design's bars.
//
//   node dev/theme-check.mjs [THEME...] [--out DIR]
//
// For each theme (default: all of them, from crates/face-web/src/page/theme-*.js), in a headless
// Chromium with shaders and 2D canvas on the CPU (SwiftShader: the slow case), painted for the chat
// window a page is read in, laid out as the page lays out its lead and text column:
// - the lead's ink: the cover ink against every pixel of the ground behind the lead's box, at phone
//   and desktop size, for every identity slot: 7:1 or better for 95% of it, never under 4.5:1;
// - the page: the painting stays still while the text scrolls over all of kit.textBox, the box the
//   text covers, so the share of that box under 4.5:1 for the ink body text is set in (the palette's
//   ink) is how much of the art each block's press must flatten. Over PAGE_UNDER of it misses: the
//   art is loud where the text runs, and pressed flat, the page reads as a wash. And with its press
//   (theme.js pagePresses) laid over it, the same box keeps the ink at 7:1 on every pixel: what keeps
//   the text easy to read, held by construction, and checked here;
// - deterministic: the same identity at the same size painted twice gives the same pixels;
// - paint time: the script and the raster after it, on a phone (its chat window at 2x, the CPU
//   slowed 4x, as a mid-range phone), under PAINT_MS;
// - state marks: the theme's map region with live agents' pools of colour, each state tone at a
//   live agent's node keeping 90% of the contrast it has on bare paper, and the region round it
//   under half the tone's colour (OKLCh chroma, 95th percentile);
// - by eye: DIR/<theme>/slots.png, every slot's painting as a thumbnail in order, for telling
//   agents apart at a glance and the theme at thumbnail size; the phone and desktop windows of slot
//   0 and 1; and region.png, the map region.
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
// The chat window's scroll area, which a page's painting fills: the card on a 1920x1080 desktop, and
// a 390x844 phone's screen less the card's header and reply box.
const SIZES = {
  desktop: { width: 878, height: 850, dpr: 1, throttle: 1 },
  phone: { width: 376, height: 634, dpr: 2, throttle: 4, mobile: true },
};
// The most of the text's box a theme's painting may leave under 4.5:1 for the page's body ink, bare:
// the five themes' worst slot is 11.0% (Tussock's land at the foot of a desktop window, which
// reads well pressed), and a margin for a new seed. Not a few percent: the press keeps the text at
// 7:1 whatever lies under it, so this bar only stops a painting so loud under the text that its
// press would leave the page a wash.
const PAGE_UNDER = 0.2;
// The state tones (the language's): never a theme's.
const TONES = { yours: "#d4621c", working: "#3b78d4", others: "#8c7fa6", done: "#3c9259", failed: "#c63f36", quiet: "#a39e95" };
const MAP_PAPER = "#f0eee6";
const SEEDS = ["retry-budget", "harness-features", "art-lead", "nightly-reconcile", "docs-pass", "dependency-sweep", "tool-review", "power-bill"];

const args = process.argv.slice(2);
const outAt = args.indexOf("--out");
const OUT = path.resolve(outAt >= 0 ? args.splice(outAt, 2)[1] : path.join(ROOT, "theme-check.ignore"));
const all = fs.readdirSync(PAGE).map((f) => /^theme-(.+)\.js$/.exec(f)?.[1]).filter(Boolean);
const themes = args.length ? args : all;

// The page every render runs in: the theme's ground filling the window behind a lead, laid out as a
// page's sheet lays out its lead and text column (sheet.css).
const VIEW = `<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<style>
html, body { margin: 0; background: var(--paper); font-size: 16px; }
.cover { position: relative; height: 100vh; overflow: hidden; }
canvas { position: absolute; inset: 0; width: 100%; height: 100%; display: block; }
.page { position: relative; max-width: 44rem; margin: 0 auto; padding: 0 1.6em; box-sizing: border-box; font: 16.5px/1.6 Georgia, serif; }
.lead { padding-top: 2.8em; min-height: 30vh; box-sizing: border-box; display: flex; flex-direction: column; justify-content: flex-end; }
h1 { margin: 0 0 .4em; font: var(--display-bold) 2em/1.15 var(--display); font-variation-settings: var(--display-var); color: var(--cover-ink); text-wrap: balance; }
p { margin: 0; color: var(--cover-body); }
@media (max-width: 760px) { .page { padding: 0 1em; font-size: 16px; } h1 { font-size: 1.7em; } .lead { min-height: 24vh; } }
.sheet { display: grid; grid-template-columns: repeat(4, 300px); gap: 6px; padding: 6px; background: #888; }
.sheet canvas { position: static; width: 300px; height: 200px; }
</style>
<div class="cover"><canvas></canvas><div class="page"><div class="lead"><h1>Should the art for every theme go to one standing agent?</h1>
<p>Your words about how a page looks go from its agent to the keeper of the design system, which hands them to the art.</p></div></div></div>
<script type="module">
import { load, wear, paintInto, paintRegion, settle, oklch, quietOf, textBox, pagePresses, meanColour } from "/page/theme.js";
const q = new URLSearchParams(location.search), theme = await load(q.get("theme"));
await wear(document.documentElement, theme);
window.__check = { theme, paintInto, paintRegion, settle, oklch, quietOf, textBox, pagePresses, meanColour };
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

// In the page: paint one identity for the window, time it, measure the lead's ink against the
// ground in its box and the body ink over the box the text covers, bare and pressed, and paint it
// again to compare.
const COVER = (who) => `(() => {
  const { theme, paintInto, settle, quietOf, textBox, pagePresses, meanColour } = window.__check;
  const p = theme.palette, canvas = document.querySelector(".cover canvas"), page = document.querySelector(".page"), lead = document.querySelector(".lead");
  const b = [...lead.children].reduce((r, el) => { const e = el.getBoundingClientRect(); return { l: Math.min(r.l, e.left), t: Math.min(r.t, e.top), r: Math.max(r.r, e.right), b: Math.max(r.b, e.bottom) }; }, { l: 1e9, t: 1e9, r: 0, b: 0 });
  const cs = getComputedStyle(page), pl = parseFloat(cs.paddingLeft), pr = parseFloat(cs.paddingRight);
  const column = { x: page.getBoundingClientRect().left + pl, w: page.clientWidth - pl - pr };
  const w = innerWidth, h = innerHeight, quiet = quietOf({ w, h }, column);
  const who = ${JSON.stringify(who)};
  const script = paintInto(canvas, theme, who, quiet, null, column);
  const ms = script + settle(canvas);
  const dpr = canvas.width / canvas.clientWidth, ctx = canvas.getContext("2d");
  const lin = (c) => (c /= 255) <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  const lum = (r, g, bl) => 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(bl);
  const hex = (c) => { const n = parseInt(c.slice(1), 16); return [n >> 16, (n >> 8) & 255, n & 255]; };
  const ratio = (L, M) => (Math.max(L, M) + 0.05) / (Math.min(L, M) + 0.05);
  const pixels = (x, y, bw, bh) => ctx.getImageData(Math.round(x * dpr), Math.round(y * dpr), Math.round(bw * dpr), Math.round(bh * dpr)).data;
  const ink = lum(...hex(p.cover_ink)), img = pixels(b.l, b.t, b.r - b.l, b.b - b.t), lead5 = [];
  for (let i = 0; i < img.length; i += 16) lead5.push(ratio(lum(img[i], img[i + 1], img[i + 2]), ink));
  lead5.sort((x, y) => x - y);
  // Where the text runs: the column, down the box the art keeps open.
  const tb = textBox({ w, h, quiet, column }), text = pixels(column.x, tb.y, column.w, tb.h), body = lum(...hex(p.ink));
  const tint = meanColour(canvas, { x: column.x * dpr, y: 0, w: column.w * dpr, h: canvas.height });
  const pr2 = pagePresses(p, tint).body, pc = hex(pr2.colour), pick = pr2.blend === "lighten" ? Math.max : Math.min;
  let under = 0, n = 0, pressed = Infinity;
  for (let i = 0; i < text.length; i += 16, n++) {
    if (ratio(lum(text[i], text[i + 1], text[i + 2]), body) < 4.5) under++;
    pressed = Math.min(pressed, ratio(lum(pick(text[i], pc[0]), pick(text[i + 1], pc[1]), pick(text[i + 2], pc[2])), body));
  }
  const digest = (c) => { const d = c.getContext("2d").getImageData(0, 0, c.width, c.height).data; let h = 2166136261; for (let i = 0; i < d.length; i++) h = Math.imul(h ^ d[i], 16777619); return h >>> 0; };
  const first = digest(canvas), again = document.createElement("canvas");
  again.style.cssText = "position:absolute;left:0;top:0;width:" + canvas.clientWidth + "px;height:" + canvas.clientHeight + "px;visibility:hidden";
  document.body.append(again);
  paintInto(again, theme, who, quiet, null, column);
  const same = first === digest(again);
  again.remove();
  return { script, ms, min: lead5[0], p5: lead5[Math.floor(lead5.length * 0.05)], under: under / n, pressed, same };
})()`;

// In the page: every slot's painting as a thumbnail, in order.
const SHEET = (seeds) => `(() => {
  const { theme, paintInto } = window.__check;
  document.querySelector(".cover").remove();
  const sheet = document.createElement("div"); sheet.className = "sheet"; document.body.append(sheet);
  theme.slots.forEach((_, slot) => { const c = document.createElement("canvas"); sheet.append(c); paintInto(c, theme, { slot, seed: ${JSON.stringify(seeds)}[slot % ${seeds.length}] }, null); });
  return theme.slots.length;
})()`;

// In the page: the theme's map region with a few live agents, and each state tone at their nodes.
const REGION = `(() => {
  const { theme, paintRegion, oklch } = window.__check;
  document.querySelector(".cover").remove();
  const w = 1200, h = 900, now = 520, pools = [0, 1, 2, 3].map((slot) => ({ x: 180 + slot * 280, y: now, r: 130, slot }));
  const art = paintRegion(theme, { slot: 0, seed: "harness-features" }, { w, h, now, scale: 1, paper: "${MAP_PAPER}", pools });
  const view = document.createElement("canvas"); view.width = w; view.height = h;
  const v = view.getContext("2d"); v.fillStyle = "${MAP_PAPER}"; v.fillRect(0, 0, w, h); v.drawImage(art, 0, 0, w, h);
  view.style.cssText = "display:block;width:" + w + "px;height:" + h + "px"; document.body.append(view);
  const hex = (r, g, b) => "#" + [r, g, b].map((x) => x.toString(16).padStart(2, "0")).join("");
  const lum = (c) => { const n = parseInt(c.slice(1), 16), f = (x) => (x /= 255) <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4; return 0.2126 * f(n >> 16) + 0.7152 * f((n >> 8) & 255) + 0.0722 * f(n & 255); };
  const contrast = (a, b) => { const A = lum(a), B = lum(b); return (Math.max(A, B) + 0.05) / (Math.min(A, B) + 0.05); };
  const R = 8, out = [];
  for (const p of pools) {
    const d = v.getImageData(p.x - 3 * R, p.y - 3 * R, 6 * R, 6 * R).data, cols = [], chroma = [];
    for (let i = 0; i < d.length; i += 4) { const c = hex(d[i], d[i + 1], d[i + 2]); cols.push([lum(c), c]); chroma.push(oklch(c)[1]); }
    cols.sort((a, b) => a[0] - b[0]); chroma.sort((a, b) => a - b);
    out.push({ slot: p.slot, median: cols[cols.length >> 1][1], chroma: chroma[Math.floor(chroma.length * 0.95)] });
  }
  return { out, tones: Object.fromEntries(Object.entries(${JSON.stringify(TONES)}).map(([k, c]) => [k, { paper: contrast(c, "${MAP_PAPER}"), chroma: oklch(c)[1], at: out.map((o) => contrast(c, o.median)) }])) };
})()`;

const missed = [];
const miss = (what) => { missed.push(what); return "  MISSED"; };
try {
  for (const name of themes) {
    const dir = path.join(OUT, name);
    fs.mkdirSync(dir, { recursive: true });
    console.log(`${name}: paint ms (script / settled), the lead ink's contrast in its box (min / 5th pct), the text's box under 4.5:1 bare / its worst pressed, painted twice`);
    const probe = await tab(name, SIZES.desktop);
    const slots = await probe.evaluate("window.__check.theme.slots.length");
    await probe.close();
    for (const [size, spec] of Object.entries(SIZES)) {
      for (let slot = 0; slot < slots; slot++) {
        const seed = SEEDS[slot % SEEDS.length], t = await tab(name, spec);
        const r = await t.evaluate(COVER({ slot, seed }));
        if (slot < 2) await t.shot(path.join(dir, `${size}-${slot}.png`));
        await t.close();
        let bars = "";
        if (r.p5 < 7 || r.min < 4.5) bars += miss(`${name} ${size} slot ${slot}: ink ${r.min.toFixed(1)} / ${r.p5.toFixed(1)}`);
        if (r.under > PAGE_UNDER) bars += miss(`${name} ${size} slot ${slot}: ${(r.under * 100).toFixed(1)}% of the text's box under 4.5:1, over ${PAGE_UNDER * 100}%`);
        if (r.pressed < 7) bars += miss(`${name} ${size} slot ${slot}: pressed, the body ink falls to ${r.pressed.toFixed(2)}:1`);
        if (!r.same) bars += miss(`${name} ${size} slot ${slot}: painted twice, different pixels`);
        if (size === "phone" && r.ms > PAINT_MS) bars += miss(`${name} phone slot ${slot}: ${Math.round(r.ms)} ms, over ${PAINT_MS}`);
        console.log(`  ${size.padEnd(7)} slot ${slot} ${seed.padEnd(18)} ${String(Math.round(r.script)).padStart(4)} / ${String(Math.round(r.ms)).padStart(4)} ms   contrast ${r.min.toFixed(1).padStart(5)} / ${r.p5.toFixed(1).padStart(5)}   text ${(r.under * 100).toFixed(1).padStart(5)}% / ${r.pressed.toFixed(1).padStart(4)}   ${r.same ? "same" : "DIFFERENT"}${bars}`);
      }
    }
    const sheet = await tab(name, { width: 1236, height: 6 + Math.ceil(slots / 4) * 206, dpr: 1, throttle: 1 });
    await sheet.evaluate(SHEET(SEEDS));
    await sheet.shot(path.join(dir, "slots.png"));
    await sheet.close();
    const region = await tab(name, { width: 1200, height: 900, dpr: 1, throttle: 1 });
    const m = await region.evaluate(REGION);
    await region.shot(path.join(dir, "region.png"));
    await region.close();
    console.log(`  state marks at live agents' nodes on its map region: contrast there / on paper, worst of ${m.out.length}; region chroma p95 there ${m.out.map((o) => o.chroma.toFixed(3)).join(" ")}`);
    const worst = Math.max(...m.out.map((o) => o.chroma));
    for (const [tone, t] of Object.entries(m.tones)) {
      const keep = Math.min(...t.at) / t.paper;
      let bars = keep < 0.9 ? miss(`${name} map: ${tone} keeps ${Math.round(keep * 100)}% of its contrast`) : "";
      // A grey tone's colour is not what tells it: only a coloured one is held to the chroma bar.
      if (t.chroma > 0.05 && worst >= t.chroma / 2) bars += miss(`${name} map: region chroma ${worst.toFixed(3)} not under half ${tone}'s ${t.chroma.toFixed(3)}`);
      console.log(`    ${tone.padEnd(8)} ${Math.round(keep * 100)}% (${Math.min(...t.at).toFixed(2)} / ${t.paper.toFixed(2)}), chroma ${t.chroma.toFixed(3)}${bars}`);
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
