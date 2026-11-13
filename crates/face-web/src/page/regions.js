import { $ } from "./base.js";
import { centre, graph, isDone, positions, shown } from "./map.js";
import { cam } from "./camera.js";
import { load, paintRegion } from "./theme.js";

// ─── Each area's ground, faint behind its part of the map ─────────────────
// An area is a chat Max started and everything under it, and every task in
// it wears its theme (the row's `look`). Its project on the map stands on
// that theme's painting (theme.js `paintRegion`), each live agent in a pool
// of its own colour. A painting is a canvas in #under, behind the lines, in
// the map's units, of the project as laid out; it is painted again only
// when the project's shape or its live agents move, or the zoom passes a
// step, and one at a time after the map has drawn, so the map never waits
// for it. Every frame the nodes are drawn, each painting is stretched over
// where its project is drawn now (`placeRegions`), so it moves with them
// while they glide. A project with no look stands on the bare ground.
const under = $("#under"), edges = $("#edges"), map = $("#map");
const painted = new Map();  // an area's top -> { key, r, canvas }: r is the box it was painted for
// Room around a project's nodes: at its sides (more at its right, for the
// names), above into its history, and below the present row, where the
// painting is fullest.
const SIDE = 70, NAMES = 170, ABOVE = 60, BELOW = 170, POOL = 90;
let queued = false, painting = 0;

// Paint what has changed, after this frame.
export function paintRegions() {
  if (queued) return;
  queued = true;
  requestAnimationFrame(() => setTimeout(() => { queued = false; next(); }));
}

// The camera moved: paint again only if the zoom passed a step.
export function zoomed() { if (step() !== painting) paintRegions(); }

// Bitmap pixels per map unit: the zoom's power of two, at up to 2.
const step = () => Math.min(2, Math.max(0.25, 2 ** Math.round(Math.log2(cam.k * Math.min(2, devicePixelRatio || 1)))));

const topOf = (c) => c.tasks.find((t) => t.look && t.look.root === t.look.seed) ?? c.root;

// A project's painting's box, with its nodes where `at` has them.
function box(c, at) {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = 0;
  for (const n of c.nodes) {
    const p = at.get(n.id);
    if (!p) continue;
    x0 = Math.min(x0, p.x); x1 = Math.max(x1, p.x + p.w); y0 = Math.min(y0, p.y); y1 = Math.max(y1, p.y + p.h);
  }
  if (x0 === Infinity) return null;
  const r = { x: Math.round(x0 - SIDE), y: Math.round(Math.min(y0, 0) - ABOVE) };
  r.w = Math.round(x1 + NAMES) - r.x; r.h = Math.round(y1 + BELOW) - r.y;
  return r;
}

// Each painting over where its project's nodes are drawn now: on the box it
// was painted for once they rest there, and stretched by the compositor
// while they glide, or while its next painting is still to come.
export function placeRegions() {
  if (!painted.size) return;
  for (const c of graph.comps) {
    const p = painted.get(topOf(c).id), now = p && box(c, shown);
    if (!now) continue;
    const { r } = p, t = now.x === r.x && now.y === r.y && now.w === r.w && now.h === r.h ? ""
      : `translate(${now.x - r.x}px, ${now.y - r.y}px) scale(${now.w / r.w}, ${now.h / r.h})`;
    if (p.canvas.style.transform !== t) p.canvas.style.transform = t;
  }
}

function wanted() {
  const out = new Map();
  for (const c of graph.comps) {
    const top = topOf(c);
    if (c.folded || !top.look) continue;
    const r = box(c, positions);
    if (!r) continue;
    const pools = c.nodes.filter((n) => n.big && !isDone(n.t) && n.t.look && positions.has(n.id)).map((n) => {
      const q = centre(positions.get(n.id));
      return { x: Math.round(q.x - r.x), y: Math.round(q.y - r.y), r: POOL, slot: n.t.look.slot };
    });
    const scale = painting = step();
    out.set(top.id, { look: top.look, r, pools, scale, key: JSON.stringify([top.look, r, pools, scale]) });
  }
  return out;
}

// One painting, then the next after the browser has had the thread back.
async function next() {
  const want = wanted();
  for (const [id, p] of painted) if (!want.has(id)) { p.canvas.remove(); painted.delete(id); }
  const [id, w] = [...want].find(([id, w]) => painted.get(id)?.key !== w.key) ?? [];
  if (!w) return;
  const ground = await load(w.look.theme);
  const paper = getComputedStyle(map).getPropertyValue("--ground").trim();
  const { slot, seed, root, line } = w.look;
  const canvas = paintRegion(ground, { slot, seed, root, line }, { w: w.r.w, h: w.r.h, now: -w.r.y, scale: w.scale, paper, pools: w.pools });
  canvas.style.cssText = `position: absolute; left: ${w.r.x}px; top: ${w.r.y}px; width: ${w.r.w}px; height: ${w.r.h}px; pointer-events: none; transform-origin: 0 0`;
  painted.get(id)?.canvas.remove();
  under.insertBefore(canvas, edges);
  painted.set(id, { key: w.key, r: w.r, canvas });
  placeRegions();
  paintRegions();
}
