// ─── The map's layout: a layered graph, and its lines ──────────────────────
// Rows are a layered graph's, flowing down, with the present on one row at
// y = 0: every live node sits on the lowest row the past reaches, but one
// that a live node depends on sits just above it, and what waits hangs
// below. Across, it is the standard layered layout Graphviz `dot` uses: a
// line that spans rows passes through a hidden point on each row between,
// rows are ordered by barycentre sweeps, and each node is placed as near
// what it links to as the room in its row allows. A life's own line is
// stiff, so it runs straight wherever it can. Nothing here asks the browser
// for geometry: sizes come in with the nodes, and lines are points.
//
// A node: { id, deps, life, light, old, big, r, lo, hi, along, top, foot,
// now, planned, fork }. `life` names its life, `light` a tool's, `old` an
// old version's, `fork` a fork's (a copy of its maker's context); `r` is a
// dot's radius, and a big node reaches `top` above its centre and `foot`
// below. `lo` and `hi` are how far it reaches
// left and right of its centre (its label is to the right), `along` how
// tall it is. `now` is a live node, `planned` one that waits on what it
// depends on. `time`, if known, is [when it began, when it ended or now], in
// ms. Nodes come after everything they depend on.

// An agent's line is LIFE wide, a tool's (`light`) TOOL. A line of a dot's
// own life runs TUCK under its edge. A taper runs from TIP up to its line's
// width over at most TAPER, then keeps it; it narrows to a point over its
// last POINT, which stops NGAP short of the dot it starts from or rejoins.
// A crossing's gap is GAP each side of the line over it, and only where the
// lines cross at STEEP or more: at a shallower angle they run along each
// other, and a gap would cut the line under into islands, so the line over
// simply covers it.
export const LIFE = 2.2;
const TOOL = 1.2;
export const GAP = 1.5;
const TIP = .5, POINT = 4, NGAP = 1.5, TUCK = -.6, TAPER = 48;
const STEEP = Math.PI / 6;
// Ports, pizza-wise: a dot's left and right slices are kept clear, and the
// lines out of its bottom, and into its top, leave or arrive within WIDE of
// upright. Its own life's lines stand upright in the middle; the others
// spread either side, in the order they run to, at most STEP apart, closer
// when more must fit. At its port a line points LEAN of the way from
// upright to straight out from the middle.
const WIDE = Math.PI / 3, STEP = Math.PI / 9, LEAN = .5;
// A big node's half width, for where lines meet it.
const BIG_HALF = 13;
// How much stiffer than a plain line a lifeline pulls: enough to keep it
// straight. How stiffly a long line resists bending, against how hard it
// pulls: a turn spreads over about sqrt(BEND) rows.
const STIFF = 40, BEND = 4;
// A life's dots, from its first, share sides in runs of BAG, half each side,
// shuffled; a dot with an odd line out sends it to its own.
const BAG = 4;
// The flattest a line between two rows runs on average, from upright.
const FLAT = Math.PI / 3;

