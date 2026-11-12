// ─── The map's layout: lanes, and their lines ──────────────────────────────
// Each life (a chat, an agent, a background call, an old version) keeps one
// x, its lane, and its own line runs straight down it between its dots;
// lanes pack as close as the rows they share allow. A life of one dot that
// its maker's next dot reads hangs beside its maker instead, a spindle. A
// spawn bends from its maker all the way into the child's first dot; a
// result bends from the last dot all the way into its reader. Rows are a
// layered graph's, flowing down, with the present on one row at y = 0:
// every live node sits on the lowest row the past reaches, but one that a
// live node depends on sits just above it, and what waits hangs below. Nothing here asks the
// browser for geometry: sizes come in with the nodes, and lines are points.
//
// A node: { id, deps, lane, light, old, big, r, lo, hi, along, top, foot,
// now, planned }. `lane` names its life, `light` a tool's, `old` an old
// version's; `r` is a dot's radius, and a big node reaches `top`
// above its centre and `foot` below. `lo` and `hi` are how far it reaches
// left and right of its centre (its label is to the right), `along` how
// tall it is. `now` is a live node, `planned` one that waits on what it
// depends on. `time`, if known, is [when it began, when it ended or now], in
// ms. Nodes come after everything they depend on.

// A lane keeps PITCH from the next lane on a row they share, or more where
// a node and its label need CLEAR to it. A life of one dot has no lane: it
// hangs beside its maker's, SPIN clear of what is next to it there.
const PITCH = 20, CLEAR = 8, SPIN = 5;
// An agent's line is LIFE wide, a tool's (`light`) TOOL. Each runs TUCK
// under the edge of its dots. A taper runs from TIP up to its line's width
// over at most TAPER, then keeps it; it narrows to a point over its last
// POINT, which stops NGAP short of the dot it starts from or rejoins. A
// crossing's gap is GAP each side of the line over it.
export const LIFE = 2.2;
const TOOL = 1.2;
const GAP = 1.5;
const TIP = .5, POINT = 4, NGAP = 1.5, TUCK = -.6, TAPER = 48;
// Ports: the lines out of a node's bottom, and into its top, spread evenly
// over the WIDE either side of upright, its own lifeline upright in the
// middle and each other line on the side facing its other end, in the
// order the lines arrive; both sides one step apart. At its port a line
// points LEAN of the way from upright to straight out from the middle.
const WIDE = Math.PI / 3, LEAN = .5;
// A line leaving or reaching a port at a slant bulges at most REACH times
// as far across as it goes.
const REACH = 1;
// A big node's half width, for where lines meet it.
const BIG_HALF = 13;

// One project laid out: each node's centre, x about its first lane and
// y = 0 on the present row, and each edge's line; and every crossing.
export function lanes(nodes) {
  const ids = new Set(nodes.map((n) => n.id));
  const V = nodes.map((n, i) => ({ n, id: n.id, key: i, deps: n.deps.filter((d) => ids.has(d)) }));
  const by = new Map(V.map((v) => [v.id, v]));
  const P = rows(V, by);
  const edges = [];
  for (const v of V) for (const d of v.deps) edges.push({ v, d: by.get(d) });
  const L = lanesOf(V, edges);
  columns(L);
  spindles(L);
  across(L);
  // Down: each row as tall as its tallest node plus 17, the present row at
  // y = 0.
  const tall = [], ys = [0];
  for (const v of V) tall[v.rank] = Math.max(tall[v.rank] ?? 0, v.n.along);
  for (let r = 1; r < tall.length; r++) ys[r] = ys[r - 1] + (tall[r - 1] ?? 0) / 2 + (tall[r] ?? 0) / 2 + 17;
  for (const v of V) { v.y = ys[v.rank] - ys[P]; v.x = v.lane.spin ? v.lane.from.lane.x + v.lane.off : v.lane.x; }
  const sides = ports(edges);
  const at = [];
  for (const v of V) (at[v.rank] ??= []).push(v);
  const drawn = new Map();
  for (const e of edges) route(e, at, drawn);
  // Then each side's ports in the order its lines, as drawn, reach the
  // height where the shortest of them ends, and those lines drawn again.
  for (const { h, step } of sides) {
    if (h.length < 2) continue;
    const y = h[0].v.y + h[0].side * Math.min(...h.map((p) => Math.abs(p.far.y - p.v.y)));
    const out = (p) => Math.abs((xAt(p.e, y) ?? p.far.x) - p.v.x);
    const was = [...h];
    h.sort((a, b) => out(a) - out(b));
    if (h.every((p, i) => p === was[i])) continue;
    place(h, step);
    for (const p of h) { drawn.delete(p.e); route(p.e, at, drawn); }
  }
  const cuts = crossings(edges);
  return { at: new Map(V.map((v) => [v.id, { x: v.x, y: v.y }])), cuts, edges: edges.map((e, i) => ({ from: e.v.id, to: e.d.id, kind: e.kind, P: e.P, W: e.W, parts: cut(e, cuts.filter((c) => c.under === i)) })) };
}

