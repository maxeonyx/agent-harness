// Harbour: a hand-tinted nautical chart, seen from above and lit by a low sun, where each agent's
// chart is a closer look at a stretch of its parent's coast.
//
// What it depicts: one invented New Zealand coast per area, charted at the scale of each agent's
// place in its area's tree. The area's first agent has the approaches: headlands, islands, banks
// and open water. Each child has a chart of a stretch of its parent's coast, about three times
// closer: a bay, then a cove, then a berth, each drawn as a real chart at its own scale, with
// finer depth lines, shallower soundings in decimetres and more detail in the coast. Siblings
// look at different stretches of the same coast. Each chart is coloured by hand: watercolour
// glazes in the shallows, pooled at their wet edges and granulating in the paper's tooth; deep
// water under the faintest wash; isobaths at a step that suits the chart's scale, heavier every fifth; a
// dotted low-water line round green drying flats; buff land modelled by the sun, warm where it
// faces it and cool where it turns away; and the land's shadow on the water. The area's own chart
// may carry a small compass rose. No saturated colour anywhere: the agent's colour is the hue of
// the glazes and the sea's wash, round the whole wheel (rose, ochre, sap green, violet, blue, teal),
// pale enough to stay a chart's tint and far enough apart to tell agents apart at a glance.
//
// Technique: one world per area, from its first agent's seed: an elevation field of simplex
// octaves through a domain warp, each octave summed without normalising, so a closer look only
// adds finer octaves and the coast stays where the parent's chart put it. A chart is a window on
// that world: the area's is the best of a few windows across the world, and a child's is aimed at
// a point on its parent's coast, picked by the child's number with a golden-ratio step along the
// coast, so siblings land far apart. Each window is chosen by scoring candidate placements on a
// coarse grid: open water under the page lead, a share of land, plenty of coast, never land on
// three sides (a tray), and a kind of coast that changes from one chart to the next (open coast,
// islands, a headland, a strait). Octaves are evaluated only as finely as their wavelength needs
// and scaled up, so a deep look costs no more than the first. Depth comes from the distance to
// land (an exact distance transform) in world units, so a cove really is shallow; the depth step,
// the tints' depths and the soundings all follow from the depths in view. Colour, relief (lit
// from the north-west) and the land's cast shadow are worked per pixel at a few CSS pixels and
// scaled up smoothly; the paper's tooth is a multiplied tile, and the granulation a colour-burned
// one. Every line is marching squares, smoothed and drawn as curves; the shallow isobaths and the
// coast are lit as Kitiro Tanaka's illuminated contours, so the terraces stand up. The lines sit
// on their own layer, erased softly round the lead.
//
// Stray text is noise, so the soundings keep well clear of the lead. The glazes, paper, relief and
// lit rings give it light and material, and the windows and their kinds keep every seed from being
// the same composition, a clear pool ringed by coast.

const FACE = '"Newsreader", Georgia, serif';

const C = {
  deep: [244, 248, 248],      // deep water: the paper
  glazeC: [0.95, 0.992, 0.962],  // a greener touch on the shallowest water, as a multiplier
  shadow: [0.84, 0.885, 0.92],   // the land's shadow on the water
  sunlit: [253, 246, 226],
  shade: [186, 176, 170],
  dry: [221, 228, 205],       // drying flats, between the tides
  land: [242, 234, 213],      // buff
  landHigh: [233, 222, 196],
  coastWash: [233, 214, 168], // the hand-coloured wash along the coast
};
const INK = {
  coast: "rgba(52, 72, 78, 0.95)",
  low: "rgba(52, 72, 78, 0.55)",
  iso: [40, 92, 108],
  isoLit: [128, 176, 190],
  topo: [128, 98, 56],
  topoLit: [200, 178, 132],
  sound: "rgba(66, 102, 114, 0.66)",
  stipple: "rgba(122, 100, 60, 0.5)",
  rose: [72, 116, 130],
};

// The agent's colour: the hue of its chart's glazes, all round the wheel from sea-green, as the tints of
// a hand-coloured chart (rose, ochre, sap green, violet, blue) were. Sea-green to slate alone, the
// first range, gave four agents that differed more in coastline than in colour.
const glazeHue = (at) => 186 + 360 * at;

export default {
  name: "harbour",
  about: "A hand-tinted chart of a harbour: depth lines, soundings and a quiet sea.",
  display: { family: "Newsreader", files: ["font-newsreader.woff2"], weights: "200 800", bold: 620, axes: '"opsz" 48' },
  palette: {
    paper: "#f4f8f8", tint: "#fbfdfd", ink: "#0f2a33", body: "#334f57", muted: "#6c8790", line: "#d6e4e8",
    accent: "#1d6a78", figure: ["#1d6a78", "#b4573e", "#7a8a3c", "#6a5a8e"], cover_ink: "#0f2a33", cover_body: "#334f57",
  },
  // The glazes' hues round the colour wheel (glazeHue), each next as far from those before as it allows.
  slots: [0, 1 / 2, 1 / 4, 3 / 4, 1 / 8, 5 / 8],
  identity,
  paint,
};

// The agent's colour for its node on the map: its glazes' hue, strong enough to read as a dot.
function identity(at) {
  return oklchHex(0.66, 0.1, glazeHue(at));
}

function paint(ctx, { w, h, dpr = 1, root, line = [], at = 0, stream, quiet, kit }) {
  const S = Math.max(w, h);
  const world = makeWorld(kit, root);
  const view = chooseView(world, kit, root, line, w, h, quiet);
  const F = buildField(world, view, w, h, S, quiet);
  const glaze = glazes(kit, at);
  fillGround(ctx, F, w, h, S, glaze);
  stippleShore(ctx, F, stream("stipple"));
  // The washes' pigment settles into the paper's tooth; then the tooth itself.
  tiles ??= makeTiles(kit.rng("harbour paper"));
  granulate(ctx, w, h, dpr);
  paperTooth(ctx, w, h, dpr);
  drawCoast(ctx, F);
  // The line-work goes on its own layer, which fades out softly round the lead.
  const layer = document.createElement("canvas");
  layer.width = Math.round(w * dpr); layer.height = Math.round(h * dpr);
  const lc = layer.getContext("2d");
  lc.setTransform(dpr, 0, 0, dpr, 0, 0);
  drawLines(lc, F, S);
  const rs = stream("rose");
  const rose = line.length === 0 && S >= 500 && rs() < 0.4 ? drawRose(lc, F, rs, S, w, h) : null;
  if (S >= 500) drawSoundings(lc, F, stream("soundings"), S, rose);
  clearRound(lc, F.q, 0.05 * S, w, h);
  ctx.save(); ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.drawImage(layer, 0, 0); ctx.restore();
}

// The glazes in the agent's hue, as multipliers of the paper: one under five steps of depth, one
// under two, and S, the faint wash over the whole sea that tells one agent's chart from another's at
// a glance.
function glazes(kit, at) {
  const hue = glazeHue(at), to = (L, Cc) => kit.rgb(oklchHex(L, Cc, hue)).map((v, i) => v / C.deep[i]);
  return { A: to(0.944, 0.032), B: to(0.905, 0.056), S: to(0.962, 0.014) };
}

// ---- The world ------------------------------------------------------------------------------
//
// World units: the area's chart spans 1 across its longer side. Octave k of the elevation has a
// frequency of 2^(k+1) per unit (k = -1 is the continents) and a fixed amplitude, so the sum at a
// point never changes when finer octaves are added for a closer look.

const SEA = 0.03;
// Each octave a little stronger than half the one above, as real coasts are rougher than smooth
// noise: so a closer look still finds a ragged coast, not the straight edge of a big slope.
const GAIN = 0.58, ROUGH = Math.log2(1 / GAIN);
const amp = (k) => (k < 0 ? 0.42 : 0.32 * GAIN ** k);
const KINDS = ["coast", "islands", "headland", "strait"];

