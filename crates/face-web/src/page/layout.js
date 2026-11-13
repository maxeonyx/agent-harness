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
// now, planned, fork }. `lane` names its life, `light` a tool's, `old` an
// old version's, `fork` a fork's (a copy of its maker's context); `r` is a
// dot's radius, and a big node reaches `top` above its centre and `foot`
// below. `lo` and `hi` are how far it reaches
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
// over at most variant.taper, then keeps it; it narrows to a point over its last
// POINT, which stops NGAP short of the dot it starts from or rejoins. A
// crossing's gap is GAP each side of the line over it, and only where the
// lines cross at variant.steep or more: at a shallower angle they run along each
// other, and a gap would cut the line under into islands, so the line over
// simply covers it.
export const LIFE = 2.2;
const TOOL = 1.2;
export const GAP = 1.5;
const TIP = .5, POINT = 4, NGAP = 1.5, TUCK = -.6;
// Ports: the lines out of a node's bottom, and into its top, spread evenly
// over the WIDE either side of upright, its own lifeline upright in the
// middle and each other line on the side facing its other end, in the
// order the lines arrive; both sides one step apart. At its port a line
// points LEAN of the way from upright to straight out from the middle.
const WIDE = Math.PI / 3, LEAN = .5;
// A line leaving or reaching a port at a slant bulges at most REACH times
// as far across as it goes.
const REACH = 1;
// A spawn or a result bends over as much of its height as it goes across,
// but at least TURN, at its maker's or its reader's end, and stands upright
// in its own lane the rest.
const TURN = 48;
// A big node's half width, for where lines meet it.
const BIG_HALF = 13;

// SPIKE (MC-graph-options): switchable parts, each a decision in the
// options doc. `place`: layered (5a5c9c0's), lanes (main's), lanes243
// (#243's column rule), short (#243, a one-dot child's lane held only on
// its dot's row). `curve`: orig (5a5c9c0's: through the gaps, rounded),
// bend (#243, over the whole height), end (#255, bent only near its slant
// end). `ports`: none (every line at its dot's middle), fixed (#240's
// 36-66-82 degrees), even (#243's, over 120 degrees), pizza (left and
// right slices reserved, even over the top and bottom sectors at most
// `gap` degrees apart, same-context lines innermost). `parallel`: overlap
// (string-pulled lines share a corner), side (lines through one gap keep
// their order, LIFE apart), fan (lines keep their hidden points' x).
// `taper`: its cap in px. `steep`: the angle in degrees a crossing needs
// for a gap.
const DEFAULT = { place: "layered", curve: "orig", ports: "pizza", gap: 20, parallel: "overlap", taper: 48, steep: 30 };
let variant = DEFAULT;
export function setVariant(v) { variant = { ...DEFAULT, ...v }; }

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
  let layers = null;
  if (variant.place === "layered") layers = layered(V, edges);
  else if (variant.place === "sugi" || variant.place === "tree") layers = placed(V, edges);
  else { columns(L); spindles(L); across(L); }
  // Down: each row as tall as its tallest node plus 17, the present row at
  // y = 0.
  const tall = [], ys = [0];
  for (const v of V) tall[v.rank] = Math.max(tall[v.rank] ?? 0, v.n.along);
  for (let r = 1; r < tall.length; r++) ys[r] = ys[r - 1] + (tall[r - 1] ?? 0) / 2 + (tall[r] ?? 0) / 2 + 17;
  for (const v of V) { v.y = ys[v.rank] - ys[P]; if (!layers) v.x = v.lane.spin ? v.lane.from.lane.x + v.lane.off : v.lane.x; }
  if (layers) gapsOf(layers, ys, P);
  const sides = variant.ports !== "none" ? ports(edges) : [];
  const at = [];
  for (const v of V) (at[v.rank] ??= []).push(v);
  const drawn = new Map();
  for (const e of edges) route(e, at, drawn);
  // Then each side's ports in the order its lines, as drawn, reach the
  // height where the shortest of them ends, and those lines drawn again.
  for (const { h, step } of sides) {
    if (h.length < 2 || mine()) continue;
    const y = h[0].v.y + h[0].side * Math.min(...h.map((p) => Math.abs(p.far.y - p.v.y)));
    const out = (p) => Math.abs((xAt(p.e, y) ?? p.far.x) - p.v.x);
    const was = [...h];
    h.sort((a, b) => fork(a) - fork(b) || out(a) - out(b));
    if (h.every((p, i) => p === was[i])) continue;
    place(h, step);
    for (const p of h) { drawn.delete(p.e); route(p.e, at, drawn); }
  }
  const cuts = crossings(edges);
  const key = (e) => `${e.v.id}>${e.d.id}`;
  const gaps = edges.map((e, i) => cuts.filter((c) => c.under === i && c.angle >= steep()).map((c) => ({ over: key(edges[c.over]), ...near(edges[c.over], c) })));
  const sheet = sheets(edges, gaps);
  return { at: new Map(V.map((v) => [v.id, { x: v.x, y: v.y }])), cuts, edges: edges.map((e, i) => ({ from: e.v.id, to: e.d.id, kind: e.kind, P: e.P, W: e.W, sheet: sheet[i], gaps: gaps[i] })) };
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
// row its life starts on to the row its last line leaves it, and it stands
// on the rows from the one below its maker's, `a`, to that last one, `b`:
// on its maker's row its line is still bending out of its maker. A life of
// one dot, made by another and read by its maker's next dot or by no one,
// with no other line, is a spindle: it hangs beside its maker's lane and
// holds none.
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
    l.a = l.from ? l.r0 + 1 : l.r0; l.b = l.r1;
    l.light = v.n.light;
    l.spin = l.nodes.length === 1 && !!l.from && !v.n.old && (!l.reader || l.reader === l.from.lane) && v.fan === (l.reader ? 2 : 1)
      && !l.from.lane.nodes.some((u) => l.r0 < u.rank && u.rank < l.r1);
    if (l.spin) l.from.lane.spins.push(l);
    else if (variant.place === "short" && l.nodes.length === 1 && l.from && !v.n.old) l.r0 = l.r1 = v.rank;
  }
  return [...L.values()].sort((a, b) => a.r0 - b.r0 || a.nodes[0].key - b.nodes[0].key);
}