// ─── Rows ──────────────────────────────────────────────────────────────────
// 1. One below the lowest dependency. 2. The present: every live node down
// to the lowest row the past reaches, but for one that a live node depends
// on (a running child of a parent that runs on), which sits just above it;
// a planned node one below what it waits on. 3. Everything else by its
// time, within its slack. Returns the present row.
function rows(V, by) {
  const below = (v) => { let r = -1; for (const d of v.deps) r = Math.max(r, by.get(d).rank); return r + 1; };
  for (const v of V) v.rank = below(v);
  let P = 0;
  for (const v of V) if (!v.n.planned && !v.n.now) P = Math.max(P, v.rank);
  const behind = new Set(), kids = new Map(V.map((v) => [v.id, []]));
  for (const v of V) for (const d of v.deps) { kids.get(d).push(v); if (v.n.now && by.get(d).n.now) behind.add(d); }
  for (const v of V) if ((v.n.now && !behind.has(v.id)) || v.n.planned) v.rank = Math.max(v.n.now ? P : 0, below(v));
  for (let i = V.length - 1; i >= 0; i--) {
    const v = V[i];
    if (behind.has(v.id)) v.rank = Math.max(below(v), Math.min(...kids.get(v.id).filter((k) => k.n.now).map((k) => k.rank)) - 1);
  }
  byTime(V, by, P, kids);
  return P;
}

// A node has slack: from one row below what it depends on (as early as it
// can be) to one row above what depends on it, or the present row if
// nothing does (as late as it can be). The critical path, which sets how
// many rows there are, and live and planned nodes have none. Every other
// node sits as far through the rows between its neighbours as the middle of
// its life is through the time between what it depends on ending and what
// depends on it beginning (or now), on the nearest row its slack allows. A
// node with no `time` began with what it depends on. A few sweeps, as its
// neighbours move with it.
function byTime(V, by, P, kids) {
  const fixed = (v) => v.n.now || v.n.planned;
  for (let i = V.length - 1; i >= 0; i--) {
    const v = V[i];
    let r = fixed(v) ? v.rank : P;
    for (const k of kids.get(v.id)) r = Math.min(r, k.late - 1);
    v.late = Math.max(v.rank, r);
    v.slack = v.late > v.rank && !fixed(v);
  }
  let now = -Infinity;
  for (const v of V) {
    let t = 0;
    for (const d of v.deps) t = Math.max(t, by.get(d).t0);
    [v.t0, v.t1] = v.n.time ?? [t, t];
    now = Math.max(now, v.t1);
  }
  for (let sweep = 0; sweep < 4; sweep++) {
    for (const v of V) {
      if (!v.slack) continue;
      let yA = 0, lo = 0, tA = v.deps.length ? -Infinity : v.t0;
      for (const d of v.deps) { const u = by.get(d); yA = Math.max(yA, u.rank); lo = Math.max(lo, u.rank + 1); tA = Math.max(tA, u.t1); }
      tA = Math.min(tA, v.t0);
      const ks = kids.get(v.id);
      let yB = ks.length ? Infinity : P, tB = ks.length ? Infinity : now;
      for (const k of ks) { yB = Math.min(yB, k.rank); tB = Math.min(tB, k.t0); }
      tB = Math.max(tB, tA);
      const f = tB > tA ? Math.min(1, Math.max(0, ((v.t0 + v.t1) / 2 - tA) / (tB - tA))) : 0.5;
      // Off its neighbours' rows when there is room.
      const g = Math.min(0.5, (yB - yA) / 2);
      const want = Math.min(yB - g, Math.max(yA + g, yA + f * (yB - yA)));
      v.rank = Math.min(v.late, Math.max(lo, Math.round(want)));
    }
  }
}