function makeWorld(kit, root) {
  const r = kit.rng(`harbour:${root}:world`);
  return { nz: kit.noise(`harbour:${root}:world`), off: Array.from({ length: 48 }, () => [r.range(-300, 300), r.range(-300, 300)]) };
}

// The finest octave a grid with this spacing can show, and how coarsely an octave can be sampled.
const kFor = (cw, per) => Math.floor(Math.log2(1 / (per * cw)));
const stepFor = (lambda, cw) => Math.max(1, Math.min(32, Math.floor(lambda / (5 * cw))));

// Adds amp * fn(i, j) at every grid point, sampling fn only every st points and filling between
// them bilinearly: a smooth field costs a few samples, however large the grid.
function band(gw, gh, st, fn, out, a) {
  if (st <= 1) {
    for (let j = 0, k = 0; j < gh; j++) for (let i = 0; i < gw; i++, k++) out[k] += a * fn(i, j);
    return;
  }
  const cgw = Math.ceil((gw - 1) / st) + 1, cgh = Math.ceil((gh - 1) / st) + 1, c = new Float32Array(cgw * cgh);
  for (let cj = 0; cj < cgh; cj++) for (let ci = 0; ci < cgw; ci++) c[cj * cgw + ci] = fn(ci * st, cj * st);
  for (let j = 0, k = 0; j < gh; j++) {
    const fy = j / st, b = Math.min(cgh - 2, Math.floor(fy)), ty = fy - b;
    for (let i = 0; i < gw; i++, k++) {
      const fx = i / st, aa = Math.min(cgw - 2, Math.floor(fx)), tx = fx - aa, q = b * cgw + aa;
      out[k] += a * ((c[q] * (1 - tx) + c[q + 1] * tx) * (1 - ty) + (c[q + cgw] * (1 - tx) + c[q + cgw + 1] * tx) * ty);
    }
  }
}

// The world's elevation on a grid whose point (i, j) is world (X0 + i cw, Y0 + j cw), with octaves
// up to kMax, and the warp's push (QX, QY), which the depth's banks also use.
//
// Each look down the line (size s, centre c) flattens the octaves much larger than itself round
// their value at its centre (a local detrend): they would otherwise tilt the whole view, so that a
// close look always found one straight coast across it. A look flattens the field its parent drew,
// and keeps its centre's height, so the coast it zooms on is the parent's coast.
function heightGrid(Wd, X0, Y0, cw, gw, gh, kMax, looks = []) {
  const N = gw * gh, E = new Float32Array(N).fill(-SEA), QX = new Float32Array(N), QY = new Float32Array(N), nz = Wd.nz;
  const stW = stepFor(1 / 16, cw);
  band(gw, gh, stW, (i, j) => nz.fbm(2 * (X0 + i * cw) + 1.7, 2 * (Y0 + j * cw) + 9.2, 3), QX, 1);
  band(gw, gh, stW, (i, j) => nz.fbm(2 * (X0 + i * cw) + 8.3, 2 * (Y0 + j * cw) + 2.8, 3), QY, 1);
  const at = (k, x, y, qx, qy) => { const f = 2 ** k, [ox, oy] = Wd.off[k + 1]; return nz.n2(f * (2 * x + 1.3 * qx) + ox, f * (2 * y + 1.3 * qy) + oy); };
  const cq = looks.map(({ c }) => [nz.fbm(2 * c[0] + 1.7, 2 * c[1] + 9.2, 3), nz.fbm(2 * c[0] + 8.3, 2 * c[1] + 2.8, 3)]);
  for (let k = -1; k <= kMax; k++) {
    // The octave as the line's looks leave it: base + a * itself. A look keeps all of an octave up
    // to 1.5 looks across, and FLAT of it from 4 across.
    let a = 1, base = 0;
    looks.forEach(({ c, s }, n) => {
      const keep = 1 - (1 - FLAT) * smooth(Math.log2(1.5), 2, Math.log2(1 / (2 ** (k + 1) * s)));
      if (keep < 1) { base = (1 - keep) * (a * at(k, c[0], c[1], cq[n][0], cq[n][1]) + base) + keep * base; a *= keep; }
    });
    band(gw, gh, stepFor(1 / (4 * 2 ** k), cw), (i, j) => {
      const q = Math.min(gh - 1, j) * gw + Math.min(gw - 1, i);
      return base + a * at(k, X0 + i * cw, Y0 + j * cw, QX[q], QY[q]);
    }, E, amp(k));
  }
  const sea = looks.at(-1)?.sea ?? 0;
  if (sea) for (let k = 0; k < N; k++) E[k] -= sea;
  return { E, QX, QY };
}
const FLAT = 0.3;

// ---- Choosing the window ---------------------------------------------------------------------

// A coarse grid round a centre, 1.8 windows wide, for scoring placements of a window of size s.
const UN = 30;
function unionGrid(Wd, cx, cy, s, looks) {
  const cw = (1.8 * s) / (UN - 1), X0 = cx - 0.9 * s, Y0 = cy - 0.9 * s;
  const { E } = heightGrid(Wd, X0, Y0, cw, UN, UN, kFor(cw, 4), looks);
  // Flattening can leave a look nearly all land or all sea; then its sea level moves, just enough
  // to bring the land to between a fifth and a half of it, for it and every look below it.
  const look = looks.at(-1);
  if (look) {
    const sorted = Float32Array.from(E).sort(), land = sorted.filter((v) => v > 0).length / sorted.length;
    const q = land < 0.22 ? sorted[Math.floor(sorted.length * 0.78)] : land > 0.5 ? sorted[Math.floor(sorted.length * 0.5)] : 0;
    if (q) { look.sea += q; for (let k = 0; k < E.length; k++) E[k] -= q; }
  }
  const L = new Uint8Array(UN * UN);
  for (let k = 0; k < L.length; k++) L[k] = E[k] > 0 ? 1 : 0;
  return { L, gw: UN, gh: UN, cw, X0, Y0, s };
}

// How good a window of nx by ny grid cells at (ox, oy) is: open water under the lead's box qb (in
// the window's cells), a fair share of land, plenty of coast, no tray, the wanted kind of coast,
// and the target (if any) inside, not at the very edge.
function scoreWindow(U, ox, oy, nx, ny, qb, kind, target) {
  const { L, gw } = U, at = (i, j) => L[(oy + j) * gw + ox + i];
  let land = 0, total = 0, qland = 0, qn = 0, coast = 0;
  const edge = [0, 0, 0, 0]; // top, right, bottom, left
  for (let j = 0; j < ny; j++) for (let i = 0; i < nx; i++) {
    const v = at(i, j);
    if (i >= qb.x0 - 1 && i <= qb.x1 + 1 && j >= qb.y0 - 1 && j <= qb.y1 + 1) { qn++; qland += v; } else { total++; land += v; }
    if (i < nx - 1 && v !== at(i + 1, j)) coast++;
    if (j < ny - 1 && v !== at(i, j + 1)) coast++;
    if (j === 0) edge[0] += v / nx;
    if (i === nx - 1) edge[1] += v / ny;
    if (j === ny - 1) edge[2] += v / nx;
    if (i === 0) edge[3] += v / ny;
  }
  const f = land / Math.max(1, total);
  let s = -14 * (qland / Math.max(1, qn));
  s -= 5 * Math.max(0, 0.2 - f) + 5 * Math.max(0, f - 0.55);
  s += 1.2 * Math.min(1, coast / (1.5 * (nx + ny)));
  const touched = edge.filter((e) => e > 0.3).length;
  if (touched >= 3) s -= 8; // a tray: the sea a pool framed by land
  s += 3 * kindOf(at, nx, ny, edge, kind);
  if (target) {
    const tx = (target[0] - U.X0) / U.cw - ox, ty = (target[1] - U.Y0) / U.cw - oy;
    const d = Math.max(Math.abs(tx - nx / 2) / (nx / 2), Math.abs(ty - ny / 2) / (ny / 2));
    s -= 3 * Math.max(0, d - 0.75);
  }
  return s;
}

