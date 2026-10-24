// Diagrams laid out by ELK (the global from elk.bundled.js): the crates and
// what they depend on, and the whole repo as nested directory boxes with a
// node per file and a line wherever one file names another.
const elk = new globalThis.ELK();
const NS = "http://www.w3.org/2000/svg";

function s(tag, attrs = {}, ...kids) {
  const el = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) if (v !== undefined && v !== null) el.setAttribute(k, v);
  el.append(...kids.filter((k) => k !== null && k !== undefined));
  return el;
}

const measure = document.createElement("canvas").getContext("2d");
function textWidth(text, font = "12px system-ui, sans-serif") {
  measure.font = font;
  return Math.ceil(measure.measureText(text).width);
}

const markers = () => s("defs", {},
  s("marker", { id: "arrow", viewBox: "0 0 8 8", refX: "7", refY: "4", markerWidth: "7", markerHeight: "7", orient: "auto-start-reverse" }, s("path", { d: "M0,0 L8,4 L0,8 z", class: "arrowhead" })));

// ELK's splines come as cubic Bézier control points, three per curve.
function pathOf(section, dx, dy) {
  const pts = [section.startPoint, ...(section.bendPoints ?? []), section.endPoint].map((p) => `${(p.x + dx).toFixed(1)},${(p.y + dy).toFixed(1)}`);
  if (pts.length > 2 && (pts.length - 1) % 3 === 0) {
    let d = `M${pts[0]}`;
    for (let i = 1; i < pts.length; i += 3) d += ` C${pts[i]} ${pts[i + 1]} ${pts[i + 2]}`;
    return d;
  }
  return `M${pts.join(" L")}`;
}

// --- crates ------------------------------------------------------------------

// Only the shortest arrows: a dependency another dependency already brings
// in is left out, so the layers read top to bottom.
function reduce(crates) {
  const deps = new Map(crates.map((c) => [c.id, c.deps.filter((d) => crates.some((x) => x.id === d))]));
  const reach = (from, seen = new Set()) => {
    for (const d of deps.get(from)) if (!seen.has(d)) { seen.add(d); reach(d, seen); }
    return seen;
  };
  const edges = [];
  for (const [a, ds] of deps) {
    for (const b of ds) if (!ds.some((c) => c !== b && reach(c).has(b))) edges.push([a, b]);
  }
  return edges;
}

export async function crateGraph(el, { crates, open }) {
  const edges = reduce(crates);
  const graph = {
    id: "root",
    layoutOptions: {
      "elk.algorithm": "layered", "elk.direction": "DOWN",
      "elk.layered.spacing.nodeNodeBetweenLayers": "36", "elk.spacing.nodeNode": "14",
      "elk.layered.crossingMinimization.semiInteractive": "false", "elk.padding": "[top=6,left=6,bottom=6,right=6]",
    },
    children: crates.map((c) => ({ id: c.id, width: textWidth(c.label, "600 13px system-ui, sans-serif") + 26 + (c.count > 0 ? 22 : 0), height: 30 })),
    edges: edges.map(([a, b], i) => ({ id: `e${i}`, sources: [a], targets: [b] })),
  };
  const out = await elk.layout(graph);
  const svg = s("svg", { viewBox: `0 0 ${out.width} ${out.height}`, class: "crategraph", style: `max-width:${out.width}px` }, markers());
  for (const e of out.edges) for (const sec of e.sections ?? []) svg.append(s("path", { d: pathOf(sec, 0, 0), class: "edge", "marker-end": "url(#arrow)" }));
  for (const n of out.children) {
    const c = crates.find((x) => x.id === n.id);
    const cls = c.structures.length === 1 ? `s${c.structures[0]}` : "all";
    const g = s("g", { class: `node ${cls}${c.changed ? " changed" : ""}`, transform: `translate(${n.x},${n.y})`, tabindex: "0" },
      s("title", {}, `${c.id}${c.count > 0 ? `: ${c.count} not built, provisional or not covered in its docs` : ""}${c.changed ? " · changed since you last looked" : ""}`),
      s("rect", { width: n.width, height: n.height, rx: 6, class: "box" }),
      s("text", { x: 13, y: 19.5, class: "label" }, c.label),
      c.count > 0 ? s("g", { transform: `translate(${n.width - 20},6)` }, s("rect", { width: 16, height: 18, rx: 8, class: "count" }), s("text", { x: 8, y: 13, class: "countt" }, String(c.count))) : null,
      c.changed ? s("rect", { x: 0, y: 4, width: 3, height: n.height - 8, class: "revbar" }) : null);
    g.addEventListener("click", () => open(c.id));
    g.addEventListener("keydown", (e) => { if (e.key === "Enter") open(c.id); });
    svg.append(g);
  }
  el.replaceChildren(svg);
}