// ─── Lanes ─────────────────────────────────────────────────────────────────
// Each node's lane, and each edge's kind. `life`: a life's own line, from one
// of its dots to the next. `grow`: a life starting, from its maker into its
// first dot (`branch` for an old version, which keeps one width). `fade`: a
// life's result, from its last dot into its reader. `port`: a line out of a
// dot its life goes on from, into another life's. A lane is held from the
// row its life starts on to the row its last line leaves it. A life of one
// dot, made by another and read by its maker's next dot or by no one, with
// no other line, is a spindle: it hangs beside its maker's lane and holds
// none.
function lanesOf(V, edges) {
  const L = new Map();
  for (const v of V) {
    let l = L.get(v.n.lane);
    if (!l) L.set(v.n.lane, l = { nodes: [], kids: 0, spins: [] });
    l.nodes.push(v);
    v.lane = l;
  }
  for (const l of L.values()) { l.last = l.nodes.reduce((a, b) => (b.rank >= a.rank ? b : a)); l.r1 = l.last.rank; }
  for (const e of edges) {
    const { v, d } = e;
    if (v.lane !== d.lane) { v.fan = (v.fan ?? 0) + 1; d.fan = (d.fan ?? 0) + 1; }
    if (v.lane === d.lane) e.kind = "life";
    else if (v.lane.nodes[0] === v && !v.lane.from) { e.kind = v.n.old ? "branch" : "grow"; v.lane.from = d; }
    else if (d === d.lane.last) { e.kind = "fade"; d.lane.r1 = Math.max(d.lane.r1, v.rank); d.lane.reader = v.lane; }
    else e.kind = "port";
  }
  for (const l of L.values()) {
    const v = l.nodes[0];
    l.r0 = l.from ? l.from.rank : v.rank;
    l.light = v.n.light;
    l.spin = l.nodes.length === 1 && !!l.from && !v.n.old && (!l.reader || l.reader === l.from.lane) && v.fan === (l.reader ? 2 : 1)
      && !l.from.lane.nodes.some((u) => l.r0 < u.rank && u.rank < l.r1);
    if (l.spin) l.from.lane.spins.push(l);
  }
  return [...L.values()].sort((a, b) => a.r0 - b.r0 || a.nodes[0].key - b.nodes[0].key);
}

// Each lane's column, in the order they start. A column holds one life at
// a time. A life with no maker takes the first free column from the middle
// rightward; a child takes the first free one on the side of its maker
// where its lines cross fewer, at the rows they bend on: a lane held
// there, and every line its dot there starts or reads; but never where its
// maker's own result would pass over its start. Even, it goes on its
// maker's outer side, or, for a maker in the middle, alternately right and
// left. Columns are an order, not places: across() packs them.
function columns(L) {
  const held = new Map();
  const free = (s, l) => !(held.get(s) || []).some((h) => h.r0 <= l.r1 && l.r0 <= h.r1);
  const next = (s, side, l) => { while (!free(s, l)) s += side; return s; };
  // The lines a line of `l` from `m` to slot `s` would cross on row `r`,
  // below it for a start (`down`), above it for a result: not a lane that
  // only ends (or only starts) on that row, which is on the other side of
  // it, nor its siblings' from the same dot, which it nests with.
  const crossed = (l, m, s, r, down) => {
    let n = 0;
    for (let i = Math.min(m, s) + 1; i < Math.max(m, s); i++)
      for (const h of held.get(i) || []) if ((down ? h.r0 <= r && r < h.r1 : h.r0 < r && r <= h.r1) && h.from !== l.from) n += 1 + (h.nodes.find((v) => v.rank === r)?.fan ?? 0);
    return n;
  };
  for (const l of L) {
    if (l.spin) continue;
    let s, side;
    if (!l.from) { s = next(0, 1, l); side = 0; }
    else {
      const m = l.from.lane;
      side = m.side || (m.kids++ % 2 ? -1 : 1);
      const a = next(m.slot + side, side, l), b = next(m.slot - side, -side, l);
      const over = (s) => (l.from === m.last && m.reader?.slot !== undefined && (s - m.slot) * (m.reader.slot - s) > 0 ? 1 : 0);
      const cost = (s) => crossed(l, m.slot, s, l.r0, true) + crossed(l, m.slot, s, l.r1, false);
      s = over(a) > over(b) || (over(a) === over(b) && cost(b) < cost(a)) ? (side = -side, b) : a;
    }
    l.slot = s; l.side = side;
    (held.get(s) ?? held.set(s, []).get(s)).push(l);
  }
}