// How well a window shows a kind of coast, 0 to 1.
const FLOOD = { seen: new Uint8Array(0), stack: new Int32Array(0) };
function kindOf(at, nx, ny, edge, kind) {
  const [t, r, b, l] = edge;
  if (kind === "strait") return Math.min(1, Math.max(Math.min(t, b) * (1 - Math.max(l, r)), Math.min(l, r) * (1 - Math.max(t, b))) * 3);
  // Land that touches the window's edges, found by flooding in from them; the rest is islands.
  const n = nx * ny;
  if (FLOOD.seen.length < n) FLOOD.seen = new Uint8Array(n), FLOOD.stack = new Int32Array(4 * n + 8);
  const seen = FLOOD.seen.fill(0, 0, n), stack = FLOOD.stack;
  let top = 0;
  const push = (i, j) => { const k = j * nx + i; if (!seen[k] && at(i, j)) { seen[k] = 1; stack[top++] = k; } };
  for (let i = 0; i < nx; i++) { push(i, 0); push(i, ny - 1); }
  for (let j = 0; j < ny; j++) { push(0, j); push(nx - 1, j); }
  let reach = 0, joined = 0, all = 0;
  const half = Math.min(nx, ny) / 2;
  while (top) {
    const k = stack[--top], i = k % nx, j = (k - i) / nx;
    joined++;
    reach = Math.max(reach, Math.min(i, nx - 1 - i, j, ny - 1 - j) / half);
    if (i > 0) push(i - 1, j);
    if (i < nx - 1) push(i + 1, j);
    if (j > 0) push(i, j - 1);
    if (j < ny - 1) push(i, j + 1);
  }
  for (let j = 0; j < ny; j++) for (let i = 0; i < nx; i++) all += at(i, j);
  const islands = all ? (all - joined) / all : 0, touched = edge.filter((e) => e > 0.15).length;
  if (kind === "islands") return Math.min(1, islands * 2.2) * (all > 0.08 * nx * ny ? 1 : 0.4);
  if (kind === "headland") return touched === 1 || touched === 2 ? Math.min(1, reach * 1.6) * (1 - islands) : 0;
  // An open coast: land along one side, sea along the other, nothing reaching far out.
  const one = Math.max(t * (1 - b), b * (1 - t), l * (1 - r), r * (1 - l));
  return touched <= 2 ? one * (1 - Math.max(0, reach - 0.45) * 2) * (1 - islands) : 0;
}

// The best placement of a window of nx by ny cells on the coarse grid.
function bestWindow(U, nx, ny, qb, kind, target) {
  let best = null;
  for (let oy = 0; oy <= U.gh - ny; oy++) for (let ox = 0; ox <= U.gw - nx; ox++) {
    const s = scoreWindow(U, ox, oy, nx, ny, qb, kind, target);
    if (!best || s > best.s) best = { s, ox, oy };
  }
  return { ...best, X: U.X0 + best.ox * U.cw, Y: U.Y0 + best.oy * U.cw, nx, ny };
}

// The point of the coast on a coarse grid nearest a target: a closer look adds finer octaves,
// which move the coast a little from where the coarser chart drew it.
function snap(U, target) {
  const { L, gw, gh, cw } = U;
  let best = null, bd = Infinity;
  for (let j = 0; j < gh - 1; j++) for (let i = 0; i < gw - 1; i++) {
    const k = j * gw + i;
    if (L[k] === L[k + 1] && L[k] === L[k + gw]) continue;
    const x = U.X0 + (i + 0.5) * cw, y = U.Y0 + (j + 0.5) * cw, d = Math.hypot(x - target[0], y - target[1]);
    if (d < bd) { bd = d; best = [x, y]; }
  }
  return best ?? target;
}

// Points on the coast inside a window, as world positions.
function coastPoints(U, win) {
  const { L, gw, cw } = U, pts = [];
  for (let j = 1; j < win.ny - 2; j++) for (let i = 1; i < win.nx - 2; i++) {
    const k = (win.oy + j) * gw + win.ox + i;
    if (L[k] !== L[k + 1] || L[k] !== L[k + gw]) pts.push([U.X0 + (win.ox + i + 0.5) * cw, U.Y0 + (win.oy + j + 0.5) * cw]);
  }
  return pts;
}

// The window this agent's chart shows: the area's chart is the best of a few placements across the
// world; each step down the line zooms about three times closer on a point of the coast, the child's
// number stepping it round the coast by the golden ratio. Every chart up the line is placed as a
// square with the lead in its upper middle, so it doesn't depend on the device; only the last is
// fitted to this screen and its lead.
function chooseView(Wd, kit, root, line, w, h, quiet) {
  const S = Math.max(w, h), r = kit.rng(`harbour:${root}:view`), base = kit.hash(root);
  const CQ = { x0: 3, y0: 3, x1: 12, y1: 7 }; // a square window's lead, in its 16 cells
  // The area's chart: the best of a spread of places in the world.
  let best = null;
  for (let c = 0; c < 40; c++) {
    const cx = r.range(-60, 60), cy = r.range(-60, 60), cw = 1 / 15;
    const { E } = heightGrid(Wd, cx - 0.5, cy - 0.5, cw, 16, 16, kFor(cw, 4));
    const U = { L: Uint8Array.from(E, (v) => (v > 0 ? 1 : 0)), gw: 16, gh: 16, cw, X0: cx - 0.5, Y0: cy - 0.5 };
    const s = scoreWindow(U, 0, 0, 16, 16, CQ, KINDS[base % 4], null);
    if (!best || s > best.s) best = { s, cx, cy };
  }
  let cx = best.cx, cy = best.cy, s = 1, target = null;
  const looks = [];
  for (let d = 0; d < line.length; d++) {
    const U = unionGrid(Wd, cx, cy, s, looks), n = Math.round(s / U.cw);
    if (target) target = snap(U, target);
    const win = bestWindow(U, n, n, CQ, KINDS[(base + 3 * d + (line[d - 1] ?? 0)) % 4], target);
    const pts = coastPoints(U, win), mx = win.X + (n / 2) * U.cw, my = win.Y + (n / 2) * U.cw;
    const rr = kit.rng(`harbour:${root}:${line.slice(0, d).join(".")}`);
    if (pts.length) {
      pts.sort((a, b) => Math.atan2(a[1] - my, a[0] - mx) - Math.atan2(b[1] - my, b[0] - mx));
      target = pts[Math.floor((((line[d] + 1) * 0.6180339887498949 + rr()) % 1) * pts.length)];
    } else target = [mx, my];
    [cx, cy] = target;
    s /= rr.range(2.6, 3.8);
    looks.push({ c: target, s, sea: looks.at(-1)?.sea ?? 0 });
  }
  // This screen's window: the same view of the world, panned so the lead sits on open water.
  const U = unionGrid(Wd, cx, cy, s, looks), per = s / U.cw;
  if (target) target = snap(U, target);
  const nx = Math.max(4, Math.round((per * w) / S)), ny = Math.max(4, Math.round((per * h) / S));
  const q = quiet ?? { x: w * 0.25, y: h * 0.2, w: w * 0.5, h: h * 0.3 }, px = S / per;
  const qb = { x0: Math.floor(q.x / px), y0: Math.floor(q.y / px), x1: Math.floor((q.x + q.w) / px), y1: Math.floor((q.y + q.h) / px) };
  const d = line.length, kind = KINDS[(base + 3 * d + (line[d - 1] ?? 0)) % 4];
  const win = bestWindow(U, nx, ny, qb, kind, target);
  return { X0: win.X, Y0: win.Y, s, depth: d, looks };
}

// ---- The chart's fields -----------------------------------------------------------------------

const smooth = (a, b, x) => { const t = Math.min(1, Math.max(0, (x - a) / (b - a))); return t * t * (3 - 2 * t); };
function boxDist(x, y, b) {
  const dx = Math.max(b.x - x, 0, x - (b.x + b.w)), dy = Math.max(b.y - y, 0, y - (b.y + b.h));
  return Math.hypot(dx, dy);
}

