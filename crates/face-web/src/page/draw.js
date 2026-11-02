import { $, el, svgEl, when } from "./base.js";
import { BIG, byId, centre, derive, glyph, graph, isDone, label, layout, look, positions, radius, routes, shortName, shown, successor, unfolded, view } from "./map.js";
import { cam, drag, edgesLayer, fit, framed, glide, nodesLayer, seam, seamFor, setFramed, setSeam } from "./camera.js";
import { ICON, layer, MARK, session } from "./blobs.js";
import { curve, stroke, taper } from "./layout.js";
import { fillTip, hoverable, tipFor } from "./hover.js";
import { notify, renderUsage } from "./chrome.js";
import { renderBuilds } from "./builds.js";
import { go, goStaying, moving, NEW, openId, renderPanel } from "./card.js";
import { renderOthers, syncCompose } from "./compose.js";
import { finding, found } from "./newchat.js";

// ─── Drawing the map ───────────────────────────────────────────────────────
// Three layers, moved by the camera together: the lines, in one SVG under
// everything (#under); the nodes, on one canvas (blobs.js: a blob for each
// big node, a mark for every other); and, over them in the world, each big
// node's and running chip's name and the place it is pressed, and the open
// card. A small dot has no element: a press on the ground finds it (`hitAt`).
export const nodeEls = new Map();
export const blobs = layer($("#map"));
const sessions = new Map(), handles = new Map();
// A node's blob session: one per task, kept while it shows.
function sessionOf(n) {
  let s = sessions.get(n.t.id);
  if (!s) sessions.set(n.t.id, s = session(n.big));
  const want = n.kind === "planned" ? "planned" : trouble(n.t) ? "held" : n.big;
  if (s.state !== want) s.set(want);
  s.letter = n.kind === "planned" ? String(n.waits) : glyph(n.t);
  s.pips = n.pips ?? 0;
  return s;
}

export function drawMap() {
  for (const [id, a] of nodeEls) if (!positions.has(id) || !elOf(id)) { a.remove(); nodeEls.delete(id); }
  for (const [id, h] of handles) if (!positions.has(id) || !graph.nodes.get(id)?.big || positions.get(id).card) { h.remove(); handles.delete(id); }
  for (const id of shown.keys()) if (!positions.has(id)) shown.delete(id);
  const folded = new Map(graph.comps.filter((c) => c.folded).map((c) => [c.root.id, c]));
  for (const [id, p] of positions) {
    const n = graph.nodes.get(id);
    // A new node grows out of what it hangs from.
    if (!shown.has(id)) {
      const from = n?.deps.map((d) => shown.get(d)).find(Boolean);
      shown.set(id, from && !p.card ? { ...p, x: from.x + from.w / 2 - p.w / 2, y: from.y + from.top - p.top } : { ...p });
    }
    const faded = finding && !found.has(n?.t.id ?? id) ? " unmatched" : "";
    if (n?.big && !p.card) {
      const s = sessionOf(n);
      if (!handles.has(id) && blobs) handles.set(id, blobs.put(BIG, s));
      handles.get(id)?.set(s);
    }
    const kind = elOf(id);
    if (!kind) continue;
    let a = nodeEls.get(id);
    if (!a) {
      a = el(kind === "work" ? "div" : "a", "node");
      if (kind === "card") a.append(el("span", "name"));
      else a.append(el("span", "hit"), el("span", "name"));
      if (kind === "work") {
        const g = el("span", "g");
        g.innerHTML = `<svg viewBox="0 0 24 24"><path d="${ICON.tools.path}"/></svg>`;
        a.append(g);
        a.addEventListener("click", () => { if (!drag?.moved) openAt(graph.nodes.get(id)); });
      } else {
        a.href = `#${id}`;
        a.addEventListener("click", (e) => { if (e.button || e.ctrlKey || e.metaKey || e.shiftKey) return; e.preventDefault(); if (!drag?.moved) go(id); });
      }
      hoverable(a, id);
      nodesLayer.append(a);
      nodeEls.set(id, a);
    }
    if (kind === "card") {
      const fc = folded.get(id), t = byId[id];
      a.className = `node archived${look(t).cls.includes("failed") ? " failed" : ""}${faded}`;
      a.style.width = `${p.w}px`;
      $(".name", a).textContent = shortName(t);
      let more = $(".more", a);
      if (fc.tasks.length > 1) {
        if (!more) { a.append(more = el("button", "more", "⌄")); more.title = "Unfold the project"; more.addEventListener("click", (e) => { e.preventDefault(); e.stopPropagation(); unfolded.add(id); refresh(); }); }
      } else more?.remove();
    } else {
      a.className = `node ${kind}${n.work?.handed ? " yours" : ""}${id === openId ? " selected" : ""}${faded}`;
      $(".name", a).textContent = label(n);
    }
  }
  drawNodes();
  if (tipFor) fillTip();
}
// Which element a node has: a card, a big node's, a running chip's (`work`:
// card.css styles `.chip`), or none.
function elOf(id) {
  const p = positions.get(id), n = graph.nodes.get(id);
  return p?.card ? "card" : n?.big ? "big" : n?.kind === "chip" && n.running ? "work" : null;
}

