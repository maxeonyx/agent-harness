// ─── The model: the view as a map ──────────────────────────────────────────
// A node is a task. A scope (the spawn call that owns its members) is a
// junction under its owner. Background work a task will hear back from
// hangs off it. Projects are the map's disconnected components.
//
// Edges: solid, a scope and its members (owned, settled together); dashed,
// a result that will arrive without anyone waiting for it now (background
// work); dotted, a task continued by a resume; flowing,
// waited on right now.
let view = null;
let byId = {};
let graph = { nodes: new Map(), edges: [], comps: [] };
// A chat of Max's he continued after it finished is one node: its newest
// resume, whose context holds all of it. Each earlier one maps to it.
let alias = {};
const live = (id) => alias[id] || id;

// What a task is doing, as the map shows it: classes for its dot, and the
// words its hover says.
function look(t) {
  const w = t.waiting || {};
  const since = w.since != null ? ` · ${ago(w.since)}` : "";
  const stopping = t.stop && w.kind !== "done" ? " cancelling" : "";
  switch (w.kind) {
    case "model": return { cls: (w.streaming ? "working streaming" : "working") + stopping, text: (stopping ? "cancelling: " : "") + (w.streaming ? "writing" : "thinking") + since };
    case "calls": {
      const kids = w.calls.flatMap((c) => c.children);
      if (kids.length) {
        const running = kids.filter((k) => byId[k] && byId[k].waiting?.kind !== "done").length;
        return { cls: "children" + stopping, text: `waiting on ${running} of ${kids.length}${since}` };
      }
      const c = w.calls[0] || {};
      return { cls: "working tool" + stopping, text: `${c.tool || "tools"}${c.input ? " " + gist(c.input) : ""}` };
    }
    case "max": return { cls: "needs" + stopping, text: "your turn" + since };
    case "starting": return { cls: "starting", text: w.limbs.length ? `making ${w.limbs.join(", ")}` : "starting" };
    case "harness": return { cls: "working" + stopping, text: "harness" + since };
    case "held": { const h = heldBy(t); return { cls: "paused", text: h && h !== t ? `paused with ${h.name}` : "paused" }; }
    case "parked": return { cls: "paused", text: "parked" + since };
    case "limited": return { cls: "paused", text: `limited until ${clock(w.until)}` };
    case "listening": return { cls: "listening" + stopping, text: w.calls.length ? `${w.calls.length} in the background` : "listening" };
    case "done": {
      const o = t.outcome || {};
      if (o.kind === "failed") return { cls: "done failed", text: `failed ${ago(w.since)} ago` };
      if (o.kind === "abandoned") return { cls: "done cancelled", text: `abandoned ${ago(w.since)} ago` };
      return { cls: "done", text: `done ${ago(w.since)} ago` };
    }
  }
  return { cls: "", text: w.kind || "" };
}
// Under its owner, a task's name without the owner's name it starts with.
const shortName = (t) => { const o = byId[parentOf(t)]; return o && t.name.startsWith(o.name + "-") ? t.name.slice(o.name.length + 1) : t.name; };
const glyph = (t) => (t.environment || t.name || "?").trim()[0].toUpperCase();
// Tasks in the order they were created.
const order = (a, b) => a.created.seq - b.created.seq;
const isDone = (t) => t.waiting?.kind === "done";
// A resume Max made of one of his own chats: the same chat, continued.
const chained = (t) => t.origin === "resume" && !t.owner && byId[t.from] && !byId[t.from].owner;
// Who a task belongs to on the map: its scope's owner, or the task a resume
// in Max's own scope continues.
const parentOf = (t) => t.owner ? live(t.owner) : t.from && byId[t.from] && !chained(t) ? live(t.from) : null;
const ancestry = (t) => { const out = []; for (let p = parentOf(t); p && byId[p] && out.length < 50; p = parentOf(byId[p])) out.unshift(byId[p]); return out; };
// What pauses a task: itself, or the nearest paused task above it.
const heldBy = (t) => !t.pause ? null : t.pause.by === t.id ? t : ancestry(t).find((a) => a.id === t.pause.by) || null;
// The calls a task is waiting on right now.
function awaited(t) {
  const w = t?.waiting || {};
  return new Set((w.calls || []).map((c) => c.id));
}

// A finished project folds to its root once it has been finished a while:
// a small card on the archive shelf, below everything live.
const FOLD_AFTER = 30 * 60 * 1000;
const unfolded = new Set();