// e: elevation, in units of the relief a window of this size sees, so every chart's land has the
// same relief (> 0 is land). m: depth of water in metres (land is negative). The grid runs a few cells past the screen,
// so the distance to land counts land just out of view. delta: the depth step of the isobaths.
function buildField(Wd, view, w, h, S, quiet) {
  const cell = Math.max(7, Math.round(S / 170)), mg = 6;
  const gw = Math.ceil(w / cell) + 2 + 2 * mg, gh = Math.ceil(h / cell) + 2 + 2 * mg, N = gw * gh, ox = -mg * cell;
  const s = view.s, wpp = s / S, cw = cell * wpp; // world units per CSS px, per cell
  const X0 = view.X0 + ox * wpp, Y0 = view.Y0 + ox * wpp;
  // The lead's box; a thumbnail is nearly all lead, so there it is only a preference.
  quiet ??= { x: w * 0.25, y: h * 0.2, w: w * 0.5, h: h * 0.3 };
  const qa = (quiet.w * quiet.h) / (w * h), qk = qa > 0.5 ? Math.max(0.3, 1 - (qa - 0.5) * 2) : 1;
  const q = { x: quiet.x + quiet.w * (1 - qk) / 2, y: quiet.y + quiet.h * (1 - qk) / 2, w: quiet.w * qk, h: quiet.h * qk };
  const { E, QX, QY } = heightGrid(Wd, X0, Y0, cw, gw, gh, kFor(cw, 2.5), view.looks);
  const e = new Float32Array(N);
  const rel = s ** ROUGH; // the relief a window of this size sees
  for (let k = 0; k < N; k++) e[k] = E[k] / rel;
  // Ridges and gullies inland, at the chart's own scale, so the hills have shape for the sun; the
  // coast stays put. And the seabed's own unevenness.
  const R = new Float32Array(N), B = new Float32Array(N), nz = Wd.nz, kv = Math.round(Math.log2(3 / s));
  for (const [k, a] of [[kv, 1], [kv + 1, 0.45]]) {
    const f = 2 ** k, [o1, o2] = Wd.off[40 + (k & 3)];
    band(gw, gh, stepFor(1 / (4 * f), cw), (i, j) => { const v = 1 - Math.abs(nz.n2(f * 2 * (X0 + i * cw) + o1, f * 2 * (Y0 + j * cw) + o2)); return v * v; }, R, a);
  }
  const fb = 2 / s;
  band(gw, gh, stepFor(1 / (4 * fb), cw), (i, j) => nz.n2(fb * (X0 + i * cw) + 71, fb * (Y0 + j * cw) - 33), B, 1);
  for (let k = 0; k < N; k++) if (e[k] > 0) e[k] += smooth(0, 0.3, e[k]) * 0.04 * R[k];
  // The window keeps the coast's big shapes clear of the lead; the finer octaves can still raise an
  // island under it, so the sea floor sinks there, and land right at the box sinks until none is left
  // inside it (on a thumbnail, where the box is only a preference, it sinks only a little).
  const reachS = 0.12 * S, cellAt = (v) => Math.floor((v - ox) / cell);
  for (let j = Math.max(0, cellAt(q.y - reachS)); j < Math.min(gh, cellAt(q.y + q.h + reachS) + 1); j++) {
    for (let i = Math.max(0, cellAt(q.x - reachS)); i < Math.min(gw, cellAt(q.x + q.w + reachS) + 1); i++) {
      const d = boxDist(ox + (i + 0.5) * cell, ox + (j + 0.5) * cell, q);
      if (d < reachS) { const k = j * gw + i; e[k] -= 0.3 * (1 - smooth(0, reachS, d)) + (qk < 1 ? 0 : Math.max(0, e[k]) * (1 - smooth(0, 0.05 * S, d))); }
    }
  }
  dropSpecks(e, gw, gh, (0.0004 * S * S) / (cell * cell));
  // Depth from the distance to land, in world units, spread by banks.
  // Each closer look is shallower water, sounded more finely: about 40 m off the area's coast, 4 m
  // in a berth. The shelf falls away over the same share of the view at every scale.
  const deepest = 40 * s ** 0.45;
  const dist = edt(e, gw, gh, cell), m = new Float32Array(N), wash = new Float32Array(N), wet = [];
  // How much land lies round each point: a big coast has a broad shelf, a lone islet only a few
  // close rings, not a target of them.
  const dens = Float32Array.from(e, (v) => (v > 0 ? 1 : 0)), rad = Math.max(2, Math.round((0.05 * S) / cell));
  boxBlur(dens, gw, gh, rad); boxBlur(dens, gw, gh, rad);
  for (let k = 0; k < N; k++) {
    wash[k] = QX[k] - QY[k] + 0.4 * B[k];
    if (e[k] > 0) { m[k] = -e[k] * 40; continue; }
    const dw = dist[k] * wpp, spread = Math.exp(0.8 * QX[k]) * Math.min(1.3, 0.3 + 1.8 * dens[k]);
    m[k] = deepest * (1 - Math.exp(-dw / (0.075 * s * spread))) * (1 + 0.22 * B[k]);
  }
  blur(m, gw, gh); blur(m, gw, gh);
  // The depth step suits the depths in view: about seven steps to the usual depth.
  for (let j = mg; j < gh - mg; j += 2) for (let i = mg; i < gw - mg; i += 2) { const v = m[j * gw + i]; if (v > 0) wet.push(v); }
  wet.sort((a, b) => a - b);
  const typical = wet.length ? wet[Math.floor(wet.length * 0.75)] : 10;
  const delta = [2, 1, 0.5, 0.2, 0.1].find((d) => d <= typical / 7) ?? 0.1;
  for (let k = 0; k < N; k++) if (e[k] <= 0) m[k] = Math.max(m[k], 0.2 * delta);
  return { e, m, wash, gw, gh, cell, ox, w, h, q, delta, depth: view.depth };
}

// Sinks islands smaller than minCells grid cells: too small to draw, they would still ring the sea
// round them with depth lines, an eye with nothing in it.
function dropSpecks(E, gw, gh, minCells) {
  const N = gw * gh, seen = new Uint8Array(N), comp = new Int32Array(N);
  for (let s0 = 0; s0 < N; s0++) {
    if (seen[s0] || E[s0] <= 0) continue;
    // The component goes into comp, which doubles as the flood's queue.
    let head = 0, tail = 0, edge = false;
    comp[tail++] = s0; seen[s0] = 1;
    while (head < tail) {
      const k = comp[head++], i = k % gw, j = (k - i) / gw;
      if (i === 0 || j === 0 || i === gw - 1 || j === gh - 1) edge = true;
      if (i > 0 && !seen[k - 1] && E[k - 1] > 0) { seen[k - 1] = 1; comp[tail++] = k - 1; }
      if (i < gw - 1 && !seen[k + 1] && E[k + 1] > 0) { seen[k + 1] = 1; comp[tail++] = k + 1; }
      if (j > 0 && !seen[k - gw] && E[k - gw] > 0) { seen[k - gw] = 1; comp[tail++] = k - gw; }
      if (j < gh - 1 && !seen[k + gw] && E[k + gw] > 0) { seen[k + gw] = 1; comp[tail++] = k + gw; }
    }
    if (!edge && tail < minCells) for (let n = 0; n < tail; n++) E[comp[n]] = -0.001;
  }
}

