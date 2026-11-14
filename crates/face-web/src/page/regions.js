import { $, fail } from "./base.js";
import { LIFE } from "./layout.js";
import { centre, CHIP_FONT, graph, label, LABEL_FONT, positions, routes, shown } from "./map.js";

// ─── Each area as a territory on the map, drawn as its theme's chart ──────
// An area is a chat Max started and everything under it, and every task in
// it wears its theme (the row's `look`). On the map each area is a
// territory: the land round its nodes, names and streams, bordered where it
// meets its neighbours, washed in its theme's colour and charted in its
// theme's ink (territories.js). A worker (regions-paint.js) works out the
// whole map's drawing when its layout or an area's look changes, one at a
// time, and keeps it, so the map never waits for it. It sends back a small
// picture of the whole map's washes, and then, each time the camera comes
// to rest (camera.js calls `rest`), the part of the map in view with half a
// screen round it, drawn at the screen's own pixels. Both sit in #under,
// behind the lines, in the map's units: while the camera moves the browser
// only moves and scales that picture, and once it rests the view is drawn
// again, sharp. Every frame the nodes are drawn, the pictures are stretched
// from where the nodes were when the map was drawn to where they are drawn
// now (`placeRegions`), so they move with them while they glide, until the
// next drawing lands. An area with no look has no territory, and every area
// stands on the bare ground where a worker cannot draw, or once it fails.
const under = $("#under"), map = $("#map");
// The most pixels a picture of the view has: a screen and the half a screen round it, fewer on a big
// screen.
const VIEW_PX = 12e6;
let worker = typeof OffscreenCanvas === "function" ? new Worker(new URL("./regions-paint.js", import.meta.url), { type: "module" }) : null;
let layer = null, washEl = null, viewEl = null;
let queued = false, inflight = null, drawn = null, failedKey = "";  // inflight, drawn: { key, r }, r the nodes' box the drawing was asked for
let cam = null, viewing = null, again = false, view = null;  // viewing: { k } asked for; view: { key, rect, k }, the picture of the view shown

// Draw what has changed, after this frame.
export function paintRegions() {
  if (!worker || queued) return;
  queued = true;
  requestAnimationFrame(() => { queued = false; if (!inflight) next(); });
}

// The camera has come to rest: draw the view again if the picture shown is
// not sharp for it or does not cover it.
export function rest(c) {
  cam = { x: Math.round(c.x), y: Math.round(c.y), k: c.k };
  ask();
}

const topOf = (c) => c.tasks.find((t) => t.look && t.look.root === t.look.seed) ?? c.root;

// The box of the nodes of the areas that wear a look, where `at` has them.
function extent(at) {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const c of graph.comps) {
    if (c.folded || !topOf(c).look) continue;
    for (const n of c.nodes) {
      const p = at.get(n.id);
      if (!p) continue;
      x0 = Math.min(x0, p.x); x1 = Math.max(x1, p.x + p.w); y0 = Math.min(y0, p.y); y1 = Math.max(y1, p.y + p.h);
    }
  }
  return x0 === Infinity ? null : { x: x0, y: y0, w: Math.max(1, x1 - x0), h: Math.max(1, y1 - y0) };
}

// The drawing over where the nodes are drawn now: as drawn once they rest
// where it was drawn for, and stretched by the compositor while they glide,
// or while the next drawing is still to come.
export function placeRegions() {
  if (!layer || !drawn?.r) return;
  const r = drawn.r, now = extent(shown);
  if (!now) return;
  const sx = now.w / r.w, sy = now.h / r.h;
  const tr = now.x === r.x && now.y === r.y && sx === 1 && sy === 1 ? "" : `translate(${now.x - r.x * sx}px, ${now.y - r.y * sy}px) scale(${sx}, ${sy})`;
  if (layer.style.transform !== tr) layer.style.transform = tr;
}

// What the map's areas are now: each with a look, its nodes' and names'
// boxes, and the map's lines with their half-widths, which the territories
// follow and the charts' ink stops short of.
const measure = document.createElement("canvas").getContext("2d"), widths = new Map();
function width(text, font) {
  const k = `${font}|${text}`;
  if (!widths.has(k)) { measure.font = font; widths.set(k, measure.measureText(text).width); }
  return widths.get(k);
}
function wanted() {
  const areas = [];
  for (const c of graph.comps) {
    const top = topOf(c);
    if (c.folded || !top.look) continue;
    const marks = [], labels = [];
    for (const n of c.nodes) {
      const p = positions.get(n.id), l = p && label(n);
      if (!p) continue;
      marks.push([p.x, p.y, p.x + p.w, p.y + p.h].map(Math.round));
      if (l) { const q = centre(p), x = q.x + (n.big ? 24 : 10); labels.push([x, q.y - 9, x + width(l, n.big ? LABEL_FONT : CHIP_FONT) + 6, q.y + 9].map(Math.round)); }
    }
    if (marks.length) areas.push({ id: top.id, theme: top.look.theme, seed: top.look.seed, name: c.root.name ?? "", marks, labels });
  }
  const lines = [...routes.values()].map((e) => e.P.map(([x, y], i) => [Math.round(x), Math.round(y), Math.round(5 * (e.W ? e.W[i] : LIFE)) / 10]));
  return { areas, lines, key: JSON.stringify([areas, lines]) };
}