// Spindles: each on the side of its maker's lane with the nearest free
// track, alternately when even, so a fan balances; a track holds spindles
// whose lines don't share a row. The shortest go first, innermost, so a
// longer one's lines nest round theirs. Each sits SPIN clear of what is
// inside it on its rows. Then each lane's reach either side on each row
// it is on, its spindles' included: from its first dot to its last, or its
// last spindle's.
function spindles(L) {
  for (const m of L) {
    if (m.spin) continue;
    const dot = new Map(m.nodes.map((v) => [v.rank, v]));
    const own = (r, s) => { const v = dot.get(r); return v ? (s > 0 ? v.n.hi : v.n.lo) : LIFE / 2; };
    m.a = m.r0; m.b = m.r1;
    m.reach = new Map();
    for (let r = m.a; r <= m.b; r++) m.reach.set(r, [own(r, -1), own(r, 1)]);
    const tracks = { [-1]: [], [1]: [] };
    let was = -1;
    for (const c of m.spins.sort((a, b) => a.r1 - a.r0 - (b.r1 - b.r0) || a.r0 - b.r0 || a.nodes[0].key - b.nodes[0].key)) {
      c.r = c.nodes[0].rank;
      const fit = (s) => { const T = tracks[s]; let i = 0; while (i < T.length && T[i].some((o) => o.r0 < c.r1 && c.r0 < o.r1)) i++; return i; };
      const a = fit(1), b = fit(-1), s = a < b ? 1 : b < a ? -1 : -was;
      was = s;
      (tracks[s][s > 0 ? a : b] ??= []).push(c);
      c.side = s;
    }
    for (const s of [-1, 1]) {
      const near = (c) => (s > 0 ? c.nodes[0].n.lo : c.nodes[0].n.hi), far = (c) => (s > 0 ? c.nodes[0].n.hi : c.nodes[0].n.lo);
      const done = [];
      for (const T of tracks[s]) for (const c of T) {
        let off = own(c.r, s) + SPIN + near(c);
        for (const o of done) if (o.r0 < c.r1 && c.r0 < o.r1) off = Math.max(off, Math.abs(o.off) + far(o) + SPIN + near(c));
        c.off = s * off;
        done.push(c);
        for (let r = c.r0 + 1; r <= Math.max(c.r, c.r1 - 1); r++) {
          const at = m.reach.get(r) ?? [LIFE / 2, LIFE / 2], i = s > 0 ? 1 : 0;
          at[i] = Math.max(at[i], off + (r === c.r ? far(c) : c.nodes[0].n.r));
          m.reach.set(r, at);
          m.a = Math.min(m.a, r); m.b = Math.max(m.b, r);
        }
      }
    }
  }
}

// Each lane's x: as near the middle as it can be, PITCH from the lanes it
// shares a row with, or more where their reach needs CLEAR, in column
// order. Lanes that never share a row may share an x. Out from column 0
// each way: leftwards first, then rightwards, where a lane may need room
// from one on the left.
function across(L) {
  const on = new Map();
  for (const l of L) if (!l.spin) for (let r = l.a; r <= l.b; r++) (on.get(r) ?? on.set(r, []).get(r)).push(l);
  const need = new Map(L.map((l) => [l, []]));
  for (const [r, here] of on) {
    here.sort((a, b) => a.slot - b.slot);
    for (let i = 1; i < here.length; i++) {
      const [p, q] = [here[i - 1], here[i]], g = Math.max(PITCH, p.reach.get(r)[1] + CLEAR + q.reach.get(r)[0]);
      need.get(q).push([p, g]);
      need.get(p).push([q, -g]);
    }
  }
  const lanes = L.filter((l) => !l.spin);
  for (const l of lanes.filter((l) => l.slot <= 0).sort((a, b) => b.slot - a.slot)) {
    l.x = 0;
    for (const [q, g] of need.get(l)) if (g < 0 && q.slot <= 0 && q.x !== undefined) l.x = Math.min(l.x, q.x + g);
  }
  for (const l of lanes.filter((l) => l.slot > 0).sort((a, b) => a.slot - b.slot)) {
    l.x = 0;
    for (const [p, g] of need.get(l)) if (g > 0) l.x = Math.max(l.x, p.x + g);
  }
}