function derive() {
  byId = Object.fromEntries(view.tasks.map((t) => [t.id, t]));
  alias = {};
  for (const t of [...view.tasks].sort(order)) {
    if (!chained(t)) continue;
    for (const [k, v] of Object.entries(alias)) if (v === t.from) alias[k] = t.id;
    alias[t.from] = t.id;
  }
  const nodes = new Map(), edges = [];
  for (const t of view.tasks) if (!alias[t.id]) nodes.set(t.id, { id: t.id, kind: "task", t });
  for (const [call, s] of Object.entries(view.scopes)) {
    const owner = live(s.owner);
    if (!byId[owner]) continue;
    const members = view.tasks.filter((t) => t.scope === call && !alias[t.id]).sort(order);
    const waiting = awaited(byId[s.owner]).has(call);
    nodes.set(`s:${call}`, { id: `s:${call}`, kind: "scope", call, scope: s, members });
    edges.push({ from: owner, to: `s:${call}`, kind: s.background ? "receives" : "owns", flow: waiting, made: s.made });
    for (const m of members) edges.push({ from: `s:${call}`, to: m.id, kind: "owns", flow: waiting && !isDone(m), made: m.created.seq });
  }
  for (const t of view.tasks) {
    if (alias[t.id]) continue;
    if (t.scope && !view.scopes[t.scope] && t.owner) edges.push({ from: live(t.owner), to: t.id, kind: "owns", made: t.created.seq });
    if (t.origin === "resume" && byId[t.from] && !chained(t)) edges.push({ from: live(t.from), to: t.id, kind: "continues" });
    const waits = awaited(t);
    for (const b of t.background || []) {
      if (b.status === "settled" || b.tool === "spawn") continue;
      const id = `b:${b.id}`;
      nodes.set(id, { id, kind: "sat", call: b, task: t.id });
      edges.push({ from: t.id, to: id, kind: "receives", flow: waits.has(b.id) || t.waiting?.kind === "listening" });
    }
  }
  // Components, by the edges between them.
  const up = new Map([...nodes.keys()].map((k) => [k, k]));
  const find = (k) => { while (up.get(k) !== k) { up.set(k, up.get(up.get(k))); k = up.get(k); } return k; };
  for (const e of edges) if (nodes.has(e.from) && nodes.has(e.to)) up.set(find(e.from), find(e.to));
  const comps = new Map();
  for (const n of nodes.values()) {
    const c = find(n.id);
    if (!comps.has(c)) comps.set(c, { nodes: [], tasks: [] });
    comps.get(c).nodes.push(n);
    if (n.kind === "task") comps.get(c).tasks.push(n.t);
  }
  const now = Date.now();
  const list = [...comps.values()].filter((c) => c.tasks.length).map((c) => {
    const root = c.tasks.find((t) => !parentOf(t)) || c.tasks[0];
    const running = c.tasks.some((t) => !isDone(t));
    const last = Math.max(...c.tasks.map((t) => t.waiting?.since || 0));
    const folded = !running && now - last > FOLD_AFTER && !unfolded.has(root.id) && !c.tasks.some((t) => t.id === openId && t.id !== root.id);
    return { root, live: running, last, folded, nodes: c.nodes, tasks: c.tasks };
  });
  list.sort((a, b) => (b.live - a.live) || order(a.root, b.root));
  graph = { nodes, edges, comps: list };
}

// ─── Layout: ELK, laid out again only when the shape changes ──────────────
const elk = window.ELK ? new ELK() : null;
const measure = (() => { const c = document.createElement("canvas").getContext("2d"); return (text, font) => { c.font = font; return c.measureText(text).width; }; })();
const DOT = 30, ROOT_DOT = 38, SAT_DOT = 20;
let positions = new Map();   // id -> {x, y, w, h}: laid out
let shown = new Map();       // id -> {x, y}: drawn now (tweened)
let shape = "";

const CARD_H = 34;
const cardStatus = (c) => `finished ${ago(c.last)} ago · ${c.tasks.length} task${c.tasks.length > 1 ? "s" : ""}`;
function sizeOf(n, comp) {
  if (comp.folded) return { w: Math.min(240, Math.max(120, measure(shortName(n.t), "500 13px Georgia") + 34 + (comp.tasks.length > 1 ? 14 : 0))), h: CARD_H };
  if (n.kind === "scope") return { w: 12, h: 12 };
  if (n.kind === "sat") return { w: Math.min(160, Math.max(90, measure(satLabel(n), "12px sans-serif") + 10)), h: SAT_DOT + 22 };
  const root = comp.root.id === n.id;
  const w = Math.min(210, Math.max(110, measure(shortName(n.t), `600 ${root ? 15.5 : 13.5}px Georgia`) + 16));
  return { w, h: (root ? ROOT_DOT : DOT) + (root ? 26 : 24) };
}
const satLabel = (n) => `${n.call.tool} ${gist(n.call.input)}`.trim();

