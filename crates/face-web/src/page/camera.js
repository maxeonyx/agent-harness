import { $, el, lerp, panel, phone, reduced } from "./base.js";
import { byId, DOT, graph, look, positions, ROOT_DOT, shown } from "./map.js";
import { drawEdges, nodeEls, placeMarks, refresh } from "./draw.js";
import { hideTip } from "./hover.js";
import { closeNode, go, NEW, openId, saved } from "./card.js";

// ─── The camera ────────────────────────────────────────────────────────────
export const map = $("#map"), world = $("#world"), nodesLayer = $("#nodes"), edgesLayer = $("#edges");
export let cam = { x: 0, y: 0, k: 1 };
export let framed = true;  // follows the map's growth until Max moves it himself
// While the map moves it is a layer of its own, so the browser moves a
// picture instead of drawing every frame. A layer keeps the zoom it was
// drawn at, so at rest it goes, and the open card's text is drawn again at
// the zoom it is read at, sharp.
let resting = 0;
function applyCamera() {
  // Whole pixels, so the open card's text stays sharp.
  const x = Math.round(cam.x), y = Math.round(cam.y);
  world.style.transform = `translate(${x}px, ${y}px) scale(${cam.k})`;
  map.classList.add("moving");
  clearTimeout(resting);
  resting = setTimeout(() => map.classList.remove("moving"), 150);
  const g = 24 * cam.k;
  map.style.backgroundSize = `${g}px ${g}px`;
  map.style.backgroundPosition = `${x}px ${y}px`;
  placeMarks();
  coverCheck();
  sleepCheck();
}
// The map is moved only by the camera, never by the browser scrolling it to
// show what has focus.
map.addEventListener("scroll", () => { map.scrollTop = 0; map.scrollLeft = 0; });
function bounds(ids) {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const [id, p] of positions) {
    if (ids && !ids.has(id)) continue;
    x0 = Math.min(x0, p.x); y0 = Math.min(y0, p.y); x1 = Math.max(x1, p.x + p.w); y1 = Math.max(y1, p.y + p.h);
  }
  return x0 === Infinity ? null : { x0, y0, x1, y1 };
}
// The camera that shows every live project at a size that reads.
export function framing() {
  const running = new Set(graph.comps.filter((c) => !c.folded).flatMap((c) => c.nodes.map((n) => n.id)));
  const b = bounds(running.size ? running : null);
  if (!b) return null;
  const top = 48, bottom = 64, side = 40;
  const w = innerWidth - side * 2, h = innerHeight - top - bottom;
  const least = phone() ? .7 : .45;
  const k = Math.max(least, Math.min(1.25, w / (b.x1 - b.x0), h / (b.y1 - b.y0)));
  const c = { k, x: side + (w - (b.x1 - b.x0) * k) / 2 - b.x0 * k, y: top + (h - (b.y1 - b.y0) * k) / 2 - b.y0 * k };
  // Too big to fit at a size that reads: start at the top, the first project centred.
  if ((b.y1 - b.y0) * k > h) c.y = top - b.y0 * k;
  if ((b.x1 - b.x0) * k > w) {
    const first = graph.comps[0] && positions.get(graph.comps[0].root.id);
    c.x = first ? innerWidth / 2 - (first.x + first.w / 2) * k : side - b.x0 * k;
  }
  return c;
}
export function fit(fly = true) {
  if (openId) { if (saved) saved.framed = true; closeNode(); return; }
  const c = framing();
  if (!c) return;
  framed = true;
  if (fly) glide(c, { T: 420 });
  else { cam = c; applyCamera(); }
}