// ─── Lines ─────────────────────────────────────────────────────────────────
// Where a ray from node v's middle, along unit u, leaves it, plus gap.
function edge(v, u, gap) {
  const n = v.n;
  const t = n.big ? 1 / Math.hypot(u[0] / BIG_HALF, u[1] / (u[1] < 0 ? n.top : n.foot)) : n.r;
  return [v.x + u[0] * (t + gap), v.y + u[1] * (t + gap)];
}
// Each line's port, out on its source's bottom or in on its reader's top.
// The 2 WIDE about upright is cut into 2n + 1 even parts, n the more of
// the lines going left or right: the node's own lifeline (or the line it
// would have) holds the middle one, and the lines each side the next ones
// out.
function ports(edges) {
  const at = new Map();
  const port = (e, v, side, far, gap) => {
    const s = Math.sign(far.x - v.x) || 1, key = `${v.id}:${side}`;
    const p = { e, v, far, gap, s, side };
    (at.get(key) ?? at.set(key, []).get(key)).push(p);
    return p;
  };
  for (const e of edges) {
    if (e.kind === "grow" || e.kind === "branch" || e.kind === "port") e.a = port(e, e.d, 1, e.v, e.kind === "branch" ? TUCK : NGAP);
    if (e.kind === "fade" || e.kind === "port") e.b = port(e, e.v, -1, e.d, NGAP);
  }
  // Inner first: nearest across, then nearest down. Returns each side's
  // ports, as { h, step }.
  const across = (p) => Math.abs(p.far.x - p.v.x), down = (p) => Math.abs(p.far.y - p.v.y), sides = [];
  for (const here of at.values()) {
    const halves = [-1, 1].map((s) => here.filter((p) => p.s === s).sort((a, b) => across(a) - across(b) || down(a) - down(b)));
    const step = 2 * WIDE / (2 * Math.max(...halves.map((h) => h.length)) + 1);
    for (const h of halves) { place(h, step); sides.push({ h, step }); }
  }
  return sides;
}
// Ports in order out from upright, `step` apart.
function place(h, step) {
  h.forEach((p, i) => {
    const g = step * (i + 1);
    p.p = edge(p.v, [p.s * Math.sin(g), p.side * Math.cos(g)], p.gap);
    p.k = p.s * Math.tan(g * LEAN) * p.side;
  });
}

