// ─── The map's layout: a layered graph, and its lines ─────────────────────
// The standard layered layout Graphviz `dot` uses, centred, flowing down,
// with no library. The present lines up on one row at y = 0: every live
// node sits on the lowest row the past reaches, and what it waits on hangs
// below it. Nothing here asks the browser for geometry: sizes come in with
// the nodes, and curves are Béziers in closed form.
//
// A node: { id, deps, kind, lo, hi, along, top, foot, now, planned }. `lo` and
// `hi` are how far it reaches left and right of its centre (its label is to
// the right), `along` how tall it is, `top` and `foot` where lines arrive
// from above and leave below. `now` is a live node, `planned` one that waits
// on what it depends on; the present row is the lowest any node but these
// and chips reaches. `life`, if known, is [when it began, when it ended or
// now], in ms. Nodes come after everything they depend on.

// Isotonic regression with minimum gaps: each node of row L as near its
// target t[i] (weighted w[i]) as it can be, in order, each at least gap[i]
// right of the one before. Into the nodes' x; scratch reused, nothing
// allocated.
let cap = 0, OFF, BS, BW, BN;
function pava(L, t, w, gap) {
  const n = L.length;
  if (n > cap) {
    cap = Math.max(64, 2 * n);
    [OFF, BS, BW] = [0, 0, 0].map(() => new Float64Array(cap));
    BN = new Int32Array(cap);
  }
  OFF[0] = 0;
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

// One project laid out: each node's centre, x about the project's centre
// line and y = 0 on the present row, and each edge's route. `prev`, a node's
// x in the last layout of this project, seeds the order within each row, so
// a change moves little.
export function layered(nodes, prev = new Map()) {
  const ids = new Set(nodes.map((n) => n.id));
  const V = nodes.map((n, i) => ({ n, id: n.id, key: i, deps: n.deps.filter((d) => ids.has(d)), up: [], down: [], lo: n.lo, hi: n.hi, along: n.along }));
  const by = new Map(V.map((v) => [v.id, v]));
  // 1. Rows: one below the lowest dependency. 2. The present: every live
  // node down to the lowest row the past reaches; a planned node one below
  // what it waits on. 3. Everything else by its time, within its slack.
  const below = (v) => { let r = -1; for (const d of v.deps) r = Math.max(r, by.get(d).rank); return r + 1; };
  for (const v of V) v.rank = below(v);
  let P = 0;
  for (const v of V) if (!v.n.planned && v.n.kind !== "chip") P = Math.max(P, v.rank);
  for (const v of V) if (v.n.now || v.n.planned) v.rank = Math.max(v.n.now ? P : 0, below(v));
  byTime(V, by, P);
  const layers = [];
  const put = (v) => (layers[v.rank] ??= []).push(v);
  V.forEach(put);
  // A long edge passes through a hidden point in each row it crosses.
  const edges = [];
  for (const v of V) {
    for (const d of v.deps) {
      const t = by.get(d), chain = [v];
      for (let r = v.rank - 1; r > t.rank; r--) {
        const x = { id: `${v.id}>${d}@${r}`, dummy: true, dep: d, rank: r, key: t.key + .5, up: [], down: [], lo: 1, hi: 1, along: 0 };
        put(x);
        chain.push(x);
      }
      chain.push(t);
      for (let i = 0; i < chain.length - 1; i++) { chain[i].up.push(chain[i + 1]); chain[i + 1].down.push(chain[i]); }
      edges.push({ from: v, to: t, chain });
    }
  }
  for (let r = 0; r < layers.length; r++) layers[r] ??= [];
  // 4. Order within each row: as it was (a new node, or a line, by where
  // what it hangs from was), then the order things began; then a few
  // barycentre sweeps, down and up.
  const seed = (v) => prev.get(v.dummy ? v.dep : v.id) ?? (v.deps?.length ? prev.get(v.deps[0]) : undefined) ?? Infinity;
  for (const L of layers) {
    for (const v of L) v.seed = seed(v);
    L.sort((a, b) => a.seed - b.seed || a.key - b.key);
    L.forEach((v, i) => (v.pos = i));
  }
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
  // 5. Across: each line a spring pulling its ends together, each row placed
  // as near its neighbours as the gaps allow (dot's objective, squared). A
  // long edge's hidden points are a much stiffer spring, so it keeps one
  // lane; every node is pulled a little to the centre line, so nothing drifts.
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
    if (e.chain.length <= 3) continue;
    const mid = e.chain.slice(1, -1);
    let lo = -Infinity, hi = Infinity, sum = 0;
    for (const u of mid) {
      const L = layers[u.rank], i = u.pos;
      if (i > 0) lo = Math.max(lo, L[i - 1].x + sep(L[i - 1], u));
      if (i < L.length - 1) hi = Math.min(hi, L[i + 1].x - sep(u, L[i + 1]));
      sum += u.x;
    }
    if (lo > hi) continue;
    const x = Math.min(hi, Math.max(lo, sum / mid.length));
    for (const u of mid) u.x = x;
  }
  // 6. Down: each row as tall as its tallest node plus 17, the present row
  // at y = 0.
  const tall = layers.map((L) => L.reduce((m, v) => Math.max(m, v.along), 0));
  const ys = [0];
  for (let r = 1; r < layers.length; r++) ys[r] = ys[r - 1] + tall[r - 1] / 2 + tall[r] / 2 + 17;
  layers.forEach((L, r) => { for (const v of L) v.y = ys[r] - ys[P]; });
  // The room each hidden point has in its row, from the nearest real node
  // on its left to the nearest on its right, less 4 px: its line passes
  // anywhere in it, drawn taut (route()).
  for (const L of layers) {
    let lo = -Infinity;
    for (const u of L) if (u.dummy) u.gap = [lo, Infinity]; else lo = u.x + u.hi + 4;
    let hi = Infinity;
    for (let i = L.length - 1; i >= 0; i--) if (L[i].dummy) L[i].gap[1] = hi; else hi = L[i].x - L[i].lo - 4;
  }
  const out = new Map();
  for (const v of V) out.set(v.id, { x: v.x, y: v.y });
  return { at: out, edges: edges.map((e) => ({ from: e.from.id, to: e.to.id, route: route(e) })) };
}