// --- the whole repo ------------------------------------------------------------

// Directories as a tree, a chain of directories holding nothing but the next
// one squashed into one box named by the whole chain.
function tree(files) {
  const root = { path: "", name: "", dirs: new Map(), files: [] };
  for (const p of files) {
    const segs = p.split("/");
    let d = root;
    for (let i = 0; i < segs.length - 1; i++) {
      if (!d.dirs.has(segs[i])) d.dirs.set(segs[i], { path: segs.slice(0, i + 1).join("/"), name: segs[i], dirs: new Map(), files: [] });
      d = d.dirs.get(segs[i]);
    }
    d.files.push(p);
  }
  const squash = (d) => {
    for (const [k, c] of d.dirs) {
      let x = c;
      let name = c.name;
      while (x.files.length === 0 && x.dirs.size === 1) {
        const [only] = x.dirs.values();
        name = `${name}/${only.name}`;
        x = only;
      }
      const merged = { ...x, name };
      d.dirs.set(k, merged);
      squash(merged);
    }
    d.count = d.files.length + [...d.dirs.values()].reduce((n, c) => n + c.count, 0);
  };
  squash(root);
  return root;
}

const boxes = (d) => [d, ...[...d.dirs.values()].flatMap(boxes)];
const inside = (p, box) => box.path === "" || p === box.path || p.startsWith(`${box.path}/`);