// Dragging the ground, the wheel and a pinch move the map. Over the open
// card, the pointer and the wheel are the card's own: its text selects and
// its log scrolls, while its header drags the map like the ground does.
const pointers = new Map();
export let drag = null;
const cards = (e) => e.target.closest?.("#panel") && !asleep && (!e.target.closest("#panel > .bar") || e.target.closest("button, [contenteditable]"));
map.addEventListener("pointerdown", (e) => {
  if (e.button !== 0 && e.pointerType === "mouse") return;
  if (cards(e)) return;
  pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
  drag = { moved: false, start: { x: e.clientX, y: e.clientY }, cam: { ...cam }, pinch: pointers.size === 2 ? pinchOf() : null };
});
map.addEventListener("pointermove", (e) => {
  if (!pointers.has(e.pointerId) || !drag) return;
  pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
  if (pointers.size === 2) {
    const p = pinchOf();
    if (!drag.pinch) { settle(); drag.pinch = p; drag.cam = { ...cam }; }
    const k = Math.max(.15, Math.min(3, drag.cam.k * p.d / drag.pinch.d));
    const wx = (drag.pinch.x - drag.cam.x) / drag.cam.k, wy = (drag.pinch.y - drag.cam.y) / drag.cam.k;
    cam = { k, x: p.x - wx * k, y: p.y - wy * k };
    drag.moved = true;
  } else {
    const dx = e.clientX - drag.start.x, dy = e.clientY - drag.start.y;
    if (!drag.moved && Math.hypot(dx, dy) < 5) return;
    if (!drag.moved) {
      // Only now is it a drag: a press that does not move stays a click on what it pressed.
      drag.moved = true;
      map.classList.add("grabbing");
      map.setPointerCapture?.(e.pointerId);
      settle();
      getSelection().removeAllRanges();
      drag.cam = { x: cam.x - dx, y: cam.y - dy, k: cam.k };
      hideTip();
    }
    cam = { ...cam, x: drag.cam.x + dx, y: drag.cam.y + dy };
  }
  framed = false;
  applyCamera();
});
const lift = (e) => {
  pointers.delete(e.pointerId);
  if (!pointers.size) { map.classList.remove("grabbing"); setTimeout(() => { drag = null; }, 0); }
  else if (drag) { drag.pinch = null; const p = [...pointers.values()][0]; drag.start = p; drag.cam = { ...cam }; }
};
map.addEventListener("pointerup", lift);
map.addEventListener("pointercancel", lift);
function pinchOf() { const [a, b] = [...pointers.values()]; return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2, d: Math.max(10, Math.hypot(a.x - b.x, a.y - b.y)) }; }
map.addEventListener("wheel", (e) => {
  if (e.target.closest("#panel") && !e.ctrlKey && !asleep) return;
  e.preventDefault();
  settle();
  hideTip();
  const k = Math.max(.15, Math.min(3, cam.k * Math.exp(-e.deltaY * (e.deltaMode ? .05 : .0015))));
  const wx = (e.clientX - cam.x) / cam.k, wy = (e.clientY - cam.y) / cam.k;
  cam = { k, x: e.clientX - wx * k, y: e.clientY - wy * k };
  framed = false;
  applyCamera();
}, { passive: false });
// On the bare ground, a click closes the open card and a double click opens
// a new chat there. A click waits a moment to know which it is; a double tap
// on a touch screen is a double click.
export let closing = 0, tapped = null;
// By the path the event took, not its target: a button its own click redraws
// is no longer in the card when the click reaches the map.
const ground = (e) => !drag?.moved && !e.composedPath().some((n) => n.matches?.(".node, #panel, .offscreen"));
map.addEventListener("click", (e) => {
  if (!ground(e) || e.detail > 1 || !openId) return;
  clearTimeout(closing);
  closing = setTimeout(closeNode, 220);
});
map.addEventListener("dblclick", (e) => { if (ground(e)) newHere(e.clientX, e.clientY); });
map.addEventListener("pointerup", (e) => {
  if (e.pointerType !== "touch" || !ground(e)) return;
  const now = performance.now();
  if (tapped && now - tapped.at < 320 && Math.hypot(e.clientX - tapped.x, e.clientY - tapped.y) < 30) { tapped = null; newHere(e.clientX, e.clientY); }
  else tapped = { at: now, x: e.clientX, y: e.clientY };
});
let lastNew = 0;
function newHere(sx, sy) {
  clearTimeout(closing);
  const now = performance.now();
  if (now - lastNew < 450) return;
  lastNew = now;
  newPoint = { x: (sx - cam.x) / cam.k, y: (sy - cam.y) / cam.k, sx, sy };
  go(NEW);
}

