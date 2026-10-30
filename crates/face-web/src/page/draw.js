import { $, clock, el, ICONS, panel, svgEl } from "./base.js";
import { byId, derive, DOT, glyph, graph, isDone, layout, live, look, positions, ROOT_DOT, routes, SAT_DOT, satLabel, shortName, shown, unfolded, view } from "./map.js";
import { cam, cardShown, drag, edgesLayer, fit, framed, glide, nodesLayer, seam, seamFor, setFramed, setSeam, target } from "./camera.js";
import { fillTip, hoverable, tipFor } from "./hover.js";
import { notify, renderUsage } from "./chrome.js";
import { renderBuilds } from "./builds.js";
import { go, goStaying, moving, NEW, openId, renderPanel } from "./card.js";
import { renderOthers, syncCompose } from "./compose.js";
import { finding, found } from "./newchat.js";

// ─── Drawing the map ───────────────────────────────────────────────────────
export const nodeEls = new Map();

export function drawMap() {
  const visible = new Set(positions.keys());
  for (const [id, elx] of nodeEls) if (!visible.has(id)) { elx.remove(); nodeEls.delete(id); shown.delete(id); }
  const roots = new Set(graph.comps.map((c) => c.root.id));
  const folded = new Map(graph.comps.filter((c) => c.folded).map((c) => [c.root.id, c]));
  const queue = (view.queue || []).map((q) => live(q.task));
  for (const id of visible) {
    const n = graph.nodes.get(id);
    if (!n) continue;
    let a = nodeEls.get(id);
    if (!a) {
      a = el(n.kind === "task" ? "a" : "div", "node");
      a.append(el("span", "dot"), el("span", "name"));
      $(".dot", a).append(el("span", "g"), el("span", "ring"), el("span", "phases"));
      if (n.kind === "task") {
        a.href = `#${id}`;
        a.addEventListener("click", (e) => { if (e.button || e.ctrlKey || e.metaKey || e.shiftKey) return; e.preventDefault(); if (!drag?.moved) go(id); });
        hoverable(a, id);
      } else if (n.kind === "sat") {
        a.addEventListener("click", () => { if (!drag?.moved) go(n.task); });
        a.style.cursor = "pointer";
        hoverable(a, id);
      }
      nodesLayer.append(a);
      nodeEls.set(id, a);
      // A new node grows out of where its owner is.
      const from = graph.edges.find((e) => e.to === id)?.from;
      const pf = from && shown.get(from);
      const p = target(id);
      shown.set(id, pf ? { x: pf.x + (pf.w || 0) / 2 - p.w / 2, y: pf.y, w: p.w, h: p.h } : { ...p });
      a.style.transform = `translate(${shown.get(id).x}px, ${shown.get(id).y}px)`;
      if (shown.size > 1) a.classList.add("enter");
    }
    const p = positions.get(id);
    a.style.width = `${p.w}px`;
    const faded = finding && !found.has(id) ? " unmatched" : "";
    if (n.kind === "task") {
      const t = n.t, l = look(t), root = roots.has(id), fc = folded.get(id), q = queue.indexOf(id);
      a.className = fc ? `node archived${l.cls.includes("failed") ? " failed" : ""}${faded}`
        : `node ${l.cls}${q >= 0 ? " needs" : ""}${q === 0 ? " next" : ""}${trouble(t) ? " trouble" : ""}${root ? " root" : ""}${id === openId ? " selected" : ""}${faded}`;
      a.style.setProperty("--d", `${root ? ROOT_DOT : DOT}px`);
      mark($(".g", a), l.cls === "paused" ? "pause" : glyph(t));
      phases($(".phases", a), t.compactions || 0);
      $(".name", a).textContent = shortName(t);
      let more = $(".more", a);
      if (fc && fc.tasks.length > 1) {
        if (!more) { a.append(more = el("button", "more", "⌄")); more.title = "Unfold the project"; more.addEventListener("click", (e) => { e.preventDefault(); e.stopPropagation(); unfolded.add(id); refresh(); }); }
      } else more?.remove();
    } else {
      a.className = `node sat working${n.call.owner === "max" ? " yours" : ""}${faded}`;
      a.style.setProperty("--d", `${SAT_DOT}px`);
      mark($(".g", a), "⋯");
      $(".name", a).textContent = satLabel(n);
    }
  }
  if (tipFor) fillTip();
}
// The phases a context compacted away, as a trail of fading pips beside its
// dot, the latest nearest: the same agent, earlier. A few say "several".
function phases(p, n) {
  n = Math.min(n, 4);
  if (p.childElementCount === n) return;
  p.replaceChildren(...Array.from({ length: n }, () => el("i")));
}
// What a dot shows: a letter, or one of the icons by its name.
function mark(g, what) {
  if (g.dataset.what === what) return;
  g.dataset.what = what;
  if (ICONS[what]) g.innerHTML = `<svg viewBox="0 0 24 24">${ICONS[what]}</svg>`;
  else g.textContent = what;
}