// Euclidean distance (px) from every cell to the nearest land cell (Felzenszwalb and Huttenlocher).
function edt(e, gw, gh, cell) {
  const INF = 1e12, f = new Float64Array(gw * gh);
  for (let k = 0; k < f.length; k++) f[k] = e[k] > 0 ? 0 : INF;
  const n = Math.max(gw, gh), v = new Int32Array(n), z = new Float64Array(n + 1), g = new Float64Array(n), out = new Float64Array(n);
  const pass = (len, base, stride) => {
    for (let q = 0; q < len; q++) g[q] = f[base + q * stride];
    let k = 0; v[0] = 0; z[0] = -INF; z[1] = INF;
    for (let q = 1; q < len; q++) {
      let s;
      while ((s = (g[q] + q * q - (g[v[k]] + v[k] * v[k])) / (2 * q - 2 * v[k])) <= z[k]) k--;
      k++; v[k] = q; z[k] = s; z[k + 1] = INF;
    }
    k = 0;
    for (let q = 0; q < len; q++) { while (z[k + 1] < q) k++; out[q] = (q - v[k]) ** 2 + g[v[k]]; }
    for (let q = 0; q < len; q++) f[base + q * stride] = out[q];
  };
  for (let i = 0; i < gw; i++) pass(gh, i, gw);
  for (let j = 0; j < gh; j++) pass(gw, j * gw, 1);
  const d = new Float32Array(gw * gh);
  for (let k = 0; k < d.length; k++) d[k] = Math.min(1e6, Math.sqrt(f[k])) * cell;
  return d;
}

// One pass of a small blur, over the water only unless `all`, so the coast stays where it is.
// A box blur of radius r cells, along rows then columns, by running sums.
function boxBlur(a, gw, gh, r) {
  const t = new Float32Array(Math.max(gw, gh)), d = 2 * r + 1;
  // One line of len values starting at base, stride apart, edges held.
  const pass = (len, base, stride) => {
    let sum = 0;
    for (let i = -r; i <= r; i++) sum += a[base + Math.min(len - 1, Math.max(0, i)) * stride];
    for (let i = 0; i < len; i++) {
      t[i] = sum / d;
      sum += a[base + Math.min(len - 1, i + r + 1) * stride] - a[base + Math.max(0, i - r) * stride];
    }
    for (let i = 0; i < len; i++) a[base + i * stride] = t[i];
  };
  for (let j = 0; j < gh; j++) pass(gw, j * gw, 1);
  for (let i = 0; i < gw; i++) pass(gh, i, gw);
}

function blur(a, gw, gh, all = false) {
  const b = Float32Array.from(a);
  for (let j = 1; j < gh - 1; j++) for (let i = 1; i < gw - 1; i++) {
    const k = j * gw + i;
    if (!all && b[k] <= 0) continue;
    a[k] = (b[k] * 4 + b[k - 1] + b[k + 1] + b[k - gw] + b[k + gw]) / 8;
  }
}

// The grid value nearest a point, and the bilinear sample at a point (CSS px).
function near(F, x, y, e) {
  const i = Math.min(F.gw - 1, Math.max(0, Math.round((x - F.ox) / F.cell))), j = Math.min(F.gh - 1, Math.max(0, Math.round((y - F.ox) / F.cell)));
  return e[j * F.gw + i];
}
function sample(F, x, y, e = F.e) {
  const fx = (x - F.ox) / F.cell, fy = (y - F.ox) / F.cell;
  const i = Math.min(F.gw - 2, Math.max(0, Math.floor(fx))), j = Math.min(F.gh - 2, Math.max(0, Math.floor(fy)));
  const tx = fx - i, ty = fy - j, k = j * F.gw + i;
  return (e[k] * (1 - tx) + e[k + 1] * tx) * (1 - ty) + (e[k + F.gw] * (1 - tx) + e[k + F.gw + 1] * tx) * ty;
}

// ---- The ground: washes, relief, light ---------------------------------------------------

const DRY = 0.03; // the coast proper; between it and 0, drying flats
// Towards the sun, in screen coordinates: a low sun in the north-west, as relief maps are lit.
const SUN = (() => { const x = -1, y = -1.2, n = Math.hypot(x, y); return [x / n, y / n]; })();
const ALT = 0.62; // the sun's height, radians

// Paints the tints, washes, relief and light at fill resolution, scaled up smoothly.
function fillGround(ctx, F, w, h, S, glaze) {
  const { gw, gh, delta: D } = F, N = gw * gh;
  // A smoothed copy of the elevation, for the relief and the land's shadow on the water.
  const eb = Float32Array.from(F.e);
  blur(eb, gw, gh, true); blur(eb, gw, gh, true);
  // Relief: how far each cell of land or seabed turns towards the sun or away from it.
  const relief = new Float32Array(N), lx = Math.cos(ALT) * SUN[0], ly = Math.cos(ALT) * SUN[1], kb = 0.1 / D;
  for (let j = 1; j < gh - 1; j++) for (let i = 1; i < gw - 1; i++) {
    const k = j * gw + i;
    let nx, ny;
    if (F.e[k] > 0) { nx = -(eb[k + 1] - eb[k - 1]) * 48; ny = -(eb[k + gw] - eb[k - gw]) * 48; }
    else { nx = (F.m[k + 1] - F.m[k - 1]) * kb; ny = (F.m[k + gw] - F.m[k - gw]) * kb; }
    // Only the turn towards or away from the sun, so bumpy ground is no darker on average than flat.
    relief[k] = (nx * lx + ny * ly) / Math.sqrt(nx * nx + ny * ny + 1);
  }
  const fs = Math.max(2.8, S / 400), fw = Math.ceil(w / fs), fh = Math.ceil(h / fs);
  const cv = document.createElement("canvas");
  cv.width = fw; cv.height = fh;
  const c = cv.getContext("2d"), img = c.createImageData(fw, fh), d = img.data;
  const off = 0.011 * S, sx = SUN[0] * off, sy = SUN[1] * off, gA = glaze.A, gB = glaze.B, gS = glaze.S;
  for (let py = 0; py < fh; py++) {
    for (let px = 0; px < fw; px++) {
      const x = (px + 0.5) * fs, y = (py + 0.5) * fs, ev = sample(F, x, y), jn = sample(F, x, y, F.wash);
      let r, g, b;
      if (ev > DRY) {
        // Land: buff, hand-washed ochre along the coast, modelled by the sun: warm where it faces
        // it, cool grey-violet where it turns away, and lighter as it rises.
        const hi = Math.min(1, (ev - DRY) * 2.2), wash = Math.exp(-(ev - DRY) * 26) * 0.75;
        r = C.land[0] + (C.landHigh[0] - C.land[0]) * hi; g = C.land[1] + (C.landHigh[1] - C.land[1]) * hi; b = C.land[2] + (C.landHigh[2] - C.land[2]) * hi;
        r += (C.coastWash[0] - r) * wash; g += (C.coastWash[1] - g) * wash; b += (C.coastWash[2] - b) * wash;
        // Lowland along the coast lies nearly flat; the hills inland take the light.
        const rl = sample(F, x, y, relief) * 2 * (0.25 + 0.75 * smooth(0.02, 0.22, ev - DRY));
        const t = rl > 0 ? Math.min(1, rl) * 0.38 : Math.min(1, -rl) * 0.4, to = rl > 0 ? C.sunlit : C.shade;
        r += (to[0] - r) * t; g += (to[1] - g) * t; b += (to[2] - b) * t;
        const mot = 1 + 0.018 * jn;
        r *= mot; g *= mot;
      } else if (ev > 0) {
        r = C.dry[0]; g = C.dry[1]; b = C.dry[2];
      } else {
        // Water: glazes of watercolour over the paper, one under five steps of depth and one under
        // two, each edge wandering a little off its isobath with its pigment pooled there, and a
        // greener touch on the shallowest water. Deep water keeps only the sea's faint wash.
        const m = sample(F, x, y, F.m) / D, base = 1 + 0.008 * jn;
        if (m > 8) { r = C.deep[0] * base * gS[0]; g = C.deep[1] * base * gS[1]; b = C.deep[2] * base * gS[2]; }
        else {
          const dA = 1 - smooth(4.5, 5.3, m + 0.55 * jn), pA = smooth(3.4, 4.9, m + 0.55 * jn) * dA;
          const dB = 1 - smooth(1.7, 2.3, m + 0.4 * jn), pB = smooth(1.1, 2.1, m + 0.4 * jn) * dB;
          const dC = 1 - smooth(0.25, 0.9, m + 0.2 * jn);
          const kA = dA * (0.8 + 0.6 * pA), kB = dB * (0.8 + 0.7 * pB), kC = dC * 0.7;
          r = C.deep[0] * base * gS[0] * (1 + (gA[0] - 1) * kA) * (1 + (gB[0] - 1) * kB) * (1 + (C.glazeC[0] - 1) * kC);
          g = C.deep[1] * base * gS[1] * (1 + (gA[1] - 1) * kA) * (1 + (gB[1] - 1) * kB) * (1 + (C.glazeC[1] - 1) * kC);
          b = C.deep[2] * base * gS[2] * (1 + (gA[2] - 1) * kA) * (1 + (gB[2] - 1) * kB) * (1 + (C.glazeC[2] - 1) * kC);
          // The seabed's slopes, faintly, under the glazes.
          const rl = 1 + Math.max(-0.05, Math.min(0.05, sample(F, x, y, relief) * 0.5)) * (1 - smooth(3, 8, m));
          r *= rl; g *= rl; b *= rl;
          // The land, standing a little proud of the sea, shades the water on its far side from the sun.
          const sh = smooth(DRY - 0.03, DRY + 0.06, sample(F, x + sx, y + sy, eb)) * 0.85;
          r *= 1 + (C.shadow[0] - 1) * sh; g *= 1 + (C.shadow[1] - 1) * sh; b *= 1 + (C.shadow[2] - 1) * sh;
        }
      }
      // The sheet in low light from the north-west: the far corner a little cooler and darker.
      const lt = smooth(0.2, 1.45, Math.hypot(x + 0.15 * w, y + 0.2 * h) / S);
      r *= 1.012 - 0.075 * lt; g *= 1.004 - 0.05 * lt; b *= 0.994 - 0.028 * lt;
      const o = (py * fw + px) * 4;
      d[o] = r; d[o + 1] = g; d[o + 2] = b; d[o + 3] = 255;
    }
  }
  c.putImageData(img, 0, 0);
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(cv, 0, 0, w, h);
}

