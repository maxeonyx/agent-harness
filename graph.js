// Renders a nested graph with ELK into an SVG: boxes for groups, nodes for leaves, edges that cross group boundaries.
// diagram(el, { nodes: [{id, label, parent?, kind?, sub?, tip?}], edges: [{from, to, label?, style?}], direction? })
// Hovering or tapping a node lights up its edges and neighbours; a node's `tip` shows below the diagram.
const elk = new ELK();
const NS = "http://www.w3.org/2000/svg";
const textWidth = (s, px = 13) => Math.max(40, s.length * px * 0.58);

async function diagram(el, spec) {
  const byId = new Map(spec.nodes.map((n) => [n.id, { ...n, children: [] }]));
  const roots = [];
  for (const n of byId.values()) (n.parent && byId.has(n.parent) ? byId.get(n.parent).children : roots).push(n);
  const toElk = (n) => {
    const label = n.label ?? n.id;
    if (n.children.length) {
      return {
        id: n.id,
        labels: [{ text: label, width: textWidth(label, 12), height: 16 }],
        layoutOptions: { "elk.padding": "[top=30,left=12,bottom=12,right=12]", "elk.nodeLabels.placement": "[H_LEFT, V_TOP, INSIDE]" },
        children: n.children.map(toElk),
      };
    }
    const lines = [label, n.sub].filter(Boolean);
    return { id: n.id, width: Math.max(...lines.map((l, i) => textWidth(l, i ? 11 : 13))) + 20, height: 16 + 17 * lines.length };
  };
  const graph = {
    id: "root",
    layoutOptions: {
      "elk.algorithm": "layered",
      "elk.direction": spec.direction ?? "RIGHT",
      "elk.hierarchyHandling": "INCLUDE_CHILDREN",
      "elk.layered.spacing.nodeNodeBetweenLayers": "36",
      "elk.spacing.nodeNode": "16",
      "elk.edgeRouting": "ORTHOGONAL",
      "elk.edgeLabels.inlineEdgeLabels": "true",
      ...(spec.layout ?? {}),
    },
    children: roots.map(toElk),
    edges: spec.edges.filter((e) => byId.has(e.from) && byId.has(e.to)).map((e, i) => ({
      id: "e" + i, sources: [e.from], targets: [e.to],
      labels: e.label ? [{ text: e.label, width: e.label.length * 11 * 0.6 + 4, height: 14 }] : [],
    })),
  };
  const out = await elk.layout(graph);
  (window.__layouts ??= {})[el.id] = out;
  const svg = document.createElementNS(NS, "svg");
  svg.setAttribute("viewBox", `0 0 ${out.width} ${out.height}`);
  svg.setAttribute("width", out.width);
  svg.classList.add("dg");
  const defs = document.createElementNS(NS, "defs");
  defs.innerHTML = `<marker id="arr-${el.id}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" class="arrowhead"/></marker>`;
  svg.append(defs);
  const layer = () => svg.appendChild(document.createElementNS(NS, "g"));
  const groupLayer = layer(), edgeLayer = layer(), leafLayer = layer(), labelLayer = layer();
  const abs = new Map(); // node id -> absolute x, y
  const nodeEls = new Map();
  const drawNode = (n, ox, oy, depth) => {
    const x = ox + n.x, y = oy + n.y;
    abs.set(n.id, { x, y });
    const src = byId.get(n.id);
    const g = document.createElementNS(NS, "g");
    g.dataset.id = n.id;
    const r = document.createElementNS(NS, "rect");
    Object.entries({ x, y, width: n.width, height: n.height, rx: n.children?.length ? 10 : 6 }).forEach(([k, v]) => r.setAttribute(k, v));
    r.setAttribute("class", (n.children?.length ? "grp d" + Math.min(depth, 3) : "leaf") + (src.kind ? " k-" + src.kind : ""));
    g.append(r);
    const label = src.label ?? src.id;
    if (n.children?.length) {
      const t = document.createElementNS(NS, "text");
      t.setAttribute("x", x + 12); t.setAttribute("y", y + 19); t.setAttribute("class", "glabel");
      t.textContent = label; g.append(t);
    } else {
      [label, src.sub].filter(Boolean).forEach((line, i) => {
        const t = document.createElementNS(NS, "text");
        t.setAttribute("x", x + n.width / 2); t.setAttribute("y", y + 21 + 17 * i);
        t.setAttribute("text-anchor", "middle"); t.setAttribute("class", i ? "nsub" : "nlabel");
        t.textContent = line; g.append(t);
      });
      nodeEls.set(n.id, g);
    }
    (n.children?.length ? groupLayer : leafLayer).append(g);
    (n.children ?? []).forEach((c) => drawNode(c, x, y, depth + 1));
  };
  out.children.forEach((c) => drawNode(c, 0, 0, 0));
  const tip = document.createElement("div");
  tip.className = "dgtip";
  tip.textContent = spec.hint ?? "";
  const edgeEls = [];
  const drawEdges = (container, ox, oy) => {
    for (const e of container.edges ?? []) {
      const spec_e = spec.edges.filter((x) => byId.has(x.from) && byId.has(x.to))[+e.id.slice(1)];
      // ELK gives edge coordinates relative to the edge's container.
      const base = e.container && e.container !== "root" ? abs.get(e.container) : { x: 0, y: 0 };
      for (const s of e.sections ?? []) {
        const pts = [s.startPoint, ...(s.bendPoints ?? []), s.endPoint].map((p) => `${p.x + base.x},${p.y + base.y}`);
        const pl = document.createElementNS(NS, "polyline");
        pl.setAttribute("points", pts.join(" "));
        pl.setAttribute("class", "edge " + (spec_e.style ?? ""));
        pl.setAttribute("marker-end", `url(#arr-${el.id})`);
        pl.dataset.from = spec_e.from; pl.dataset.to = spec_e.to;
        edgeLayer.append(pl);
        edgeEls.push(pl);
        // a wide invisible twin so a thin edge is easy to hover
        const hit = pl.cloneNode(); hit.setAttribute("class", "hit"); hit.removeAttribute("marker-end");
        edgeLayer.append(hit);
        const name = (id) => byId.get(id).label ?? id;
        const say = () => { focusEdge(pl); tip.textContent = `${name(spec_e.from)} → ${name(spec_e.to)}` + (spec_e.tip ? ": " + spec_e.tip : spec_e.label ? ": " + spec_e.label : ""); };
        hit.addEventListener("mouseenter", say); hit.addEventListener("click", say);
      }
      for (const l of e.labels ?? []) {
        const t = document.createElementNS(NS, "text");
        t.setAttribute("x", l.x + base.x); t.setAttribute("y", l.y + base.y + 11);
        t.setAttribute("class", "elabel"); t.textContent = l.text;
        labelLayer.append(t);
      }
    }
    for (const c of container.children ?? []) drawEdges(c, ox, oy);
  };
  drawEdges(out, 0, 0);
  const wrap = document.createElement("div");
  wrap.className = "dgwrap";
  wrap.append(svg);
  el.replaceChildren(wrap, tip);
  fit(svg, out.width);
  function focusEdge(pl) {
    svg.classList.add("focused");
    for (const p of edgeEls) p.classList.toggle("on", p === pl);
    for (const [nid, g] of nodeEls) g.classList.toggle("on", nid === pl.dataset.from || nid === pl.dataset.to);
  }
  const focus = (id) => {
    svg.classList.toggle("focused", !!id);
    const near = new Set([id]);
    for (const p of edgeEls) {
      const on = p.dataset.from === id || p.dataset.to === id;
      p.classList.toggle("on", on);
      if (on) { near.add(p.dataset.from); near.add(p.dataset.to); }
    }
    for (const [nid, g] of nodeEls) g.classList.toggle("on", near.has(nid));
    const src = byId.get(id);
    tip.textContent = id ? (src.label ?? src.id) + (src.tip ? ": " + src.tip : "") : spec.hint ?? "";
  };
  for (const [id, g] of nodeEls) {
    g.addEventListener("mouseenter", () => focus(id));
    g.addEventListener("click", () => focus(id));
  }
  svg.addEventListener("mouseleave", () => focus(null));
}

// Shrink a diagram to the space it has, down to 70 %; past that it keeps that size and scrolls.
// Natural size where it fits; scaled down with its container to 70%, then scrolled (see svg.dg in the page's CSS).
function fit(svg, natural) {
  svg.setAttribute("width", natural);
  svg.style.setProperty("--least", `${Math.round(natural * 0.7)}px`);
}