// A node has slack: from one row below what it depends on (as early as it
// can be) to one row above what depends on it, or the present row if
// nothing does (as late as it can be). The critical path, which sets how
// many rows there are, and live and planned nodes have none. Every other
// node sits as far through the rows between its neighbours as the middle of
// its life is through the time between what it depends on ending and what
// depends on it beginning (or now), on the nearest row its slack allows. A
// node with no `life` began with what it depends on. A few sweeps, as its
// neighbours move with it.
function byTime(V, by, P) {
  const fixed = (v) => v.n.now || v.n.planned;
  const kids = new Map(V.map((v) => [v.id, []]));
  for (const v of V) for (const d of v.deps) kids.get(d).push(v);
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
    [v.t0, v.t1] = v.n.life ?? [t, t];
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

// An edge's route, from the dependent's top up to its dependency's foot:
// the shortest line through the gaps its hidden points leave in the rows it
// crosses (a funnel, as in string pulling). So a long edge runs at one
// steady slope, and turns only at the edge of a gap, round a node in its way.
function route(e) {
  const v = e.from, t = e.to;
  const end = [t.x, t.y + t.n.foot];
  const gaps = e.chain.slice(1, -1).map((u) => [u.gap[0], u.gap[1], u.y]);
  gaps.push([end[0], end[0], end[1]]);
  let a = [v.x, v.y - v.n.top];
  const path = [a];
  let k = 0;
  while (k < gaps.length) {
    let lo = -Infinity, hi = Infinity, loK = -1, hiK = -1, turn = null;
    for (let j = k; j < gaps.length; j++) {
      const [l, h, y] = gaps[j], dy = a[1] - y;
      const sl = (l - a[0]) / dy, sh = (h - a[0]) / dy;
      if (sl > hi) { turn = [hiK, 1]; break; }
      if (sh < lo) { turn = [loK, 0]; break; }
      if (sl > lo) { lo = sl; loK = j; }
      if (sh < hi) { hi = sh; hiK = j; }
    }
    if (!turn) break;
    const [j, side] = turn;
    a = [gaps[j][side], gaps[j][2]];
    path.push(a);
    k = j + 1;
  }
  path.push(end);
  return path;
}

// A route smoothed: it leaves and arrives vertically and rounds each turn,
// a cubic a stretch whose handles lie along the chord of the turn's
// neighbours (Catmull-Rom), at most BEND long, so a straight stretch stays
// straight.
const BEND = 22;
export function curve(p) {
  const n = p.length - 1;
  const dir = p.map((_, i) => {
    if (i === 0 || i === n) return [0, -1];
    const x = p[i + 1][0] - p[i - 1][0], y = p[i + 1][1] - p[i - 1][1], l = Math.hypot(x, y) || 1;
    return [x / l, y / l];
  });
  const segs = [];
  for (let i = 0; i < n; i++) {
    const a = p[i], b = p[i + 1];
    const end = Math.max(0, Math.min((a[1] - b[1]) * .55, BEND));
    const mid = Math.min(Math.hypot(b[0] - a[0], b[1] - a[1]) / 3, BEND);
    const h0 = i === 0 ? end : mid, h1 = i + 1 === n ? end : mid;
    segs.push([a, [a[0] + dir[i][0] * h0, a[1] + dir[i][1] * h0], [b[0] - dir[i + 1][0] * h1, b[1] - dir[i + 1][1] * h1], b]);
  }
  return segs;
}
const f1 = (v) => (Math.round(v * 10) / 10).toString();
// A curve as a stroke's path.
export function stroke(segs) {
  let d = `M${f1(segs[0][0][0])},${f1(segs[0][0][1])}`;
  for (const [, c1, c2, b] of segs) d += `C${f1(c1[0])},${f1(c1[1])} ${f1(c2[0])},${f1(c2[1])} ${f1(b[0])},${f1(b[1])}`;
  return d;
}
// A curve as a taper: a filled outline `w0` wide at the dependent's end
// narrowing to `w1` where it depends, sampled from the Béziers. Its outline
// runs out along its left and back along its right, so every taper winds
// the same way, and many in one path fill as their union.
export function taper(segs, w0, w1) {
  const P = [];
  for (let s = 0; s < segs.length; s++) {
    const [a, c, d, b] = segs[s];
    // A sample each 6 px or so along its control polygon, 2 to 10 of them.
    const S = Math.max(2, Math.min(10, Math.ceil((Math.hypot(c[0] - a[0], c[1] - a[1]) + Math.hypot(d[0] - c[0], d[1] - c[1]) + Math.hypot(b[0] - d[0], b[1] - d[1])) / 6)));
    for (let j = s ? 1 : 0; j <= S; j++) {
      const t = j / S, u = 1 - t;
      const x = u * u * u * a[0] + 3 * u * u * t * c[0] + 3 * u * t * t * d[0] + t * t * t * b[0];
      const y = u * u * u * a[1] + 3 * u * u * t * c[1] + 3 * u * t * t * d[1] + t * t * t * b[1];
      let tx = 3 * u * u * (c[0] - a[0]) + 6 * u * t * (d[0] - c[0]) + 3 * t * t * (b[0] - d[0]);
      let ty = 3 * u * u * (c[1] - a[1]) + 6 * u * t * (d[1] - c[1]) + 3 * t * t * (b[1] - d[1]);
      const l = Math.hypot(tx, ty) || 1;
      tx /= l; ty /= l;
      P.push([x, y, -ty, tx]);
    }
  }
  const len = [0];
  for (let i = 1; i < P.length; i++) len[i] = len[i - 1] + Math.hypot(P[i][0] - P[i - 1][0], P[i][1] - P[i - 1][1]);
  const L = len[len.length - 1] || 1, left = [], right = [];
  P.forEach(([x, y, nx, ny], i) => {
    const h = (w0 + (w1 - w0) * (len[i] / L)) / 2;
    left.push(`${f1(x + nx * h)},${f1(y + ny * h)}`);
    right.push(`${f1(x - nx * h)},${f1(y - ny * h)}`);
  });
  return `M${left.join("L")}L${right.reverse().join("L")}Z`;
}