// A tile of device pixels drawn by fn(x, y) -> [r, g, b, a], repeated over a whole canvas.
function tile(size, fn) {
  const t = document.createElement("canvas");
  t.width = t.height = size;
  const c = t.getContext("2d"), img = c.createImageData(size, size), d = img.data;
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
    const [r, g, b, a] = fn(x, y), o = (y * size + x) * 4;
    d[o] = r; d[o + 1] = g; d[o + 2] = b; d[o + 3] = a;
  }
  c.putImageData(img, 0, 0);
  return t;
}

// Clumped noise for a tile: white noise averaged over 2x2, wrapping at the edges.
function clumps(r, size) {
  const a = new Float32Array(size * size), b = new Float32Array(size * size);
  for (let k = 0; k < a.length; k++) a[k] = r();
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
    const x1 = (x + 1) % size, y1 = ((y + 1) % size) * size;
    b[y * size + x] = (a[y * size + x] + a[y * size + x1] + a[y1 + x] + a[y1 + x1]) / 4;
  }
  return b;
}

// The paper's tooth and the washes' granules are the same for every agent: made once per page.
let tiles = null;
function makeTiles(r) {
  const gs = 96, gn = clumps(r.fork("granules"), gs);
  const granules = tile(gs, (x, y) => { const v = Math.max(0, Math.min(1, (gn[y * gs + x] - 0.6) * 9)); return [255 - 26 * v, 255 - 16 * v, 255 - 11 * v, 255]; });
  const ts = 128, tn = clumps(r.fork("tooth"), ts), rf = r.fork("fibres"), fib = new Float32Array(ts * ts);
  for (let f = 0; f < 6; f++) {
    let x = rf() * ts, y = rf() * ts, a = rf() * 6.2832;
    const len = 10 + rf() * 30, dark = 0.35 + rf() * 0.5;
    for (let s = 0; s < len; s++) {
      a += (rf() - 0.5) * 0.3; x += Math.cos(a); y += Math.sin(a);
      const k = (((y | 0) % ts + ts) % ts) * ts + (((x | 0) % ts + ts) % ts);
      fib[k] = Math.max(fib[k], dark);
    }
  }
  const tooth = tile(ts, (x, y) => { const k = y * ts + x, v = 255 - Math.max(0, (0.5 - tn[k]) * 40) - fib[k] * 10; return [v, v, v + 1, 255]; });
  return { granules, tooth };
}

// Pigment granulating in the tooth of the paper: a colour-burn of clumped specks darkens a wash
// in proportion to its pigment and leaves bare paper all but untouched, so no mask is needed.
function granulate(ctx, w, h, dpr) {
  ctx.save();
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.globalCompositeOperation = "color-burn";
  ctx.fillStyle = ctx.createPattern(tiles.granules, "repeat");
  ctx.fillRect(0, 0, Math.round(w * dpr), Math.round(h * dpr));
  ctx.restore();
}

// The paper itself: a fine tooth and a few fibres, multiplied over everything.
function paperTooth(ctx, w, h, dpr) {
  ctx.save();
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.globalCompositeOperation = "multiply";
  ctx.globalAlpha = 0.55;
  ctx.fillStyle = ctx.createPattern(tiles.tooth, "repeat");
  ctx.fillRect(0, 0, Math.round(w * dpr), Math.round(h * dpr));
  ctx.restore();
}

// ---- Lines --------------------------------------------------------------------------------

// Marching squares at one level, joined into polylines in CSS px, each smoothed once by corner
// cutting. Edge ids: 2k is the edge from grid point k to its right-hand neighbour, 2k + 1 the
// edge to the point below. Only the cells listed in `cells` are looked at (see `crossing`).
function contour(F, level, e, seen, cells) {
  const { gw, cell, ox } = F, A = seen.a, B = seen.b, G = seen.g, gen = ++seen.gen, segs = [];
  const link = (edge, s) => { if (G[edge] !== gen) { G[edge] = gen; A[edge] = s; B[edge] = -1; } else B[edge] = s; };
  const add = (p, q) => { const s = segs.length / 2; segs.push(p, q); link(p, s); link(q, s); };
  for (let n = 0; n < cells.length; n++) {
    const k = cells[n];
    const a = e[k] > level, b = e[k + 1] > level, c = e[k + gw + 1] > level, d = e[k + gw] > level;
    if (a === b && b === c && c === d) continue;
    const cs = (a ? 8 : 0) | (b ? 4 : 0) | (c ? 2 : 0) | (d ? 1 : 0);
    const top = 2 * k, bottom = 2 * (k + gw), left = 2 * k + 1, right = 2 * (k + 1) + 1;
    switch (cs) {
      case 1: case 14: add(left, bottom); break;
      case 2: case 13: add(bottom, right); break;
      case 3: case 12: add(left, right); break;
      case 4: case 11: add(top, right); break;
      case 6: case 9: add(top, bottom); break;
      case 7: case 8: add(left, top); break;
      default: {
        const mid = (e[k] + e[k + 1] + e[k + gw] + e[k + gw + 1]) / 4 > level;
        if ((cs === 5) === mid) { add(left, top); add(bottom, right); } else { add(left, bottom); add(top, right); }
      }
    }
  }
  const point = (edge) => {
    const vert = edge & 1, idx = edge >> 1, i = idx % gw, j = (idx / gw) | 0;
    const a = e[idx], b = vert ? e[idx + gw] : e[idx + 1], t = (level - a) / (b - a);
    return vert ? [i * cell + ox, (j + t) * cell + ox] : [(i + t) * cell + ox, j * cell + ox];
  };
  const n = segs.length / 2, used = new Uint8Array(n), lines = [];
  const other = (edge, s) => (A[edge] === s ? B[edge] : A[edge]);
  for (let s0 = 0; s0 < n; s0++) {
    if (used[s0]) continue;
    used[s0] = 1;
    const fwd = [segs[2 * s0], segs[2 * s0 + 1]], back = [];
    for (const [list, start] of [[fwd, 1], [back, 0]]) {
      let edge = segs[2 * s0 + start], s = s0;
      for (;;) {
        const t = other(edge, s);
        if (t < 0 || used[t]) break;
        used[t] = 1;
        edge = segs[2 * t] === edge ? segs[2 * t + 1] : segs[2 * t];
        list.push(edge); s = t;
      }
    }
    lines.push(cut(back.reverse().concat(fwd).map(point)));
  }
  return lines;
}

