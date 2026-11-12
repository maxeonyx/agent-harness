// ─── The map's layout: lanes, and their lines ──────────────────────────────
// Each life (a chat, an agent, a background call, an old version) keeps one
// column, its lane, and its own line runs straight down it through its dots.
// A spawn grows out of its maker into the child's lane; a result runs down
// the finished lane and thins into its reader. Rows are a layered graph's,
// flowing down, with the present on one row at y = 0: every live node sits
// on the lowest row the past reaches, but one that a live node depends on
// sits just above it, and what waits hangs below. Nothing here asks the
// browser for geometry: sizes come in with the nodes, and lines are points.
//
// A node: { id, deps, lane, big, r, lo, hi, along, top, foot, now, planned }.
// `lane` names its life; `r` is a dot's radius, and a big node reaches `top`
// above its centre and `foot` below. `lo` and `hi` are how far it reaches
// left and right of its centre (its label is to the right), `along` how
// tall it is. `now` is a live node, `planned` one that waits on what it
// depends on. `time`, if known, is [when it began, when it ended or now], in
// ms. Nodes come after everything they depend on.

// Lanes PITCH apart, wider where a node and its label need CLEAR to the
// next lane.
const PITCH = 20, CLEAR = 8;
// A lifeline keeps one width and runs TUCK under the edge of its dots. A
// taper runs from that width down to TIP, then narrows to a point over its
// last POINT, which stops NGAP short of the dot it starts from or rejoins.
// A crossing's gap is GAP each side of the line over it.
export const LIFE = 2.2, GAP = 1.5;
const TIP = .5, POINT = 4, NGAP = 1.5, TUCK = -.6;
// Ports: a line leaves a node's bottom, or lands on its top, this far round
// from upright on the side facing its other end, nearest innermost; the
// centre is the lifeline's. At its port a line points LEAN of the way from
// straight down (or up) to straight out from the middle.
const SLOTS = [36, 66, 82].map((d) => (d * Math.PI) / 180), LEAN = .5;
const slotsFor = (n) => (n <= SLOTS.length ? SLOTS : Array.from({ length: n }, (_, i) => SLOTS[0] + ((SLOTS[2] - SLOTS[0]) * i) / (n - 1)));
// A bend is at most BEND times as tall as it goes across, and at most REACH
// times that over its port's slope, past which it would bulge.
const BEND = 1.3, REACH = 3;
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
  const slot = columns(L);
  // Down: each row as tall as its tallest node plus 17, the present row at
  // y = 0. Across: each lane's x.
  const tall = [];
  for (const v of V) tall[v.rank] = Math.max(tall[v.rank] ?? 0, v.n.along);
  const ys = [0];
  for (let r = 1; r < tall.length; r++) ys[r] = ys[r - 1] + (tall[r - 1] ?? 0) / 2 + (tall[r] ?? 0) / 2 + 17;
  const xs = across(L, slot, tall.length);
  for (const v of V) { v.x = xs.get(v.lane.slot); v.y = ys[v.rank] - ys[P]; }
  ports(edges);
  heights(edges, V);
  for (const e of edges) route(e);
  const cuts = crossings(edges);
  const at = new Map(V.map((v) => [v.id, { x: v.x, y: v.y }]));
  return { at, cuts, edges: edges.map((e) => ({ from: e.v.id, to: e.d.id, kind: e.kind, P: e.P, W: e.W })) };
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
// life's result, down its lane from its last dot into its reader. `port`: a
// line out of a dot its life goes on from, into another life's. A lane is
// held from the row its life starts on to the row its last line leaves it.
function lanesOf(V, edges) {
  const L = new Map();
  for (const v of V) {
    let l = L.get(v.n.lane);
    if (!l) L.set(v.n.lane, l = { nodes: [], kids: 0 });
    l.nodes.push(v);
    v.lane = l;
  }
  for (const l of L.values()) { l.last = l.nodes.reduce((a, b) => (b.rank >= a.rank ? b : a)); l.r1 = l.last.rank; }
  for (const e of edges) {
    const { v, d } = e;
    if (v.lane === d.lane) e.kind = "life";
    else if (v.lane.nodes[0] === v && !v.lane.from) { e.kind = v.n.old ? "branch" : "grow"; v.lane.from = d; }
    else if (d === d.lane.last) { e.kind = "fade"; d.lane.r1 = Math.max(d.lane.r1, v.rank); }
    else e.kind = "port";
  }
  for (const l of L.values()) l.r0 = l.from ? l.from.rank : l.nodes[0].rank;
  return [...L.values()].sort((a, b) => a.r0 - b.r0 || a.nodes[0].key - b.nodes[0].key);
}

