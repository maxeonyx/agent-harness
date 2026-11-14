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
// - the art under the text: the painting stays still while the text scrolls over all of
//   kit.textBox, the box the text covers, and each block's veil flattens there whatever would break
//   its ink's bar. Over PAGE_CHANGED of that box flattened misses: the art is loud where the text
//   runs, and the reader sees a pale band slide over it as he scrolls;
// - the page as drawn: a sample page (a lead, an end with its uses, facts with a table, a plan with
//   every state's mark, an ask) laid out by sheet.css on the painting, with the veil theme.js
//   `veilStyle` sets, scrolled through the window; with its ink hidden, every line of text against
//   every pixel behind it: titles and body at 7:1, quiet text at 5:1, links at 4.5:1, marks at 3:1;
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
// The most of the text's box the veil may flatten: what a reader sees change under the text as it
// scrolls over the painting.
const PAGE_CHANGED = 0.05;
// Each kind of text on a painting, by the custom property it is set in, and the contrast it keeps.
const BARS = { "--cover-ink": 7, "--ink": 7, "--body": 7, "--muted": 5, "--accent": 4.5, "--fig-1": 3, "--fig-2": 3, "--fig-3": 3, "--fig-4": 3, "--yours": 3 };
// The state tones (the language's): never a theme's.
const TONES = { yours: "#d4621c", working: "#3b78d4", others: "#8c7fa6", done: "#3c9259", failed: "#c63f36", quiet: "#a39e95" };
const MAP_PAPER = "#f0eee6";
const SEEDS = ["retry-budget", "harness-features", "art-lead", "nightly-reconcile", "docs-pass", "dependency-sweep", "tool-review", "power-bill"];

const args = process.argv.slice(2);
const outAt = args.indexOf("--out");
const OUT = path.resolve(outAt >= 0 ? args.splice(outAt, 2)[1] : path.join(ROOT, "theme-check.ignore"));
const all = fs.readdirSync(PAGE).map((f) => /^theme-(.+)\.js$/.exec(f)?.[1]).filter(Boolean);
const themes = args.length ? args : all;