const smooth = (s) => s * s * s * (10 + s * (6 * s - 15));
// q runs 0 to 1, flat at 0, flat and straight at 1; r leaves 0 at slope 1
// and comes back to 0 flat and straight at 1.
const q = (s) => 1 - (1 - s) ** 3 * (1 + 3 * s), r = (s) => s * (1 - s) ** 3;
// A bend, as x of y, from (x0, y0) leaving at slope dx/dy k0 to (x1, y1)
// arriving at slope k1, over all its height. An upright end is straight
// there, curvature zero, so it meets an upright line with no change in its
// curvature either. A slant end's bulge is at most REACH times dx. With
// one end upright, `late` > 1 moves the turn away from it, towards the
// other end.
function bend(P, x0, y0, k0, x1, y1, k1, late = 1) {
  const dx = x1 - x0, dy = y1 - y0, n = Math.max(8, Math.ceil((Math.abs(dx) + dy) / 3));
  const lean = (k) => Math.sign(k) * Math.min(Math.abs(k) * dy, REACH * Math.abs(dx)), b0 = lean(k0), b1 = lean(k1);
  for (let i = P.length ? 1 : 0; i <= n; i++) {
    const s = i / n, w = (t) => 1 - (1 - t) ** late;
    const x = k0 && !k1 ? x0 + dx * q(w(s)) + b0 * r(s)
      : k1 && !k0 ? x1 - dx * q(w(1 - s)) - b1 * r(1 - s)
        : x0 + dx * smooth(s) + b0 * r(s) - b1 * r(1 - s);
    P.push([x, y0 + dy * s]);
  }
  return P;
}
// A taper's width s from its thin end: TIP to w along a smootherstep over
// TAPER (or all of a shorter line), so the outline too meets the lifeline
// with no change, then w; and over the last POINT eased down to nothing,
// so the end is a point.
function taper(s, L, w) {
  const t = Math.min(1, s / POINT);
  return TIP * t * (2 - t) + (w - TIP) * smooth(Math.min(1, s / Math.min(L, TAPER)));
}
// An edge's ends, its points, and its widths if it tapers or is a tool's.
// A life's own line runs upright between its dots; a spawn bends from its
// port all the way into its first dot, and a result from its last dot all
// the way into its reader's port: turning in the middle, or, where that
// would pass over a node or cross a line already drawn, as much nearer its
// slant end as clears the most.
function route(e, onRow, drawn) {
  const { v, d } = e;
  const P0 = e.a ? e.a.p : edge(d, [0, 1], TUCK), P1 = e.b ? e.b.p : edge(v, [0, -1], TUCK);
  let P = [P0, P1];
  if (e.kind !== "life") {
    const x0 = Math.min(P0[0], P1[0]), x1 = Math.max(P0[0], P1[0]);
    const between = [];
    for (let r = d.rank; r <= v.rank; r++) for (const u of onRow[r] ?? []) if (u !== v && u !== d && u.x > x0 - 12 && u.x < x1 + 12) between.push(u);
    const near = [...drawn.values()].filter((o) => o.x0 <= x1 && x0 <= o.x1 && o.P[0][1] <= P1[1] && P0[1] <= o.P.at(-1)[1]);
    let fewest = Infinity;
    for (const late of [1, 2, 4, .5, .25]) {
      const Q = bend([], P0[0], P0[1], e.a?.k ?? 0, P1[0], P1[1], e.b?.k ?? 0, late);
      let n = 0;
      for (const u of between) if (over(Q, u)) n += 100;
      for (const o of near) n += meets(Q, o.P);
      if (n < fewest) { fewest = n; P = Q; }
      if (!n) break;
    }
  }
  drawn.set(e, { P, x0: Math.min(P0[0], P1[0]), x1: Math.max(P0[0], P1[0]) });
  e.P = P;
  const w = (e.kind === "fade" ? d : v).lane.light ? TOOL : LIFE;
  if (e.kind === "branch" || (e.kind === "life" && w === LIFE)) { e.W = null; return; }
  if (e.kind === "life") { e.W = [w, w]; return; }
  const len = [0];
  for (let i = 1; i < P.length; i++) len[i] = len[i - 1] + Math.hypot(P[i][0] - P[i - 1][0], P[i][1] - P[i - 1][1]);
  const Lt = len[len.length - 1] || 1;
  e.W = len.map((s) => (e.kind === "grow" ? taper(s, Lt, w) : taper(Lt - s, Lt, w)));
}

// How many times two lines, both running down, cross.
function meets(A, B) {
  let n = 0, j0 = 1;
  for (let i = 1; i < A.length; i++) {
    const [a, b] = [A[i - 1], A[i]];
    while (j0 < B.length && B[j0][1] < a[1]) j0++;
    for (let j = j0; j < B.length && B[j - 1][1] <= b[1]; j++) if (meet(a, b, B[j - 1], B[j])) n++;
  }
  return n;
}
// Whether a line passes over node u, or within LIFE of its edge.
function over(P, u) {
  const reach = (u.n.big ? BIG_HALF : u.n.r) + LIFE;
  for (let i = 1; i < P.length; i++) {
    const [a, b] = [P[i - 1], P[i]];
    if ((a[1] < u.y - reach && b[1] < u.y - reach) || (a[1] > u.y + reach && b[1] > u.y + reach)) continue;
    const vx = b[0] - a[0], vy = b[1] - a[1], t = Math.max(0, Math.min(1, ((u.x - a[0]) * vx + (u.y - a[1]) * vy) / (vx * vx + vy * vy || 1)));
    if (Math.hypot(a[0] + t * vx - u.x, a[1] + t * vy - u.y) < reach) return true;
  }
  return false;
}