// Each lane's column, in the order they start. A column holds one life at
// a time and never moves. A life with no maker takes the first free column
// from the middle rightward; a child takes the first free one on its
// maker's outer side, the maker's first children alternately right and left
// when it is in the middle.
function columns(L) {
  const held = new Map();
  const free = (s, l) => !(held.get(s) || []).some((h) => h.r0 <= l.r1 && l.r0 <= h.r1);
  const next = (s, side, l) => { while (!free(s, l)) s += side; return s; };
  // The lanes a line from `m` to slot `s` would cross on row `r`.
  const crossed = (m, s, r) => {
    let n = 0;
    for (let i = Math.min(m, s) + 1; i < Math.max(m, s); i++) n += (held.get(i) || []).filter((h) => h.r0 <= r && r <= h.r1).length;
    return n;
  };
  for (const l of L) {
    let s, side;
    if (!l.from) { s = next(0, 1, l); side = 0; }
    else {
      const m = l.from.lane;
      side = m.side;
      if (!side) {
        // A maker in the middle alternates its children right and left,
        // unless the other side is free of lanes to cross.
        side = m.kids++ % 2 ? -1 : 1;
        const a = next(m.slot + side, side, l), b = next(m.slot - side, -side, l);
        const cost = (s) => crossed(m.slot, s, l.r0) + crossed(m.slot, s, l.r1);
        if (cost(b) < cost(a)) side = -side;
      }
      s = next(m.slot + side, side, l);
    }
    l.slot = s; l.side = side;
    (held.get(s) ?? held.set(s, []).get(s)).push(l);
  }
  return held;
}

// Each column's x: PITCH right of the one before, or more where, on some
// row, a node and its label need CLEAR to the next lane on that row: from
// its first dot to its last (above and below them its lines bend away).
function across(L, held, nrows) {
  const slots = [...held.keys()].sort((a, b) => a - b);
  const at = new Map();
  for (const l of L) for (const v of l.nodes) at.set(`${l.slot}:${v.rank}`, v);
  const need = new Map(slots.map((s) => [s, []]));
  for (let r = 0; r < nrows; r++) {
    let prev = null, reach = 0;
    for (const s of slots) {
      if (!held.get(s).some((l) => l.nodes[0].rank <= r && r <= l.last.rank)) continue;
      const v = at.get(`${s}:${r}`), lo = v ? v.n.lo : LIFE / 2;
      if (prev != null) need.get(s).push([prev, reach + CLEAR + lo]);
      prev = s; reach = v ? v.n.hi : LIFE / 2;
    }
  }
  // Out from slot 0 each way, so a column sits as near the middle as its
  // neighbours let it: leftwards first, then rightwards, where a gap may
  // be to a column on the left.
  const xs = new Map();
  let last = 0;
  for (const s of slots.filter((s) => s <= 0).reverse()) {
    let x = s === 0 ? 0 : xs.get(last) - PITCH * (last - s);
    for (const [r, g] of need.entries()) if (r > s && r <= 0) for (const [p, h] of g) if (p === s) x = Math.min(x, xs.get(r) - h);
    xs.set(s, x);
    last = s;
  }
  last = slots.filter((s) => s <= 0).at(-1);
  for (const s of slots.filter((s) => s > 0)) {
    let x = last == null ? 0 : xs.get(last) + PITCH * (s - last);
    for (const [p, g] of need.get(s)) x = Math.max(x, xs.get(p) + g);
    xs.set(s, x);
    last = s;
  }
  return xs;
}