// Each project is laid out on its own, then the projects are shelved in
// reading order, live ones first, in rows as wide as the screen: on a phone
// they stack, and a project keeps its shape wherever it lands.
const ELK_OPTIONS = {
  "elk.algorithm": "layered", "elk.direction": "DOWN",
  "elk.spacing.nodeNode": "26", "elk.layered.spacing.nodeNodeBetweenLayers": "34",
  "elk.layered.considerModelOrder.strategy": "NODES_AND_EDGES",
  "elk.layered.crossingMinimization.forceNodeModelOrder": "true",
  "elk.layered.nodePlacement.strategy": "BRANDES_KOEPF",
  "elk.layered.nodePlacement.bk.fixedAlignment": "BALANCED",
  "elk.padding": "[top=0,left=0,bottom=0,right=0]",
};
const GAP = 90;
async function layout() {
  const parts = graph.comps.map((c) => {
    if (c.folded) { const z = sizeOf(graph.nodes.get(c.root.id), c); return { archived: c.last, children: [{ id: c.root.id, width: z.w, height: z.h }], edges: [] }; }
    const ns = c.folded ? [graph.nodes.get(c.root.id)] : depthFirst(c);
    const ids = new Set(ns.map((n) => n.id));
    const children = ns.map((n) => { const z = sizeOf(n, c); return { id: n.id, width: z.w, height: z.h }; });
    // Only what owns what is laid out, so each project is a tree; a
    // continuation is drawn between wherever its two ends land.
    const edges = graph.edges.filter((e) => e.kind !== "continues" && ids.has(e.from) && ids.has(e.to)).map((e, i) => ({ id: `e${i}`, sources: [e.from], targets: [e.to] }));
    return { children, edges };
  });
  const sig = JSON.stringify([parts.map((p) => [p.children.map((c) => [c.id, Math.round(c.width), c.height]), p.edges.map((e) => [e.sources[0], e.targets[0]])]), Math.round(innerWidth / 200)]);
  if (sig === shape) return false;
  shape = sig;
  const live = parts.filter((p) => p.archived == null);
  const archive = parts.filter((p) => p.archived != null).sort((a, b) => b.archived - a.archived);
  const laid = await Promise.all(live.map(async (p) => {
    if (!elk) { let x = 0; return { w: 0, h: 0, nodes: p.children.map((c) => { const n = { ...c, x, y: 0 }; x += c.width + 30; return n; }) }; }
    const out = await elk.layout({ id: "p", layoutOptions: ELK_OPTIONS, children: p.children, edges: p.edges });
    return { w: out.width, h: out.height, nodes: out.children };
  }));
  const cards = archive.map((p) => { const c = p.children[0]; return { w: c.width, h: c.height, nodes: [{ ...c, x: 0, y: 0 }] }; });
  // Shelves as wide as the screen at an easy reading zoom; the archive's
  // cards, most recently finished first, on shelves of their own below.
  const row = Math.max((innerWidth - 80) / (phone() ? .75 : 1), ...laid.map((l) => l.w));
  const shelves = [];
  const shelve = (items, gap) => {
    const first = shelves.length;
    for (const l of items) {
      const last = shelves.length > first ? shelves[shelves.length - 1] : null;
      if (last && last.w + gap + l.w <= row) { last.items.push(l); last.w += gap + l.w; last.h = Math.max(last.h, l.h); }
      else shelves.push({ items: [l], w: l.w, h: l.h, gap });
    }
  };
  shelve(laid, GAP);
  if (cards.length && laid.length) shelves.push({ items: [], w: 0, h: GAP / 2, gap: 0 });
  shelve(cards, 14);
  // Each shelf centred under the widest.
  const widest = Math.max(0, ...shelves.map((r) => r.w));
  positions = new Map();
  let y = 0;
  for (const r of shelves) {
    let x = (widest - r.w) / 2;
    for (const l of r.items) {
      for (const c of l.nodes) positions.set(c.id, { x: x + c.x, y: y + c.y, w: c.width, h: c.height });
      x += l.w + r.gap;
    }
    y += r.h + (r.gap === GAP ? GAP : 14);
  }
  return true;
}
// A component's nodes, owners before what they own, in a stable order.
function depthFirst(c) {
  const inComp = new Set(c.nodes.map((n) => n.id));
  const out = [], seen = new Set();
  const kids = (id) => graph.edges.filter((e) => e.from === id && e.kind !== "continues" && inComp.has(e.to))
    .sort((a, b) => (a.made ?? 0) - (b.made ?? 0));
  const walk = (id) => { if (seen.has(id)) return; seen.add(id); out.push(graph.nodes.get(id)); for (const e of kids(id)) walk(e.to); };
  walk(c.root.id);
  for (const n of c.nodes) walk(n.id);
  return out;
}