// Every node where it is shown now: the elements moved, the blobs placed
// and the marks written for the canvas.
let marks = new Float32Array(0);
const rgb = (r, g, b) => [r / 255, g / 255, b / 255];
const INK = rgb(20, 20, 19), PAPER = rgb(250, 249, 245), YOURS = rgb(212, 105, 28), GREEN = rgb(79, 138, 85), RED = rgb(179, 53, 44);
export function drawNodes() {
  if (marks.length < MARK * shown.size) marks = new Float32Array(MARK * shown.size * 2);
  let k = 0;
  for (const [id, s] of shown) {
    const n = graph.nodes.get(id), c = centre(s), a = nodeEls.get(id);
    if (a) a.style.transform = s.card ? `translate(${s.x}px, ${s.y}px)` : `translate(${c.x}px, ${c.y}px)`;
    const h = handles.get(id);
    if (h) { h.at(c.x, c.y); h.hide(id === openId); }
    if (!n || n.big || s.card) continue;
    const dim = finding && !found.has(n.t.id) ? .12 : 1;
    const m = k++ * MARK;
    marks[m] = c.x; marks[m + 1] = c.y; marks[m + 2] = radius(n);
    if (n.kind === "leg") {
      // The own node of a finished task Max started (a chat, an aside) in
      // its outcome's colour, so it stands out from the agents' finished
      // legs; any other leg ink.
      const o = id === n.t.id && isDone(n.t) && !n.t.started_by ? n.t.outcome?.kind : null;
      const fill = o === "failed" ? RED : o && o !== "abandoned" ? GREEN : INK;
      marks.set(fill, m + 4); marks[m + 7] = (n.old ? .15 : fill === INK ? .42 : .85) * dim;
      marks[m + 11] = 0;
    } else if (n.kind === "planned") {
      // A planned join: paper, with a dotted ring.
      marks[m + 3] = 1.3;
      marks.set(PAPER, m + 4); marks[m + 7] = dim;
      marks.set(INK, m + 8); marks[m + 11] = .5 * dim;
      marks[m + 12] = Math.round((2 * Math.PI * radius(n)) / 3.2);
    } else if (n.running) {
      // Running background work: paper, ringed, under its gear.
      marks[m + 3] = 1.4;
      marks.set(PAPER, m + 4); marks[m + 7] = dim;
      marks.set(n.work.handed ? YOURS : INK, m + 8); marks[m + 11] = .7 * dim;
      marks[m + 12] = 0;
    } else {
      // Finished background work: a faint small dot.
      marks.set(INK, m + 4); marks[m + 7] = .25 * dim;
      marks[m + 11] = 0;
    }
  }
  blobs?.marks(marks, k);
}

// The small node at a point on the screen, if any: within 12 px of it.
export function hitAt(sx, sy) {
  const x = (sx - cam.x) / cam.k, y = (sy - cam.y) / cam.k, r = 12 / cam.k;
  let best = null, bd = r;
  for (const [id, s] of shown) {
    const n = graph.nodes.get(id);
    if (!n || n.big || s.card || nodeEls.has(id)) continue;
    const c = centre(s), d = Math.hypot(c.x - x, c.y - y);
    if (d < bd) { bd = d; best = n; }
  }
  return best;
}
// A small node opens its task's chat where its stretch begins: a leg at its
// first entry, a chip or planned join at the call that started its work.
export function openAt(n) {
  if (!n) return;
  go(n.t.id, n.open || null);
}