// One pass of Chaikin's corner cutting: the grid's facets rounded off, the ends kept.
function cut(p) {
  if (p.length < 3) return p;
  const out = [p[0]];
  for (let k = 0; k < p.length - 1; k++) {
    const [ax, ay] = p[k], [bx, by] = p[k + 1];
    out.push([0.75 * ax + 0.25 * bx, 0.75 * ay + 0.25 * by], [0.25 * ax + 0.75 * bx, 0.25 * ay + 0.75 * by]);
  }
  out.push(p[p.length - 1]);
  return out;
}

// A small closed ring: a knoll or a pit with nothing on it, which at a glance reads as an eye.
function speck(p, size) {
  const [ax, ay] = p[0], [bx, by] = p[p.length - 1];
  if (Math.hypot(ax - bx, ay - by) > 3) return false;
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const [x, y] of p) { x0 = Math.min(x0, x); y0 = Math.min(y0, y); x1 = Math.max(x1, x); y1 = Math.max(y1, y); }
  return Math.hypot(x1 - x0, y1 - y0) < size;
}

// The cells whose corners span some of [lo, hi]: the only ones a contour in that range can cross.
function crossing(F, e, lo, hi) {
  const { gw, gh } = F, out = [];
  for (let j = 0; j < gh - 1; j++) for (let i = 0; i < gw - 1; i++) {
    const k = j * gw + i, a = e[k], b = e[k + 1], c = e[k + gw], d = e[k + gw + 1];
    if (Math.max(a, b, c, d) >= lo && Math.min(a, b, c, d) <= hi) out.push(k);
  }
  return Int32Array.from(out);
}

// Each polyline drawn as a quadratic B-spline: a curve through the midpoints of its segments.
function stroke(ctx, lines, style, width, dash) {
  ctx.beginPath();
  for (const p of lines) {
    if (p.length < 3) continue;
    ctx.moveTo(p[0][0], p[0][1]);
    for (let k = 1; k < p.length - 1; k++) ctx.quadraticCurveTo(p[k][0], p[k][1], (p[k][0] + p[k + 1][0]) / 2, (p[k][1] + p[k + 1][1]) / 2);
    ctx.lineTo(p[p.length - 1][0], p[p.length - 1][1]);
  }
  ctx.strokeStyle = style; ctx.lineWidth = width;
  ctx.setLineDash(dash || []);
  ctx.stroke();
}

// The contours cut into short segments, each knowing how far it faces the sun: s = 1 where the
// slope below it faces the sun, -1 where it faces away. `down` is +1 if the field grows downhill
// (depth), -1 if it falls (elevation).
function litSegments(F, lines, field, down) {
  const segs = [], hh = F.cell * 0.6;
  for (const p of lines) {
    if (p.length < 3) continue;
    const pts = [p[0]];
    let m0 = p[0];
    for (let k = 1; k < p.length - 1; k++) {
      const c = p[k], m1 = [(c[0] + p[k + 1][0]) / 2, (c[1] + p[k + 1][1]) / 2];
      pts.push([0.25 * m0[0] + 0.5 * c[0] + 0.25 * m1[0], 0.25 * m0[1] + 0.5 * c[1] + 0.25 * m1[1]], m1);
      m0 = m1;
    }
    pts.push(p[p.length - 1]);
    for (let k = 0; k < pts.length - 1; k++) {
      const [x0, y0] = pts[k], [x1, y1] = pts[k + 1], dx = x1 - x0, dy = y1 - y0, len = Math.hypot(dx, dy);
      if (len < 1e-6) continue;
      const nx = -dy / len, ny = dx / len, mx = (x0 + x1) / 2, my = (y0 + y1) / 2;
      const up = down * (near(F, mx + nx * hh, my + ny * hh, field) - near(F, mx - nx * hh, my - ny * hh, field)) > 0 ? 1 : -1;
      segs.push(x0, y0, x1, y1, up * (nx * SUN[0] + ny * SUN[1]));
    }
  }
  return segs;
}

// Kitiro Tanaka's illuminated contours: each ring drawn fine and pale where it faces the sun and
// heavier and darker where it turns away, so the terraces stand up in the light. look(s) gives
// [style, width] for a bucket of s; `shift` moves a stroke towards the sun (a highlight).
function strokeLit(ctx, segs, look, shift = 0) {
  const B = 5, paths = [], ends = new Float64Array(2 * B).fill(NaN), ox = SUN[0] * shift, oy = SUN[1] * shift;
  for (let bk = 0; bk < B; bk++) paths.push(new Path2D());
  for (let k = 0; k < segs.length; k += 5) {
    const bk = Math.min(B - 1, Math.floor(((segs[k + 4] + 1) / 2) * B)), p = paths[bk];
    // A run of segments in one bucket stays one subpath, so it has no caps along it.
    if (ends[2 * bk] !== segs[k] || ends[2 * bk + 1] !== segs[k + 1]) p.moveTo(segs[k] + ox, segs[k + 1] + oy);
    p.lineTo(segs[k + 2] + ox, segs[k + 3] + oy);
    ends[2 * bk] = segs[k + 2]; ends[2 * bk + 1] = segs[k + 3];
  }
  for (let bk = 0; bk < B; bk++) {
    const [style, width] = look(-1 + (2 * bk + 1) / B);
    if (!style) continue;
    ctx.strokeStyle = style; ctx.lineWidth = width;
    ctx.stroke(paths[bk]);
  }
}

const rgba = (a, b, t, al) => `rgba(${a.map((x, i) => Math.round(x + (b[i] - x) * t)).join(",")},${al.toFixed(3)})`;

function drawLines(ctx, F, S) {
  const seen = { a: new Int32Array(2 * F.gw * F.gh), b: new Int32Array(2 * F.gw * F.gh), g: new Int32Array(2 * F.gw * F.gh), gen: 0 };
  const D = F.delta, small = 0.035 * S;
  ctx.save();
  ctx.lineJoin = "round"; ctx.lineCap = "round";
  // Faint contours on the land.
  const land = crossing(F, F.e, DRY + 0.11, DRY + 0.44);
  for (let k = 1; k <= 4; k++) {
    const major = k % 2 === 0;
    stroke(ctx, contour(F, DRY + k * 0.11, F.e, seen, land).filter((p) => !speck(p, small * 0.6)), rgba(INK.topoLit, INK.topo, 0.6, major ? 0.3 : 0.18), major ? 0.8 : 0.5);
  }
  // Isobaths every step to ten steps, heavier every fifth, fading with depth, lit; in the washes, a
  // glint of paper along each ring's sunward edge. Small rings round nothing are left out.
  const wet = crossing(F, F.m, D, 10 * D);
  for (let n = 1; n <= 10; n++) {
    const m = n * D, major = n % 5 === 0, fade = 1 - (n - 1) / 15;
    const lines = contour(F, m, F.m, seen, wet).filter((p) => !speck(p, small));
    // Out in deep water the rings are too faint for their light to show.
    if (n > 5) { stroke(ctx, lines, rgba(INK.isoLit, INK.iso, 0.5, (major ? 0.66 : 0.46) * fade * 0.8), major ? 1.0 : 0.6); continue; }
    const segs = litSegments(F, lines, F.m, 1);
    strokeLit(ctx, segs, (s) => [s > 0.1 ? `rgba(255,255,255,${(0.8 * s).toFixed(3)})` : null, 1.1], 0.75);
    strokeLit(ctx, segs, (s) => [rgba(INK.isoLit, INK.iso, (1 - s) / 2, (major ? 0.66 : 0.46) * fade * (0.7 + 0.5 * Math.max(0, -s))), (major ? 1.0 : 0.55) * (0.8 + 0.8 * Math.max(0, -s))]);
  }
  ctx.restore();
}

