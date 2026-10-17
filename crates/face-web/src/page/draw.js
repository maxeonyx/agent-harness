// ─── Drawing the map ───────────────────────────────────────────────────────
const nodeEls = new Map();

function drawMap() {
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
      $(".dot", a).append(el("span", "g"), el("span", "ring"));
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
        : `node ${l.cls}${t.origin === "fork" ? " fork" : ""}${q >= 0 ? " needs" : ""}${q === 0 ? " next" : ""}${trouble(t) ? " trouble" : ""}${root ? " root" : ""}${id === openId ? " selected" : ""}${faded}`;
      a.style.setProperty("--d", `${root ? ROOT_DOT : DOT}px`);
      mark($(".g", a), l.cls === "paused" ? "pause" : glyph(t));
      $(".name", a).textContent = shortName(t);
      let more = $(".more", a);
      if (fc && fc.tasks.length > 1) {
        if (!more) { a.append(more = el("button", "more", "⌄")); more.title = "Unfold the project"; more.addEventListener("click", (e) => { e.preventDefault(); e.stopPropagation(); unfolded.add(id); refresh(); }); }
      } else more?.remove();
    } else {
      a.className = `node sat working${faded}`;
      a.style.setProperty("--d", `${SAT_DOT}px`);
      mark($(".g", a), "⋯");
      $(".name", a).textContent = satLabel(n);
    }
  }
  if (tipFor) fillTip();
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
function drawEdges() {
  const paths = [];
  for (const e of graph.edges) {
    const a = anchor(e.from, "out"), b = anchor(e.to, "in");
    if (!a || !b) continue;
    if (e.kind === "continues") {
      const p = shown.get(e.from), q = shown.get(e.to), d = DOT / 2;
      const x0 = p.x + p.w / 2, y0 = p.y + d, x1 = q.x + q.w / 2, y1 = q.y + d;
      const lift = Math.min(60, 12 + Math.abs(x1 - x0) / 6);
      const path = svgEl("path");
      path.setAttribute("d", `M${x0 + Math.sign(x1 - x0) * (d + 3)},${y0} C${x0},${Math.min(y0, y1) - lift} ${x1},${Math.min(y0, y1) - lift} ${x1 - Math.sign(x1 - x0) * (d + 3)},${y1}`);
      path.setAttribute("class", "edge continues");
      paths.push(path);
      continue;
    }
    // Out of the bottom of the source's dot, below its name, into the top of the target's.
    const sx = a.x, sy = shown.get(e.from).y + shown.get(e.from).h - 4;
    const dy = Math.max(18, (b.y - sy) / 2);
    const path = svgEl("path");
    path.setAttribute("d", b.y > sy ? `M${sx},${sy} C${sx},${sy + dy} ${b.x},${b.y - dy} ${b.x},${b.y}` : `M${a.x},${a.y} C${a.x + 60},${a.y} ${b.x + 60},${b.y} ${b.x},${b.y}`);
    path.setAttribute("class", `edge ${e.kind}${e.flow ? " flow" : ""}${e.max ? " max" : ""}`);
    paths.push(path);
  }
  edgesLayer.replaceChildren(...paths);
}

// Why a task's last turn did not respond, while it matters.
function trouble(t) {
  const x = t.trouble;
  if (!x || isDone(t)) return null;
  if (x.until != null) return x.until > Date.now() ? `limited until ${clock(x.until)}` : null;
  return t.failures ? `failed ${t.failures}× · ${x.text}` : null;
}

// ─── The whole page, from the view ─────────────────────────────────────────
let busy = false, again = false, shownBefore = false;
// The view folded into the map: what it is, where everything goes.
async function rebuild() {
  derive();
  const changed = await layout();
  if (openId) seam = seamFor(openId) || seam;
  drawMap();
  return changed;
}
async function refresh() {
  if (!view) return;
  if (busy) { again = true; return; }
  busy = true;
  try {
    // A card opening or closing finishes before the map moves under it.
    await moving;
    const changed = await rebuild();
    glide(null, { T: shownBefore ? 420 : 0 });
    if (changed && framed && !openId) fit(shownBefore);
    shownBefore = true;
    render();
  } finally {
    busy = false;
    if (again) { again = false; refresh(); }
  }
}

function render() {
  if (!view) return;
  $("#empty").hidden = view.tasks.length > 0 || !!openId;
  renderUsage();
  const q = view.queue || [];
  document.title = q.length ? `(${q.length}) harness` : "harness";
  notify(q);
  if (openId && openId !== NEW && live(openId) !== openId) { stay = true; go(live(openId)); }
  else if (openId) renderPanel();
  syncCompose();
  renderOthers();
  placeMarks();
}

// ─── What needs Max, off screen ────────────────────────────────────────────
// What waits on Max where the camera does not show it is a mark at the
// screen's edge, toward it, the next one haloed; a click opens it.
const marksEl = $("#marks"), markEls = new Map();
function placeMarks() {
  if (!view) return;
  const m = 20, seen = new Set(), placed = [];
  // The open card covers what is under it: no mark goes there.
  const card = openId && !panel.hidden ? panel.getBoundingClientRect() : null;
  (view.queue || []).forEach((q, i) => {
    const id = live(q.task), p = shown.get(id);
    if (!p || id === openId) return;
    const x = cam.x + (p.x + p.w / 2) * cam.k, y = cam.y + (p.y + DOT / 2) * cam.k;
    if (x >= 0 && x <= innerWidth && y >= 0 && y <= innerHeight) return;
    seen.add(id);
    let a = markEls.get(id);
    if (!a) {
      a = el("button", "offscreen");
      a.type = "button";
      a.innerHTML = `<svg viewBox="0 0 24 24">${ICONS.toward}</svg>`;
      a.onclick = () => go(id);
      marksEl.append(a);
      markEls.set(id, a);
    }
    a.title = byId[id]?.name || "";
    a.classList.toggle("next", i === 0);
    // Marks toward the same corner line up along the edge rather than stack.
    let cx = Math.max(m, Math.min(innerWidth - m, x)), cy = Math.max(m, Math.min(innerHeight - m - 40, y));
    const along = cx === m || cx === innerWidth - m ? [0, 30] : [30, 0];
    while (placed.some(([px, py]) => Math.hypot(px - cx, py - cy) < 28)) { cx += along[0]; cy += along[1]; }
    placed.push([cx, cy]);
    a.hidden = !!card && cx > card.left - 12 && cx < card.right + 12 && cy > card.top - 12 && cy < card.bottom + 12;
    a.style.left = `${cx}px`;
    a.style.top = `${cy}px`;
    $("svg", a).style.transform = `rotate(${Math.atan2(y - cy, x - cx)}rad)`;
  });
  for (const [id, a] of markEls) if (!seen.has(id)) { a.remove(); markEls.delete(id); }
}