// The page every render runs in: a page as the chat window shows it, sheet.css laying out a sample
// page over the theme's ground, in a scroll the window's size.
const P = (s) => `<p>${s}</p>`;
const STEP = (state, mark, title, now, body = "") => `<li class="step ${state}"><div class="head"><span class="mark">${mark}</span><span class="title">${title}</span><span class="now">${now}</span></div>${body}</li>`;
const USE = (state, mark, what, stands) => `<li class="use ${state}"><span>${what}</span><span class="stands"><span class="mark">${mark}</span><span>${stands}</span></span></li>`;
const SAMPLE = `<header class="lead new"><h1>A cold build takes 6 minutes; a shared cache makes it 52 s</h1></header>
<section class="end"><p class="goal">Every clone on the VM builds warm, and CI reads the same cache.</p><ul class="uses">
${USE("done", "✓", "A new clone builds in under a minute", "one clone on it: 52 s")}${USE("working", "◐", "CI builds read the cache", "half the jobs")}${USE("failed", "✕", "Release builds stay clean", "one cached by mistake")}</ul></section>
<section class="now">${P("Every clone's cargo points <code>RUSTC_WRAPPER</code> at one sccache, so a new clone builds warm. The cache sits on the VM's disk, 4.1 GB after a week, and <a href=\"#\">its notes</a> say how to clear it.")}
<table><tr><th>Build</th><th class="numeric">Without cache</th><th class="numeric">With cache</th></tr><tr><td>harness-brain</td><td class="numeric">6 min 12 s</td><td class="numeric">52 s</td></tr><tr><td>face-tab (wasm)</td><td class="numeric">1 min 40 s</td><td class="numeric">18 s</td></tr><tr><td>theme-check</td><td class="numeric">2 min 3 s</td><td class="numeric">31 s</td></tr></table>
${P("A bigger VM halves the time but costs every month, and every clone still pays it once.")}<blockquote>Measured on the VM at 14:02, three cold builds each.</blockquote></section>
<section class="plan"><ol class="steps">${STEP("done", "✓", "A cold build's time is known", "6 min 12 s, the slowest of three")}
${STEP("working", "◐", "The VM's clones share one build cache", "one clone on it", P("Each clone's config points at the one cache; the rest follow as they are next opened."))}
${STEP("yours", "●", "CI builds read the same cache", "CI builds cold: 6 min", `<div class="ask"><p class="question">Where should CI keep its build cache?</p><p class="now">GitHub's Actions cache holds 10 GB a repo; an S3 bucket costs a little every month.</p><p class="otherwise">Until you answer, I finish the VM side and the docs.</p></div>`)}
${STEP("failed", "✕", "Release builds skip the cache", "one release build cached")}${STEP("", "○", "AGENTS.md says how to build", "AGENTS.md and the README each say part")}</ol></section>
<section class="why">${P("Cold builds are most of an agent's first ten minutes in a new clone, and agents make a clone for every task.")}${P("One cache on the VM costs nothing more; CI's cache is the only choice that costs money, so it is yours.")}${P("Release builds stay out of the cache so what ships is built from nothing.")}</section>`;
const VIEW = `<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<link rel="stylesheet" href="/page/sheet.css">
<style>
html, body { margin: 0; background: var(--paper); font-size: 16px; --serif: Georgia, serif; --sans: system-ui, sans-serif; --mono: monospace; }
.scroll { position: relative; height: 100vh; overflow: hidden auto; }
.thumbs { display: grid; grid-template-columns: repeat(4, 300px); gap: 6px; padding: 6px; background: #888; }
.thumbs canvas { display: block; width: 300px; height: 200px; }
</style>
<div class="scroll"><div class="sheet"><div class="window"><canvas></canvas></div>${SAMPLE}</div></div>
<script type="module">
import { load, wear, paintInto, paintRegion, settle, oklch, quietOf, textBox, veilStyle, paintedInks, beyond, pageVeil, meanColour } from "/page/theme.js";
const q = new URLSearchParams(location.search), theme = await load(q.get("theme"));
await wear(document.documentElement, theme);
const sheet = document.querySelector(".sheet");
await wear(sheet, theme);
for (const [k, v] of Object.entries(paintedInks(theme.palette))) sheet.style.setProperty(k, v);
sheet.style.setProperty("--win-w", innerWidth + "px");
sheet.style.setProperty("--win-h", innerHeight + "px");
// Every line of text on the page in the window now: its box, its colour and the bar it keeps.
const rgbOf = (c) => c.match(/[\\d.]+/g).slice(0, 3).map(Number);
window.__lines = (bars) => {
  const probe = document.createElement("i"); document.body.append(probe);
  const kinds = Object.entries(bars).map(([v, k]) => { probe.style.color = getComputedStyle(sheet).getPropertyValue(v); return [getComputedStyle(probe).color, v, k]; });
  probe.remove();
  const out = [], walk = document.createTreeWalker(sheet, NodeFilter.SHOW_TEXT);
  while (walk.nextNode()) {
    const n = walk.currentNode; if (!n.textContent.trim()) continue;
    const colour = getComputedStyle(n.parentElement).color, kind = kinds.find(([c]) => c === colour) ?? [colour, "unknown " + colour, 7];
    const r = document.createRange(); r.selectNodeContents(n);
    for (const b of r.getClientRects()) {
      const t = Math.max(0, b.top), bt = Math.min(innerHeight, b.bottom);
      if (b.width >= 2 && bt - t >= 2) out.push({ x: b.left, y: t, w: b.width, h: bt - t, rgb: rgbOf(colour), kind: kind[1], bar: kind[2], text: n.textContent.trim().slice(0, 40) });
    }
  }
  return out;
};
// A screenshot of the window, with the ink hidden, against each line's colour: each kind's worst.
window.__measure = async (png, lines) => {
  const img = new Image(); img.src = "data:image/png;base64," + png; await img.decode();
  const c = document.createElement("canvas"); c.width = img.width; c.height = img.height;
  const x = c.getContext("2d", { willReadFrequently: true }); x.drawImage(img, 0, 0);
  const d = img.width / innerWidth, lin = (v) => (v /= 255) <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  const lum = (r, g, b) => 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
  return lines.map((l) => {
    const px = x.getImageData(Math.round(l.x * d), Math.round(l.y * d), Math.max(1, Math.round(l.w * d)), Math.max(1, Math.round(l.h * d))).data, L = lum(...l.rgb);
    let min = Infinity;
    for (let i = 0; i < px.length; i += 4) { const M = lum(px[i], px[i + 1], px[i + 2]); min = Math.min(min, (Math.max(L, M) + 0.05) / (Math.min(L, M) + 0.05)); }
    return { ...l, min };
  });
};
window.__check = { theme, sheet, paintInto, paintRegion, settle, oklch, quietOf, textBox, veilStyle, beyond, pageVeil, meanColour };
window.__ready = true;
</script>`;

const server = http.createServer((req, res) => {
  const url = new URL(req.url, "http://x");
  if (url.pathname === "/") return res.writeHead(200, { "content-type": "text/html" }).end(VIEW);
  const name = path.basename(url.pathname), file = path.join(PAGE, name);
  if (!url.pathname.startsWith("/page/") || !fs.existsSync(file)) return res.writeHead(404).end();
  const type = file.endsWith(".js") ? "text/javascript" : file.endsWith(".css") ? "text/css" : file.endsWith(".woff2") ? "font/woff2" : "application/octet-stream";
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
  const capture = () => s("Page.captureScreenshot", { format: "png" });
  const shot = async (file) => fs.writeFileSync(file, Buffer.from((await capture()).data, "base64"));
  return { evaluate, capture, shot, close: () => send("Target.closeTarget", { targetId }) };
}

// In the page: paint one identity for the window, time it, measure the lead's ink against the
// ground in its box and what the veil flattens in the box the text covers, paint it again to
// compare, then lay the veil over it and hide the page's ink, ready to be measured as drawn.
const COVER = (who, bars) => `(() => {
  const { theme, sheet, paintInto, settle, quietOf, textBox, veilStyle, beyond, pageVeil, meanColour } = window.__check;
  const p = theme.palette, canvas = sheet.querySelector(".window > canvas"), h1 = sheet.querySelector(".lead h1").getBoundingClientRect();
  const b = { l: h1.left, t: h1.top, r: h1.right, b: h1.bottom };
  const cs = getComputedStyle(sheet), pl = parseFloat(cs.paddingLeft), pr = parseFloat(cs.paddingRight);
  const column = { x: sheet.getBoundingClientRect().left + pl, w: sheet.clientWidth - pl - pr };
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
  // Where the text runs: the column, down the box the art keeps open; how much of it the veil flattens.
  const tb = textBox({ w, h, quiet, column });
  const veil = pageVeil(p, meanColour(canvas, { x: column.x * dpr, y: 0, w: column.w * dpr, h: canvas.height }));
  const rows = beyond(canvas, veil.colour, veil.blend, { rows: 64, cols: 64, box: { x: column.x * dpr, y: tb.y * dpr, w: column.w * dpr, h: tb.h * dpr } });
  const changed = rows.reduce((s, r) => s + r, 0) / rows.length;
  const digest = (c) => { const d = c.getContext("2d").getImageData(0, 0, c.width, c.height).data; let h = 2166136261; for (let i = 0; i < d.length; i++) h = Math.imul(h ^ d[i], 16777619); return h >>> 0; };
  const first = digest(canvas), again = document.createElement("canvas");
  again.style.cssText = "position:absolute;left:0;top:0;width:" + canvas.clientWidth + "px;height:" + canvas.clientHeight + "px;visibility:hidden";
  document.body.append(again);
  paintInto(again, theme, who, quiet, null, column);
  const same = first === digest(again);
  again.remove();
  const yours = getComputedStyle(document.documentElement).getPropertyValue("--yours").trim() || "#d4691c";
  const style = veilStyle({ palette: p, canvas, dpr, column, yours, name: "soft" });
  for (const [k, v] of Object.entries(style.vars)) sheet.style.setProperty(k, v);
  const frames = document.createElement("style"); frames.textContent = style.frames; document.head.append(frames);
  const lines = window.__lines(${JSON.stringify(bars)});
  const hide = document.createElement("style"); hide.className = "hide-ink";
  hide.textContent = ".sheet, .sheet * { color: transparent !important; -webkit-text-fill-color: transparent !important; text-decoration-color: transparent !important; border-color: transparent !important }";
  document.head.append(hide);
  hide.disabled = true;
  const s = document.querySelector(".scroll");
  return { script, ms, min: lead5[0], p5: lead5[Math.floor(lead5.length * 0.05)], changed, same, scroll: s.scrollHeight - s.clientHeight, lines: lines.length };
})()`;
// In the page: scrolled to y, every line of text in the window, once the soft veils have caught up.
const LINES = (y, bars) => `(async () => {
  document.querySelector(".hide-ink").disabled = true;
  document.querySelector(".scroll").scrollTop = ${y};
  await new Promise((ok) => requestAnimationFrame(() => requestAnimationFrame(ok)));
  const lines = window.__lines(${JSON.stringify(bars)});
  document.querySelector(".hide-ink").disabled = false;
  await new Promise((ok) => requestAnimationFrame(() => requestAnimationFrame(ok)));
  return lines;
})()`;

// In the page: every slot's painting as a thumbnail, in order.
const SHEET = (seeds) => `(() => {
  const { theme, paintInto } = window.__check;
  document.querySelector(".scroll").remove();
  const thumbs = document.createElement("div"); thumbs.className = "thumbs"; document.body.append(thumbs);
  theme.slots.forEach((_, slot) => { const c = document.createElement("canvas"); thumbs.append(c); paintInto(c, theme, { slot, seed: ${JSON.stringify(seeds)}[slot % ${seeds.length}] }, null); });
  return theme.slots.length;
})()`;

// In the page: the theme's map region with a few live agents, and each state tone at their nodes.
const REGION = `(() => {
  const { theme, paintRegion, oklch } = window.__check;
  document.querySelector(".scroll").remove();
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
    console.log(`${name}: paint ms (script / settled), the lead ink's contrast in its box (min / 5th pct), the text's box the veil flattens, the page as drawn (worst line: titles and body / quiet / links / marks), painted twice`);
    const probe = await tab(name, SIZES.desktop);
    const slots = await probe.evaluate("window.__check.theme.slots.length");
    await probe.close();
    for (const [size, spec] of Object.entries(SIZES)) {
      for (let slot = 0; slot < slots; slot++) {
        const seed = SEEDS[slot % SEEDS.length], t = await tab(name, spec);
        const r = await t.evaluate(COVER({ slot, seed }, BARS));
        if (slot < 2) await t.shot(path.join(dir, `${size}-${slot}.png`));
        // The page scrolled through the window, a little over half a window at a time.
        const worst = {};
        for (let y = 0; ; y = Math.min(r.scroll, y + Math.round(spec.height * 0.55))) {
          const lines = await t.evaluate(LINES(y, BARS));
          const png = (await t.capture()).data;
          for (const l of await t.evaluate(`window.__measure(${JSON.stringify(png)}, ${JSON.stringify(lines)})`)) {
            const k = l.bar === 7 ? "ink" : l.bar === 5 ? "quiet" : l.bar === 4.5 ? "links" : "marks";
            if (!worst[k] || l.min < worst[k].min) worst[k] = { ...l, y };
            if (l.kind.startsWith("unknown")) worst.unknown = l;
          }
          if (y >= r.scroll) break;
        }
        await t.close();
        let bars = "";
        if (r.p5 < 7 || r.min < 4.5) bars += miss(`${name} ${size} slot ${slot}: ink ${r.min.toFixed(1)} / ${r.p5.toFixed(1)}`);
        if (r.changed > PAGE_CHANGED) bars += miss(`${name} ${size} slot ${slot}: the veil flattens ${(r.changed * 100).toFixed(1)}% of the text's box, over ${PAGE_CHANGED * 100}%`);
        for (const [k, l] of Object.entries(worst)) if (l.min < l.bar) bars += miss(`${name} ${size} slot ${slot}: ${k} "${l.text}" (${l.kind}) at ${l.min.toFixed(2)}:1 scrolled ${l.y}px, under ${l.bar}:1`);
        if (!r.same) bars += miss(`${name} ${size} slot ${slot}: painted twice, different pixels`);
        if (size === "phone" && r.ms > PAINT_MS) bars += miss(`${name} phone slot ${slot}: ${Math.round(r.ms)} ms, over ${PAINT_MS}`);
        const page = ["ink", "quiet", "links", "marks"].map((k) => (worst[k] ? worst[k].min.toFixed(1) : "-").padStart(5)).join(" /");
        console.log(`  ${size.padEnd(7)} slot ${slot} ${seed.padEnd(18)} ${String(Math.round(r.script)).padStart(4)} / ${String(Math.round(r.ms)).padStart(4)} ms   contrast ${r.min.toFixed(1).padStart(5)} / ${r.p5.toFixed(1).padStart(5)}   flattened ${(r.changed * 100).toFixed(1).padStart(5)}%   page ${page}   ${r.same ? "same" : "DIFFERENT"}${bars}`);
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