// ─── Lines ─────────────────────────────────────────────────────────────────
// Where a ray from node v's middle, along unit u, leaves it, plus gap.
function edge(v, u, gap) {
  const n = v.n;
  const t = n.big ? 1 / Math.hypot(u[0] / BIG_HALF, u[1] / (u[1] < 0 ? n.top : n.foot)) : n.r;
  return [v.x + u[0] * (t + gap), v.y + u[1] * (t + gap)];
}
// Each line's port, out on its source's bottom or in on its reader's top.
function ports(edges) {
  const at = new Map();
  const slot = (v, side, far, gap) => {
    const s = Math.sign(far.x - v.x) || 1, key = `${v.id}:${side}:${s}`;
    const p = { v, far, gap, s, side, group: key };
    (at.get(key) ?? at.set(key, []).get(key)).push(p);
    return p;
  };
  for (const e of edges) {
    if (e.kind === "grow" || e.kind === "branch" || e.kind === "port") e.a = slot(e.d, 1, e.v, e.kind === "branch" ? TUCK : NGAP);
    if (e.kind === "fade" || e.kind === "port") e.b = slot(e.v, -1, e.d, NGAP);
  }
  for (const here of at.values()) {
    const slots = slotsFor(here.length);
    here.sort((a, b) => Math.abs(a.far.x - a.v.x) - Math.abs(b.far.x - b.v.x)).forEach((p, i) => {
      const g = slots[i];
      p.p = edge(p.v, [p.s * Math.sin(g), p.side * Math.cos(g)], p.gap);
      p.k = p.s * Math.tan(g * LEAN) * p.side;
    });
  }
}
const span = (dx, dy, k) => Math.max(0, Math.min(dy, BEND * Math.abs(dx), k ? (REACH * Math.abs(dx)) / Math.abs(k) : Infinity));
// Each line's ends, and how tall its bend is. The lines on one side of one
// node all take the tallest any of them wants, no more than the shortest
// has room for: bent over the same height they stay nested, and only draw
// apart.
function heights(edges, V) {
  const groups = new Map();
  for (const e of edges) {
    const { v, d } = e;
    if (e.kind === "life") { e.P0 = edge(d, [0, 1], TUCK); e.P1 = edge(v, [0, -1], TUCK); continue; }
    if (e.kind === "fade") { e.P0 = edge(d, [0, 1], TUCK); e.P1 = e.b.p; e.k = e.b.k; e.group = e.b.group; }
    else if (e.kind === "port") { e.P0 = e.a.p; e.P1 = e.b.p; continue; }
    else { e.P0 = e.a.p; e.P1 = edge(v, [0, -1], TUCK); e.k = e.a.k; e.group = e.a.group; }
    (groups.get(e.group) ?? groups.set(e.group, []).get(e.group)).push(e);
  }
  for (const G of groups.values()) {
    const dy = (e) => e.P1[1] - e.P0[1];
    const H = Math.min(...G.map(dy), Math.max(...G.map((e) => span(e.P1[0] - e.P0[0], dy(e), e.k))));
    for (const e of G) e.h = H;
  }
  clear([...groups.values()].flat(), V);
  for (const G of groups.values()) {
    const H = Math.min(...G.map((e) => e.most), Math.max(...G.map((e) => e.least), Math.min(...G.map((e) => e.h))));
    for (const e of G) e.h = H;
  }
}