// The lines: an edge's route as laid out, bent to follow its two ends
// wherever they are shown, each point moving as the end nearer it does.
// Each kind of line is one path of all of them: the tapers filled solid in
// one fill (each wound the same way, so where they overlap they add up and
// don't darken), old versions' in another, and the dashed ones stroked.
function line(key, r) {
  const [from, to] = key.split(">");
  const move = (id) => { const p = positions.get(id), s = shown.get(id); return s && p ? [s.x - p.x, s.y - p.y] : [0, 0]; };
  const m0 = move(from), m1 = move(to), still = !m0[0] && !m0[1] && !m1[0] && !m1[1];
  if (still && r.d) return r.d;
  const y0 = r.pts[0][1], y1 = r.pts[r.pts.length - 1][1];
  const pts = still ? r.pts : r.pts.map(([x, y]) => {
    const u = y1 === y0 ? 0 : Math.max(0, Math.min(1, (y - y0) / (y1 - y0)));
    return [x + m0[0] + (m1[0] - m0[0]) * u, y + m0[1] + (m1[1] - m0[1]) * u];
  });
  const segs = curve(pts), d = r.dashed ? stroke(segs) : taper(segs, 2.2, .5);
  if (still) r.d = d;
  return d;
}
const paths = {};
export function drawEdges() {
  if (!paths.live) for (const k of ["old", "live", "dash"]) { paths[k] = svgEl("path"); paths[k].setAttribute("class", k); paths[k].d = ""; edgesLayer.append(paths[k]); }
  const ds = { old: [], live: [], dash: [] };
  for (const [key, r] of routes) ds[r.dashed ? "dash" : r.old ? "old" : "live"].push(line(key, r));
  for (const k in ds) {
    const d = ds[k].join("");
    if (paths[k].d !== d) { paths[k].d = d; paths[k].setAttribute("d", d); }
  }
}

// Why a task is stuck, while it matters: a place it starts in that could
// not be made, or its last turn not responding.
export function trouble(t) {
  const failed = t.waiting?.kind === "starting" ? t.waiting.failed || [] : [];
  if (failed.length) return `${failed.map((f) => `Couldn't make ${f.limb}: ${f.why}.`).join(" ")} The harness keeps trying: abandon it, or start again elsewhere.`;
  const x = t.trouble;
  if (!x || isDone(t)) return null;
  if (x.until != null) return x.until > Date.now() ? `Limited until ${when(x.until)}` : null;
  return t.failures ? `Failed ${t.failures === 1 ? "once" : `${t.failures} times`}: ${x.text}` : null;
}

// ─── The whole page, from the view ─────────────────────────────────────────
let shownBefore = false;
// The view folded into the map: what it is, where everything goes.
export async function rebuild() {
  derive(openId);
  const changed = layout();
  if (openId) setSeam(seamFor(openId) || seam);
  drawMap();
  return changed;
}
// The map redrawn from the view. It resolves once a layout that began after
// the call has landed, so a caller can rely on what this view put there:
// a call while one runs waits for the one after it, which all such calls share.
let running = null, queued = null;
export function refresh() {
  if (!view) return Promise.resolve();
  if (running) return queued ??= running.catch(() => {}).then(() => { queued = null; return refresh(); });
  running = (async () => {
    try {
      // A card opening or closing finishes before the map moves under it.
      await moving;
      const changed = await rebuild();
      glide(null, { T: shownBefore ? 420 : 0 });
      if (changed && framed && !openId) fit(shownBefore, true);
      shownBefore = true;
      render();
    } finally {
      running = null;
    }
  })();
  return running;
}

export function render() {
  if (!view) return;
  $("#empty").hidden = view.tasks.length > 0 || !!openId;
  renderUsage();
  renderBuilds();
  const q = view.queue || [];
  document.title = q.length ? `(${q.length}) Harness` : "Harness";
  notify(q);
  if (openId && openId !== NEW && successor(openId) !== openId) goStaying(successor(openId));
  else if (openId) renderPanel();
  syncCompose();
  renderOthers();
  placeMarks();
}

