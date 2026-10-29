import { ago, clock, gist, phone } from "./base.js";
import { openId } from "./card.js";

// ─── The model: the view as a map ──────────────────────────────────────────
// A node is a task, under the task that owns it. Background work a task
// will hear back from hangs off it. Projects are the map's disconnected
// components.
//
// Edges: solid, a task and a member of a spawn it waits on (owned, settled
// together); dashed, a result that will arrive without anyone waiting for
// it now (background work); dotted, a task continued by a resume; flowing,
// waited on right now.
export let view = null;
// The tab's rows, one per object, as the page draws them.
function assemble(v) {
  const out = { as_of: v.as_of, seq: Number(v.as_of.split(".")[1]), tasks: [], queue: [], drafts: [], environments: [], workspaces: [] };
  for (const [key, row] of Object.entries(v.rows)) {
    const id = key.slice(key.indexOf(":") + 1);
    if (row.Task) out.tasks.push(row.Task);
    else if (row.Queue) out.queue = row.Queue;
    else if (row.Harness) Object.assign(out, row.Harness);
    else if (row.Draft) out.drafts.push({ id, ...row.Draft });
  }
  return out;
}
export function takeView(v) { view = assemble(v); }

export let byId = {};
export let graph = { nodes: new Map(), edges: [], comps: [] };
// A chat of Max's he continued after it finished is one node: its newest
// resume, whose context holds all of it. Each earlier one maps to it.
let alias = {};
export const live = (id) => alias[id] || id;

