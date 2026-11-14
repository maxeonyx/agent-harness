import { $, fail } from "./base.js";
import { centre, CHIP_FONT, graph, label, LABEL_FONT, positions, routes, shown } from "./map.js";

// ─── Each area as a territory on the map, drawn as its theme's chart ──────
// An area is a chat Max started and everything under it, and every task in
// it wears its theme (the row's `look`). On the map each area is a
// territory: the land round its nodes, names and streams, bordered where it
// meets its neighbours, washed in its theme's colour and charted in its
// theme's ink (territories.js). A worker (regions-paint.js) draws the whole
// map at once, since each border is shared, so the map never waits for it;
// each area comes back as SVG in the map's units and a small picture of its
// wash, put in a box in #under, behind the lines, which the camera moves and
// the browser draws sharp at every zoom. The map is drawn again only when
// its layout or an area's look changes, never as the camera moves, one
// drawing at a time, and shown in one step when it lands. Every frame the
// nodes are drawn, each area's box is stretched over where its nodes are
// drawn now (`placeRegions`), so it moves with them while they glide. An
// area with no look has no territory, and every area stands on the bare
// ground where a worker cannot draw, or once it fails.
const under = $("#under"), edges = $("#edges"), map = $("#map");
const drawn = new Map();   // an area's top -> { el, r, t }: r its nodes' box when drawn, t its drawing's box
let worker = typeof OffscreenCanvas === "function" ? new Worker(new URL("./regions-paint.js", import.meta.url), { type: "module" }) : null;
let queued = false, inflight = null, shownKey = "", failedKey = "";

// Draw what has changed, after this frame.
export function paintRegions() {
  if (!worker || queued) return;
  queued = true;
  requestAnimationFrame(() => { queued = false; if (!inflight) next(); });
}

const topOf = (c) => c.tasks.find((t) => t.look && t.look.root === t.look.seed) ?? c.root;

// An area's nodes' box, with its nodes where `at` has them.
function box(c, at) {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const n of c.nodes) {
    const p = at.get(n.id);
    if (!p) continue;
    x0 = Math.min(x0, p.x); x1 = Math.max(x1, p.x + p.w); y0 = Math.min(y0, p.y); y1 = Math.max(y1, p.y + p.h);
  }
  return x0 === Infinity ? null : { x: x0, y: y0, w: Math.max(1, x1 - x0), h: Math.max(1, y1 - y0) };
}

// Each area's drawing over where its nodes are drawn now: as drawn once they
// rest where it was drawn for, and stretched by the compositor while they
// glide, or while its next drawing is still to come.
export function placeRegions() {
  if (!drawn.size) return;
  for (const c of graph.comps) {
    const d = drawn.get(topOf(c).id), now = d && box(c, shown);
    if (!now) continue;
    const { r, t } = d, sx = now.w / r.w, sy = now.h / r.h;
    const tr = now.x === r.x && now.y === r.y && now.w === r.w && now.h === r.h ? ""
      : `translate(${now.x - r.x + (t.x - r.x) * (sx - 1)}px, ${now.y - r.y + (t.y - r.y) * (sy - 1)}px) scale(${sx}, ${sy})`;
    if (d.el.style.transform !== tr) d.el.style.transform = tr;
  }
}

// What the map's areas are now: each with a look, its nodes' and names'
// boxes, and the map's lines, which the territories follow.
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
    const r = box(c, positions);
    if (!r) continue;
    const marks = [];
    for (const n of c.nodes) {
      const p = positions.get(n.id), l = p && label(n);
      if (!p) continue;
      marks.push([p.x, p.y, p.x + p.w, p.y + p.h].map(Math.round));
      if (l) { const q = centre(p), x = q.x + (n.big ? 24 : 10); marks.push([x, q.y - 9, x + width(l, n.big ? LABEL_FONT : CHIP_FONT) + 6, q.y + 9].map(Math.round)); }
    }
    const { theme, slot, seed, root, line } = top.look;
    areas.push({ id: top.id, theme, seed, who: { slot, seed, root, line }, name: c.root.name ?? "", marks, r });
  }
  const lines = [...routes.values()].map((e) => e.P.map(([x, y]) => [Math.round(x), Math.round(y)]));
  return { areas, lines, key: JSON.stringify([areas.map(({ r, ...a }) => a), lines]) };
}

// Ask the worker for the map as it is now, if that is not what is shown.
function next() {
  const w = wanted();
  if (w.key === shownKey || w.key === failedKey) return;
  inflight = { key: w.key, rs: new Map(w.areas.map((a) => [a.id, a.r])) };
  const paper = getComputedStyle(map).getPropertyValue("--ground").trim();
  worker.postMessage({ key: w.key, areas: w.areas.map(({ r, ...a }) => a), lines: w.lines, paper });
}

// The map has been drawn: show it, if the map is still as it was drawn for,
// and ask for the next.
worker?.addEventListener("message", ({ data: { key, areas, error } }) => {
  const { rs } = inflight;
  inflight = null;
  if (error) { fail("drawing the areas on the map, which stay bare", error); failedKey = key; }
  else if (wanted().key === key) show(areas, rs, key);
  else for (const a of areas) a.tint.close();
  next();
});

function show(areas, rs, key) {
  const now = new Set(areas.map((a) => a.id));
  for (const [id, d] of drawn) if (!now.has(id)) { d.el.remove(); drawn.delete(id); }
  for (const { id, box: t, svg, tint } of areas) {
    let d = drawn.get(id);
    if (!d) {
      const el = document.createElement("div");
      el.style.cssText = "position: absolute; pointer-events: none; transform-origin: 0 0";
      under.insertBefore(el, edges);
      d = { el };
      drawn.set(id, d);
    }
    const canvas = document.createElement("canvas");
    Object.assign(canvas, { width: tint.width, height: tint.height });
    canvas.style.cssText = "position: absolute; left: 0; top: 0; width: 100%; height: 100%";
    canvas.getContext("bitmaprenderer").transferFromImageBitmap(tint);
    d.el.replaceChildren(canvas);
    d.el.insertAdjacentHTML("beforeend", svg);
    Object.assign(d.el.style, { left: `${t.x}px`, top: `${t.y}px`, width: `${t.w}px`, height: `${t.h}px` });
    Object.assign(d, { r: rs.get(id), t });
  }
  shownKey = key;
  placeRegions();
}

// The worker could not load, or died: every area stands on the bare ground.
worker?.addEventListener("error", (e) => {
  fail("drawing the areas on the map, which stay bare", e.message ?? "the worker failed");
  worker.terminate();
  worker = null;
  inflight = null;
  for (const d of drawn.values()) d.el.remove();
  drawn.clear();
});