// ─── What needs Max, off screen ────────────────────────────────────────────
// What waits on Max where the camera does not show it is a mark at the
// screen's edge, a pin whose point is toward it, the next one haloed; a
// click brings it to the middle of the screen, where a click on it opens it.
const marksEl = $("#marks"), markEls = new Map();
export function placeMarks() {
  if (!view) return;
  // The marks run round a rectangle inset from the screen, clear of the
  // bottom bar; each sits where the line from the middle to its node crosses
  // it, so it slides as the camera moves, round corners too.
  const m = 20, l = m, t = m, r = innerWidth - m, b = innerHeight - m - 40;
  const w = r - l, h = b - t, P = 2 * (w + h), ox = innerWidth / 2, oy = (t + b) / 2;
  const marks = [];
  (view.queue || []).forEach((q, i) => {
    const id = q.task, p = shown.get(id);
    if (!p || id === openId) return;
    const c = centre(p), x = cam.x + c.x * cam.k, y = cam.y + c.y * cam.k;
    if (x >= 0 && x <= innerWidth && y >= 0 && y <= innerHeight) return;
    const dx = x - ox, dy = y - oy, f = Math.min(Math.abs(w / 2 / dx), Math.abs(h / 2 / dy));
    const ex = ox + dx * f, ey = oy + dy * f;
    // Distance along the perimeter, clockwise from the top left corner.
    const s = Math.abs(ey - t) < 0.5 ? ex - l : Math.abs(ex - r) < 0.5 ? w + ey - t : Math.abs(ey - b) < 0.5 ? w + h + r - ex : 2 * w + h + b - ey;
    marks.push({ id, x, y, s, d: Math.hypot(dx, dy), next: i === 0 });
  });
  // Neighbours closer than a mark's width push each other apart evenly, so a
  // group spreads and moves as one. Nodes in line with the middle, as a
  // column is when one of it is centred, go in order of distance.
  marks.sort((u, v) => (Math.abs(u.s - v.s) < 0.5 ? u.d - v.d : u.s - v.s));
  const n = marks.length, gap = Math.min(30, P / Math.max(n, 1));
  const apart = (i) => marks[(i + 1) % n].s - marks[i].s + (i === n - 1 ? P : 0);
  for (let k = 0; n > 1 && k < 16 * n; k++) {
    let moved = false;
    for (let i = 0; i < n; i++) {
      const u = marks[i], v = marks[(i + 1) % n], d = apart(i);
      if (d < gap - 0.01) { const e = (gap - d) / 2; u.s -= e; v.s += e; moved = true; }
    }
    if (!moved) break;
  }
  const at = (s) => {
    s = ((s % P) + P) % P;
    return s < w ? [l + s, t] : s < w + h ? [r, t + s - w] : s < 2 * w + h ? [r - (s - w - h), b] : [l, b - (s - 2 * w - h)];
  };
  // A mark never covers what Max presses: one under the open card or the
  // bottom bar's buttons fades out.
  const ui = [...document.querySelectorAll("#panel:not([hidden]), #bottom > :not(.spacer)")]
    .map((e) => e.getBoundingClientRect()).filter((r) => r.width > 0);
  const seen = new Set();
  for (const { id, x, y, s, next } of marks) {
    seen.add(id);
    let a = markEls.get(id);
    if (!a) {
      a = el("button", "offscreen");
      a.type = "button";
      // The button's circle, r 12, and a point 6 beyond it on the right,
      // meeting the circle where a line from the point touches it.
      a.innerHTML = '<svg viewBox="-18 -18 36 36"><path d="M18 0 8-8.944A12 12 0 1 0 8 8.944z"/><circle r="3.5"/></svg>';
      a.onclick = () => centreOn(id);
      marksEl.append(a);
      markEls.set(id, a);
    }
    a.title = byId[id]?.name || "";
    a.classList.toggle("next", next);
    const [cx, cy] = at(s);
    a.classList.toggle("under", ui.some((u) => cx > u.left - 12 && cx < u.right + 12 && cy > u.top - 12 && cy < u.bottom + 12));
    a.style.transform = `translate(${cx}px, ${cy}px)`;
    $("svg", a).style.transform = `rotate(${Math.atan2(y - cy, x - cx)}rad)`;
  }
  for (const [id, a] of markEls) if (!seen.has(id)) { a.remove(); markEls.delete(id); }
}
// The camera, at the zoom it has, moved to put a node in the middle.
function centreOn(id) {
  const p = shown.get(id);
  if (!p) return;
  setFramed(false);
  const c = centre(p);
  glide({ k: cam.k, x: innerWidth / 2 - c.x * cam.k, y: innerHeight / 2 - c.y * cam.k }, { T: 420 });
}