// Bends kept clear of nodes and of each other. A bend hangs down from its
// port (a start) or up from it (a result) across the lanes between its
// ends. One that would reach a node in its way stops short of it; one that
// passes a node on its port's own row reaches far enough to pass it on the
// far side. Two of different lives that would overlap share the room
// between them: a start and a result in proportion to how tall each wants
// to be, two starts (or two results) the nearer its port the shorter.
const ROOM = 3;
function clear(B, V) {
  const band = (e) => {
    const down = e.kind !== "fade", [y, x0, x1] = down ? [e.P0[1], e.P0[0], e.P1[0]] : [e.P1[1], e.P1[0], e.P0[0]];
    return { e, down, y, at: down ? e.d.y : e.v.y, from: x0, x0: Math.min(x0, x1), x1: Math.max(x0, x1), dx: x1 - x0, most: Math.abs(e.P1[1] - e.P0[1]), least: 0 };
  };
  const bs = B.map(band);
  for (const b of bs) {
    for (const u of V) {
      if (u === b.e.v || u === b.e.d || u.x <= b.x0 || u.x >= b.x1) continue;
      const above = u.n.big ? u.n.top : u.n.r, below = u.n.big ? u.n.foot : u.n.r;
      if (Math.abs(u.y - b.at) < 1) {
        // Past it where the bend has gone (u.x - from) of its way across.
        const s = along((u.x - b.from) / b.dx), dy = b.down ? u.y + below + ROOM - b.y : b.y - (u.y - above - ROOM);
        b.least = Math.max(b.least, dy / s);
      } else if (b.down && u.y > b.y) b.e.h = Math.min(b.e.h, u.y - above - ROOM - b.y);
      else if (!b.down && u.y < b.y) b.e.h = Math.min(b.e.h, b.y - u.y - below - ROOM);
    }
  }
  const life = (e) => (e.kind === "fade" ? e.d.lane : e.v.lane);
  for (let pass = 0; pass < 3; pass++) {
    for (let i = 0; i < bs.length; i++) for (let j = i + 1; j < bs.length; j++) {
      const a = bs[i], b = bs[j];
      if (a.x1 <= b.x0 || b.x1 <= a.x0 || life(a.e) === life(b.e)) continue;
      const [ta, ba] = a.down ? [a.y, a.y + a.e.h] : [a.y - a.e.h, a.y], [tb, bb] = b.down ? [b.y, b.y + b.e.h] : [b.y - b.e.h, b.y];
      if (ba <= tb || bb <= ta) continue;
      if (a.down !== b.down) {
        const [g, f] = a.down ? [a, b] : [b, a];
        const room = f.y - g.y - ROOM;
        if (room <= 0) continue;
        const s = room / (g.e.h + f.e.h);
        g.e.h *= s; f.e.h *= s;
      } else if (a.y !== b.y) {
        const [hi, lo] = a.y < b.y ? [a, b] : [b, a];
        if (a.down) hi.e.h = Math.min(hi.e.h, lo.y - hi.y - ROOM);
        else lo.e.h = Math.min(lo.e.h, lo.y - hi.y - ROOM);
      }
    }
  }
  for (const b of bs) { b.e.most = b.most; b.e.least = b.least; b.e.h = Math.min(b.most, Math.max(2, b.least, b.e.h)); }
}
// How far along a bend from its upright end it has gone f of the way
// across: q(s) = f.
function along(f) {
  let lo = 0, hi = 1;
  for (let i = 0; i < 20; i++) { const m = (lo + hi) / 2; if (q(m) < f) lo = m; else hi = m; }
  return Math.max(.05, (lo + hi) / 2);
}

const smooth = (s) => s * s * s * (10 + s * (6 * s - 15));
// q runs 0 to 1, flat at 0, flat and straight at 1; r leaves 0 at slope 1
// and comes back to 0 flat and straight at 1.
const q = (s) => 1 - (1 - s) ** 3 * (1 + 3 * s), r = (s) => s * (1 - s) ** 3;
// A bend, as x of y, from (x0, y0) leaving at slope dx/dy k0 to (x1, y1)
// arriving at slope k1. An upright end is straight there, curvature zero,
// so it meets an upright line with no change in its curvature either.
function bend(P, x0, y0, k0, x1, y1, k1) {
  const dx = x1 - x0, dy = y1 - y0, n = Math.max(8, Math.ceil((Math.abs(dx) + dy) / 3));
  for (let i = P.length ? 1 : 0; i <= n; i++) {
    const s = i / n;
    const x = k0 && !k1 ? x0 + dx * q(s) + k0 * dy * r(s)
      : k1 && !k0 ? x1 - dx * q(1 - s) - k1 * dy * r(1 - s)
        : x0 + dx * smooth(s) + (k0 || 0) * dy * r(s) - (k1 || 0) * dy * r(1 - s);
    P.push([x, y0 + dy * s]);
  }
  return P;
}
// Upright, a point each STEP, or just its ends.
function straight(P, x, y0, y1, step) {
  const n = step ? Math.max(1, Math.ceil((y1 - y0) / step)) : 1;
  for (let i = P.length ? 1 : 0; i <= n; i++) P.push([x, y0 + ((y1 - y0) * i) / n]);
  return P;
}
// A taper's width s from its thin end, of L: TIP to the lifeline's along a
// smootherstep, so the outline too meets the lifeline with no change, and
// over the last POINT eased down to nothing, so the end is a point.
function taper(s, L) {
  const t = Math.min(1, s / POINT);
  return TIP * t * (2 - t) + (LIFE - TIP) * smooth(Math.min(1, s / L));
}
// An edge's points, and its widths if it tapers. A spawn bends from its
// port upright into its lane; a result runs straight down out of the one
// that finished, then bends into its reader's port.
function route(e) {
  const [x0, y0] = e.P0, [x1, y1] = e.P1;
  const P = [];
  if (e.kind === "life") straight(P, x0, y0, y1, 0);
  else if (e.kind === "fade") { if (y1 - e.h > y0) straight(P, x0, y0, y1 - e.h, 4); bend(P, x0, y1 - e.h, 0, x1, y1, e.k); }
  else if (e.kind === "port") bend(P, x0, y0, e.a.k, x1, y1, e.b.k);
  else { bend(P, x0, y0, e.k, x1, y0 + e.h, 0); if (y1 > y0 + e.h) straight(P, x1, y0 + e.h, y1, e.kind === "grow" ? 4 : 0); }
  e.P = P;
  if (e.kind === "life" || e.kind === "branch") { e.W = null; return; }
  const len = [0];
  for (let i = 1; i < P.length; i++) len[i] = len[i - 1] + Math.hypot(P[i][0] - P[i - 1][0], P[i][1] - P[i - 1][1]);
  const Lt = len[len.length - 1] || 1;
  e.W = len.map((s) => (e.kind === "grow" ? taper(s, Lt) : taper(Lt - s, Lt)));
}