// Ask the worker for the map as it is now, if that is not what is drawn.
function next() {
  const w = wanted();
  if (w.key === drawn?.key || w.key === failedKey) return;
  inflight = { key: w.key, r: extent(positions) };
  worker.postMessage({ type: "draw", key: w.key, areas: w.areas, lines: w.lines, paper: getComputedStyle(map).getPropertyValue("--ground").trim() });
}

// The part of the map to draw for the camera: the screen and up to half a
// screen round it, its corner on a device pixel, at the screen's own
// pixels, or fewer on a screen too big for that.
function wantedView() {
  const W = map.clientWidth, H = map.clientHeight, dpr = devicePixelRatio || 1, screen = W * H * dpr * dpr;
  const f = Math.max(0, Math.min(0.5, (Math.sqrt(VIEW_PX / screen) - 1) / 2)), q = Math.min(1, Math.sqrt(VIEW_PX / (screen * (1 + 2 * f) ** 2)));
  const sx = Math.floor(-f * W * dpr) / dpr, sy = Math.floor(-f * H * dpr) / dpr, Wpx = Math.round(W * (1 + 2 * f) * dpr * q), Hpx = Math.round(H * (1 + 2 * f) * dpr * q);
  const { x, y, k } = cam;
  return { rect: { x: (sx - x) / k, y: (sy - y) / k, w: Wpx / (dpr * q) / k, h: Hpx / (dpr * q) / k }, W: Wpx, H: Hpx, dpr: dpr * q, screen: { x: -x / k, y: -y / k, w: W / k, h: H / k } };
}

// Ask for the view, unless the picture shown is of this drawing, at this
// zoom, and covers the screen.
function ask() {
  if (!worker || !cam || !drawn) return;
  if (viewing) { again = true; return; }
  const w = wantedView(), s = w.screen, r = view?.rect;
  if (view && view.key === drawn.key && view.k === cam.k && s.x >= r.x && s.y >= r.y && s.x + s.w <= r.x + r.w && s.y + s.h <= r.y + r.h) return;
  viewing = { k: cam.k };
  worker.postMessage({ type: "view", key: drawn.key, rect: w.rect, W: w.W, H: w.H, dpr: w.dpr });
}

function canvas() {
  const c = document.createElement("canvas");
  c.style.cssText = "position: absolute";
  return c;
}
// A picture of the map, over the map units in rect.
function put(el, bitmap, rect) {
  Object.assign(el, { width: bitmap.width, height: bitmap.height });
  el.getContext("bitmaprenderer").transferFromImageBitmap(bitmap);
  Object.assign(el.style, { left: `${rect.x}px`, top: `${rect.y}px`, width: `${rect.w}px`, height: `${rect.h}px` });
}
// The whole map's washes show only where the view's picture does not.
function hole() {
  if (!washEl || !view || !drawn?.rect) return;
  const R = drawn.rect, r = view.rect, x0 = r.x - R.x + 1, y0 = r.y - R.y + 1, x1 = x0 + r.w - 2, y1 = y0 + r.h - 2;
  washEl.style.clipPath = `polygon(evenodd, 0 0, 100% 0, 100% 100%, 0 100%, 0 0, ${x0}px ${y0}px, ${x1}px ${y0}px, ${x1}px ${y1}px, ${x0}px ${y1}px, ${x0}px ${y0}px)`;
}
function clear() {
  layer?.remove();
  layer = washEl = viewEl = null;
  view = null;
}

worker?.addEventListener("message", ({ data }) => {
  const { type, key, error } = data;
  if (type === "draw") {
    const asked = inflight;
    inflight = null;
    if (error) { fail("drawing the areas on the map, which stay bare", error); failedKey = key; next(); return; }
    // Shown even if the map has moved on since it was asked for: stretched
    // over where the nodes are now until the next drawing lands.
    drawn = { key, r: asked.r, rect: data.rect };
    if (!data.wash) clear();
    else {
      if (!layer) {
        layer = document.createElement("div");
        layer.style.cssText = "position: absolute; left: 0; top: 0; pointer-events: none; transform-origin: 0 0";
        washEl = canvas(); viewEl = canvas();
        layer.append(washEl, viewEl);
        under.prepend(layer);
      }
      put(washEl, data.wash, data.rect);
      hole();
      placeRegions();
      ask();
    }
    next();
  } else if (type === "view") {
    const asked = viewing;
    viewing = null;
    if (error) fail("drawing the map in view, which shows its washes only", error);
    else if (data.bitmap && layer && key === drawn?.key) {
      put(viewEl, data.bitmap, data.rect);
      view = { key, rect: data.rect, k: asked?.k };
      hole();
    } else data.bitmap?.close();
    if (again) { again = false; ask(); }
  }
});

// The worker could not load, or died: every area stands on the bare ground.
worker?.addEventListener("error", (e) => {
  fail("drawing the areas on the map, which stay bare", e.message ?? "the worker failed");
  worker.terminate();
  worker = null;
  inflight = null;
  clear();
});