function anchor(id, end) {
  const p = shown.get(id);
  const n = graph.nodes.get(id);
  if (!p || !n) return null;
  const cx = p.x + p.w / 2;
  const d = n.kind === "sat" ? SAT_DOT : graph.comps.some((c) => c.root.id === id) ? ROOT_DOT : DOT;
  return end === "out" ? { x: cx, y: p.y + d + 2 } : { x: cx, y: p.y - 2 };
}
// An edge along the curve the layout gave it, bent to follow its two ends
// wherever they have moved since: each point moves as the end nearer it does.
// An open chat's card is its node: what it leads to leaves from the card's foot.
function laidCurve(e) {
  const pts = routes.get(`${e.from}>${e.to}`);
  if (!pts) return null;
  const move = (id) => {
    const p = positions.get(id), s = shown.get(id), c = id === openId && e.from === id && cardShown;
    return c ? { x: c.x + c.w / 2 - p.x - p.w / 2, y: c.y + c.h - p.y - p.h } : { x: s.x - p.x, y: s.y - p.y };
  };
  const m0 = move(e.from), m1 = move(e.to), y0 = pts[0].y, y1 = pts[pts.length - 1].y;
  const q = pts.map((p) => { const u = y1 === y0 ? 0 : Math.max(0, Math.min(1, (p.y - y0) / (y1 - y0))); return `${p.x + m0.x + (m1.x - m0.x) * u},${p.y + m0.y + (m1.y - m0.y) * u}`; });
  return (q.length - 1) % 3 ? `M${q.join(" L")}` : `M${q[0]} C${q.slice(1).join(" ")}`;
}
// Out of the bottom of the source's dot, below its name, into the top of the target's.
function straight(e, a, b) {
  const sx = a.x, sy = shown.get(e.from).y + shown.get(e.from).h - 4;
  const dy = Math.max(18, (b.y - sy) / 2);
  return b.y > sy ? `M${sx},${sy} C${sx},${sy + dy} ${b.x},${b.y - dy} ${b.x},${b.y}` : `M${a.x},${a.y} C${a.x + 60},${a.y} ${b.x + 60},${b.y} ${b.x},${b.y}`;
}
export function drawEdges() {
  const paths = [];
  for (const e of graph.edges) {
    const a = anchor(e.from, "out"), b = anchor(e.to, "in");
    if (!a || !b) continue;
    const d = laidCurve(e) || straight(e, a, b);
    const cls = `edge ${e.kind}${e.flow ? " flow" : ""}`;
    // A fork's edge is hollow: its owner's context, carried down into it.
    // A fresh agent's is a line: it starts from its mandate alone.
    for (const part of graph.nodes.get(e.to)?.t?.origin === "fork" ? [" tube", " core"] : [""]) {
      const path = svgEl("path");
      path.setAttribute("d", d);
      path.setAttribute("class", cls + part);
      paths.push(path);
    }
  }
  edgesLayer.replaceChildren(...paths);
}

// Why a task is stuck, while it matters: a place it starts in that could
// not be made, or its last turn not responding.
export function trouble(t) {
  const failed = t.waiting?.kind === "starting" ? t.waiting.failed || [] : [];
  if (failed.length) return `${failed.map((f) => `Couldn't make ${f.limb}: ${f.why}.`).join(" ")} The harness keeps trying: abandon it, or start again elsewhere.`;
  const x = t.trouble;
  if (!x || isDone(t)) return null;
  if (x.until != null) return x.until > Date.now() ? `Limited until ${clock(x.until)}` : null;
  return t.failures ? `Failed ${t.failures === 1 ? "once" : `${t.failures} times`}: ${x.text}` : null;
}

// ─── The whole page, from the view ─────────────────────────────────────────
let shownBefore = false;
// The view folded into the map: what it is, where everything goes.
export async function rebuild() {
  derive();
  const changed = await layout();
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
      if (changed && framed && !openId) fit(shownBefore);
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
  if (openId && openId !== NEW && live(openId) !== openId) goStaying(live(openId));
  else if (openId) renderPanel();
  syncCompose();
  renderOthers();
  placeMarks();
}

// ─── What needs Max, off screen ────────────────────────────────────────────
// What waits on Max where the camera does not show it is a mark at the
// screen's edge, toward it, the next one haloed; a click brings it to the
// middle of the screen, where a click on it opens it.
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
    const id = live(q.task), p = shown.get(id);
    if (!p || id === openId) return;
    const x = cam.x + (p.x + p.w / 2) * cam.k, y = cam.y + (p.y + DOT / 2) * cam.k;
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
  // The open card covers what is under it: a mark there fades out.
  const card = openId && !panel.hidden ? panel.getBoundingClientRect() : null;
  const seen = new Set();
  for (const { id, x, y, s, next } of marks) {
    seen.add(id);
    let a = markEls.get(id);
    if (!a) {
      a = el("button", "offscreen");
      a.type = "button";
      a.innerHTML = `<svg viewBox="0 0 24 24">${ICONS.toward}</svg>`;
      a.onclick = () => centre(id);
      marksEl.append(a);
      markEls.set(id, a);
    }
    a.title = byId[id]?.name || "";
    a.classList.toggle("next", next);
    const [cx, cy] = at(s);
    a.classList.toggle("under", !!card && cx > card.left - 12 && cx < card.right + 12 && cy > card.top - 12 && cy < card.bottom + 12);
    a.style.left = `${cx}px`;
    a.style.top = `${cy}px`;
    $("svg", a).style.transform = `rotate(${Math.atan2(y - cy, x - cx)}rad)`;
  }
  for (const [id, a] of markEls) if (!seen.has(id)) { a.remove(); markEls.delete(id); }
}
// The camera, at the zoom it has, moved to put a node in the middle.
function centre(id) {
  const p = shown.get(id);
  if (!p) return;
  setFramed(false);
  glide({ k: cam.k, x: innerWidth / 2 - (p.x + p.w / 2) * cam.k, y: innerHeight / 2 - (p.y + p.h / 2) * cam.k }, { T: 420 });
}