// ─── The open card, in the map ─────────────────────────────────────────────
// An open node is a card in the world, where the node is: its top at the
// node's top, centred on it. The map makes room at the card's seams without
// being laid out again: what is wholly beside the node's column moves out
// sideways, and what is below the node moves down, each by the card's size,
// so everything keeps its place relative to everything else.
const SEAM = 28;
export let seam = null;       // the open card's place: {id, o, W, H, sx, sy}
export let cardShown = null;  // the card's visible rect in the world, now
export let newPoint = null;   // where a new chat's card opens, in the world
// Another area moves the camera's state through these: an import is read-only.
export const setFramed = (f) => { framed = f; }, setSeam = (s) => { seam = s; };
export const setCardShown = (r) => { cardShown = r; }, setNewPoint = (p) => { newPoint = p; };
const centrePoint = () => ({ x: (innerWidth / 2 - cam.x) / cam.k, y: (innerHeight * .3 - cam.y) / cam.k, sx: innerWidth / 2, sy: innerHeight * .3 });

// Room kept right of the card for the task's controls, which float there
// (on a phone they float inside it).
const controlsRoom = () => phone() ? 0 : 48;
// A new chat's card is short; on a phone its tuning wraps to a line of its own.
function cardSize(id) {
  const m = phone() ? 6 : 16;
  const W = Math.min(id === NEW ? 640 : 880, innerWidth - 2 * m - controlsRoom());
  const H = id === NEW ? (phone() ? 200 : 168) : innerHeight - 2 * m;
  return { W, H };
}
export function seamFor(id) {
  let o;
  if (id === NEW) { const p = newPoint || (newPoint = centrePoint()); o = { x: p.x - 1, y: p.y - 14, w: 2, h: 2 }; }
  else { const p = positions.get(id); if (!p) return null; o = { x: p.x, y: p.y, w: p.w, h: p.h }; }
  const { W, H } = cardSize(id);
  return { id, o, W, H, sx: Math.max(0, (W - o.w) / 2) + SEAM, sy: Math.max(0, H - o.h) + SEAM };
}
// On whole pixels, so its text is sharp where it rests.
export const cardRect = (s) => ({ x: Math.round(s.o.x + s.o.w / 2 - s.W / 2), y: Math.round(s.o.y), w: s.W, h: s.H });
// Where a node goes with the card open.
export function target(id) {
  const p = positions.get(id);
  if (!seam || !p || id === seam.id) return p;
  const { o, sx, sy } = seam;
  let dx = 0, dy = 0;
  if (p.x + p.w <= o.x + 1) dx = -sx; else if (p.x >= o.x + o.w - 1) dx = sx;
  if (p.y >= o.y + o.h - 1) dy = sy;
  return dx || dy ? { ...p, x: p.x + dx, y: p.y + dy } : p;
}
// A node's dot, or a new chat's point: what a card opens from and closes to.
export function dotRect(id) {
  if (id === NEW) { const p = newPoint || centrePoint(); return { x: p.x - 10, y: p.y - 14, w: 20, h: 20 }; }
  const p = shown.get(id) || positions.get(id);
  if (!p) return null;
  if (graph.comps.some((c) => c.folded && c.root.id === id)) return { x: p.x, y: p.y, w: p.w, h: p.h };
  const d = graph.comps.some((c) => c.root.id === id) ? ROOT_DOT : DOT;
  return { x: p.x + p.w / 2 - d / 2, y: p.y, w: d, h: d };
}
// The camera that shows a card at its own size, moved as little as it takes
// for all of it to show; `at` is a point of the world held where it is on
// screen, so the card opens where Max was looking. Its controls show too.
export function camFor(card, at) {
  const m = phone() ? 6 : 16;
  const p = at || { x: card.x + card.w / 2, y: card.y, sx: innerWidth / 2, sy: m };
  const c = { k: 1, x: p.sx - p.x, y: p.sy - p.y };
  const r = { ...card, w: card.w + controlsRoom() };
  const left = c.x + r.x, top = c.y + r.y;
  if (r.w > innerWidth - 2 * m) c.x = (innerWidth - r.w) / 2 - r.x;
  else if (left < m) c.x += m - left;
  else if (left + r.w > innerWidth - m) c.x -= left + r.w - (innerWidth - m);
  if (top < m) c.y += m - top;
  else if (top + r.h > innerHeight - m) c.y -= top + r.h - (innerHeight - m);
  return c;
}
// A node's place as the card will have it, held where the node is on screen.
export function heldAt(id) {
  const p = positions.get(id), s = shown.get(id) || p;
  if (!p) return null;
  return { x: p.x + p.w / 2, y: p.y, sx: cam.x + (s.x + s.w / 2) * cam.k, sy: cam.y + s.y * cam.k };
}
function placeCard(b) {
  panel.style.left = `${b.x}px`; panel.style.top = `${b.y}px`;
  panel.style.width = `${b.w}px`; panel.style.height = `${b.h}px`;
}
// Shows the part `v` of a card whose box is `b`: growing down and outwards
// from where it opens, as the node opening into the room made for it.
function clipCard(v, b) {
  const dx = v.x + v.w / 2 - (b.x + b.w / 2), dy = v.y - b.y;
  const sx = Math.max(0, (b.w - v.w) / 2), sy = Math.max(0, b.h - v.h);
  panel.style.transform = dx || dy ? `translate(${dx}px, ${dy}px)` : "";
  panel.style.clipPath = sx > .5 || sy > .5 ? `inset(0 ${sx}px ${sy}px ${sx}px round ${Math.min(18, v.w / 2, v.h / 2)}px)` : "";
}
const lerpRect = (a, b, e) => ({ x: lerp(a.x, b.x, e), y: lerp(a.y, b.y, e), w: lerp(a.w, b.w, e), h: lerp(a.h, b.h, e) });
// The floating controls hide when the card would sit under them.
let usageRight = 0;
export const setUsageRight = (r) => { usageRight = r; };
export function coverCheck() {
  let covered = false;
  if (openId && cardShown) {
    const l = cam.x + cardShown.x * cam.k, r = l + cardShown.w * cam.k;
    const t = cam.y + cardShown.y * cam.k, b = t + cardShown.h * cam.k;
    covered = b > innerHeight - 60 && t < innerHeight && (l < usageRight + 24 || r > innerWidth - 120);
  }
  document.body.classList.toggle("covered", covered);
}

