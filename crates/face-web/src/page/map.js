import { ago, when } from "./base.js";
import { layered } from "./layout.js";

// ─── The model: the view as a map ──────────────────────────────────────────
// The map is a graph of how contexts began, compacted, forked, spawned,
// joined and were edited: the view's legs (`legs`, `leg` and `work` on each
// task). A node is a leg, a stretch of one task's context; an edge points
// from the newer node back to what it depends on. Background work that is
// not a spawn is a chip, hung from the leg that started it, and a task
// with work out has a planned join under it, depending on that work, until
// the work's results come in: on every member of a scope that still runs,
// a finished one or an aside included. Projects are the graph's connected
// pieces.
//
// Each live task is big: its planned join while a member of its runs or it
// waits on its work, so it sits below what it will take in, hollow only
// while it waits; its current leg otherwise. Everything else is a small
// dot. A task's own node (big, or its last leg once it is done) has the
// task's id, so the card, the queue and the URL name it the way they name
// the task.
export let view = null;
// The tab's rows, one per object, as the page draws them.
function assemble(v) {
  const out = { as_of: v.as_of, seq: Number(v.as_of.split(".")[1]), tasks: [], queue: [], drafts: [], roles: [], workspaces: [] };
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

// What a task is doing, as the map shows it: classes for its dot, and the
// words its hover says.
export function look(t) {
  const w = t.waiting || {};
  const since = w.since != null ? ` for ${ago(w.since)}` : "";
  const stopping = t.stop && w.kind !== "done" ? " cancelling" : "";
  switch (w.kind) {
    case "model": return { cls: (w.streaming ? "working streaming" : "working") + stopping, text: (stopping ? "Abandoning: " : "") + (w.streaming ? "Writing" : "Thinking") + since };
    case "calls": {
      const kids = w.calls.flatMap((c) => c.children);
      if (kids.length) {
        const running = kids.filter((k) => byId[k] && byId[k].waiting?.kind !== "done").length;
        return { cls: "children" + stopping, text: `Waiting on ${running} of ${kids.length}${since}` };
      }
      const c = w.calls[0] || {};
      return { cls: "working tool" + stopping, text: `${c.tool || "Tools"}` };
    }
    case "max": return { cls: "needs" + stopping, text: "Your turn" + since };
    case "starting": return { cls: "starting", text: w.limbs.length ? `Making ${w.limbs.join(", ")}` : "Starting" };
    case "harness": return { cls: "working" + stopping, text: "In the harness" + since };
    case "held": { const h = heldBy(t); return { cls: "paused", text: h && h !== t ? `Paused with ${h.name}` : "Paused" }; }
    case "parked": return { cls: "paused", text: "Parked" + since };
    case "limited": return { cls: "stuck", text: `Limited until ${when(w.until)}` };
    case "signedOut": return { cls: "stuck", text: "Waits for a sign-in" };
    case "listening": return { cls: "listening" + stopping, text: w.calls.length ? `${w.calls.length} in the background` : "Listening" };
    case "done": {
      const o = t.outcome || {};
      if (o.kind === "failed") return { cls: "done failed", text: `Failed ${ago(w.since)} ago` };
      if (o.kind === "abandoned") return { cls: "done cancelled", text: `Abandoned ${ago(w.since)} ago` };
      return { cls: "done", text: `Done ${ago(w.since)} ago` };
    }
  }
  return { cls: "", text: w.kind || "" };
}
// Under its owner, a task's name without the owner's name it starts with.
export const shortName = (t) => { const o = byId[parentOf(t)]; return o && t.name.startsWith(o.name + "-") ? t.name.slice(o.name.length + 1) : t.name; };
export const glyph = (t) => (t.role || t.name || "?").trim()[0].toUpperCase();
// Tasks in the order they were created.
export const order = (a, b) => a.created.seq - b.created.seq;
export const isDone = (t) => t.waiting?.kind === "done";
// A resume Max made of one of his own chats: the same chat, continued.
const chained = (t) => t.origin === "resume" && !t.owner && byId[t.from] && !byId[t.from].owner;
// Who a task belongs to: its scope's owner, or the task a resume (not one
// of Max's own chats continued) carries on from.
export const parentOf = (t) => t.owner ? t.owner : t.from && byId[t.from] && !chained(t) ? t.from : null;
// Where one of Max's chats went on: the newest resume of it, followed.
export function successor(id) {
  for (let seen = 0, next = id; seen < 50; seen++) {
    const r = view.tasks.filter((t) => t.from === next && chained(t)).sort(order).at(-1);
    if (!r) return next;
    next = r.id;
  }
  return id;
}
const ancestry = (t) => { const out = []; for (let p = parentOf(t); p && byId[p] && out.length < 50; p = parentOf(byId[p])) out.unshift(byId[p]); return out; };
// What pauses a task: itself, or the nearest paused task above it.
export const heldBy = (t) => !t.pause ? null : t.pause.by === t.id ? t : ancestry(t).find((a) => a.id === t.pause.by) || null;

// What a task's blob shows (blobs.js's states). Paused, it waits on Max
// to resume it, not on its children, though they finish what they run.
export function stateOf(t) {
  const w = t.waiting || {};
  switch (w.kind) {
    case "model": return w.streaming ? "streaming" : "first";
    case "calls": return !waitsOnKids(t) ? "tools" : t.pause ? "paused" : "children";
    case "max": return "max";
    case "done": return "done";
    case "starting": return "starting";
    case "held": case "parked": return "paused";
    case "limited": case "signedOut": return "held";
    case "listening": return "listening";
  }
  return "brain";
}
// Waiting on its children: on a spawn, or a `wait` on one.
const waitsOnKids = (t) => t.waiting?.kind === "calls" && t.waiting.calls.some((c) => c.children?.length);
// Waiting on its work: its planned join is big and hollow.
const waitsOnWork = (t) => t.waiting?.kind === "listening" || waitsOnKids(t);

// A finished project folds to its root once it has been finished a while,
// or at once if Max abandoned it (his chat as he last left it, so a
// reply to it, which carries it on, brings it back): a small card on the archive shelf, below
// everything live.
const FOLD_AFTER = 30 * 60 * 1000;
export const unfolded = new Set();
export const cardStatus = (c) => `Finished ${ago(c.last)} ago`;

// Each task's legs into nodes, its work into chips, a planned join for
// work out, the chains of compactions folded, and the projects.
export function derive(openId) {
  byId = Object.fromEntries(view.tasks.map((t) => [t.id, t]));
  const tasks = [...view.tasks].sort(order);
  const nodes = new Map();
  const add = (n) => { nodes.set(n.id, n); return n; };
  for (const t of tasks) {
    const legs = t.legs || [];
    const mine = new Set(legs.map((l) => l.key));
    // The path to the head: from its leg, back along its own legs. Any
    // other leg of it is an old version.
    const path = new Set();
    for (let k = t.leg; k && !path.has(k);) {
      path.add(k);
      k = legs.find((l) => l.key === k)?.on.find((d) => mine.has(d.leg))?.leg;
    }
    for (const l of legs) {
      add({ id: l.key, kind: "leg", t, began: l.began, at: l.at, open: l.first, old: !path.has(l.key),
        carry: l.began === "carry" && path.has(l.key), deps: l.on.map((d) => d.leg ?? `w:${d.work}`) });
    }
    for (const w of t.work || []) add({ id: `w:${w.call}`, kind: "chip", t, work: w, running: w.running, open: w.at, deps: [w.leg] });
  }
  // A planned join under each live task with work out: members and calls
  // whose results it has not taken in yet, while any of them runs. A
  // finished member waits in it with the rest of its scope.
  const taken = new Map(tasks.map((t) => [t.id, new Set((t.legs || []).flatMap((l) => l.on.map((d) => d.leg ?? `w:${d.work}`)))]));
  const members = new Map();
  for (const t of tasks) if (t.owner && byId[t.owner]) (members.get(t.owner) ?? members.set(t.owner, []).get(t.owner)).push(t);
  const live = new Set(tasks.filter((t) => !isDone(t)).map((t) => t.scope));
  for (const t of tasks) {
    if (isDone(t) || !t.leg) continue;
    const seen = taken.get(t.id);
    const open = [], done = [];
    for (const m of members.get(t.id) || []) {
      if (m.leg && !seen.has(m.leg) && live.has(m.scope)) (isDone(m) ? done : open).push({ id: m.leg, at: m.legs?.[0]?.on.find((d) => d.leg)?.at });
    }
    const kids = open.length;
    for (const w of t.work || []) if (w.running && !seen.has(`w:${w.call}`)) open.push({ id: `w:${w.call}`, at: w.at });
    if (open.length) add({ id: `p:${t.id}`, kind: "planned", t, kids, waits: open.length, open: open[0].at, deps: [t.leg, ...open.map((o) => o.id), ...done.map((o) => o.id)] });
  }
  // Big: each live task's own node. A task's own node, big or not, takes
  // the task's id. A planned join big while its task runs on is solid, and
  // so is its line to that task's leg.
  const rename = new Map(), ownOf = new Map();
  for (const t of tasks) {
    const planned = nodes.get(`p:${t.id}`);
    const own = !isDone(t) && planned && (planned.kids || waitsOnWork(t)) ? planned : nodes.get(t.leg);
    if (!own) continue;
    if (!isDone(t)) own.big = stateOf(t);
    if (own === planned && !waitsOnWork(t)) own.solid = t.leg;
    rename.set(own.id, t.id);
    ownOf.set(t.leg, own.id);
  }
  // A planned join waits on each member's own node, so joins chain through
  // members that wait on their own work.
  for (const n of nodes.values()) {
    if (n.kind === "planned") n.deps = n.deps.map((d) => d === n.t.leg ? d : ownOf.get(d) ?? d);
  }
  // Nodes after everything they depend on: depth first, in the order they
  // began. Then the chains of compactions fold: a carry absorbs the leg
  // before it when that is its own task's, small, and nothing else hangs
  // from it.
  const began = (n) => n.at ?? n.t.created.at ?? 0;
  const sorted = [], state = new Map();
  const visit = (n) => {
    if (state.get(n.id)) return;
    state.set(n.id, 1);
    for (const d of n.deps) { const m = nodes.get(d); if (m) visit(m); }
    sorted.push(n);
  };
  for (const n of [...nodes.values()].sort((a, b) => began(a) - began(b))) visit(n);
  const uses = new Map();
  for (const n of sorted) for (const d of n.deps) uses.set(d, (uses.get(d) ?? 0) + 1);
  const kept = new Map();
  for (const n of sorted) {
    n.deps = n.deps.filter((d) => nodes.has(d));
    if (n.kind === "leg" && n.began === "carry") {
      const pd = n.deps.find((d) => kept.get(d)?.t === n.t);
      const p = pd && kept.get(pd);
      if (p && p.kind === "leg" && !p.big && !rename.has(p.id) && uses.get(p.id) === 1) {
        kept.delete(p.id);
        n.deps = [...p.deps, ...n.deps.filter((d) => d !== p.id)];
      }
    }
    kept.set(n.id, n);
  }
  // Renamed, and the pips: the compactions on the path to its head.
  const out = new Map();
  for (const n of kept.values()) {
    n.id = rename.get(n.id) ?? n.id;
    n.deps = n.deps.map((d) => rename.get(d) ?? d);
    if (n.big) n.pips = (n.t.legs || []).filter((l) => l.began === "carry" && nodes.get(l.key)?.carry).length;
    out.set(n.id, n);
  }
  const edges = [];
  for (const n of out.values()) for (const d of n.deps) if (out.has(d)) edges.push({ from: n.id, to: d });
  // Projects: the connected pieces.
  const up = new Map([...out.keys()].map((k) => [k, k]));
  const find = (k) => { while (up.get(k) !== k) { up.set(k, up.get(up.get(k))); k = up.get(k); } return k; };
  for (const e of edges) up.set(find(e.from), find(e.to));
  const comps = new Map();
  for (const n of out.values()) {
    const c = find(n.id);
    if (!comps.has(c)) comps.set(c, { nodes: [], tasks: new Set() });
    comps.get(c).nodes.push(n);
    comps.get(c).tasks.add(n.t);
  }
  const now = Date.now();
  const list = [...comps.values()].map((c) => {
    const ts = [...c.tasks].sort(order);
    const root = ts[0];
    const running = ts.some((t) => !isDone(t));
    const last = Math.max(...ts.map((t) => t.waiting?.since || 0));
    const over = byId[successor(root.id)].outcome?.kind === "abandoned" || now - last > FOLD_AFTER;
    const folded = !running && over && !unfolded.has(root.id) && !ts.some((t) => t.id === openId && t.id !== root.id);
    return { root, live: running, last, folded, nodes: c.nodes, tasks: ts };
  });
  list.sort((a, b) => order(a.root, b.root));
  graph = { nodes: out, edges, comps: list };
}

// ─── Layout: laid out again only where the shape changes ──────────────────
// A node's size, from what it is: a big node is a blob (blobs.js) in a
// BIG px box, reaching 12 px above its centre, 8 below and 21 to each side;
// its label sits to its right.
export const BIG = 64, BIG_REACH = { top: 12, foot: 8, side: 21 };
const SERIF = getComputedStyle(document.documentElement).getPropertyValue("--serif") || "Georgia, serif";
const SANS = getComputedStyle(document.documentElement).getPropertyValue("--sans") || "sans-serif";
export const LABEL_FONT = `600 12.5px ${SERIF}`, CHIP_FONT = `12px ${SANS}`;
export const radius = (n) => (n.kind === "chip" ? (n.running ? 7.5 : 2.4) : n.kind === "planned" ? 4.6 : 3.4);
export const label = (n) => (n.big ? shortName(n.t) : n.kind === "chip" && n.running ? n.work.name || "background" : "");
const widths = new Map();
const ctx2d = document.createElement("canvas").getContext("2d");
function measure(text, font) {
  const k = font + "\n" + text;
  if (!widths.has(k)) {
    if (ctx2d.font !== font) ctx2d.font = font;
    widths.set(k, ctx2d.measureText(text).width);
  }
  return widths.get(k);
}
// How far a node reaches, from its centre: across (its label to the right)
// and along the flow, and where lines meet it.
function extent(n) {
  const r = n.big ? BIG_REACH.side : radius(n), l = label(n);
  const lw = l ? measure(l, n.big ? LABEL_FONT : CHIP_FONT) + 7 : 0;
  return { lo: r + 3, hi: r + 3 + lw, along: n.big ? 2 * BIG_REACH.side - 6 : 2 * r + (l ? 4 : 0),
    top: n.big ? BIG_REACH.top : r + 2, foot: n.big ? BIG_REACH.foot : r + 3,
    now: (!!n.big && n.kind !== "planned") || (n.kind === "chip" && n.running), planned: n.kind === "planned" };
}

// A leg's life, in ms: from when it began to when the next leg of its task
// began, or its task finished, or now.
function life(n) {
  if (n.kind !== "leg" || typeof n.at !== "number") return undefined;
  const next = (n.t.legs || []).map((l) => l.at).filter((a) => typeof a === "number" && a > n.at);
  const end = next.length ? Math.min(...next) : isDone(n.t) ? n.t.waiting.since : Date.now();
  return [n.at, Math.max(n.at, end)];
}

export let positions = new Map();   // id -> {x, y, w, h, top}: its box laid out; its centre is (x + w / 2, y + top)
export let shown = new Map();       // id -> {x, y, w, h, top}: drawn now (tweened)
export let routes = new Map();      // "from>to" -> {pts, dashed, old}: an edge's route, laid out
export const centre = (p) => ({ x: p.x + p.w / 2, y: p.y + p.top });
let shape = "";
const laid = new Map();             // a project's shape -> its layout
const lastX = new Map();            // a project's root -> each node's x as last laid out

// Each project laid out on its own (layout.js), and again only when its
// shape changes. Projects sit side by side in the order they began, PROJECT
// px apart, their present rows on one line, y = 0. Finished projects' cards
// shelve below.
const PROJECT = 56, CARD_H = 34;
export function layout() {
  const parts = graph.comps.map((c) => {
    if (c.folded) {
      const w = Math.min(240, Math.max(120, measure(shortName(c.root), `500 13px ${SERIF}`) + 34 + (c.tasks.length > 1 ? 14 : 0)));
      return { c, sig: `card ${c.root.id} ${Math.round(w)}`, w };
    }
    const ns = c.nodes.map((n) => ({ id: n.id, deps: n.deps, ...extent(n), life: life(n), kind: n.kind, old: n.old, spent: n.kind === "chip" && !n.running, solid: n.solid }));
    let sig = "";
    for (const n of ns) sig += `${n.id}<${n.deps.join(",")}|${n.kind}${n.now ? "!" : ""}${n.old || n.spent ? "~" : ""}${n.solid ? "=" : ""}${Math.round(n.hi)};`;
    return { c, sig, ns };
  });
  const sig = parts.map((p) => p.sig).join("\n") + (innerWidth > 760);
  if (sig === shape) return false;
  shape = sig;
  const was = new Map(laid);
  laid.clear();
  positions = new Map();
  routes = new Map();
  let x = 0, y1 = 0;
  for (const p of parts) {
    if (p.c.folded) continue;
    let l = was.get(p.sig);
    if (!l) {
      l = layered(p.ns, lastX.get(p.c.root.id));
      l.x0 = Infinity; l.x1 = -Infinity; l.y1 = 0;
      for (const n of p.ns) { const q = l.at.get(n.id); l.x0 = Math.min(l.x0, q.x - n.lo); l.x1 = Math.max(l.x1, q.x + n.hi); l.y1 = Math.max(l.y1, q.y + n.along / 2); }
      lastX.set(p.c.root.id, new Map([...l.at].map(([id, q]) => [id, q.x])));
    }
    laid.set(p.sig, l);
    // Its boxes and lines where it sits, kept while it sits there, so
    // its lines are drawn once.
    const dx = x - l.x0;
    if (l.dx !== dx) {
      l.dx = dx;
      l.boxes = p.ns.map((n) => { const q = l.at.get(n.id), r = n.lo - 3; return [n.id, { x: q.x + dx - r, y: q.y - n.top, w: 2 * r, h: n.top + n.foot, top: n.top }]; });
      const by = new Map(p.ns.map((n) => [n.id, n]));
      // Dotted to what is planned, but for a solid one's line to its own leg; faint from an old version or finished background work.
      const planned = (id) => by.get(id).kind === "planned", faint = (id) => by.get(id).old || by.get(id).spent;
      const dashed = (e) => (planned(e.from) && by.get(e.from).solid !== e.to) || planned(e.to);
      l.lines = l.edges.map((e) => [`${e.from}>${e.to}`, { pts: e.route.map(([a, b]) => [a + dx, b]), dashed: dashed(e), old: faint(e.from) || faint(e.to) }]);
    }
    for (const [id, b] of l.boxes) positions.set(id, b);
    for (const [k, r] of l.lines) routes.set(k, r);
    y1 = Math.max(y1, l.y1);
    x = l.x1 + dx + PROJECT;
  }
  // The archive: its cards, most recently finished first, on shelves below.
  const cards = parts.filter((p) => p.c.folded).sort((a, b) => b.c.last - a.c.last);
  const room = Math.max(360, innerWidth - 40);
  // Shelved just below the fold of the first framing, which puts the present row two thirds down.
  let cx = 0, cy = Math.max(y1 + 80, innerHeight / 3 + 40);
  for (const p of cards) {
    if (cx && cx + p.w > room) { cx = 0; cy += CARD_H + 14; }
    positions.set(p.c.root.id, { x: cx, y: cy, w: p.w, h: CARD_H, top: CARD_H / 2, card: true });
    cx += p.w + 14;
  }
  return true;
}
// The fonts the labels are measured in, found while the view is on its way:
// the first measure of each can take a while.
setTimeout(() => { for (const f of [LABEL_FONT, CHIP_FONT, `500 13px ${SERIF}`]) measure("Harness", f); });