export function codeLayout(el, { files, refs, changed, colour, focus, pick }) {
  const root = tree(files);
  const all = boxes(root).slice(1);
  let expanded = new Set(all.filter((b) => b.path === "crates").map((b) => b.path));
  if (focus !== null) for (const b of all) if (inside(focus, b)) expanded.add(b.path);
  let scale = 1;
  let zoomed = false;
  let selected = focus !== null && files.includes(focus) ? focus : null;
  let generation = 0;

  // The visible node a file is drawn as: itself, or the outermost closed box holding it.
  const rep = (p) => {
    let d = root;
    for (;;) {
      const next = [...d.dirs.values()].find((c) => p.startsWith(`${c.path}/`));
      if (next === undefined) return p;
      if (!expanded.has(next.path)) return next.path;
      d = next;
    }
  };

  // Each box is laid out on its own: its children by ELK's layered algorithm
  // when lines join any of them (a line between two files stands in for its
  // ends' boxes at the level where they meet), packed into rows when none
  // do. The lines themselves are drawn over the result, file to file.
  async function draw() {
    const gen = ++generation;
    el.replaceChildren(Object.assign(document.createElement("p"), { className: "muted", textContent: "Laying out…" }));
    const node = (d) => {
      const col = colour(d.path);
      if (!expanded.has(d.path)) {
        const label = `${d.name}/  ${d.count}`;
        return { id: d.path, width: textWidth(label, "600 12px system-ui, sans-serif") + 22, height: 26, kind: "closed", d, col };
      }
      return { id: d.path, kind: "open", d, col, edges: [], children: children(d) };
    };
    const children = (d) => [...[...d.dirs.values()].sort((x, y) => x.name.localeCompare(y.name)).map(node), ...d.files.sort().map((f) => {
      const name = f.split("/").at(-1);
      return { id: f, width: textWidth(name) + 14, height: 20, kind: "file", name, col: colour(f) };
    })];
    const graph = { id: "root", kind: "open", d: root, col: null, edges: [], children: children(root) };
    const byId = new Map();
    const parent = new Map();
    const index = (n) => { byId.set(n.id, n); for (const c of n.children ?? []) { parent.set(c.id, n.id); index(c); } };
    index(graph);
    const up = (id) => { const out = [id]; for (let p = parent.get(id); p !== undefined; p = parent.get(p)) out.push(p); return out; };

    // One line per pair of visible nodes; ↔ when each names the other.
    const pairs = new Map();
    for (const [a, b] of refs) {
      const ra = rep(a);
      const rb = rep(b);
      if (ra === rb) continue;
      const [x, y] = ra < rb ? [ra, rb] : [rb, ra];
      const e = pairs.get(`${x}\t${y}`) ?? { x, y, fwd: 0, back: 0 };
      if (ra < rb) e.fwd++; else e.back++;
      pairs.set(`${x}\t${y}`, e);
    }
    const lines = [...pairs.values()].map((e) => {
      const [from, to] = e.fwd > 0 ? [e.x, e.y] : [e.y, e.x];
      return { from, to, mutual: e.fwd > 0 && e.back > 0, weight: e.fwd + e.back };
    });
    const stand = new Set();
    for (const l of lines) {
      const ua = up(l.from);
      const ub = up(l.to);
      const i = ua.findIndex((x) => ub.includes(x));
      const lca = ua[i];
      const key = `${lca}\t${ua[i - 1]}\t${ub[ub.indexOf(lca) - 1]}`;
      if (stand.has(key)) continue;
      stand.add(key);
      byId.get(lca).edges.push({ id: `p${stand.size}`, sources: [ua[i - 1]], targets: [ub[ub.indexOf(lca) - 1]] });
    }
    for (const n of byId.values()) {
      if (n.kind !== "open") continue;
      const label = n === graph ? "agent-harness/" : `▾ ${n.d.name}/`;
      n.layoutOptions = {
        "elk.padding": `[top=${n === graph ? 30 : 26},left=8,bottom=8,right=8]`,
        "elk.nodeSize.constraints": "[MINIMUM_SIZE]",
        "elk.nodeSize.minimum": `(${textWidth(label, "600 12px system-ui, sans-serif") + 20}, 40)`,
        "elk.spacing.nodeNode": "8", "elk.aspectRatio": "1.6",
        ...(n.edges.length > 0
          ? { "elk.algorithm": "layered", "elk.direction": "RIGHT", "elk.layered.spacing.nodeNodeBetweenLayers": "36", "elk.layered.considerModelOrder.strategy": "NODES_AND_EDGES" }
          : { "elk.algorithm": "rectpacking" }),
      };
    }

    const t0 = performance.now();
    const out = await elk.layout(graph);
    if (gen !== generation) return;
    const took = performance.now() - t0;

    const abs = new Map();
    const place = (n, x, y) => {
      abs.set(n.id, { x: x + (n.x ?? 0), y: y + (n.y ?? 0), w: n.width, h: n.height });
      for (const c of n.children ?? []) place(c, x + (n.x ?? 0), y + (n.y ?? 0));
    };
    place(out, 0, 0);
    if (!zoomed) scale = Math.max(0.45, Math.min(1, (el.clientWidth - 4) / out.width));
    const svg = s("svg", { width: out.width * scale, height: out.height * scale, viewBox: `0 0 ${out.width} ${out.height}`, class: "codemap" }, markers());
    const layerBoxes = s("g");
    const layerNodes = s("g");
    const layerEdges = s("g");
    svg.append(layerBoxes, layerNodes, layerEdges);
    layerBoxes.append(s("rect", { x: 1, y: 1, width: out.width - 2, height: out.height - 2, rx: 8, class: "repo" }), s("text", { x: 12, y: 19, class: "boxlabel" }, "agent-harness/"));
    const changedIn = (p) => [...changed].some((c) => inside(c, { path: p }));
    const nodes = new Map();
    for (const [id, { x, y, w, h }] of abs) {
      if (id === "root") continue;
      const info = byId.get(id);
      const cls = info.col === null ? "" : ` s${info.col}`;
      let g;
      if (info.kind === "open") {
        g = s("g", { class: `dir open${cls}` },
          s("rect", { x, y, width: w, height: h, rx: 6, class: "box" }),
          s("text", { x: x + 8, y: y + 17, class: "boxlabel toggle" }, s("title", {}, "Close this box"), `▾ ${info.d.name}/`));
        g.querySelector(".toggle").addEventListener("click", () => { expanded.delete(id); draw(); });
        layerBoxes.append(g);
        continue;
      }
      if (info.kind === "closed") {
        g = s("g", { class: `dir closed${cls}${changedIn(id) ? " changed" : ""}`, tabindex: "0" },
          s("title", {}, `${id}/: ${info.d.count} files. Click to open.`),
          s("rect", { x, y, width: w, height: h, rx: 5, class: "box" }),
          s("text", { x: x + 9, y: y + 17, class: "boxlabel" }, `${info.d.name}/`, s("tspan", { class: "n" }, `  ${info.d.count}`)),
          changedIn(id) ? s("rect", { x, y: y + 4, width: 3, height: h - 8, class: "revbar" }) : null);
        g.addEventListener("click", () => { expanded.add(id); draw(); });
      } else {
        g = s("g", { class: `file${cls}${changed.has(id) ? " changed" : ""}${selected === id ? " picked" : ""}`, tabindex: "0" },
          s("title", {}, id),
          s("rect", { x, y, width: w, height: h, rx: 4, class: "box" }),
          s("text", { x: x + 7, y: y + 14, class: "label" }, info.name),
          changed.has(id) ? s("rect", { x, y: y + 3, width: 3, height: h - 6, class: "revbar" }) : null);
        g.addEventListener("click", () => {
          svg.querySelector(".picked")?.classList.remove("picked");
          g.classList.add("picked");
          selected = id;
          pick(id);
        });
      }
      layerNodes.append(g);
      nodes.set(id, g);
    }
    const incident = new Map();
    for (const l of lines) {
      const a = abs.get(l.from);
      const b = abs.get(l.to);
      const right = b.x + b.w / 2 >= a.x + a.w / 2;
      const [sx, ex] = right ? [a.x + a.w, b.x] : [a.x, b.x + b.w];
      const sy = a.y + a.h / 2;
      const ey = b.y + b.h / 2;
      const bend = Math.max(24, Math.abs(ex - sx) / 2) * (right ? 1 : -1);
      const p = s("path", {
        d: `M${sx},${sy} C${sx + bend},${sy} ${ex - bend},${ey} ${ex},${ey}`,
        class: `edge${l.mutual ? " mutual" : ""}`, "stroke-width": (0.8 + Math.log2(l.weight) * 0.6).toFixed(2),
        "marker-end": "url(#arrow)", "marker-start": l.mutual ? "url(#arrow)" : undefined,
      }, s("title", {}, `${l.from} ${l.mutual ? "↔" : "→"} ${l.to}${l.weight > 1 ? ` (${l.weight} references)` : ""}`));
      layerEdges.append(p);
      for (const [end, other] of [[l.from, l.to], [l.to, l.from]]) {
        if (!incident.has(end)) incident.set(end, []);
        incident.get(end).push({ path: p, other });
      }
    }
    for (const [id, g] of nodes) {
      g.addEventListener("mouseenter", () => {
        svg.classList.add("focus");
        g.classList.add("hl");
        for (const { path, other } of incident.get(id) ?? []) { path.classList.add("hl"); nodes.get(other)?.classList.add("hl"); }
      });
      g.addEventListener("mouseleave", () => {
        svg.classList.remove("focus");
        for (const x of svg.querySelectorAll(".hl")) x.classList.remove("hl");
      });
    }
    const status = Object.assign(document.createElement("p"), { className: "note", textContent: `${files.length} files, ${nodes.size} shown, ${lines.length} lines · laid out in ${Math.round(took)} ms` });
    const scroller = Object.assign(document.createElement("div"), { className: "mapscroll" });
    scroller.append(svg);
    el.replaceChildren(status, scroller);
    if (selected !== null && abs.has(selected)) {
      const { x, y } = abs.get(selected);
      scroller.scrollTo({ left: x * scale - scroller.clientWidth / 2, top: y * scale - scroller.clientHeight / 2 });
      pick(selected);
    }
  }

  draw().catch((e) => {
    el.replaceChildren(Object.assign(document.createElement("div"), { className: "error", textContent: `The code layout didn't lay out: ${e.message}` }));
  });

  return {
    expand(mode) {
      expanded = new Set(mode === "all" ? all.map((b) => b.path) : all.filter((b) => b.path === "crates").map((b) => b.path));
      draw();
    },
    zoom(f) {
      zoomed = true;
      scale *= f;
      const svg = el.querySelector("svg");
      if (svg === null) return;
      const [, , w, hgt] = svg.getAttribute("viewBox").split(" ").map(Number);
      svg.setAttribute("width", w * scale);
      svg.setAttribute("height", hgt * scale);
    },
  };
}