// The card fits the window it is in: a phone's keyboard opening shrinks it.
addEventListener("resize", () => {
  const next = openId && seam && seamFor(openId);
  if (next) {
    const was = cardShown;
    seam = next;
    const r = cardRect(seam);
    placeCard(r);
    cardShown = r;
    cam = camFor(r, was && { x: r.x + r.w / 2, y: r.y, sx: cam.x + (was.x + was.w / 2) * cam.k, sy: cam.y + was.y * cam.k });
    applyCamera();
  }
  refresh();
});

// ─── The card asleep ───────────────────────────────────────────────────────
// Zoomed out too far to read, or moved mostly off screen, the open card is
// one button where it is: its name, marked as the open one. A click fits the
// camera to it again; zooming or moving back wakes it on its own.
const wake = el("button", "wake"), wakeLabel = el("span");
wake.type = "button";
wake.title = "Back to it";
wake.append(wakeLabel);
$("#panel").append(wake);
let asleep = false;
wake.addEventListener("click", () => { if (!drag?.moved) refocus(); });
export function refocus() {
  if (!openId || !seam) return;
  framed = false;
  fly(camFor(cardRect(seam)), { T: 320 });
}
function sleepCheck() {
  let sleep = false;
  if (openId && cardShown && !flight) {
    const r = cardShown, k = cam.k;
    const l = cam.x + r.x * k, t = cam.y + r.y * k, w = r.w * k, h = r.h * k;
    const vl = Math.max(0, l), vt = Math.max(0, t), vr = Math.min(innerWidth, l + w), vb = Math.min(innerHeight, t + h);
    const seen = Math.max(0, vr - vl) * Math.max(0, vb - vt) / (w * h);
    // A little apart, so it does not flicker at the edge.
    sleep = asleep ? k < .75 || seen < .6 : k < .7 || seen < .5;
    // Its name in the middle of what shows of it, at a size that reads.
    if (sleep && seen > 0) {
      wakeLabel.style.left = `${((vl + vr) / 2 - l) / k}px`;
      wakeLabel.style.top = `${((vt + vb) / 2 - t) / k}px`;
      wakeLabel.style.fontSize = `${16 / k}px`;
    }
  }
  if (sleep) {
    const t = byId[openId], key = t ? `${look(t).cls}\n${t.name}` : "";
    if (wakeLabel.dataset.key !== key) {
      wakeLabel.dataset.key = key;
      wakeLabel.replaceChildren(el("i", `mini ${t ? look(t).cls : "starting"}`), t ? t.name : "New chat");
    }
  }
  if (sleep === asleep) return;
  asleep = sleep;
  panel.classList.toggle("asleep", sleep);
}