// One project laid out: each node's centre, y = 0 on the present row, and
// each edge's line.
export function layOut(nodes) {
  const ids = new Set(nodes.map((n) => n.id));
  const V = nodes.map((n, i) => ({ n, id: n.id, key: i, deps: n.deps.filter((d) => ids.has(d)) }));
  const by = new Map(V.map((v) => [v.id, v]));
  const P = rows(V, by);
  const edges = [];
  for (const v of V) for (const d of v.deps) edges.push({ v, d: by.get(d) });
  kinds(V, edges);
  const layers = layered(V, edges);
  // Down: each row as tall as its tallest node plus 17, and apart enough
  // that no line between two rows runs flatter than FLAT on average, the
  // present row at y = 0.
  const tall = [], wide = [], ys = [0];
  for (const v of V) tall[v.rank] = Math.max(tall[v.rank] ?? 0, v.n.along);
  for (const L of layers) for (const u of L) for (const w of u.up) wide[w.rank] = Math.max(wide[w.rank] ?? 0, Math.abs(u.x - w.x));
  for (let r = 1; r < tall.length; r++) ys[r] = ys[r - 1] + Math.max((tall[r - 1] ?? 0) / 2 + (tall[r] ?? 0) / 2 + 17, (wide[r - 1] ?? 0) / Math.tan(FLAT));
  for (const [r, L] of layers.entries()) for (const u of L) u.y = ys[r] - ys[P];
  ports(edges);
  for (const e of edges) route(e);
  const cuts = crossings(edges);
  const key = (e) => `${e.v.id}>${e.d.id}`;
  const gaps = edges.map((e, i) => cuts.filter((c) => c.under === i && c.angle >= STEEP).map((c) => ({ over: key(edges[c.over]), ...near(edges[c.over], c) })));
  const sheet = sheets(edges, gaps);
  return { at: new Map(V.map((v) => [v.id, { x: v.x, y: v.y }])), edges: edges.map((e, i) => ({ from: e.v.id, to: e.d.id, kind: e.kind, P: e.P, W: e.W, sheet: sheet[i], gaps: gaps[i] })) };
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

// ─── Lives ─────────────────────────────────────────────────────────────────
// Each node's life, and each edge's kind. `life`: a life's own line, from
// one of its dots to the next. `grow`: a life starting, from its maker into
// its first dot (`branch` for an old version, which keeps one width).
// `fade`: a life's result, from its last dot into its reader. `port`: a
// line out of a dot its life goes on from, into another life's. An end is
// its dot's own, `top` or `foot` (the line leaves its upper dot or reaches
// its lower one), where the line is that dot's life going on: a life's own
// line at both ends, a start into the life it starts, a result out of the
// life that ends: 2 for the dot's own lifeline. An old version and a fork
// are their maker's context carrying on, so are its own too, but beside its
// lifeline: 1. Else 0.
function kinds(V, edges) {
  const L = new Map();
  for (const v of V) {
    let l = L.get(v.n.life);
    if (!l) L.set(v.n.life, l = { first: v, light: v.n.light });
    l.last = v;
    v.life = l;
  }
  for (const e of edges) {
    const { v, d } = e;
    if (v.life === d.life) e.kind = "life";
    else if (v.life.first === v && !v.life.from) { e.kind = v.n.old ? "branch" : "grow"; v.life.from = d; }
    else if (d === d.life.last) e.kind = "fade";
    else e.kind = "port";
    e.top = e.kind === "life" || e.kind === "fade" ? 2 : e.kind === "branch" || (e.kind === "grow" && v.n.fork) ? 1 : 0;
    e.foot = e.kind === "port" || e.kind === "fade" ? 0 : 2;
  }
}

// ─── Placement ─────────────────────────────────────────────────────────────
// Each edge becomes a chain, through a hidden point on each row it crosses.
// Rows are ordered by barycentre sweeps, where a dot's lines into the row
// sit in its port order (its lifeline in the middle, its other own lines
// either side of it, the rest either side of those), so siblings fan out
// either side of the line their maker goes on down. Each sweep ends by
// swapping neighbours that cross less swapped, and the order kept is the
// sweep's with fewest crossings. Then across, each line
// a spring pulling its ends together, each row placed as near its
// neighbours as the gaps allow (PAVA). A line's spring is shared evenly
// by its links, so a long line pulls its ends as hard as a short one and
// runs straight between them where nothing is in its way; it is also a
// beam, so where something is, it bends round it over several rows rather
// than at a corner. A lifeline is a much stiffer spring, so a life runs
// straight and its forks pack around it. After each sweep, each life moves
// with all it started as one (`lives`). Sets each node's x; each edge's
// hidden points, top down, as `mid`.
// Returns the rows.
function layered(V, edges) {
  const layers = [];
  const put = (v) => (layers[v.rank] ??= []).push(v);
  for (const v of V) { Object.assign(v, { up: [], down: [], lo: v.n.lo, hi: v.n.hi, own: new Map(), k: new Map(), line: v.n.life }); put(v); }
  for (const e of edges) {
    const { v, d } = e, chain = [v];
    for (let r = v.rank - 1; r > d.rank; r--) {
      const x = { dummy: true, rank: r, key: d.key + .5, up: [], down: [], lo: 1, hi: 1, own: new Map(), k: new Map(), line: e.kind === "life" ? v.line : null, life: e.kind === "grow" || e.kind === "branch" ? v.life : d.life };
      put(x);
      chain.push(x);
    }
    chain.push(d);
    for (let i = 0; i < chain.length - 1; i++) {
      const a = chain[i], b = chain[i + 1];
      a.up.push(b); b.down.push(a);
      // Along its own line: the next point up (`lu`) and down (`ld`).
      if (e.kind === "life" || a.dummy) a.lu = b;
      if (e.kind === "life" || b.dummy) b.ld = a;
      const k = (e.kind === "life" ? STIFF : 1) * (chain.length - 1);
      a.k.set(b, k); b.k.set(a, k);
    }
    if (e.foot) v.own.set(chain[1], e.foot);
    if (e.top) d.own.set(chain.at(-2), e.top);
    e.mid = chain.slice(1, -1).reverse();
  }
  for (let r = 0; r < layers.length; r++) layers[r] ??= [];
  for (const L of layers) { L.sort((a, b) => a.key - b.key); L.forEach((v, i) => (v.pos = i)); }
  odd(V);
  // Where u's line to w sits among u's lines that way, from -1/2 to 1/2 of
  // a place: in its port order, each level's lines either side of the more
  // central ones, in their order so far. Down, the lines a dot starts, half
  // each side, an odd one out on its side; up, the lines that come back to
  // it, each on the side it comes from.
  const nest = (u, way, them, l) => {
    const out = them.filter((x) => (u.own.get(x) ?? 0) === l).sort((a, b) => a.pos - b.pos), inner = them.filter((x) => (u.own.get(x) ?? 0) > l);
    if (!inner.length) return out;
    const mid = inner.reduce((s, x) => s + x.pos, 0) / inner.length;
    const half = way === "up" ? out.filter((x) => x.pos < mid).length : (out.length + (u.odd.get(l) === -1 ? 1 : 0)) >> 1;
    return [...out.slice(0, half), ...nest(u, way, inner, l + 1), ...out.slice(half)];
  };
  const slot = (u, w, way) => (u[way].length < 2 ? 0 : (nest(u, way, u[way], 0).indexOf(w) - (u[way].length - 1) / 2) / u[way].length);
  let best, fewest = Infinity;
  for (let it = 0; it < 9; it++) {
    const down = it % 2 === 0;
    for (const L of down ? layers.slice(1) : layers.slice(0, -1).reverse()) {
      for (const v of L) {
        const nb = down ? v.up : v.down;
        v.bary = nb.length ? nb.reduce((s, u) => s + u.pos + slot(u, v, down ? "down" : "up"), 0) / nb.length : v.pos;
      }
      L.sort((a, b) => a.bary - b.bary || a.pos - b.pos);
      L.forEach((v, i) => (v.pos = i));
    }
    transpose(layers);
    const n = layers.reduce((s, L) => s + crossCount(L), 0);
    if (n < fewest) { fewest = n; best = layers.map((L) => [...L]); }
  }
  best.forEach((L, r) => { layers[r] = L; L.forEach((v, i) => (v.pos = i)); });
  const sep = (a, b) => a.hi + b.lo + (a.dummy || b.dummy ? 4 : 8);
  const gaps = layers.map((L) => L.slice(1).map((v, i) => sep(L[i], v)));
  for (const [r, L] of layers.entries()) {
    let x = 0;
    L.forEach((v, i) => { if (i) x += gaps[r][i - 1]; v.x = x; });
    for (const v of L) v.x -= x / 2;
  }
  for (const L of layers) for (const v of L) v.nb = [...v.up, ...v.down];
  const most = layers.reduce((m, L) => Math.max(m, L.length), 0), trees = lives(layers, gaps);
  const t = new Float64Array(most), w = new Float64Array(most);
  for (let it = 0; it < 24; it++) {
    const down = it % 2 === 0, both = it >= 10;
    for (let j = 0; j < layers.length; j++) {
      const r = down ? j : layers.length - 1 - j, L = layers[r];
      if (!L.length) continue;
      for (let i = 0; i < L.length; i++) {
        const v = L[i], nb = both ? v.nb : down ? v.up : v.down;
        if (!nb.length) { t[i] = v.x; w[i] = .02; }
        else {
          let sx = 0, sw = 0;
          for (const u of nb) { const k = v.k.get(u); sx += k * u.x; sw += k; }
          // A line also bends as a beam, as little as it can, at each point
          // inside it, a lifeline's dots too: its own turn, and the turns
          // of its line's points either side.
          const a = v.lu, b = v.ld;
          if (both && a && b) {
            const B = BEND * Math.min(v.k.get(a), v.k.get(b));
            sx += 2 * B * (a.x + b.x); sw += 4 * B;
            if (a.line === v.line && a.lu) { sx += B * (2 * a.x - a.lu.x); sw += B; }
            if (b.line === v.line && b.ld) { sx += B * (2 * b.x - b.ld.x); sw += B; }
          }
          t[i] = sx / sw; w[i] = sw;
        }
      }
      pava(L, t, w, gaps[r]);
    }
    for (const move of trees) move();
  }
  return layers;
}
// Each life with all it started, the lives started from its dots and
// theirs, deepest first: a rigid body, so a sweep need not creep it point
// by point to where its lines out pull it. Each moves, without bending, to
// where its lines to the rest balance, but only as far as the room in every
// row it spans allows: the whole way to sit alongside what is there, or as
// far as it fits. A hidden point is the life's its line starts, for a
// start, or else the life's it leaves.
function lives(layers, gaps) {
  const anc = new Map(), own = new Map();
  const above = (l) => anc.get(l) ?? anc.set(l, new Set([l, ...(l.from ? above(l.from.life) : [])])).get(l);
  for (const L of layers) for (const p of L) for (const l of above(p.life)) (own.get(l) ?? own.set(l, []).get(l)).push(p);
  return [...own].sort((a, b) => anc.get(b[0]).size - anc.get(a[0]).size).map(([l, P]) => () => {
    const inside = (u) => anc.get(u.life).has(l);
    let f = 0, w = 0, lo = -Infinity, hi = Infinity;
    for (const p of P) {
      for (const u of p.nb) if (!inside(u)) { const k = p.k.get(u); f += k * (u.x - p.x); w += k; }
      const L = layers[p.rank], i = p.pos, g = gaps[p.rank];
      if (i > 0 && !inside(L[i - 1])) lo = Math.max(lo, L[i - 1].x + g[i - 1] - p.x);
      if (i + 1 < L.length && !inside(L[i + 1])) hi = Math.min(hi, L[i + 1].x - g[i] - p.x);
    }
    const s = w && lo <= hi ? Math.min(hi, Math.max(lo, f / w)) : 0;
    for (const p of P) p.x += s;
  });
}
// Which side each dot's odd one out goes, where a level of the lines it
// starts is odd round more central ones. A life's dots, in the order they
// came, are dealt sides from bags of BAG, half each side, each shuffled by
// its life and its place along it: a life stays balanced without a pattern,
// and a new node moves no earlier choice. A dot's further odd levels take
// turns from its own side. Sets each node's `odd`: level to -1 (left) or 1
// (right).
function odd(V) {
  const along = new Map();
  for (const v of V) {
    const pos = along.get(v.n.life) ?? 0;
    along.set(v.n.life, pos + 1);
    v.odd = new Map();
    const level = (x) => v.own.get(x) ?? 0;
    let s = bag(v.n.life, Math.floor(pos / BAG))[pos % BAG];
    for (let l = 0; v.down.some((x) => level(x) > l); l++) {
      if (v.down.filter((x) => level(x) === l).length % 2 === 0) continue;
      v.odd.set(l, s);
      s = -s;
    }
  }
}
// The i-th bag of sides of a life: BAG / 2 of each, in an order drawn from
// its name and i (FNV-1a seeding mulberry32).
function bag(life, i) {
  let h = 2166136261;
  for (const c of `${life}/${i}`) h = Math.imul(h ^ c.charCodeAt(0), 16777619);
  const draw = () => {
    h = (h + 0x6d2b79f5) | 0;
    let t = Math.imul(h ^ (h >>> 15), 1 | h);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 2 ** 32;
  };
  const sides = Array.from({ length: BAG }, (_, j) => (j < BAG / 2 ? -1 : 1));
  for (let j = BAG - 1; j > 0; j--) {
    const r = Math.floor(draw() * (j + 1));
    [sides[j], sides[r]] = [sides[r], sides[j]];
  }
  return sides;
}
// Crossings between row L and the row above: inversions of the upper ends
// of its lines, in order of their lower ends, by a Fenwick tree.
function crossCount(L) {
  const ends = [];
  for (const v of L) for (const u of v.up) ends.push([v.pos, u.pos]);
  ends.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const n = ends.reduce((m, [, b]) => Math.max(m, b + 1), 0), T = new Int32Array(n + 1);
  let c = 0;
  ends.forEach(([, b], seen) => {
    for (let i = b + 1; i > 0; i -= i & -i) c -= T[i];
    c += seen;
    for (let i = b + 1; i <= n; i += i & -i) T[i]++;
  });
  return c;
}
// Neighbours swapped while that strictly lessens their crossings with the
// rows above and below.
function transpose(layers) {
  const c = (u, w, side) => { let n = 0; for (const a of u[side]) for (const b of w[side]) if (a.pos > b.pos) n++; return n; };
  for (let pass = 0, better = true; better && pass < 20; pass++) {
    better = false;
    for (const L of layers) for (let i = 0; i + 1 < L.length; i++) {
      const u = L[i], w = L[i + 1];
      if (c(w, u, "up") + c(w, u, "down") < c(u, w, "up") + c(u, w, "down")) { L[i] = w; L[i + 1] = u; u.pos = i + 1; w.pos = i; better = true; }
    }
  }
}
// Isotonic regression with minimum gaps: each node of row L as near its
// target t[i] (weighted w[i]) as it can be, in order, each at least gap[i]
// right of the one before.
function pava(L, t, w, gap) {
  const n = L.length, OFF = [0], BS = [], BW = [], BN = [];
  for (let i = 1; i < n; i++) OFF[i] = OFF[i - 1] + gap[i - 1];
  let k = 0;
  for (let i = 0; i < n; i++) {
    BS[k] = w[i] * (t[i] - OFF[i]); BW[k] = w[i]; BN[k] = 1; k++;
    while (k > 1 && BS[k - 2] / BW[k - 2] > BS[k - 1] / BW[k - 1]) { BS[k - 2] += BS[k - 1]; BW[k - 2] += BW[k - 1]; BN[k - 2] += BN[k - 1]; k--; }
  }
  for (let b = 0, i = 0; b < k; b++) {
    const m = BS[b] / BW[b];
    for (let j = 0; j < BN[b]; j++, i++) L[i].x = m + OFF[i];
  }
}

// ─── Lines ─────────────────────────────────────────────────────────────────
// Where a ray from node v's middle, along unit u, leaves it, plus gap.
function edge(v, u, gap) {
  const n = v.n;
  const t = n.big ? 1 / Math.hypot(u[0] / BIG_HALF, u[1] / (u[1] < 0 ? n.top : n.foot)) : n.r;
  return [v.x + u[0] * (t + gap), v.y + u[1] * (t + gap)];
}
// Each end not its dot's own gets a port, `a` on its upper dot's bottom or
// `b` on its lower dot's top: `p` where it leaves or arrives, `k` the slope
// (dx/dy) it does so at. Each line goes to the side its next point lies
// on, from the dot's own line's next point if it has one, or else from the
// dot; each side's lines in the order of those points, nearest the middle
// innermost, one step apart, the same step both sides.
function ports(edges) {
  const at = new Map(), own = new Map();
  const mine = (key, l, x) => { if (l > (own.get(key)?.l ?? 0)) own.set(key, { l, x }); };
  const add = (v, side, e, next) => {
    const key = `${v.id}:${side}`;
    (at.get(key) ?? at.set(key, { v, side, h: [] }).get(key)).h.push({ e, next });
  };
  for (const e of edges) {
    const below = e.mid[0] ?? e.v, above = e.mid.at(-1) ?? e.d;
    if (e.top) mine(`${e.d.id}:1`, e.top, below.x); else add(e.d, 1, e, below);
    if (e.foot) mine(`${e.v.id}:-1`, e.foot, above.x); else add(e.v, -1, e, above);
  }
  for (const [key, { v, side, h }] of at) {
    const mid = own.get(key)?.x ?? v.x;
    const halves = [-1, 1].map((s) => h.filter((p) => (p.next.x < mid ? -1 : 1) === s).sort((a, b) => s * (a.next.x - b.next.x)));
    const step = Math.min(STEP, WIDE / Math.max(...halves.map((h) => h.length)));
    halves.forEach((h, j) => h.forEach(({ e }, i) => {
      const s = j ? 1 : -1, g = step * (i + 1);
      e[side > 0 ? "a" : "b"] = { p: edge(v, [s * Math.sin(g), side * Math.cos(g)], NGAP), k: s * side * Math.tan(g * LEAN) };
    }));
  }
}

const smooth = (s) => s * s * s * (10 + s * (6 * s - 15));
// A taper's width s from its thin end: TIP to w along a smootherstep over
// TAPER (or all of a shorter line), so the outline too meets the lifeline
// with no change, then w; and over the last POINT eased down to nothing,
// so the end is a point.
function taper(s, L, w) {
  const t = Math.min(1, s / POINT);
  return TIP * t * (2 - t) + (w - TIP) * smooth(Math.min(1, s / Math.min(L, TAPER)));
}
// An edge's points, and its widths if it tapers or is a tool's. A line runs
// from its upper end through its hidden points to its lower end, which
// keep lines side by side apart. Its x is a smooth function of its y
// through those points that never swings past them (monotone cubic,
// Fritsch-Butland). It leaves and arrives at its ports' slopes.
function route(e) {
  const { v, d } = e;
  const P0 = e.a ? e.a.p : edge(d, [0, 1], TUCK);
  const P1 = e.b ? e.b.p : edge(v, [0, -1], TUCK);
  e.P = curve([P0, ...e.mid.map((u) => [u.x, u.y]), P1], e.a?.k ?? 0, e.b?.k ?? 0);
  const w = (e.kind === "fade" ? d : v).life.light ? TOOL : LIFE;
  if (e.kind === "branch" || ((e.kind === "life" || (e.kind === "grow" && v.n.fork)) && w === LIFE)) { e.W = null; return; }
  if (e.kind === "life") { e.W = e.P.map(() => w); return; }
  const len = [0];
  for (let i = 1; i < e.P.length; i++) len[i] = len[i - 1] + Math.hypot(e.P[i][0] - e.P[i - 1][0], e.P[i][1] - e.P[i - 1][1]);
  const Lt = len.at(-1) || 1;
  e.W = len.map((s) => (e.kind === "grow" ? taper(s, Lt, w) : taper(Lt - s, Lt, w)));
}
// x of y through points p, top down, leaving at slope k0 and arriving at
// k1, sampled each 6 px or so; a straight stretch stays two points. Inside,
// each point's slope is 0 where the line turns back and otherwise a
// weighted harmonic mean of its two stretches' (Fritsch-Butland), so it
// never overshoots; an end's slope is its port's, but at most three times
// its stretch's where they lean the same way.
function curve(p, k0, k1) {
  const n = p.length - 1, h = [], m = [];
  for (let i = 0; i < n; i++) { h[i] = Math.max(p[i + 1][1] - p[i][1], 1e-6); m[i] = (p[i + 1][0] - p[i][0]) / h[i]; }
  const end = (k, s) => (k * s > 0 && Math.abs(k) > 3 * Math.abs(s) ? 3 * s : k);
  const t = p.map((_, i) => {
    if (i === 0) return end(k0, m[0]);
    if (i === n) return end(k1, m[n - 1]);
    const [a, b] = [m[i - 1], m[i]];
    return a * b <= 0 ? 0 : (3 * (h[i - 1] + h[i])) / ((2 * h[i] + h[i - 1]) / a + (h[i] + 2 * h[i - 1]) / b);
  });
  const P = [p[0]];
  for (let i = 0; i < n; i++) {
    const [x0, y0] = p[i], [x1, y1] = p[i + 1];
    const S = Math.abs(t[i] - m[i]) + Math.abs(t[i + 1] - m[i]) < 1e-9 ? 1 : Math.max(2, Math.ceil(Math.hypot(x1 - x0, y1 - y0) / 6));
    for (let j = 1; j <= S; j++) {
      const s = j / S, s2 = s * s, s3 = s2 * s;
      P.push([(2 * s3 - 3 * s2 + 1) * x0 + (s3 - 2 * s2 + s) * h[i] * t[i] + (3 * s2 - 2 * s3) * x1 + (s3 - s2) * h[i] * t[i + 1], y0 + (y1 - y0) * s]);
    }
  }
  return P;
}

// ─── Crossings ─────────────────────────────────────────────────────────────
// Every place two lines of different lives cross, away from a node both
// end at: the more upright one goes over, the other under. Each as
// { over, under, at, seg, angle }: `over` and `under` the indices of the
// edges, `seg` the over one's segment there, ending at P[seg], `angle`
// between the two there, from 0 to a right angle. A line out of a dot into
// another life's counts as a life of its own. Segments are bucketed in a
// grid, so this is near linear.
function crossings(edges) {
  const CELL = 24, grid = new Map(), out = [];
  const segs = [];
  edges.forEach((e, i) => {
    for (let j = 1; j < e.P.length; j++) {
      const a = e.P[j - 1], b = e.P[j], s = { i, j, a, b };
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
      out.push({ over: o.i, under: u.i, at: p, seg: o.j, angle: angle(A, B) });
    }
  }
  return out;
}
// Whose line it is, by a name: a result's is the life that finished; a
// line out of a dot into another life's is its own.
const own = (e) => (e.kind === "port" ? `${e.d.id}>${e.v.id}` : (e.kind === "fade" ? e.d : e.v).n.life);
const slope = (s) => Math.abs((s.b[0] - s.a[0]) / (s.b[1] - s.a[1] || 1e-9));
const angle = (s, t) => {
  const u = [s.b[0] - s.a[0], s.b[1] - s.a[1]], v = [t.b[0] - t.a[0], t.b[1] - t.a[1]];
  return Math.atan2(Math.abs(u[0] * v[1] - u[1] * v[0]), Math.abs(u[0] * v[0] + u[1] * v[1]));
};
function meet(p, q, r, s) {
  const d = (q[0] - p[0]) * (s[1] - r[1]) - (q[1] - p[1]) * (s[0] - r[0]);
  if (!d) return null;
  const t = ((r[0] - p[0]) * (s[1] - r[1]) - (r[1] - p[1]) * (s[0] - r[0])) / d;
  const u = ((r[0] - p[0]) * (q[1] - p[1]) - (r[1] - p[1]) * (q[0] - p[0])) / d;
  return t >= 0 && t <= 1 && u >= 0 && u <= 1 ? [p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])] : null;
}

