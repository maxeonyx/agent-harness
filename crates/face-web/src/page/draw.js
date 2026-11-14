import { $, el, svgEl, when } from "./base.js";
import { BIG, byId, centre, derive, glyph, graph, isDone, label, layout, LINE, look, positions, radius, remeasure, routes, shortName, shown, stateOf, successor, unfolded, view, waitingOn } from "./map.js";
import { cam, drag, edgesLayer, fit, framed, glide, nodesLayer, seam, seamFor, setFramed, setSeam } from "./camera.js";
import { ICON, layer, MARK, session, STATES } from "./blobs.js";
import { GAP, LIFE, outline, polyline } from "./layout.js";
import { fillTip, hoverable, tipFor } from "./hover.js";
import { notify, renderUsage } from "./chrome.js";
import { renderBuilds } from "./builds.js";
import { go, goStaying, moving, NEW, openId, renderPanel } from "./card.js";
import { renderOthers, syncCompose } from "./compose.js";
import { finding, found } from "./newchat.js";
import { paintRegions, placeRegions } from "./regions.js";

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
  return dress(s, n.t);
}
// What task t's blob shows, on its map node and in its chat's dock alike:
// its state, hollow with how many it waits on for its letter while its own
// node is a planned join waiting on its work, and its compactions; or
// `state` in place of its own.
export function dress(s, t, jump = false, state = stateFor(t)) {
  const n = graph.nodes.get(t.id);
  s.set(state, jump);
  s.hollow = n?.kind === "planned" && !n.solid;
  s.letter = s.hollow ? String(waitingOn(t).n) : glyph(t);
  s.pips = n?.pips ?? 0;
  return s;
}
function stateFor(t) { return trouble(t) ? "held" : stateOf(t); }
// The colours of task t's blob, for what shows its state as a dot: its fill,
// its edge and its letter.
function colours(t) {
  const [fill, edge, ink] = STATES[stateFor(t)].col;
  return { fill: `rgb(${fill})`, edge: `rgb(${edge})`, ink: `rgb(${ink})` };
}
// An element painted in task t's blob's colours, or cleared for none.
export function paint(d, t) {
  const c = t && colours(t);
  d.style.background = c?.fill || "";
  d.style.borderColor = c?.edge || "";
  d.style.color = c?.ink || "";
  return d;
}
// A task's state as a small dot (the card's header, the card asleep): its
// blob's colours, in a shape `look`'s classes give; a chat not started yet
// is a dashed ring.
export function dot(d, t) {
  d.className = `mini ${t ? look(t).cls : "starting"}`;
  return paint(d, t);
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
        g.innerHTML = `<svg viewBox="0 0 24 24"><path d="${ICON[graph.nodes.get(id).work.tool === "ask" ? "max" : "tools"].path}"/></svg>`;
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
      a.className = `node archived${faded}`;
      a.style.setProperty("--st", colours(t).fill);
      a.style.width = `${p.w}px`;
      $(".name", a).textContent = shortName(t);
      let more = $(".more", a);
      if (fc.tasks.length > 1) {
        if (!more) { a.append(more = el("button", "more", "⌄")); more.title = "Unfold the project"; more.addEventListener("click", (e) => { e.preventDefault(); e.stopPropagation(); unfolded.add(id); refresh(); }); }
      } else more?.remove();
    } else {
      a.className = `node ${kind}${n.yours ? " yours" : ""}${id === openId ? " selected" : ""}${faded}`;
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

// Every node where it is shown now: the elements moved, the blobs placed,
// the marks written for the canvas, and the areas' grounds under them.
let marks = new Float32Array(0);
const rgb = (r, g, b) => [r / 255, g / 255, b / 255];
const INK = rgb(20, 20, 19), PAPER = rgb(250, 249, 245), STRETCH = rgb(131, 129, 123), OLD = rgb(196, 194, 186), RUNNING = rgb(58, 109, 176);
// A ring is in the colour of the life it is on, one step darker.
const RING = { ink: rgb(143, 141, 134), tool: rgb(111, 138, 161), max: rgb(201, 124, 66) };
export function drawNodes() {
  if (marks.length < MARK * shown.size) marks = new Float32Array(MARK * shown.size * 2);
  let k = 0;
  for (const [id, s] of shown) {
    const n = graph.nodes.get(id), c = centre(s), a = nodeEls.get(id);
    if (a) a.style.transform = s.card ? `translate(${s.x}px, ${s.y}px)` : `translate(${c.x}px, ${c.y}px)`;
    const h = handles.get(id);
    if (h) { h.at(c.x, c.y); h.hide(id === openId); }
    // A finished call is a gear (drawEdges), and Max's running one a card.
    if (!n || n.big || s.card || (n.kind === "chip" && (!n.running || n.yours))) continue;
    const dim = finding && !found.has(n.t.id) ? .12 : 1;
    const m = k++ * MARK;
    marks[m] = c.x; marks[m + 1] = c.y; marks[m + 2] = radius(n);
    marks[m + 3] = 0; marks[m + 11] = 0; marks[m + 12] = 0;
    if (n.kind === "leg") {
      // A finished task's own node in its blob's colour, as its chat's dock
      // shows it; an old version's pale; any other the stretch's grey.
      const st = id === n.t.id && isDone(n.t) ? STATES[stateFor(n.t)].col[0] : null;
      marks.set(st ? rgb(...st) : n.old ? OLD : STRETCH, m + 4); marks[m + 7] = dim;
    } else if (n.kind === "planned") {
      // A planned join: paper, with a dotted ring.
      marks[m + 3] = 1.3;
      marks.set(PAPER, m + 4); marks[m + 7] = dim;
      marks.set(INK, m + 8); marks[m + 11] = .5 * dim;
      marks[m + 12] = Math.round((2 * Math.PI * radius(n)) / 3.2);
    } else {
      // Where work was passed on: a ring in its life's colour. Running work:
      // paper, ringed blue, under its gear.
      marks[m + 3] = 1.4;
      marks.set(PAPER, m + 4); marks[m + 7] = dim;
      marks.set(n.kind === "ring" ? RING[n.line] : RUNNING, m + 8); marks[m + 11] = dim;
    }
  }
  blobs?.marks(marks, k);
  eraseNames();
  placeRegions();
}

// Behind each name the lines are erased, softly, in the name's own shape a
// few px wider, so the ground under them shows and the name reads over
// crossing lines on any ground: its text in the lines' mask, set where the
// name is and as it is (map.css `.erase`). A name not shown (its card is
// open) or faded by a search erases nothing.
const erased = new Map();
function eraseNames() {
  begin();
  for (const [id, t] of erased) if (!nodeEls.has(id)) { t.remove(); erased.delete(id); }
  for (const [id, a] of nodeEls) {
    const s = shown.get(id), n = graph.nodes.get(id);
    const off = !s || s.card || !n || id === openId || (finding && !found.has(n.t.id));
    let t = erased.get(id);
    if (off) { t?.remove(); erased.delete(id); continue; }
    if (!t) { erased.set(id, t = svgEl("text")); t.at = ""; nameMask.append(t); }
    const c = centre(s), at = `${f2(c.x)},${f2(c.y)}`, text = label(n), cls = n.big ? "big" : "work";
    if (t.textContent !== text) t.textContent = text;
    if (t.getAttribute("class") !== cls) t.setAttribute("class", cls);
    if (t.at !== at) { t.at = at; t.setAttribute("x", f2(c.x)); t.setAttribute("y", f2(c.y)); }
  }
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
// A node opens its task's chat: its own node at the end, any other where
// its stretch begins: a leg at its first entry, a chip at the call that
// started its work.
export function openAt(n) {
  if (!n) return;
  go(n.t.id, n.id === n.t.id ? null : n.open || null);
}

// The lines: an edge's line as laid out, bent to follow its two ends
// wherever they are shown, each point moving as the end nearer it does.
// Lines of one colour and kind are one path: the tapers filled (each wound
// the same way, so where they overlap they fill as one), the lines of one
// width stroked, and the dashed ones each under one mask of dashes. The
// lines that pass under others are drawn first, by their sheet (layout.js),
// each sheet through one mask that erases from it a band GAP wider each
// side than each line over one of them, near where they cross. Then each
// finished call's gear. All of it is under one mask more, which erases it
// behind the names (`eraseNames`).
function bend(key) {
  const [from, to] = key.split(">"), r = routes.get(key);
  const move = (id) => { const p = positions.get(id), s = shown.get(id); return s && p ? [s.x - p.x, s.y - p.y] : [0, 0]; };
  const m0 = move(from), m1 = move(to);
  if (!r || (!m0[0] && !m0[1] && !m1[0] && !m1[1])) return null;
  const y0 = r.P[0][1], y1 = r.P.at(-1)[1];
  return ([x, y]) => {
    const u = y1 === y0 ? 0 : Math.max(0, Math.min(1, (y - y0) / (y1 - y0)));
    return [x + m0[0] + (m1[0] - m0[0]) * u, y + m0[1] + (m1[1] - m0[1]) * u];
  };
}
function line(key, r) {
  const at = bend(key), ats = r.gaps.map((g) => bend(g.over)), still = !at && ats.every((a) => !a);
  if (still && r.d) return r;
  const P = at ? r.P.map(at) : r.P, out = { d: r.W ? outline(P, r.W) : polyline(P), mid: r.dashed ? polyline(P) : "", s: "", f: "" };
  r.gaps.forEach((g, i) => {
    const Q = ats[i] ? g.P.map(ats[i]) : g.P;
    if (g.W) out.f += outline(Q, g.W.map((w) => w + 2 * GAP));
    else out.s += polyline(Q);
  });
  if (still) Object.assign(r, out);
  return out;
}
const paths = new Map(), sheets = [];
let dashes = null, lines = null, nameMask = null;
function pathFor(k, attrs, parent = lines) {
  let p = paths.get(k);
  if (!p) {
    p = svgEl("path");
    for (const [a, v] of Object.entries(attrs)) p.setAttribute(a, v);
    p.d = "";
    paths.set(k, p);
    parent.append(p);
  }
  return p;
}
const ink = (r) => (r.W ? { fill: r.colour } : { fill: "none", stroke: r.colour, "stroke-width": LIFE, "stroke-linejoin": "round" });
const everywhere = { x: -1e5, y: -1e5, width: 2e5, height: 2e5 };
// A sheet: its mask, white but for its bands (paths `Ns` and `Nf`), and
// its two groups through it, the dashed one inside the mask of dashes;
// stacked as the lines over them are.
function sheet(n) {
  if (!sheets[n]) {
    const mask = svgEl("mask"), rect = svgEl("rect"), g = svgEl("g"), dg = svgEl("g");
    for (const [a, v] of Object.entries({ id: `sheet${n}`, maskUnits: "userSpaceOnUse", ...everywhere })) mask.setAttribute(a, v);
    for (const [a, v] of Object.entries({ fill: "#fff", ...everywhere })) rect.setAttribute(a, v);
    mask.append(rect);
    edgesLayer.querySelector("defs").append(mask);
    pathFor(`${n}s`, { fill: "none", stroke: "#000", "stroke-width": LIFE + 2 * GAP }, mask);
    pathFor(`${n}f`, { fill: "#000" }, mask);
    for (const e of [g, dg]) e.setAttribute("mask", `url(#sheet${n})`);
    dashes.g.after(g);
    dashes.g.prepend(dg);
    for (const c of Object.values(LINE)) pathFor(`${n}f${c}`, { fill: c }, g);
    for (const c of Object.values(LINE)) pathFor(`${n}s${c}`, ink({ colour: c }), g);
    sheets[n] = { g, dg };
  }
  return sheets[n];
}
// The lines' SVG, made once: the mask of dashes, the mask of names (white
// but for the names, blurred), and the group of everything under it.
function begin() {
  if (dashes) return;
  const all = 'maskUnits="userSpaceOnUse" x="-1e5" y="-1e5" width="2e5" height="2e5"';
  edgesLayer.innerHTML = `<defs><mask id="dashes" ${all}><path fill="none" stroke="#fff" stroke-width="8" stroke-dasharray="3.5 3"/></mask>`
    + `<filter id="soft" x="-.2" y="-1" width="1.4" height="3"><feGaussianBlur stdDeviation="1.5"/></filter>`
    + `<mask id="names" ${all}><rect fill="#fff" x="-1e5" y="-1e5" width="2e5" height="2e5"/><g class="erase" filter="url(#soft)"></g></mask></defs>`
    + '<g mask="url(#names)"><g mask="url(#dashes)"></g></g>';
  dashes = { mask: edgesLayer.querySelector("#dashes path"), g: edgesLayer.querySelector('[mask="url(#dashes)"]'), d: "" };
  lines = edgesLayer.querySelector('[mask="url(#names)"]');
  nameMask = edgesLayer.querySelector(".erase");
  // Stacked in this order: the sheets, fills, strokes, gears.
  for (const c of Object.values(LINE)) pathFor(`f${c}`, { fill: c });
  for (const c of Object.values(LINE)) pathFor(`s${c}`, { fill: "none", stroke: c, "stroke-width": LIFE, "stroke-linejoin": "round" });
  for (const c of Object.values(GEAR)) pathFor(`g${c}`, { fill: c, "fill-rule": "evenodd" });
}
export function drawEdges() {
  begin();
  const ds = new Map(), add = (k, d) => { if (d) (ds.get(k) ?? ds.set(k, []).get(k)).push(d); };
  let mask = "";
  for (const [key, r] of routes) {
    const { d, mid, s, f } = line(key, r), n = r.sheet, k = `${n ?? ""}${r.dashed ? "d" : ""}${r.W ? "f" : "s"}${r.colour}`;
    // A dashed line: whole under the mask, its centre in the mask's dashes.
    if (r.dashed) mask += mid;
    if (n !== undefined || r.dashed) pathFor(k, ink(r), n === undefined ? dashes.g : sheet(n)[r.dashed ? "dg" : "g"]);
    add(k, d);
    if (n !== undefined) { add(`${n}s`, s); add(`${n}f`, f); }
  }
  for (const [id, s] of shown) {
    const n = graph.nodes.get(id);
    if (n?.kind === "chip" && !n.running && !s.card) { const c = centre(s); add(`g${GEAR[n.end]}`, gear(c.x, c.y, radius(n))); }
  }
  if (dashes.d !== mask) { dashes.d = mask; dashes.mask.setAttribute("d", mask); }
  for (const [k, p] of paths) {
    const d = (ds.get(k) || []).join("");
    if (p.d !== d) { p.d = d; p.setAttribute("d", d); }
  }
}
// A finished call's gear: green done, red failed.
const GEAR = { green: "#4f8a55", red: "#b3352c" };
const f2 = (v) => (Math.round(v * 100) / 100).toString();
function gear(cx, cy, R) {
  const r = R * .71, n = 8, step = (2 * Math.PI) / n, pts = [];
  for (let k = 0; k < n; k++) {
    const a = k * step - Math.PI / 2;
    for (const [rad, da] of [[r, -.34], [R, -.2], [R, .2], [r, .34]]) pts.push(`${f2(cx + rad * Math.cos(a + da * step))},${f2(cy + rad * Math.sin(a + da * step))}`);
  }
  const h = R * .32;
  return `M${pts.join("L")}ZM${f2(cx + h)},${f2(cy)}A${h},${h} 0 1 0 ${f2(cx - h)},${f2(cy)}A${h},${h} 0 1 0 ${f2(cx + h)},${f2(cy)}Z`;
}

// Why a task is stuck, while it matters: a place it starts in that could
// not be made, or its last turn not responding.
export function trouble(t) {
  const failed = t.waiting?.kind === "starting" ? t.waiting.failed || [] : [];
  if (failed.length) return `${failed.map((f) => `Couldn't make ${f.limb}: ${f.why}.`).join(" ")} The harness keeps trying: abandon it, or start again elsewhere.`;
  const x = t.trouble;
  if (!x || isDone(t)) return null;
  if (x.until != null) return x.until > Date.now() ? `Limited until ${when(x.until)}` : null;
  if (!t.failures) return null;
  const times = t.failures === 1 ? "once" : `${t.failures} times in a row`;
  const next = x.retry == null ? "" : ` Trying again at ${new Date(x.retry).toLocaleTimeString([], { hour12: false })}.`;
  return `Waiting on the provider: it failed ${times}, last with ${x.text}.${next}`;
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
// A face that arrives after the labels were measured changes their widths.
document.fonts.addEventListener("loadingdone", () => { remeasure(); refresh(); });

export function render() {
  if (!view) return;
  paintRegions();
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
  // bottom bar's buttons or an archive card fades out.
  const ui = [...document.querySelectorAll("#panel:not([hidden]), #bottom > :not(.spacer)")]
    .map((e) => e.getBoundingClientRect()).filter((r) => r.width > 0);
  for (const s of shown.values()) {
    if (!s.card) continue;
    const left = cam.x + s.x * cam.k, top = cam.y + s.y * cam.k;
    ui.push({ left, top, right: left + s.w * cam.k, bottom: top + s.h * cam.k });
  }
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