// ─── Motion ────────────────────────────────────────────────────────────────
// One frame loop moves all that moves: the nodes, the card, the camera; the
// edges and the marks at the screen's edges follow each frame.
const anims = new Map();
let raf = 0;
function animate(name, T, step) {
  anims.get(name)?.done();
  if (document.hidden) { anims.delete(name); step(1); drawEdges(); applyCamera(); return Promise.resolve(); }
  return new Promise((done) => {
    anims.set(name, { t0: performance.now(), T: reduced() ? 0 : T, step, done });
    if (!raf) raf = requestAnimationFrame(frame);
  });
}
function frame(now) {
  raf = 0;
  for (const [name, a] of anims) {
    const u = a.T ? Math.min(1, (now - a.t0) / a.T) : 1;
    a.step(1 - Math.pow(1 - u, 3));
    if (u >= 1) { anims.delete(name); a.done(); }
  }
  drawEdges();
  applyCamera();
  if (anims.size) raf = requestAnimationFrame(frame);
}
// Everything that is moving, where it is going, now.
document.addEventListener("visibilitychange", () => { if (document.hidden) settle(); });
function settle() {
  for (const a of anims.values()) { a.step(1); a.done(); }
  anims.clear();
  drawEdges();
  applyCamera();
}
// Everything to where it belongs: the nodes to their places around the open
// card's seams, the card from `cardFrom` to `cardTo` in its box, the camera
// to `to`. With no `to` and the card open, the camera keeps the card where
// it is on screen, and the map moves around it.
// A card opening, closing or moving is a flight: what else would move
// waits for it to land rather than cutting it short.
let flight = null;
export async function fly(to, opts) {
  const f = glide(to, { ...opts, strong: true });
  flight = f;
  await f;
  if (flight === f) flight = null;
}
export function glide(to = null, { T = 240, cardFrom, cardTo, box, strong = false } = {}) {
  if (flight && !strong) return flight.then(() => glide(to, { T, cardFrom, cardTo, box }));
  const r1 = cardTo !== undefined ? cardTo : seam ? cardRect(seam) : null;
  const r0 = cardFrom !== undefined ? cardFrom : cardShown || r1;
  const b = box !== undefined ? box : r1;
  const from = new Map([...shown].map(([id, p]) => [id, { ...p }]));
  const goal = new Map([...positions.keys()].map((id) => [id, target(id)]));
  const c0 = { ...cam };
  if (!to && seam && r0 && r1 && b === r1 && (r0.x !== r1.x || r0.y !== r1.y)) to = { k: cam.k, x: cam.x - (r1.x - r0.x) * cam.k, y: cam.y - (r1.y - r0.y) * cam.k };
  const still = !to && [...goal].every(([id, p]) => { const f = from.get(id); return f && Math.abs(f.x - p.x) < .5 && Math.abs(f.y - p.y) < .5; })
    && (!r0 && !r1 || r0 && r1 && Math.abs(r0.w - r1.w) < .5 && Math.abs(r0.h - r1.h) < .5 && Math.abs(r0.x - r1.x) < .5 && Math.abs(r0.y - r1.y) < .5);
  if (still) return Promise.resolve();
  if (b) placeCard(b);
  return animate("glide", T, (e) => {
    for (const [id, p] of goal) {
      const f = from.get(id) || p;
      const s = { x: lerp(f.x, p.x, e), y: lerp(f.y, p.y, e), w: p.w, h: p.h };
      shown.set(id, s);
      const a = nodeEls.get(id);
      if (a) a.style.transform = `translate(${s.x}px, ${s.y}px)`;
    }
    if (to) cam = { k: lerp(c0.k, to.k, e), x: lerp(c0.x, to.x, e), y: lerp(c0.y, to.y, e) };
    if (r0 && r1 && b) { cardShown = lerpRect(r0, r1, e); clipCard(cardShown, b); }
  });
}