// What a task is doing, as the map shows it: classes for its dot, and the
// words its hover says.
export function look(t) {
  const w = t.waiting || {};
  const since = w.since != null ? ` · ${ago(w.since)}` : "";
  const stopping = t.stop && w.kind !== "done" ? " cancelling" : "";
  switch (w.kind) {
    case "model": return { cls: (w.streaming ? "working streaming" : "working") + stopping, text: (stopping ? "abandoning: " : "") + (w.streaming ? "writing" : "thinking") + since };
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
    case "signedOut": return { cls: "paused", text: "waits for a sign-in" };
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
export const shortName = (t) => { const o = byId[parentOf(t)]; return o && t.name.startsWith(o.name + "-") ? t.name.slice(o.name.length + 1) : t.name; };
export const glyph = (t) => (t.environment || t.name || "?").trim()[0].toUpperCase();
// Tasks in the order they were created.
export const order = (a, b) => a.created.seq - b.created.seq;
export const isDone = (t) => t.waiting?.kind === "done";
// A resume Max made of one of his own chats: the same chat, continued.
const chained = (t) => t.origin === "resume" && !t.owner && byId[t.from] && !byId[t.from].owner;
// Who a task belongs to on the map: its scope's owner, or the task a resume
// in Max's own scope continues.
export const parentOf = (t) => t.owner ? live(t.owner) : t.from && byId[t.from] && !chained(t) ? live(t.from) : null;
const ancestry = (t) => { const out = []; for (let p = parentOf(t); p && byId[p] && out.length < 50; p = parentOf(byId[p])) out.unshift(byId[p]); return out; };
// What pauses a task: itself, or the nearest paused task above it.
export const heldBy = (t) => !t.pause ? null : t.pause.by === t.id ? t : ancestry(t).find((a) => a.id === t.pause.by) || null;
// The calls a task is waiting on right now.
function awaited(t) {
  const w = t?.waiting || {};
  return new Set((w.calls || []).map((c) => c.id));
}

// A finished project folds to its root once it has been finished a while:
// a small card on the archive shelf, below everything live.
const FOLD_AFTER = 30 * 60 * 1000;
export const unfolded = new Set();

export function derive() {
  byId = Object.fromEntries(view.tasks.map((t) => [t.id, t]));
  alias = {};
  for (const t of [...view.tasks].sort(order)) {
    if (!chained(t)) continue;
    for (const [k, v] of Object.entries(alias)) if (v === t.from) alias[k] = t.id;
    alias[t.from] = t.id;
  }
  const nodes = new Map(), edges = [];
  for (const t of view.tasks) if (!alias[t.id]) nodes.set(t.id, { id: t.id, kind: "task", t });
  for (const t of view.tasks) {
    if (alias[t.id]) continue;
    // A spawn's members hang from their owner.
    if (t.owner && byId[live(t.owner)]) {
      const flow = awaited(byId[t.owner]).has(t.scope) && !isDone(t);
      edges.push({ from: live(t.owner), to: t.id, kind: "owns", flow, made: t.created.seq });
    }
    if (t.origin === "resume" && byId[t.from] && !chained(t)) edges.push({ from: live(t.from), to: t.id, kind: "continues" });
    const waits = awaited(t);
    for (const b of [...(t.background || []), ...(t.handed || [])]) {
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
export const DOT = 30, ROOT_DOT = 38, SAT_DOT = 20;
export let positions = new Map();   // id -> {x, y, w, h}: laid out
export let shown = new Map();       // id -> {x, y}: drawn now (tweened)
export let routes = new Map();      // "from>to" -> the points of its edge's curve, laid out
let shape = "";

const CARD_H = 34;
export const cardStatus = (c) => `finished ${ago(c.last)} ago · ${c.tasks.length} task${c.tasks.length > 1 ? "s" : ""}`;
function sizeOf(n, comp) {
  if (comp.folded) return { w: Math.min(240, Math.max(120, measure(shortName(n.t), "500 13px Georgia") + 34 + (comp.tasks.length > 1 ? 14 : 0))), h: CARD_H };
  if (n.kind === "sat") return { w: Math.min(160, Math.max(90, measure(satLabel(n), "12px sans-serif") + 10)), h: SAT_DOT + 22 };
  const root = comp.root.id === n.id;
  const w = Math.min(210, Math.max(110, measure(shortName(n.t), `600 ${root ? 15.5 : 13.5}px Georgia`) + 16));
  return { w, h: (root ? ROOT_DOT : DOT) + (root ? 26 : 24) };
}
export const satLabel = (n) => `${n.call.owner === "max" ? "yours: " : ""}${n.call.name || `${n.call.tool} ${gist(n.call.input)}`}`.trim();

// Each project is laid out on its own, as the graph it is: every edge, of
// whatever kind, holds what it ends at below where it starts. No row holds
// more than fits the screen, so a task with many children wraps them onto
// rows below. Then the projects are shelved in reading order, live ones
// first, in rows as wide as the screen: on a phone they stack, and a
// project keeps its shape wherever it lands. A project is laid out again
// only when its own shape changes.
const NODE_GAP = 26, EDGE_GAP = 12;
const ELK_OPTIONS = {
  "elk.algorithm": "layered", "elk.direction": "DOWN",
  "elk.layered.layering.strategy": "INTERACTIVE", "elk.edgeRouting": "SPLINES",
  "elk.spacing.edgeEdge": `${EDGE_GAP / 2}`, "elk.spacing.edgeNode": `${EDGE_GAP / 2}`,
  "elk.spacing.nodeNode": `${NODE_GAP}`, "elk.layered.spacing.nodeNodeBetweenLayers": "34",
  "elk.layered.considerModelOrder.strategy": "NODES_AND_EDGES",
  "elk.layered.crossingMinimization.forceNodeModelOrder": "true",
  "elk.layered.nodePlacement.strategy": "NETWORK_SIMPLEX",
  "elk.padding": "[top=0,left=0,bottom=0,right=0]",
};
const GAP = 90;
let laidOut = new Map();  // a project's shape -> where its nodes go
export async function layout() {
  // Shelves as wide as the screen at an easy reading zoom.
  const screen = (innerWidth - 80) / (phone() ? .75 : 1);
  const parts = graph.comps.map((c) => {
    if (c.folded) { const z = sizeOf(graph.nodes.get(c.root.id), c); return { archived: c.last, children: [{ id: c.root.id, width: z.w, height: z.h }], edges: [] }; }
    const ns = depthFirst(c);
    const ids = new Set(ns.map((n) => n.id));
    const children = ns.map((n) => { const z = sizeOf(n, c); return { id: n.id, width: z.w, height: z.h }; });
    const edges = graph.edges.filter((e) => ids.has(e.from) && ids.has(e.to)).map((e, i) => ({ id: `e${i}`, sources: [e.from], targets: [e.to] }));
    const room = Math.round(screen);
    const sig = JSON.stringify([children.map((c) => [c.id, Math.round(c.width), c.height]), edges.map((e) => [e.sources[0], e.targets[0]]), room]);
    return { children, edges, room, sig };
  });
  const sig = JSON.stringify([parts.map((p) => p.sig || p.children[0].id), Math.round(innerWidth / 200)]);
  if (sig === shape) return false;
  shape = sig;
  const live = parts.filter((p) => p.archived == null);
  const archive = parts.filter((p) => p.archived != null).sort((a, b) => b.archived - a.archived);
  const was = laidOut;
  laidOut = new Map();
  const laid = await Promise.all(live.map(async (p) => {
    let l = was.get(p.sig);
    if (!l && !elk) { let x = 0; l = { w: 0, h: 0, nodes: p.children.map((c) => { const n = { ...c, x, y: 0 }; x += c.width + 30; return n; }) }; }
    if (!l) {
      const out = await elk.layout({ id: "p", layoutOptions: ELK_OPTIONS, children: rows(p.children, p.edges, p.room), edges: p.edges });
      const at = (q) => ({ x: q.x, y: q.y });
      const curves = out.edges.flatMap((e) => (e.sections || []).map((s) => [`${e.sources[0]}>${e.targets[0]}`, [at(s.startPoint), ...(s.bendPoints || []).map(at), at(s.endPoint)]]));
      l = { w: out.width, h: out.height, nodes: out.children, curves };
    }
    laidOut.set(p.sig, l);
    return l;
  }));
  const cards = archive.map((p) => { const c = p.children[0]; return { w: c.width, h: c.height, nodes: [{ ...c, x: 0, y: 0 }] }; });
  // The archive's cards, most recently finished first, on shelves of their own below.
  const row = Math.max(screen, ...laid.map((l) => l.w));
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
  routes = new Map();
  let y = 0;
  for (const r of shelves) {
    let x = (widest - r.w) / 2;
    for (const l of r.items) {
      for (const c of l.nodes) positions.set(c.id, { x: x + c.x, y: y + c.y, w: c.width, h: c.height });
      for (const [k, pts] of l.curves || []) routes.set(k, pts.map((q) => ({ x: x + q.x, y: y + q.y })));
      x += l.w + r.gap;
    }
    y += r.h + (r.gap === GAP ? GAP : 14);
  }
  return true;
}
// The project's nodes, each given the row it goes on: the first below every
// node with an edge into it that has room, taking them in order, so a
// task's children fill the rows under it as words fill lines.
function rows(children, edges, room) {
  const row = new Map(), wide = [];
  const into = (id) => edges.filter((e) => e.targets[0] === id).map((e) => e.sources[0]);
  for (let left = children; left.length;) {
    // The first in order whose sources all have rows (or any, in a cycle).
    const c = left.find((n) => into(n.id).every((s) => row.has(s))) || left[0];
    left = left.filter((n) => n !== c);
    const from = into(c.id).filter((s) => row.has(s)).map((s) => row.get(s));
    let r = Math.max(0, ...from.map((s) => s + 1));
    while (wide[r] && wide[r] + c.width > room) r++;
    wide[r] = (wide[r] || 0) + c.width + NODE_GAP;
    // An edge past a row takes a little of its room.
    for (const s of from) for (let k = s + 1; k < r; k++) wide[k] = (wide[k] || 0) + EDGE_GAP;
    row.set(c.id, r);
  }
  return children.map((c) => ({ ...c, x: 0, y: row.get(c.id) * 100 }));
}
// A component's nodes, owners before what they own, in a stable order.
function depthFirst(c) {
  const inComp = new Set(c.nodes.map((n) => n.id));
  const out = [], seen = new Set();
  const kids = (id) => graph.edges.filter((e) => e.from === id && inComp.has(e.to))
    .sort((a, b) => (a.made ?? 0) - (b.made ?? 0));
  const walk = (id) => { if (seen.has(id)) return; seen.add(id); out.push(graph.nodes.get(id)); for (const e of kids(id)) walk(e.to); };
  walk(c.root.id);
  for (const n of c.nodes) walk(n.id);
  return out;
}