// Each lane's column, in the order they start. A column holds one life at
// a time, with a row clear between them: a life read on the row another's
// maker makes it on may share its column, one bending into its reader
// above that row as the other bends out below. A life with no maker takes
// the first free column from the middle rightward; a child takes the
// first free one on the side of its maker where its lines cross fewer, at
// the rows they bend on: a lane held there, and every line its dot there
// starts or reads; but never where its maker's own result would pass over
// its start. Even, it goes on its maker's outer side, or, for a maker in
// the middle, alternately right and left. Columns are an order, not
// places: across() packs them.
function columns(L) {
  const held = new Map();
  const end = (l) => (l.reader ? l.r1 - 1 : l.r1);
  const free = variant.place === "lanes243" ? (s, l) => !(held.get(s) || []).some((h) => h.r0 <= l.r1 && l.r0 <= h.r1)
    : (s, l) => !(held.get(s) || []).some((h) => h.a <= end(l) + 1 && l.a <= end(h) + 1);
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
    m.reach = new Map();
    for (let r = m.r0; r <= m.r1; r++) m.reach.set(r, [own(r, -1), own(r, 1)]);
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

// ─── Layered placement (5a5c9c0's) ─────────────────────────────────────────
// The standard layered layout Graphviz `dot` uses: a long edge passes
// through a hidden point in each row it crosses; rows ordered by barycentre
// sweeps; then across, each line a spring pulling its ends together, each
// row placed as near its neighbours as the gaps allow (PAVA). A long edge's
// hidden points are a much stiffer spring, so it keeps one lane; every node
// is pulled a little to the centre line. Sets each node's x; each edge's
// hidden points, top down, as `mid`. Returns the rows.
function layered(V, edges) {
  const layers = [];
  const put = (v) => (layers[v.rank] ??= []).push(v);
  for (const v of V) { Object.assign(v, { up: [], down: [], lo: v.n.lo, hi: v.n.hi, along: v.n.along }); put(v); }
  for (const e of edges) {
    const { v, d } = e, chain = [v];
    for (let r = v.rank - 1; r > d.rank; r--) {
      const x = { dummy: true, rank: r, key: d.key + .5, up: [], down: [], lo: 1, hi: 1, along: 0 };
      put(x);
      chain.push(x);
    }
    chain.push(d);
    for (let i = 0; i < chain.length - 1; i++) { chain[i].up.push(chain[i + 1]); chain[i + 1].down.push(chain[i]); }
    e.mid = chain.slice(1, -1).reverse();
  }
  for (let r = 0; r < layers.length; r++) layers[r] ??= [];
  for (const L of layers) { L.sort((a, b) => a.key - b.key); L.forEach((v, i) => (v.pos = i)); }
  for (let it = 0; it < 9; it++) {
    const down = it % 2 === 0;
    for (const L of down ? layers.slice(1) : layers.slice(0, -1).reverse()) {
      for (const v of L) {
        const nb = down ? v.up : v.down;
        v.bary = nb.length ? nb.reduce((s, u) => s + u.pos, 0) / nb.length : v.pos;
      }
      L.sort((a, b) => a.bary - b.bary || a.pos - b.pos);
      L.forEach((v, i) => (v.pos = i));
    }
  }
  const sep = (a, b) => a.hi + b.lo + (a.dummy || b.dummy ? 4 : 8);
  const CENTRE = 1;
  const stiff = (a, b) => (a.dummy && b.dummy ? 40 : a.dummy || b.dummy ? 2 : 1);
  const gaps = layers.map((L) => L.slice(1).map((v, i) => sep(L[i], v)));
  for (const [r, L] of layers.entries()) {
    let x = 0;
    L.forEach((v, i) => { if (i) x += gaps[r][i - 1]; v.x = x; });
    for (const v of L) v.x -= x / 2;
  }
  for (const L of layers) for (const v of L) v.nb = [...v.up, ...v.down];
  const most = layers.reduce((m, L) => Math.max(m, L.length), 0);
  const t = new Float64Array(most), w = new Float64Array(most);
  for (let it = 0; it < 24; it++) {
    const down = it % 2 === 0, both = it >= 10;
    for (let j = 0; j < layers.length; j++) {
      const r = down ? j : layers.length - 1 - j, L = layers[r];
      if (!L.length) continue;
      for (let i = 0; i < L.length; i++) {
        const v = L[i], nb = both ? v.nb : down ? v.up : v.down;
        if (!nb.length) { t[i] = v.x; w[i] = .02; }
        // A chain of single links stays straight.
        else if (both && !v.dummy && v.up.length === 1 && v.up[0].down.length === 1 && !v.up[0].dummy) { t[i] = v.up[0].x; w[i] = 1; }
        else if (both && !v.dummy && v.down.length === 1 && v.down[0].up.length === 1 && !v.down[0].dummy) { t[i] = v.down[0].x; w[i] = 1; }
        else {
          let sx = 0, sw = 0;
          for (const u of nb) { const k = stiff(u, v); sx += k * u.x; sw += k; }
          t[i] = sx / sw; w[i] = sw;
        }
        t[i] = (t[i] * w[i]) / (w[i] + CENTRE); w[i] += CENTRE;
      }
      pava(L, t, w, gaps[r]);
    }
  }
  // A long edge's hidden points go to the one x nearest their mean that
  // keeps every row's gaps; one crossed by another keeps its bends.
  for (const e of edges) {
    if (e.mid.length < 2) continue;
    let lo = -Infinity, hi = Infinity, sum = 0;
    for (const u of e.mid) {
      const L = layers[u.rank], i = u.pos;
      if (i > 0) lo = Math.max(lo, L[i - 1].x + sep(L[i - 1], u));
      if (i < L.length - 1) hi = Math.min(hi, L[i + 1].x - sep(u, L[i + 1]));
      sum += u.x;
    }
    if (lo > hi) continue;
    const x = Math.min(hi, Math.max(lo, sum / e.mid.length));
    for (const u of e.mid) u.x = x;
  }
  return layers;
}
// ─── Placement by order (spike MC-graph-layout) ────────────────────────────
// The whole Sugiyama pipeline on the rows above: a long edge a hidden point
// in each row it crosses; each row's order, for as few crossings as can be
// found; then each point's x. `place`: sugi (the order from barycentre
// sweeps with transposition, a life's own line held in the middle of what
// ties with it) or tree (the order read off the tree of lives: each life's
// children either side of its line, balanced, a longer one outside a
// shorter, then transposition). `coords`: qp (each line a spring, x the
// least energy that keeps every row's gaps, the main life pulled to the
// middle) or bk (Brandes and Koepf: lines aligned straight where they can
// be, the four alignments' median). `hug`: with qp, the stiffness of the
// spring holding each point to its parent life in its row (sugi-qp2 is 2).
// Sets x and `mid` as layered() does.
const mine = () => variant.place === "sugi" || variant.place === "tree";
function placed(V, edges) {
  const layers = [];
  const put = (u) => (layers[u.rank] ??= []).push(u);
  for (const v of V) { Object.assign(v, { up: [], down: [], lo: v.n.lo, hi: v.n.hi, life: v.n.lane }); put(v); }
  for (const e of edges) {
    const { v, d } = e, chain = [v];
    // A hidden point is on the life the line belongs to: a result's to the
    // child that ends, anything else to the life it runs into.
    const life = e.kind === "fade" ? d.n.lane : v.n.lane;
    for (let r = v.rank - 1; r > d.rank; r--) {
      const x = { dummy: true, e, rank: r, key: d.key + .5, up: [], down: [], lo: 1, hi: 1, life, own: e.kind === "life" };
      put(x);
      chain.push(x);
    }
    chain.push(d);
    for (let i = 0; i < chain.length - 1; i++) { chain[i].up.push(chain[i + 1]); chain[i + 1].down.push(chain[i]); }
    e.mid = chain.slice(1, -1).reverse();
  }
  for (let r = 0; r < layers.length; r++) layers[r] ??= [];
  for (const L of layers) for (const u of L) u.nb = [...u.up, ...u.down];
  if (variant.place === "tree") treeOrder(V, edges, layers);
  else sweepOrder(layers);
  transpose(layers);
  const sep = (a, b) => a.hi + b.lo + (a.dummy || b.dummy ? 4 : 8);
  if (variant.coords === "bk") brandesKoepf(layers, sep, V[0].n.lane);
  else springs(layers, sep, V[0].n.lane, parents(edges));
  return layers;
}
const renumber = (L) => L.forEach((u, i) => (u.pos = i));
// Crossings between two rows, the lower one's lines up to the upper.
function crossCount(lower) {
  const pairs = [];
  for (const u of lower) for (const w of u.up) pairs.push([u.pos, w.pos]);
  pairs.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  // Inversions of the upper ends, by a Fenwick tree.
  const n = pairs.reduce((m, p) => Math.max(m, p[1] + 1), 0), T = new Int32Array(n + 1);
  let c = 0, seen = 0;
  for (const [, b] of pairs) {
    let s = 0;
    for (let i = b + 1; i > 0; i -= i & -i) s += T[i];
    c += seen - s; seen++;
    for (let i = b + 1; i <= n; i += i & -i) T[i]++;
  }
  return c;
}
const allCrossings = (layers) => layers.reduce((s, L, r) => s + (r ? crossCount(L) : 0), 0);
// Barycentre sweeps, down then up, keeping the order with fewest crossings.
// What ties sits in its old order, but a life's own line, the one on the
// life of the point it ties on, goes to the middle of the tie: a life's
// branches balance either side of it.
function sweepOrder(layers) {
  for (const L of layers) { L.sort((a, b) => a.key - b.key); renumber(L); }
  let best = layers.map((L) => [...L]), fewest = allCrossings(layers);
  for (let it = 0; it < 16 && fewest; it++) {
    const down = it % 2 === 0;
    for (const L of down ? layers.slice(1) : layers.slice(0, -1).reverse()) {
      for (const u of L) {
        const nb = down ? u.up : u.down;
        u.bary = nb.length ? nb.reduce((s, w) => s + w.pos, 0) / nb.length : u.pos;
        u.tie = nb.length === 1 ? nb[0] : null;
      }
      L.sort((a, b) => a.bary - b.bary || a.pos - b.pos);
      for (let i = 0; i < L.length;) {
        let j = i + 1;
        while (j < L.length && L[j].bary === L[i].bary) j++;
        const c = L.slice(i, j).findIndex((u) => u.tie && u.life === u.tie.life);
        if (j - i > 2 && c >= 0) { const [m] = L.splice(i + c, 1); L.splice(i + ((j - i - 1) >> 1), 0, m); }
        i = j;
      }
      renumber(L);
    }
    transpose(layers);
    const n = allCrossings(layers);
    if (n < fewest) { fewest = n; best = layers.map((L) => [...L]); }
  }
  best.forEach((L, r) => { layers[r] = L; renumber(L); });
}
// Neighbours swapped while that strictly lessens the crossings with the
// rows above and below.
function transpose(layers) {
  const c = (u, w, side) => {
    let n = 0;
    for (const a of u[side]) for (const b of w[side]) if (a.pos > b.pos) n++;
    return n;
  };
  for (let pass = 0, better = true; better && pass < 20; pass++) {
    better = false;
    for (const L of layers) for (let i = 0; i + 1 < L.length; i++) {
      const [u, w] = [L[i], L[i + 1]];
      if (c(w, u, "up") + c(w, u, "down") < c(u, w, "up") + c(u, w, "down")) { L[i] = w; L[i + 1] = u; u.pos = i + 1; w.pos = i; better = true; }
    }
  }
}
// The order of the tree of lives: each life's children either side of its
// line, by when they start, each on the side less deep over its rows (in
// turn when even), a longer one outside a shorter; read off left to right,
// each child with its own children round it. Every row in that order.
function treeOrder(V, edges, layers) {
  const lives = new Map();
  for (const v of V) { const l = lives.get(v.n.lane) ?? lives.set(v.n.lane, { id: v.n.lane, a: v.rank, b: v.rank, kids: [], key: v.key }).get(v.n.lane); l.b = Math.max(l.b, v.rank); }
  for (const e of edges) {
    const c = lives.get(e.v.n.lane), m = lives.get(e.d.n.lane);
    if (e.kind === "grow" || e.kind === "branch") { if (!c.from) { c.from = m; c.a = e.d.rank; m.kids.push(c); } }
    else if (e.kind === "fade") m.b = Math.max(m.b, e.v.rank);
  }
  const at = new Map();
  let i = 0;
  const walk = (l) => {
    const side = { [-1]: [], [1]: [] };
    let was = 1;
    for (const c of l.kids.sort((p, q) => p.a - q.a || q.b - p.b || p.key - q.key)) {
      const deep = (s) => side[s].filter((o) => o.a <= c.b && c.a <= o.b).length;
      const s = deep(-1) < deep(1) ? -1 : deep(1) < deep(-1) ? 1 : (was = -was);
      side[s].push(c);
    }
    const outward = (s) => side[s].sort((p, q) => p.b - p.a - (q.b - q.a) || p.a - q.a);
    for (const c of outward(-1).reverse()) walk(c);
    at.set(l.id, i++);
    for (const c of outward(1)) walk(c);
  };
  for (const l of [...lives.values()].filter((l) => !l.from).sort((p, q) => p.key - q.key)) walk(l);
  for (const L of layers) { L.sort((a, b) => at.get(a.life) - at.get(b.life) || (a.own ? -1 : 0) - (b.own ? -1 : 0) || a.key - b.key); renumber(L); }
}
// Each line a spring, three times stiffer between two hidden points, twice
// between a hidden point and a node, and four times stiffer again on a
// life's own line, so lines run straight but may lean; the main life's
// nodes pulled to x = 0. The least energy keeping each row's gaps, by
// sweeps of rows, each row exact given its neighbours (PAVA).
// A life's parent: the life it grew or branched from.
function parents(edges) {
  const from = new Map();
  for (const e of edges) if ((e.kind === "grow" || e.kind === "branch") && !from.has(e.v.n.lane)) from.set(e.v.n.lane, e.d.n.lane);
  return from;
}
// Each point also held by a spring of `hug` to its parent life in its row,
// where that life passes (its dot, or its own line): a child keeps near the
// line it came from, both pulled together, and no free space is left between.
function springs(layers, sep, main, from) {
  const hug = variant.hug ?? 0;
  if (hug) for (const L of layers) {
    const at = new Map();
    for (const u of L) if (!u.dummy || u.own) at.set(u.life, u);
    for (const u of L) {
      const p = at.get(from.get(u.life));
      if (p && p !== u) { (u.side ??= []).push(p); (p.side ??= []).push(u); }
    }
  }
  const stiff = (a, b) => (a.dummy && b.dummy ? 8 : a.dummy || b.dummy ? 2 : 1) * (a.life === b.life ? 4 : 1);
  const gaps = layers.map((L) => L.slice(1).map((v, i) => sep(L[i], v)));
  for (const [r, L] of layers.entries()) {
    let x = 0;
    L.forEach((v, i) => { if (i) x += gaps[r][i - 1]; v.x = x; });
    for (const v of L) v.x -= x / 2;
  }
  const most = layers.reduce((m, L) => Math.max(m, L.length), 0);
  const t = new Float64Array(most), w = new Float64Array(most);
  for (let it = 0; it < 120; it++) {
    const down = it % 2 === 0;
    for (let j = 0; j < layers.length; j++) {
      const r = down ? j : layers.length - 1 - j, L = layers[r];
      for (let i = 0; i < L.length; i++) {
        const v = L[i], pull = v.life === main && !v.dummy ? 4 : .01;
        let sx = 0, sw = pull;
        for (const u of v.nb) { const k = stiff(u, v); sx += k * u.x; sw += k; }
        for (const u of v.side ?? []) { sx += hug * u.x; sw += hug; }
        t[i] = sx / sw; w[i] = sw;
      }
      pava(L, t, w, gaps[r]);
    }
  }
}
// Brandes and Koepf: in each of four directions each point aligned in one
// block with its median neighbour, unless that crosses an alignment already
// made or a line between two hidden points (those keep straight first); the
// blocks packed against that side; then each x the mean of the middle two
// of the four. The main life's mean x is moved to 0.
function brandesKoepf(layers, sep, main) {
  const all = layers.flat();
  // Lines between two hidden points that a line out of a node crosses: the
  // node's line may not align across them.
  const marked = new Set();
  for (let r = 1; r < layers.length; r++) {
    const L = layers[r];
    let k0 = 0, l = 0;
    for (let l1 = 0; l1 < L.length; l1++) {
      const inner = L[l1].dummy && L[l1].up[0]?.dummy ? L[l1].up[0] : null;
      if (l1 === L.length - 1 || inner) {
        const k1 = inner ? inner.pos : layers[r - 1].length - 1;
        for (; l <= l1; l++) for (const u of L[l].up) if ((u.pos < k0 || u.pos > k1) && !(L[l].dummy && u.dummy)) marked.add(`${u.pos}:${r}:${L[l].pos}`);
        k0 = k1;
      }
    }
  }
  const runs = [];
  for (const vert of [0, 1]) for (const horiz of [0, 1]) {
    const rows = vert ? [...layers].reverse() : layers;
    const ord = (L) => (horiz ? [...L].reverse() : L);
    const nbOf = (u) => (vert ? u.down : u.up);
    const root = new Map(), align = new Map();
    for (const u of all) { root.set(u, u); align.set(u, u); }
    for (const L of rows) {
      let lastPos = -Infinity;
      for (const v of ord(L)) {
        const nb = [...nbOf(v)].sort((a, b) => (horiz ? b.pos - a.pos : a.pos - b.pos));
        if (!nb.length) continue;
        const m = nb.length;
        for (const i of [...new Set([Math.floor((m - 1) / 2), Math.ceil((m - 1) / 2)])]) {
          if (align.get(v) !== v) break;
          const u = nb[i], p = horiz ? -u.pos : u.pos;
          const key = vert ? `${v.pos}:${u.rank}:${u.pos}` : `${u.pos}:${v.rank}:${v.pos}`;
          if (!marked.has(key) && p > lastPos) {
            align.set(u, v); root.set(v, root.get(u)); align.set(v, root.get(v)); lastPos = p;
          }
        }
      }
    }
    // Pack the blocks: each as far toward its side as its row neighbours
    // allow (a longest path over blocks; classes kept apart by a shift).
    const sink = new Map(all.map((u) => [u, u])), shift = new Map(), x = new Map();
    const pred = (v) => { const L = layers[v.rank], i = horiz ? v.pos + 1 : v.pos - 1; return i >= 0 && i < L.length ? L[i] : null; };
    const gapOf = (a, b) => (horiz ? sep(b, a) : sep(a, b));
    const placeBlock = (v) => {
      if (x.has(v)) return;
      x.set(v, 0);
      let w = v;
      do {
        const p = pred(w);
        if (p) {
          const u = root.get(p);
          placeBlock(u);
          if (sink.get(v) === v) sink.set(v, sink.get(u));
          if (sink.get(v) !== sink.get(u)) shift.set(sink.get(u), Math.min(shift.get(sink.get(u)) ?? Infinity, x.get(v) - x.get(u) - gapOf(p, w)));
          else x.set(v, Math.max(x.get(v), x.get(u) + gapOf(p, w)));
        }
        w = align.get(w);
      } while (w !== v);
    };
    for (const u of all) if (root.get(u) === u) placeBlock(u);
    const X = new Map();
    for (const u of all) {
      const rt = root.get(u), s = shift.get(sink.get(rt));
      X.set(u, (horiz ? -1 : 1) * (x.get(rt) + (s !== undefined && s < Infinity ? s : 0)));
    }
    runs.push(X);
  }
  // Align the four to the narrowest, then the mean of the middle two.
  const span = runs.map((X) => { const xs = [...X.values()]; return [Math.min(...xs), Math.max(...xs)]; });
  const n = span.reduce((b, s, i) => (s[1] - s[0] < span[b][1] - span[b][0] ? i : b), 0);
  runs.forEach((X, i) => {
    const d = i % 2 ? span[n][1] - span[i][1] : span[n][0] - span[i][0];
    for (const [u, v] of X) X.set(u, v + d);
  });
  for (const u of all) { const xs = runs.map((X) => X.get(u)).sort((a, b) => a - b); u.x = (xs[1] + xs[2]) / 2; }
  const ms = all.filter((u) => u.life === main && !u.dummy);
  const c = ms.reduce((s, u) => s + u.x, 0) / (ms.length || 1);
  for (const u of all) u.x -= c;
}

// The room each hidden point has in its row, from the nearest real node on
// its left to the nearest on its right, less 4 px: its line passes anywhere
// in it, drawn taut (funnel()).
function gapsOf(layers, ys, P) {
  for (const [r, L] of layers.entries()) {
    let lo = -Infinity;
    for (const u of L) { u.y = ys[r] - ys[P]; if (u.dummy) u.gap = [lo, Infinity]; else lo = u.x + u.hi + 4; }
    let hi = Infinity;
    for (let i = L.length - 1; i >= 0; i--) if (L[i].dummy) L[i].gap[1] = hi; else hi = L[i].x - L[i].lo - 4;
    if (variant.parallel === "fan") for (const u of L) if (u.dummy) u.gap = [u.x, u.x];
    // Side by side: the hidden points in one gap keep their order, each
    // ROOM inside the next, so lines pulled to one corner lie LIFE apart.
    if (variant.parallel === "side") {
      const ROOM = LIFE + .4;
      for (let i = 0; i < L.length;) {
        if (!L[i].dummy) { i++; continue; }
        let j = i;
        while (j < L.length && L[j].dummy) j++;
        for (let k = i; k < j; k++) {
          const g = L[k].gap;
          g[0] += (k - i) * ROOM; g[1] -= (j - 1 - k) * ROOM;
          if (g[0] > g[1]) g[0] = g[1] = (g[0] + g[1]) / 2;
        }
        i = j;
      }
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
// The shortest line from P0 down to P1 through the gaps its hidden points
// leave in the rows it crosses (a funnel, as in string pulling): its turns.
function funnel(P0, P1, mid) {
  const gaps = mid.map((u) => [u.gap[0], u.gap[1], u.y]);
  gaps.push([P1[0], P1[0], P1[1]]);
  let a = P0, k = 0;
  const path = [a];
  while (k < gaps.length - 1) {
    let lo = -Infinity, hi = Infinity, loK = -1, hiK = -1, turn = null;
    for (let j = k; j < gaps.length; j++) {
      const [l, h, y] = gaps[j], dy = y - a[1] || 1e-9;
      const sl = (l - a[0]) / dy, sh = (h - a[0]) / dy;
      if (sl > hi) { turn = [hiK, 1]; break; }
      if (sh < lo) { turn = [loK, 0]; break; }
      if (sl > lo) { lo = sl; loK = j; }
      if (sh < hi) { hi = sh; hiK = j; }
    }
    if (!turn || turn[0] === gaps.length - 1) break;
    const [j, side] = turn;
    a = [gaps[j][side], gaps[j][2]];
    path.push(a);
    k = j + 1;
  }
  path.push(P1);
  return path;
}
// 5a5c9c0's curve through turns: it leaves and arrives along k0 and k1
// (dx/dy) and rounds each turn, a cubic a stretch whose handles lie along
// the chord of the turn's neighbours (Catmull-Rom), at most BEND long, so a
// straight stretch stays straight. Sampled each 6 px or so.
const BEND = 22;
function rounded(p, k0, k1) {
  const n = p.length - 1, unit = (x, y) => { const l = Math.hypot(x, y) || 1; return [x / l, y / l]; };
  const dir = p.map((_, i) => (i === 0 ? unit(k0, 1) : i === n ? unit(k1, 1) : unit(p[i + 1][0] - p[i - 1][0], p[i + 1][1] - p[i - 1][1])));
  const P = [];
  for (let i = 0; i < n; i++) {
    const a = p[i], b = p[i + 1];
    const end = Math.max(0, Math.min((b[1] - a[1]) * .55, BEND));
    const mid = Math.min(Math.hypot(b[0] - a[0], b[1] - a[1]) / 3, BEND);
    const h0 = i === 0 ? end : mid, h1 = i + 1 === n ? end : mid;
    const c = [a[0] + dir[i][0] * h0, a[1] + dir[i][1] * h0], d = [b[0] - dir[i + 1][0] * h1, b[1] - dir[i + 1][1] * h1];
    const S = Math.max(2, Math.ceil((Math.hypot(c[0] - a[0], c[1] - a[1]) + Math.hypot(d[0] - c[0], d[1] - c[1]) + Math.hypot(b[0] - d[0], b[1] - d[1])) / 6));
    for (let j = i ? 1 : 0; j <= S; j++) {
      const t = j / S, u = 1 - t;
      P.push([0, 1].map((k) => u * u * u * a[k] + 3 * u * u * t * c[k] + 3 * u * t * t * d[k] + t * t * t * b[k]));
    }
  }
  return P;
}
// #243's bend through turns: each stretch bent over its height, leaving
// and arriving along the chord of the turn's neighbours.
function through(p, k0, k1) {
  const n = p.length - 1, P = [];
  const k = (i) => (i === 0 ? k0 : i === n ? k1 : (p[i + 1][0] - p[i - 1][0]) / (p[i + 1][1] - p[i - 1][1] || 1e-9));
  for (let i = 0; i < n; i++) bend(P, p[i][0], p[i][1], k(i), p[i + 1][0], p[i + 1][1], k(i + 1));
  return P;
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
// out. A fork is its maker carrying on, so like an old version's line
// its line starts under its maker's dot, the full width of a lifeline.
function ports(edges) {
  const at = new Map();
  const port = (e, v, side, far, gap) => {
    // Placed by order: the line's next point, its first hidden one or its
    // other end, so ports keep the rows' order.
    if (mine() && e.mid.length) far = side > 0 ? e.mid[0] : e.mid.at(-1);
    const s = Math.sign(far.x - v.x) || 1, key = `${v.id}:${side}`;
    const p = { e, v, far, gap, s, side };
    (at.get(key) ?? at.set(key, []).get(key)).push(p);
    return p;
  };
  for (const e of edges) {
    if (e.kind === "grow" || e.kind === "branch" || e.kind === "port") e.a = port(e, e.d, 1, e.v, e.kind === "branch" || (e.kind === "grow" && e.v.n.fork) ? TUCK : NGAP);
    if (e.kind === "fade" || e.kind === "port") e.b = port(e, e.v, -1, e.d, NGAP);
  }
  // Inner first: nearest across, then nearest down. Returns each side's
  // ports, as { h, step }.
  const across = (p) => Math.abs(p.far.x - p.v.x), down = (p) => Math.abs(p.far.y - p.v.y), sides = [];
  for (const here of at.values()) {
    const halves = [-1, 1].map((s) => here.filter((p) => p.s === s).sort((a, b) => fork(a) - fork(b) || across(a) - across(b) || down(a) - down(b)));
    let step = 2 * WIDE / (2 * Math.max(...halves.map((h) => h.length)) + 1);
    if (variant.ports === "pizza") step = Math.min(step, (variant.gap * Math.PI) / 180);
    for (const h of halves) { place(h, step); sides.push({ h, step }); }
  }
  return sides;
}
// Under pizza ports, a fork's line (its maker carrying on) goes innermost.
const fork = (p) => (variant.ports === "pizza" && p.e.kind === "grow" && p.e.v.n.fork ? 0 : 1);
// #240's ports: this far round from upright, innermost first.
const SLOTS = [36, 66, 82].map((d) => (d * Math.PI) / 180);
const slotsFor = (n) => (n <= SLOTS.length ? SLOTS : Array.from({ length: n }, (_, i) => SLOTS[0] + ((SLOTS[2] - SLOTS[0]) * i) / (n - 1)));
const steep = () => (variant.steep * Math.PI) / 180;
// Ports in order out from upright, `step` apart.
function place(h, step) {
  const fixed = variant.ports === "fixed" && slotsFor(h.length);
  h.forEach((p, i) => {
    const g = fixed ? fixed[i] : step * (i + 1);
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
// variant.taper (or all of a shorter line), so the outline too meets the lifeline
// with no change, then w; and over the last POINT eased down to nothing,
// so the end is a point.
function taper(s, L, w) {
  const t = Math.min(1, s / POINT);
  return TIP * t * (2 - t) + (w - TIP) * smooth(Math.min(1, s / Math.min(L, variant.taper)));
}
// An edge's ends, its points, and its widths if it tapers or is a tool's.
// A life's own line runs upright between its dots; a spawn bends from its
// port into its lane, and a result from its lane into its reader's port,
// over as much height as it goes across (at least TURN, at most all of
// it): turning in the middle, or, where that would pass over a node or
// cross a line already drawn, as much nearer its slant end as clears the
// most.
function route(e, onRow, drawn) {
  const { v, d } = e;
  // Without ports, a taper's thin end stops NGAP short of its dot.
  const P0 = e.a ? e.a.p : edge(d, [0, 1], e.kind === "port" || (e.kind === "grow" && !v.n.fork) ? NGAP : TUCK);
  const P1 = e.b ? e.b.p : edge(v, [0, -1], e.kind === "fade" || e.kind === "port" ? NGAP : TUCK);
  const k0 = e.a?.k ?? 0, k1 = e.b?.k ?? 0, pts = e.mid ? funnel(P0, P1, e.mid) : [P0, P1];
  let P = pts;
  if (pts.length === 2 && P0[0] === P1[0] && !k0 && !k1) P = pts;
  else if (variant.curve === "orig") P = rounded(pts, k0, k1);
  else if (pts.length > 2) P = through(pts, k0, k1);
  else {
    const x0 = Math.min(P0[0], P1[0]), x1 = Math.max(P0[0], P1[0]);
    const turn = Math.max(TURN, x1 - x0), long = variant.curve === "end" && e.kind !== "port" && P1[1] - P0[1] > turn, fade = e.kind === "fade";
    const B0 = long && fade ? [P0[0], P1[1] - turn] : P0, B1 = long && !fade ? [P1[0], P0[1] + turn] : P1;
    const between = [];
    for (let r = d.rank; r <= v.rank; r++) for (const u of onRow[r] ?? []) if (u !== v && u !== d && u.x > x0 - 12 && u.x < x1 + 12) between.push(u);
    const near = [...drawn.values()].filter((o) => o.x0 <= x1 && x0 <= o.x1 && o.P[0][1] <= P1[1] && P0[1] <= o.P.at(-1)[1]);
    let fewest = Infinity;
    for (const late of [1, 2, 4, .5, .25]) {
      const B = bend([], B0[0], B0[1], k0, B1[0], B1[1], k1, late);
      const Q = !long ? B : fade ? [P0, ...B] : [...B, P1];
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
  if (e.kind === "branch" || ((e.kind === "life" || (e.kind === "grow" && v.n.fork)) && w === LIFE)) { e.W = null; return; }
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
    if (close(a, b, [u.x, u.y], reach)) return true;
  }
  return false;
}

// ─── Crossings ─────────────────────────────────────────────────────────────
// Every place two lines of different lives cross, away from a node both
// end at: the more upright one goes over, the other under. Each as
// { over, under, at, seg, angle, forced }: `over` and `under` the indices
// of the edges, `seg` the over one's segment there, ending at P[seg],
// `angle` between the two there, from 0 to a right angle, and forced
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
      out.push({ over: o.i, under: u.i, at: p, seg: o.j, angle: angle(A, B), forced: false });
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
// apart, within (LIFE + GAP) / sin(steep) of a crossing at steep, the
// shallowest that has a gap; SPAN is twice that, room for them to bend.
function near(e, { at, seg }) {
  // A gap at every crossing still takes 15 degrees' band, not an endless one.
  const SPAN = 2 * (LIFE + GAP) / Math.sin(Math.max(steep(), Math.PI / 12));
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