// ─── Crossings ─────────────────────────────────────────────────────────────
// Every place two lines of different lives cross, away from a node both
// end at: the more upright one stays whole, the other is cut GAP each side
// of it. Each as { over, at, dir, forced }: `over` the index of the edge
// drawn over, `dir` its direction there, and forced when the other is a bend across that one's lane, which
// no routing could avoid. Segments are bucketed in a grid, so this is near
// linear.
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
      if (A.i === B.i || lifeOf(ea) === lifeOf(eb)) continue;
      const p = meet(A.a, A.b, B.a, B.b);
      if (!p) continue;
      const shared = [ea.v, ea.d].filter((n) => n === eb.v || n === eb.d);
      if (shared.some((n) => Math.hypot(p[0] - n.x, p[1] - n.y) < (n.n.big ? 24 : n.n.r + 10))) continue;
      const key = `${Math.min(A.i, B.i)}:${Math.max(A.i, B.i)}:${Math.round(p[0] / 4)}:${Math.round(p[1] / 4)}`;
      if (seen.has(key)) continue;
      seen.add(key);
      const [o, u] = slope(A) <= slope(B) ? [A, B] : [B, A];
      const ue = edges[u.i], lx = o.a[0];
      const m = Math.hypot(o.b[0] - o.a[0], o.b[1] - o.a[1]) || 1;
      out.push({ over: o.i, under: u.i, at: p, dir: [(o.b[0] - o.a[0]) / m, (o.b[1] - o.a[1]) / m], forced: o.up && ue.kind !== "life" && Math.min(ue.P[0][0], ue.P.at(-1)[0]) < lx && lx < Math.max(ue.P[0][0], ue.P.at(-1)[0]) });
    }
  }
  return out;
}
// Whose line it is: a result's is the life that finished.
const lifeOf = (e) => (e.kind === "fade" || e.kind === "port" ? e.d.lane : e.v.lane);
const slope = (s) => Math.abs((s.b[0] - s.a[0]) / (s.b[1] - s.a[1] || 1e-9));
function meet(p, q, r, s) {
  const d = (q[0] - p[0]) * (s[1] - r[1]) - (q[1] - p[1]) * (s[0] - r[0]);
  if (!d) return null;
  const t = ((r[0] - p[0]) * (s[1] - r[1]) - (r[1] - p[1]) * (s[0] - r[0])) / d;
  const u = ((r[0] - p[0]) * (q[1] - p[1]) - (r[1] - p[1]) * (q[0] - p[0])) / d;
  return t >= 0 && t <= 1 && u >= 0 && u <= 1 ? [p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])] : null;
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
