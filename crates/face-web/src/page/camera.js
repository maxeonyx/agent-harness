import { $, el, lerp, panel, phone, reduced } from "./base.js";
import { byId, centre, drawn, graph, look, positions, shown, view } from "./map.js";
import { blobs, dot, drawEdges, drawNodes, hitAt, openAt, placeMarks, refresh } from "./draw.js";
import { onFrame } from "./blobs.js";
import { hideTip } from "./hover.js";
import { closeNode, go, NEW, openId, saved } from "./card.js";
import { glideCut, glideNote } from "./glidelog.js";
import { zoomed } from "./regions.js";

// ─── The camera ────────────────────────────────────────────────────────────
const map = $("#map"), world = $("#world"), under = $("#under"), grid = $("#grid");
export const nodesLayer = $("#nodes"), edgesLayer = $("#edges");
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
  world.style.transform = under.style.transform = `translate(${x}px, ${y}px) scale(${cam.k})`;
  blobs?.camera(x, y, cam.k);
  map.classList.add("moving");
  clearTimeout(resting);
  resting = setTimeout(() => map.classList.remove("moving"), 150);
  const g = 24 * cam.k, size = `${g}px ${g}px`;
  if (grid.style.backgroundSize !== size) grid.style.backgroundSize = size;
  grid.style.transform = `translate(${(x % g + g) % g - g}px, ${(y % g + g) % g - g}px)`;
  placeMarks();
  zoomed();
  coverCheck();
  sleepCheck();
}
// The map is moved only by the camera, never by the browser scrolling it to
// show what has focus.
map.addEventListener("scroll", () => { map.scrollTop = 0; map.scrollLeft = 0; });
function bounds(ids) {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const [id, p] of positions) {
    if (!ids.has(id)) continue;
    x0 = Math.min(x0, p.x); y0 = Math.min(y0, p.y); x1 = Math.max(x1, p.x + p.w); y1 = Math.max(y1, p.y + p.h);
  }
  return x0 === Infinity ? null : { x0, y0, x1, y1 };
}
// The camera the map opens with, and fits to: at its own size, the present
// row a third of the way up from the bottom of the room the bottom bar
// leaves, and the most recently active chat in the middle, but never past
// the edge of what is drawn: across and down, all of the map that fits is
// centred, and a map bigger than the room fills it, an edge of the map
// short of the room's edge only where the map ends both ways. While it
// follows the map, it keeps to that chat.
let focus = null;
export function framing() {
  if (!drawn) return null;
  const big = [...graph.nodes.values()].filter((n) => n.big && positions.has(n.id));
  if (!big.some((n) => n.id === focus)) focus = latest(big);
  const p = focus && positions.get(focus);
  const bar = $("#bottom").getBoundingClientRect();
  const w = innerWidth, h = bar.height ? bar.top : innerHeight, m = phone() ? 6 : 16;
  // The camera along one axis that puts `at` there, kept to the map from a to b.
  const keep = (at, a, b, room) => b - a <= room - 2 * m ? (room - a - b) / 2 : Math.min(m - a, Math.max(room - m - b, at));
  return {
    k: 1,
    x: Math.round(keep(p ? w / 2 - centre(p).x : Infinity, drawn.x0, drawn.x1, w)),
    y: Math.round(keep(p ? h * 2 / 3 : Infinity, drawn.y0, drawn.y1, h)),
  };
}
// The big node of the chat that, itself or through the agents under it, did
// something last; an agent's only when no chat is live.
function latest(big) {
  const when = new Map();
  const chat = (t) => { for (let i = 0; t.owner && byId[t.owner] && i < 50; i++) t = byId[t.owner]; return t; };
  for (const t of view.tasks) {
    const at = Math.max(t.waiting?.since || 0, ...(t.legs || []).map((l) => l.at));
    for (const k of [t.id, chat(t).id]) when.set(k, Math.max(when.get(k) ?? 0, at));
  }
  const chats = big.filter((n) => !n.t.owner);
  let best = null;
  for (const n of chats.length ? chats : big) if (!best || (when.get(n.t.id) ?? 0) > (when.get(best.t.id) ?? 0)) best = n;
  return best?.id ?? null;
}
export function fit(fly = true, keep = false) {
  if (!keep) focus = null;
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
// A drag let go while moving coasts on, slowing to a stop; a press stops it
// where it is, and is only that, not a click on what it pressed. A drag by
// the card's header moves the card like a window, and stops where let go:
// on a phone the card fills the screen, and a coast would throw it off.
const pointers = new Map();
export let drag = null;
let caught = false;  // the press stopped a coast
const cards = (e) => e.target.closest?.("#panel") && !asleep && (!e.target.closest("#panel > .bar") || e.target.closest("button, [contenteditable]"));
map.addEventListener("pointerdown", (e) => {
  if (e.button !== 0 && e.pointerType === "mouse") return;
  caught = stop("coast");
  if (cards(e)) return;
  pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
  drag = { moved: caught, start: { x: e.clientX, y: e.clientY }, cam: { ...cam }, pinch: pointers.size === 2 ? pinchOf() : null, trail: [], card: !asleep && !!e.target.closest?.("#panel") };
});
// A touch's click can come after its drag is forgotten.
map.addEventListener("click", (e) => { if (caught) { caught = false; e.stopPropagation(); e.preventDefault(); } }, true);
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
    // Back to the last sample from 100 ms ago, for the speed it is let go at:
    // every sample the screen took, not only the one a slow frame was given.
    const samples = e.getCoalescedEvents?.() ?? [];
    for (const s of samples.length ? samples : [e]) drag.trail.push({ t: s.timeStamp, x: s.clientX, y: s.clientY });
    while (drag.trail.length > 1 && e.timeStamp - drag.trail[1].t >= 100) drag.trail.shift();
    if (!drag.panning && Math.hypot(dx, dy) < 5) return;
    if (!drag.panning) {
      // Only now is it a drag: a press that does not move stays a click on what it pressed.
      drag.moved = drag.panning = true;
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
  if (!pointers.delete(e.pointerId)) return;
  if (!pointers.size) {
    map.classList.remove("grabbing");
    // A touch that dragged is not a tap, so no click comes to forget it.
    if (drag?.panning) caught = false;
    if (drag?.panning) {
      const c = drag.card ? { no: "dragged the card" } : e.type === "pointerup" ? coast(drag.trail, e.timeStamp) : { no: "the browser took the touch" };
      glideNote(e, drag.trail, c);
    }
    setTimeout(() => { drag = null; }, 0);
  }
  else if (drag) { drag.pinch = null; drag.trail = []; const p = [...pointers.values()][0]; drag.start = p; drag.cam = { ...cam }; }
};
map.addEventListener("pointerup", lift);
map.addEventListener("pointercancel", lift);
function pinchOf() { const [a, b] = [...pointers.values()]; return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2, d: Math.max(10, Math.hypot(a.x - b.x, a.y - b.y)) }; }
map.addEventListener("wheel", (e) => {
  if (e.target.closest("#panel") && !e.ctrlKey && !asleep) return;
  e.preventDefault();
  stop("coast");
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
export let closing = 0;
let tapped = null;
// By the path the event took, not its target: a button its own click redraws
// is no longer in the card when the click reaches the map.
const ground = (e) => !drag?.moved && !e.composedPath().some((n) => n.matches?.(".node, #panel, .offscreen"));
map.addEventListener("click", (e) => {
  if (!ground(e) || e.detail > 1) return;
  const hit = hitAt(e.clientX, e.clientY);
  if (hit) { clearTimeout(closing); openAt(hit); return; }
  if (!openId) return;
  clearTimeout(closing);
  closing = setTimeout(closeNode, 220);
});
map.addEventListener("dblclick", (e) => { if (ground(e) && !hitAt(e.clientX, e.clientY)) newHere(e.clientX, e.clientY); });
map.addEventListener("pointerup", (e) => {
  if (e.pointerType !== "touch" || !ground(e) || hitAt(e.clientX, e.clientY)) return;
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
// node's top, centred on it. The map makes room for it without being laid
// out again. Lines from the node's four corners out to the same corners of
// infinity cut the map into four: what is above stays, what is left moves
// left, what is right moves right, and what is below moves down, each by
// as much as the card needs. The lines bend round every project but the
// one the node is in: another project goes whole, the way most of it lies,
// so it keeps its shape.
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
// A chat's card is as tall as the window; a new chat's, as what it holds at
// its width, so its fields can wrap and its words grow without spilling.
function cardSize(id) {
  const m = phone() ? 6 : 16;
  const W = Math.min(id === NEW ? 640 : 880, innerWidth - 2 * m - controlsRoom());
  const H = id === NEW ? contentHeight(W) : innerHeight - 2 * m;
  return { W, H };
}
function contentHeight(W) {
  panel.style.width = `${W}px`;
  return [...panel.children].reduce((h, c) => h + c.offsetHeight, panel.offsetHeight - panel.clientHeight);
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
  const d = moves().get(id);
  return d ? { ...p, x: p.x + d.x, y: p.y + d.y } : p;
}
// How far each node moves for the open card: by the quarter its box lies
// in, or its whole project's if the project is clear of the node: the one
// it is farthest into past the node's edges.
let moved = { seam: null, positions: null, by: new Map() };
function moves() {
  if (moved.seam === seam && moved.positions === positions) return moved.by;
  const { o, sx, sy } = seam;
  const away = (b) => {
    const left = o.x - b.x1, right = b.x0 - (o.x + o.w), up = o.y - b.y1, down = b.y0 - (o.y + o.h);
    const most = Math.max(left, right, up, down);
    return most === up ? null : most === down ? { x: 0, y: sy } : { x: most === left ? -sx : sx, y: 0 };
  };
  const by = new Map();
  for (const c of graph.comps) {
    const ids = c.nodes.map((n) => n.id).filter((id) => positions.has(id));
    if (!ids.length) continue;
    const b = bounds(new Set(ids));
    const cut = c.nodes.some((n) => n.id === seam.id);
    for (const id of ids) {
      const p = positions.get(id);
      const d = id !== seam.id && away(cut ? { x0: p.x, y0: p.y, x1: p.x + p.w, y1: p.y + p.h } : b);
      if (d) by.set(id, d);
    }
  }
  moved = { seam, positions, by };
  return by;
}
// A node's dot, or a new chat's point: what a card opens from and closes to.
export function dotRect(id) {
  if (id === NEW) { const p = newPoint || centrePoint(); return { x: p.x - 10, y: p.y - 14, w: 20, h: 20 }; }
  const p = shown.get(id) || positions.get(id);
  if (!p) return null;
  if (p.card) return { x: p.x, y: p.y, w: p.w, h: p.h };
  const c = centre(p);
  return { x: c.x - 10, y: c.y - 10, w: 20, h: 20 };
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
// When the card would sit under the floating controls, those at the right
// hide, and the usage and the build at the left join the card's bottom bar.
const bottom = $("#bottom"), corner = $("#corner");
let cornerRight = 0;
export function coverCheck() {
  if (corner.parentElement === bottom) cornerRight = corner.getBoundingClientRect().right;
  let covered = false;
  if (openId && cardShown) {
    const l = cam.x + cardShown.x * cam.k, r = l + cardShown.w * cam.k;
    const t = cam.y + cardShown.y * cam.k, b = t + cardShown.h * cam.k;
    covered = b > innerHeight - 60 && t < innerHeight && (l < cornerRight + 24 || r > innerWidth - 120);
  }
  document.body.classList.toggle("covered", covered);
  const dock = covered ? $(".compose .dock", panel) : null;
  if (dock && corner.parentElement !== dock) dock.append(corner);
  if (!dock && corner.parentElement !== bottom) bottom.prepend(corner);
}

// The card fits the window it is in, a phone's keyboard opening shrinks it,
// and a new chat's card grows and shrinks with what it holds.
addEventListener("resize", () => { reseat(); refresh(); });
new ResizeObserver(reseat).observe($(".compose", panel));
function reseat() {
  const next = openId && seam && seamFor(openId);
  if (!next || (next.W === seam.W && next.H === seam.H)) return;
  const was = cardShown;
  seam = next;
  const r = cardRect(seam);
  placeCard(r);
  cardShown = r;
  cam = camFor(r, was && { x: r.x + r.w / 2, y: r.y, sx: cam.x + (was.x + was.w / 2) * cam.k, sy: cam.y + was.y * cam.k });
  applyCamera();
  refresh();
}

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
      wakeLabel.replaceChildren(dot(el("i"), t), t ? t.name : "New chat");
    }
  }
  if (sleep === asleep) return;
  asleep = sleep;
  panel.classList.toggle("asleep", sleep);
}

// ─── Motion ────────────────────────────────────────────────────────────────
// One frame loop moves all that moves: the nodes, the card, the camera; the
// marks at the screen's edges follow each frame, and the nodes and lines do
// while what moves is more than the camera (`nodes`). It runs at the start
// of the canvas's frame (blobs.js), so the canvas draws the camera the
// layers have.
const anims = new Map();
let looping = false;
const easeOut = (u) => 1 - Math.pow(1 - u, 3);
function animate(name, T, step, nodes = true, ease = easeOut) {
  anims.get(name)?.done();
  if (document.hidden) { anims.delete(name); step(1); drawNodes(); drawEdges(); applyCamera(); return Promise.resolve(); }
  return new Promise((done) => {
    anims.set(name, { t0: performance.now(), T, step, done, nodes, ease });
    if (!looping) { looping = true; onFrame(frame); }
  });
}
function frame(now) {
  let nodes = false;
  for (const [name, a] of anims) {
    const u = a.T ? Math.max(0, Math.min(1, (now - a.t0) / a.T)) : 1;
    a.step(a.ease(u));
    nodes ||= a.nodes;
    if (u >= 1) { anims.delete(name); a.done(); }
  }
  if (nodes) { drawNodes(); drawEdges(); }
  applyCamera();
  looping = anims.size > 0;
  return looping;
}
// Stops a motion where it is; whether it was moving.
function stop(name) {
  const a = anims.get(name);
  if (!a) return false;
  if (name === "coast") glideCut();
  anims.delete(name);
  a.done();
  return true;
}
// A drag let go goes on at the speed it had over its last 100 ms, unless it
// held still for the 80 ms before its lift (the rules Chromium's own fling
// goes by: a touch screen can be that slow to tell of a lift), slowed by
// drag (the speed decays by e every TAU) and by friction (a steady FRICTION
// off it), until it stops. It carries on the drag Max made, as the phone's
// own scrolling does, so it coasts with reduced motion too. Drag
// carries a fast flick far and friction stops a slow one soon, as a phone's
// own scrolling does, going a little less far:
// a flick at 1, 3 and 6 px/ms goes about 170, 1200 and 3600 px, in 0.35,
// 0.9 and 1.4 s, where Chromium on Android goes 190, 1300 and 4400.
const TAU = 1500, FRICTION = .0025;  // ms, px/ms²
// It returns what it made of the trail, for glidelog.js.
function coast(trail, t) {
  const a = trail[0], b = trail.at(-1);
  if (a === undefined || a === b) return { no: "too few samples" };
  if (t - b.t > 80) return { no: "held still before the lift" };
  const dt = Math.max(16, b.t - a.t), vx = (b.x - a.x) / dt, vy = (b.y - a.y) / dt;
  const v = Math.hypot(vx, vy);
  if (v < .1) return { no: "too slow", v };
  // The speed is (v + f) e^(-t/TAU) - f, with f = FRICTION TAU: none at T.
  const f = FRICTION * TAU, T = TAU * Math.log(1 + v / f);
  const along = (t) => TAU * (v + f) * (1 - Math.exp(-t / TAU)) - f * t;
  const D = along(T), c0 = { ...cam }, t0 = performance.now();
  let frames = 0;
  const ran = animate("coast", T, (e) => { frames++; cam = { ...cam, x: c0.x + vx / v * D * e, y: c0.y + vy / v * D * e }; }, false, (u) => along(u * T) / D)
    .then(() => ({ frames, took: performance.now() - t0 }));
  return { v, D, T, ran };
}
// Everything that is moving, where it is going, now.
document.addEventListener("visibilitychange", () => { if (document.hidden) settle(); });
function settle() {
  if (anims.has("coast")) glideCut();
  for (const a of anims.values()) { a.step(1); a.done(); }
  anims.clear();
  drawNodes();
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
  if (to) stop("coast");
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
  return animate("glide", reduced() ? 0 : T, (e) => {
    for (const [id, p] of goal) {
      const f = from.get(id) || p;
      shown.set(id, { ...p, x: lerp(f.x, p.x, e), y: lerp(f.y, p.y, e) });
    }
    if (to) cam = { k: lerp(c0.k, to.k, e), x: lerp(c0.x, to.x, e), y: lerp(c0.y, to.y, e) };
    if (r0 && r1 && b) { cardShown = lerpRect(r0, r1, e); clipCard(cardShown, b); }
  });
}