// ─── Crossings ─────────────────────────────────────────────────────────────
// Every place two lines of different lives cross, away from a node both
// end at: the more upright one stays whole, the other is cut GAP each side
// of it. Each as { over, under, at, dir, forced }: `over` and `under` the
// indices of the edges, `dir` the over one's direction there, and forced
// when no routing could avoid it: as many of two lives' crossings as the
// times their order across changes from one of their dots' rows to the
// next, where the dots pin them. A line out of a dot into another life's
// counts as a life of its own. Segments are bucketed in a grid, so this is
// near linear.
export function crossings(edges) {
  const CELL = 24, grid = new Map(), out = [];
  const segs = [];
  edges.forEach((e, i) => {
    for (let j = 1; j < e.P.length; j++) {
      const a = e.P[j - 1], b = e.P[j], s = { i, a, b, up: a[0] === b[0] };
      const k = segs.push(s) - 1;
      for (let cx = Math.floor(Math.min(a[0], b[0]) / CELL); cx <= Math.floor(Math.max(a[0], b[0]) / CELL); cx++)
        for (let cy = Math.floor(Math.min(a[1], b[1]) / CELL); cy <= Math.floor(Math.max(a[1], b[1]) / CELL); cy++) {
          const c = `${cx},${cy}`;
          (grid.get(c) ?? grid.set(c, []).get(c)).push(k);
        }
    }
  });
  const seen = new Set();
  for (const cell of grid.values()) {
    for (let x = 0; x < cell.length; x++) for (let y = x + 1; y < cell.length; y++) {
      const A = segs[cell[x]], B = segs[cell[y]], ea = edges[A.i], eb = edges[B.i];
      if (A.i === B.i || own(ea) === own(eb)) continue;
      const p = meet(A.a, A.b, B.a, B.b);
      if (!p) continue;
      const shared = [ea.v, ea.d].filter((n) => n === eb.v || n === eb.d);
      if (shared.some((n) => Math.hypot(p[0] - n.x, p[1] - n.y) < (n.n.big ? 24 : n.n.r + 10))) continue;
      const key = `${Math.min(A.i, B.i)}:${Math.max(A.i, B.i)}:${Math.round(p[0] / 4)}:${Math.round(p[1] / 4)}`;
      if (seen.has(key)) continue;
      seen.add(key);
      const [o, u] = slope(A) <= slope(B) ? [A, B] : [B, A];
      const m = Math.hypot(o.b[0] - o.a[0], o.b[1] - o.a[1]) || 1;
      out.push({ over: o.i, under: u.i, at: p, dir: [(o.b[0] - o.a[0]) / m, (o.b[1] - o.a[1]) / m], forced: false });
    }
  }
  const lives = chains(edges), left = new Map();
  for (const x of out) {
    const [a, b] = [own(edges[x.over]), own(edges[x.under])], key = [a, b].sort().join(" ");
    if (!left.has(key)) left.set(key, swaps(lives.get(a), lives.get(b)));
    if (left.get(key) > 0) { x.forced = true; left.set(key, left.get(key) - 1); }
  }
  return out;
}
// Each life's line from top to bottom, through the middles of its dots,
// and the heights of its dots.
function chains(edges) {
  const out = new Map();
  for (const e of [...edges].sort((a, b) => a.d.y - b.d.y)) {
    const k = own(e);
    const c = out.get(k) ?? out.set(k, { P: [], pins: [] }).get(k);
    c.P.push([e.d.x, e.d.y], ...e.P, [e.v.x, e.v.y]);
    c.pins.push(e.d.y, e.v.y);
  }
  return out;
}
const xAt = (c, y) => {
  for (let i = 1; i < c.P.length; i++) {
    const [a, b] = [c.P[i - 1], c.P[i]];
    if ((a[1] <= y && y <= b[1]) || (b[1] <= y && y <= a[1])) return b[1] === a[1] ? a[0] : a[0] + ((b[0] - a[0]) * (y - a[1])) / (b[1] - a[1]);
  }
  return null;
};
// How often two lives' order across changes from one pinned height to the
// next, over the heights both span.
function swaps(A, B) {
  const lo = Math.max(A.P[0][1], B.P[0][1]), hi = Math.min(A.P.at(-1)[1], B.P.at(-1)[1]);
  let n = 0, was = 0;
  for (const y of [...new Set([...A.pins, ...B.pins])].filter((y) => lo <= y && y <= hi).sort((a, b) => a - b)) {
    const a = xAt(A, y), b = xAt(B, y);
    if (a === null || b === null || Math.abs(a - b) < .5) continue;
    if (was && Math.sign(a - b) !== was) n++;
    was = Math.sign(a - b);
  }
  return n;
}
// Whose line it is, by a name: a result's is the life that finished; a
// line out of a dot into another life's is its own.
const own = (e) => (e.kind === "port" ? `${e.d.id}>${e.v.id}` : (e.kind === "fade" ? e.d : e.v).n.lane);
const slope = (s) => Math.abs((s.b[0] - s.a[0]) / (s.b[1] - s.a[1] || 1e-9));
function meet(p, q, r, s) {
  const d = (q[0] - p[0]) * (s[1] - r[1]) - (q[1] - p[1]) * (s[0] - r[0]);
  if (!d) return null;
  const t = ((r[0] - p[0]) * (s[1] - r[1]) - (r[1] - p[1]) * (s[0] - r[0])) / d;
  const u = ((r[0] - p[0]) * (q[1] - p[1]) - (r[1] - p[1]) * (q[0] - p[0])) / d;
  return t >= 0 && t <= 1 && u >= 0 && u <= 1 ? [p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])] : null;
}