// The low-water line, dotted, round the drying flats; then the coast, heavier on its shadowed side.
// Drawn on the chart itself, not the layer that fades round the lead: land never lacks its outline.
function drawCoast(ctx, F) {
  const seen = { a: new Int32Array(2 * F.gw * F.gh), b: new Int32Array(2 * F.gw * F.gh), g: new Int32Array(2 * F.gw * F.gh), gen: 0 };
  ctx.save();
  ctx.lineJoin = "round"; ctx.lineCap = "round";
  const coast = crossing(F, F.e, 0, DRY);
  stroke(ctx, contour(F, 0, F.e, seen, coast), INK.low, 0.8, [0.6, 2.4]);
  const cs = litSegments(F, contour(F, DRY, F.e, seen, coast), F.e, -1);
  strokeLit(ctx, cs, (s) => [INK.coast, 0.95 + 0.9 * Math.max(0, -s)]);
  ctx.restore();
}

// Erase a box from a layer with a soft edge: the box drawn small with a blurred shadow (cast from
// off-canvas, which works where ctx.filter doesn't), then scaled up over the layer.
function clearRound(c, b, soft, w, h) {
  const k = 8, off = 10000, m = document.createElement("canvas");
  m.width = Math.ceil(w / k); m.height = Math.ceil(h / k);
  const mc = m.getContext("2d"), pad = soft * 0.35;
  mc.shadowColor = "#000"; mc.shadowBlur = soft / k; mc.shadowOffsetX = off;
  mc.fillRect((b.x - pad) / k - off, (b.y - pad) / k, (b.w + 2 * pad) / k, (b.h + 2 * pad) / k);
  c.save();
  c.globalCompositeOperation = "destination-out";
  c.imageSmoothingEnabled = true;
  c.drawImage(m, 0, 0, m.width * k, m.height * k);
  c.restore();
}

// ---- Shore, soundings, rose ---------------------------------------------------------------------

function stippleShore(ctx, F, r) {
  ctx.save();
  ctx.fillStyle = INK.stipple;
  const n = Math.round((F.w * F.h) / 45);
  for (let k = 0; k < n; k++) {
    const x = r() * F.w, y = r() * F.h, e = sample(F, x, y) - DRY;
    if (e < 0 || e > 0.08) continue;
    if (r() > Math.exp(-e * 50)) continue;
    ctx.fillRect(x, y, 0.8, 0.8);
  }
  ctx.restore();
}

// Soundings: small numerals in the water, densest near the shore and well away from the lead, where
// they would read as stray text; in decimetres where the chart is close enough for them to matter.
function drawSoundings(ctx, F, r, S, rose) {
  ctx.save();
  ctx.fillStyle = INK.sound;
  ctx.textAlign = "center"; ctx.textBaseline = "middle";
  // Soundings only on the shelf, thickest by the shore: out over flat water a scatter of the same
  // few numbers reads as stray text, not as a survey.
  const D = F.delta, placed = [], want = Math.round((F.w * F.h) / 15000), gap = 34;
  for (let k = 0; k < want * 14 && placed.length < want; k++) {
    const x = r.range(10, F.w - 10), y = r.range(10, F.h - 10);
    if (sample(F, x, y) > -0.01) continue;
    const m = sample(F, x, y, F.m);
    if (m > 6 * D || m < 0.3 * D) continue;
    if (boxDist(x, y, F.q) < 0.17 * S) continue;
    if (rose && Math.hypot(x - rose.x, y - rose.y) < rose.R * 1.25) continue;
    if (r() > 1.1 - m / (6 * D)) continue;
    if (placed.some(([px, py]) => Math.abs(px - x) < gap && Math.abs(py - y) < gap * 0.6)) continue;
    placed.push([x, y]);
    const whole = Math.floor(m), tenth = Math.floor((m - whole) * 10);
    ctx.font = `400 10px ${FACE}`;
    if ((m < 5.5 * D || D <= 0.5) && tenth > 0) {
      ctx.fillText(String(whole), x - 2, y);
      ctx.font = `400 7.5px ${FACE}`;
      ctx.fillText(String(tenth), x + 3.8, y + 2.8);
    } else ctx.fillText(String(whole), x, y);
  }
  ctx.restore();
}

function drawRose(ctx, F, r, S, w, h) {
  const R = Math.min(0.06 * S, 0.24 * Math.min(w, h)), D = F.delta;
  let at = null;
  for (let k = 0; k < 80 && !at; k++) {
    const x = r.range(R * 1.2, w - R * 1.2), y = r.range(h * 0.25, h - R * 1.4);
    if (boxDist(x, y, F.q) < R + 0.06 * S) continue;
    let ok = sample(F, x, y, F.m) > 5 * D;
    for (let q = 0; q < 16 && ok; q++) {
      const a = (q / 16) * 6.2832, px = x + R * 1.3 * Math.cos(a), py = y + R * 1.3 * Math.sin(a);
      if (sample(F, px, py, F.m) < 2.5 * D) ok = false;
    }
    if (ok) at = [x, y];
  }
  if (!at) return null;
  const col = (a) => `rgba(${INK.rose.join(",")},${a})`;
  ctx.save();
  ctx.translate(at[0], at[1]); ctx.rotate(r.range(-0.1, 0.1));
  ctx.lineWidth = 0.6;
  ctx.strokeStyle = col(0.36);
  for (const rr of [R, R * 0.93]) { ctx.beginPath(); ctx.arc(0, 0, rr, 0, 6.2832); ctx.stroke(); }
  ctx.beginPath();
  for (let deg = 0; deg < 360; deg += 5) {
    const a = (deg / 180) * Math.PI, inner = deg % 30 === 0 ? 0.85 : deg % 10 === 0 ? 0.89 : 0.91;
    ctx.moveTo(R * 0.93 * Math.cos(a), R * 0.93 * Math.sin(a));
    ctx.lineTo(R * inner * Math.cos(a), R * inner * Math.sin(a));
  }
  ctx.stroke();
  // The star: cardinal points longest, each point half inked.
  const point = (a, len, wid) => {
    const tip = [len * Math.sin(a), -len * Math.cos(a)];
    const l = [-wid * Math.cos(a), -wid * Math.sin(a)], rr = [wid * Math.cos(a), wid * Math.sin(a)];
    ctx.beginPath(); ctx.moveTo(0, 0); ctx.lineTo(...l); ctx.lineTo(...tip); ctx.closePath();
    ctx.fillStyle = col(0.2); ctx.fill();
    ctx.beginPath(); ctx.moveTo(0, 0); ctx.lineTo(...rr); ctx.lineTo(...tip); ctx.closePath();
    ctx.strokeStyle = col(0.34); ctx.stroke();
  };
  for (let k = 0; k < 4; k++) point((k * Math.PI) / 2, R * 0.82, R * 0.07);
  ctx.fillStyle = col(0.55);
  ctx.font = `600 ${Math.max(9, Math.round(R * 0.15))}px ${FACE}`;
  ctx.textAlign = "center";
  ctx.fillText("N", 0, -R - 4);
  ctx.restore();
  return { x: at[0], y: at[1], R };
}

// OKLCH to hex, lowering the chroma until the colour fits in sRGB (as the kit's fromOklch does), so
// a ground's identity needs no kit to be worked out.
function oklchHex(L, Cc, hDeg) {
  const rad = (hDeg * Math.PI) / 180, gam = (x) => (x <= 0.0031308 ? 12.92 * x : 1.055 * x ** (1 / 2.4) - 0.055);
  for (let c = Cc; ; c = Math.max(0, c - 0.005)) {
    const a = c * Math.cos(rad), b = c * Math.sin(rad);
    const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3, m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3, s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
    const v = [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s, -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s];
    if (c === 0 || v.every((x) => x >= -1e-4 && x <= 1.0001)) return "#" + v.map((x) => Math.round(255 * gam(Math.min(1, Math.max(0, x)))).toString(16).padStart(2, "0")).join("");
  }
}