// The over line of a crossing near where it crosses, as { P, W }: SPAN
// along it each way. The page erases that band, GAP wider than it each
// side, from the line under it, so the gap follows both lines and touches
// nothing else. Two lines LIFE wide clear each other's gap once LIFE + GAP
// apart, within (LIFE + GAP) / sin(STEEP) of a crossing at STEEP, the
// shallowest that has a gap; SPAN is twice that, room for them to bend.
const SPAN = 2 * (LIFE + GAP) / Math.sin(STEEP);
function near(e, { at, seg }) {
  const w = (i) => e.W?.[i] ?? LIFE, [a, b] = [e.P[seg - 1], e.P[seg]];
  const t = Math.hypot(at[0] - a[0], at[1] - a[1]) / (Math.hypot(b[0] - a[0], b[1] - a[1]) || 1);
  const mid = [...at, w(seg - 1) + t * (w(seg) - w(seg - 1))];
  const walk = (i, step) => {
    const out = [];
    let p = mid, s = 0;
    for (; i >= 0 && i < e.P.length; i += step) {
      const q = [...e.P[i], w(i)], d = Math.hypot(q[0] - p[0], q[1] - p[1]);
      if (s + d >= SPAN) return [...out, p.map((v, k) => v + ((SPAN - s) / d) * (q[k] - v))];
      out.push(q); s += d; p = q;
    }
    return out;
  };
  const Q = [...walk(seg - 1, -1).reverse(), mid, ...walk(seg, 1)];
  return { P: Q.map(([x, y]) => [x, y]), W: e.W && Q.map((q) => q[2]) };
}
// Each line under others on a sheet, the page's one mask for all of a
// sheet's bands: the first sheet where none of its bands touches another
// line, and no other line's band touches it, so a band erases only the line
// it was made for. Few sheets, so few masks. Each edge's sheet, or
// undefined.
function sheets(edges, gaps) {
  const box = (P, r) => { const xs = P.map((p) => p[0]), ys = P.map((p) => p[1]); return [Math.min(...xs) - r, Math.min(...ys) - r, Math.max(...xs) + r, Math.max(...ys) + r]; };
  const lines = edges.map((e) => ({ P: e.P, box: box(e.P, LIFE) }));
  const bands = gaps.map((G) => G.map((g) => ({ ...g, box: box(g.P, LIFE + 2 * GAP) })));
  const apart = (a, b) => a[2] < b[0] || b[2] < a[0] || a[3] < b[1] || b[3] < a[1];
  const touches = (i, j) => bands[i].some((g) => {
    if (apart(g.box, lines[j].box)) return false;
    const P = lines[j].P;
    for (let m = 1; m < P.length; m++) {
      const c = P[m - 1], q = P[m];
      if (apart(g.box, [Math.min(c[0], q[0]), Math.min(c[1], q[1]), Math.max(c[0], q[0]), Math.max(c[1], q[1])])) continue;
      for (let k = 1; k < g.P.length; k++) {
        const a = g.P[k - 1], p = g.P[k], r = Math.max(g.W?.[k - 1] ?? LIFE, g.W?.[k] ?? LIFE) / 2 + GAP + LIFE / 2;
        if (meet(a, p, c, q) || close(c, q, a, r) || close(c, q, p, r) || close(a, p, c, r) || close(a, p, q, r)) return true;
      }
    }
    return false;
  });
  const on = [], out = [];
  gaps.forEach((G, i) => {
    if (!G.length) return;
    let s = on.findIndex((S) => S.every((j) => !touches(i, j) && !touches(j, i)));
    if (s < 0) s = on.push([]) - 1;
    on[s].push(i);
    out[i] = s;
  });
  return out;
}
// Whether point p is within r of the segment from a to b.
function close(a, b, p, r) {
  const vx = b[0] - a[0], vy = b[1] - a[1], t = Math.max(0, Math.min(1, ((p[0] - a[0]) * vx + (p[1] - a[1]) * vy) / (vx * vx + vy * vy || 1)));
  return Math.hypot(a[0] + t * vx - p[0], a[1] + t * vy - p[1]) < r;
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