// A line in parts, cut where it passes under another: a gap GAP clear of
// the over line each side, measured across it, so nothing else is covered.
// Each part { P, W }.
function cut(e, under) {
  const len = [0];
  for (let i = 1; i < e.P.length; i++) len[i] = len[i - 1] + Math.hypot(e.P[i][0] - e.P[i - 1][0], e.P[i][1] - e.P[i - 1][1]);
  const gaps = under.map(({ at, dir }) => {
    let best = 0, d = Infinity, u = [0, 1];
    for (let i = 1; i < e.P.length; i++) {
      const [a, b] = [e.P[i - 1], e.P[i]], vx = b[0] - a[0], vy = b[1] - a[1], L = vx * vx + vy * vy || 1;
      const t = Math.max(0, Math.min(1, ((at[0] - a[0]) * vx + (at[1] - a[1]) * vy) / L));
      const q = Math.hypot(a[0] + t * vx - at[0], a[1] + t * vy - at[1]);
      if (q < d) { d = q; best = len[i - 1] + t * (len[i] - len[i - 1]); u = [vx / Math.sqrt(L), vy / Math.sqrt(L)]; }
    }
    const g = (LIFE + GAP) / Math.max(.3, Math.abs(u[0] * dir[1] - u[1] * dir[0]));
    return [best - g, best + g];
  }).sort((a, b) => a[0] - b[0]);
  const parts = [];
  let from = 0;
  for (const [g0, g1] of [...gaps, [Infinity, Infinity]]) {
    if (g0 > from) parts.push(piece(e, len, from, Math.min(g0, len.at(-1))));
    from = Math.max(from, g1);
    if (from >= len.at(-1)) break;
  }
  return parts;
}
// The part of a line from length s0 along it to s1.
function piece(e, len, s0, s1) {
  const P = [], W = e.W && [];
  const point = (s) => {
    const i = Math.max(1, len.findIndex((l) => l >= s)), t = (s - len[i - 1]) / (len[i] - len[i - 1] || 1);
    P.push([e.P[i - 1][0] + t * (e.P[i][0] - e.P[i - 1][0]), e.P[i - 1][1] + t * (e.P[i][1] - e.P[i - 1][1])]);
    if (W) W.push(e.W[i - 1] + t * (e.W[i] - e.W[i - 1]));
  };
  point(s0);
  for (let i = 0; i < e.P.length; i++) if (len[i] > s0 && len[i] < s1) { P.push(e.P[i]); if (W) W.push(e.W[i]); }
  point(s1);
  return { P, W };
}

// ─── Drawing a line ────────────────────────────────────────────────────────
const f1 = (v) => (Math.round(v * 10) / 10).toString();
// A line of one width, as a stroke's path.
export const polyline = (P) => `M${P.map((p) => `${f1(p[0])},${f1(p[1])}`).join("L")}`;
// A taper, as a filled outline: out along its left, back along its right,
// so every outline winds the same way and many in one path fill as their
// union.
export function outline(P, W) {
  const left = [], right = [];
  P.forEach((p, i) => {
    const a = P[Math.max(0, i - 1)], b = P[Math.min(P.length - 1, i + 1)];
    const tx = b[0] - a[0], ty = b[1] - a[1], m = Math.hypot(tx, ty) || 1, h = W[i] / 2;
    left.push(`${f1(p[0] - (ty / m) * h)},${f1(p[1] + (tx / m) * h)}`);
    right.push(`${f1(p[0] + (ty / m) * h)},${f1(p[1] - (tx / m) * h)}`);
  });
  return `M${left.join("L")}L${right.reverse().join("L")}Z`;
}
