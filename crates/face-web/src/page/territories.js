// The map's areas as territories, each drawn as its theme's chart: what regions-paint.js draws for
// regions.js, off the page's thread.
//
// What it depicts: an old atlas, hand-tinted. Every area is a territory, the land within `reach` of
// its nodes, names and streams; where two areas come near each other the land between is shared out
// down the middle, so the areas make one landmass with one coast, cut into territories by seams that
// run to the coast. Each is tinted in a faded watercolour wash of its theme's hue (`map.hue`), darker
// where the wash dried at its edge, mottled, thinning to bare paper in patches, grainy, and paler up
// its history; the land hides the map's dot grid. Inside it, its theme draws its subject as a map
// (`chart(ctx, scene)`): Harbour's as a nautical chart, Tussock's as a topographic sheet. The chart is
// fullest round the present row and fades up the history; near the streams its ink thins, and it
// stops crisply short of every stream and node, as an engraver breaks a contour at a road. The names
// on the map carry their own halo (map.css). Every territory has its name lettered in spaced capitals
// up its history, smaller where it is narrow.
//
// Technique. `territories` works the map out once per layout, as a drawing in map units. Each area
// has a distance field on a grid of `cell` map units, within its own window (its nodes', names' and
// streams' box, and room round it for every claim), seeded with its node and name boxes and the
// points of its streams and wobbled by low noise of its own. The two nearest areas at each cell give
// the land, max(reach - d1, (2 reach + corridor - d1 - d2) / 2), whose zero line is the coast, and
// each area's cell, (d_other - d) / 2, whose edge inside the land is a seam: the same midline from
// either side, so a seam is one line. A territory is its cell clipped by the land, so a seam meets the
// coast in a clean T. Each area's wash is a small picture at `tintScale` pixels per map unit, and its
// chart is recorded from a canvas context as paths and words in map units. `render` draws any part
// of the drawing at any scale, sharp at the scale it is drawn at: the washes and the paper's grain,
// the charts' ink with the graph knocked out of it and fading up the history, then the seams, the
// coast, the water lines and the names.
import { noise, rng, fromOklch, oklch, kit, rgb, mix, simplify } from "./theme.js";

const LOOK = {
  reach: 170,      // map units of land round an area's nodes, names and streams
  corridor: 150,   // land narrower than 2 * reach + this between two areas is shared: one border, no bay
  cell: 8,         // the fields' grid, in map units
  wander: 30,      // how far a border wanders from the true midline, in map units
  crinkle: 12,     // and how far a coast crinkles, in bays and headlands a few dozen units across,
  jitter: 4,       // and finer still
  smooth: 2,       // passes of a 3x3 blur over each field, which rounds the corners where claims meet
  clear: 34,       // no border nearer a node or a name than this
  halo: 2.2,       // the gap the charts' ink keeps round every stream and node, beyond the stroke, in map units,
  haloPx: 2.5,     // and never narrower than this many pixels as drawn
  fade: [10, 20, 30], // beyond the gap the ink is a sixth lighter within each of these distances of a stream
  bridge: 40,      // a sliver of paper narrower than about twice this between two territories is closed
  ribbon: [2.5, 4, 0.22], // the band of tint just inside the border: inset, width, opacity
  coast: 1.15,     // the coast's line, in map units, and never under 0.75 px as drawn; a seam is lighter and finer
  seam: 0.8,
  waterlines: [[5, 0.34], [11, 0.22], [19, 0.12]], // the lines round the coast, out on the paper, as an old chart draws its shores: distance and strength
  wash: 0.55,      // the wash's body, as opacity over the map's ground
  mottle: 70,      // the size of the patches where the wash thinned to paper, in map units
  wet: 0.42,       // and its dried edge, at the coast, fading over `wetIn` map units inside it
  wetIn: 7,
  grain: 0.05,     // the paper's grain, as the opacity of its darkest speck
  letter: 15, letterEvery: 1500, // the area's name: its size, and how often up its history
  rise: 2400,      // how far up its history the chart reaches: full ink to 30 units above the now line, half by 360, gone over its last 600
  nameAbove: 540,  // the area's name is lettered at least this far above the now line, where the chart is quiet
  ink: 0.8,        // the charts' ink, as a share of what they draw
  tintScale: 0.5,  // the wash, in pixels per map unit
};
// An area's hue as its wash's body and dried edge, the band inside its border, and its lettering.
const inks = (hue, k = 1) => ({ fill: fromOklch(0.965, 0.013 * k, hue), wet: fromOklch(0.83, 0.03 * k, hue), band: fromOklch(0.8, 0.04 * k, hue), line: fromOklch(0.5, 0.035 * k, hue) });
// The coast, the seams and the water lines belong to the whole map: one neutral ink.
const COAST = fromOklch(0.46, 0.014, 80), SEAM = fromOklch(0.56, 0.012, 80);
const ss = (a, b, t) => { t = Math.min(1, Math.max(0, (t - a) / (b - a))); return t * t * (3 - 2 * t); };
// How much of its ink a chart keeps at height y: all of it to 30 units above the now line, half by
// 360, and none past `rise`.
const hist = (y) => (y > -30 ? 1 : 0.5 + 0.5 * ss(-360, -30, y)) * ss(-LOOK.rise, -LOOK.rise + 600, y);

// areas: [{ id, theme (the module), seed, name, marks: [[x0, y0, x1, y1]], labels: [[x0, y0, x1, y1]] }],
// each mark a node's box and each label a node's name's box, in map units, the now line at y = 0 and
// history above it. lines: the map's lines, each [[x, y, half-width], ...]. paper: the map's ground.
// Returns the drawing `render` draws: each area with land as { id, box, tint, clip, band, ink, texts,
// seams, names }, the land, coast and water lines, the graph to knock out of the ink, the drawing's
// bounds, and `unknown`: the context members a chart used that the recorder lacks.
export function territories(areas, lines, paper) {
  const { reach, cell } = LOOK;
  for (const a of areas) {
    if (typeof a.theme.map?.hue !== "number") throw new Error(`the ${a.theme.name} theme has no map.hue`);
    if (typeof a.theme.chart !== "function") throw new Error(`the ${a.theme.name} theme has no chart(ctx, scene)`);
  }
  const boxes = (a) => [...a.marks, ...a.labels];
  const bbox = (list) => list.reduce((b, [x0, y0, x1, y1]) => [Math.min(b[0], x0), Math.min(b[1], y0), Math.max(b[2], x1), Math.max(b[3], y1)], [Infinity, Infinity, -Infinity, -Infinity]);
  let [X0, Y0, X1, Y1] = bbox(areas.flatMap(boxes));
  if (X0 === Infinity) return { areas: [], bounds: null, unknown: [] };
  // Room round the marks for every claim: a shared corridor reaches reach + corridor / 2 out, a bridge
  // lifts it further, and the wobble moves it about.
  const pad = reach + LOOK.corridor / 2 + LOOK.bridge + 1.8 * LOOK.wander + 1.5 * LOOK.crinkle + LOOK.waterlines.at(-1)[0] + 3 * cell;
  X0 = Math.floor((X0 - pad) / cell) * cell; Y0 = Math.floor((Y0 - pad) / cell) * cell;
  const nx = Math.ceil((X1 + pad - X0) / cell) + 1, ny = Math.ceil((Y1 + pad - Y0) / cell) + 1, N = nx * ny;
  const gx = (i) => X0 + i * cell, gy = (j) => Y0 + j * cell;
  const cellOf = (x, y) => { const i = Math.round((x - X0) / cell), j = Math.round((y - Y0) / cell); return i >= 0 && j >= 0 && i < nx && j < ny ? j * nx + i : -1; };
  // A window of the grid: the cells within `pad` of a box. `at` takes a cell of the whole grid to the
  // window's (-1 outside it); each(W, f) calls f(q, c, i, j) for each of its cells, q the window's
  // index, c the grid's, and i, j the window's column and row.
  const win = ([x0, y0, x1, y1]) => {
    const i0 = Math.max(0, Math.floor((x0 - pad - X0) / cell)), j0 = Math.max(0, Math.floor((y0 - pad - Y0) / cell));
    const i1 = Math.min(nx - 1, Math.ceil((x1 + pad - X0) / cell)), j1 = Math.min(ny - 1, Math.ceil((y1 + pad - Y0) / cell));
    const w = i1 - i0 + 1, h = j1 - j0 + 1;
    return { i0, j0, w, h, n: w * h, at: (c) => { if (c < 0) return -1; const i = (c % nx) - i0, j = Math.floor(c / nx) - j0; return i >= 0 && j >= 0 && i < w && j < h ? j * w + i : -1; } };
  };
  const each = (W, f) => { for (let j = 0, q = 0; j < W.h; j++) for (let i = 0, c = (W.j0 + j) * nx + W.i0; i < W.w; i++, q++, c++) f(q, c, i, j); };
  const seedsIn = (W, list) => {
    const s = new Uint8Array(W.n);
    for (const [x0, y0, x1, y1] of list)
      for (let j = Math.max(W.j0, Math.floor((y0 - Y0) / cell)); j <= Math.min(W.j0 + W.h - 1, Math.ceil((y1 - Y0) / cell)); j++)
        for (let i = Math.max(W.i0, Math.floor((x0 - X0) / cell)); i <= Math.min(W.i0 + W.w - 1, Math.ceil((x1 - X0) / cell)); i++) s[(j - W.j0) * W.w + i - W.i0] = 1;
    return s;
  };

  // Each area's true distance to its nodes and names, near them: for the clearing round them, and to
  // give each stream's points to the area whose nodes are nearest.
  const rawBest = new Float32Array(N).fill(Infinity), rawK = new Int16Array(N).fill(-1);
  areas.forEach((a, k) => {
    const W = win(bbox(boxes(a))), r = edt(seedsIn(W, boxes(a)), W.w, W.h);
    each(W, (q, c) => { const v = Math.sqrt(r[q]) * cell; if (v < rawBest[c]) { rawBest[c] = v; rawK[c] = k; } });
  });
  const lineSeeds = new Uint8Array(N), own = areas.map(() => []);
  for (const [x, y] of along(lines, cell)) { const c = cellOf(x, y); if (c < 0) continue; lineSeeds[c] = 1; if (rawK[c] >= 0) own[rawK[c]].push([x, y]); }
  const lineD = edt(lineSeeds, nx, ny).map((v) => Math.sqrt(v) * cell);
  // The graph as it is drawn, exactly: each stream's segments with their half-widths, and each node's
  // box, filed in buckets, so a chart can keep its symbols off it (`calm`).
  const HB = 24, graph = new Map(), halo = LOOK.halo;
  const file = (x0, y0, x1, y1, item) => {
    for (let j = Math.floor(y0 / HB); j <= Math.floor(y1 / HB); j++) for (let i = Math.floor(x0 / HB); i <= Math.floor(x1 / HB); i++) {
      const key = i * 65536 + j;
      (graph.get(key) ?? graph.set(key, []).get(key)).push(item);
    }
  };
  for (const P of lines) for (let i = 1; i < P.length; i++) {
    const [ax, ay, aw = 1.1] = P[i - 1], [bx, by, bw = 1.1] = P[i], m = Math.max(aw, bw) + halo;
    file(Math.min(ax, bx) - m, Math.min(ay, by) - m, Math.max(ax, bx) + m, Math.max(ay, by) + m, [ax, ay, bx, by, aw, bw]);
  }
  for (const a of areas) for (const [x0, y0, x1, y1] of a.marks) file(x0 - halo, y0 - halo, x1 + halo, y1 + halo, [x0, y0, x1, y1]);
  const open = (x, y) => {
    const list = graph.get(Math.floor(x / HB) * 65536 + Math.floor(y / HB));
    if (list) for (const it of list) {
      if (it.length === 4) { const dx = Math.max(it[0] - x, 0, x - it[2]), dy = Math.max(it[1] - y, 0, y - it[3]); if (dx * dx + dy * dy < halo * halo) return false; continue; }
      const [ax, ay, bx, by, aw, bw] = it, vx = bx - ax, vy = by - ay, L2 = vx * vx + vy * vy;
      const t = L2 ? Math.max(0, Math.min(1, ((x - ax) * vx + (y - ay) * vy) / L2)) : 0, r = aw + (bw - aw) * t + halo;
      if ((x - ax - t * vx) ** 2 + (y - ay - t * vy) ** 2 < r * r) return false;
    }
    return true;
  };
  const calm = (x, y) => (open(x, y) ? 1 : 0);

  // Each area's distance to its nodes, names and streams within its window, wobbled by its own noise
  // and crinkled near where a coast can run by the map's; and its wash's bloom, the slow unevenness of
  // a wash laid by hand, on a coarse grid.
  const C = 4, cn = noise("atlas:coast"), crink = new Float32Array(N).fill(NaN);
  const crinkAt = (c, x, y) => (Number.isNaN(crink[c]) ? (crink[c] = LOOK.crinkle * cn.fbm(x / 70, y / 70, 2) + LOOK.jitter * cn.n2(x / 24, y / 24)) : crink[c]);
  const fields = areas.map((a, k) => {
    const W = win(bbox([...boxes(a), ...own[k].map(([x, y]) => [x, y, x, y])])), s = seedsIn(W, boxes(a));
    for (const [x, y] of own[k]) { const q = W.at(cellOf(x, y)); if (q >= 0) s[q] = 1; }
    const d = edt(s, W.w, W.h);
    const n = noise(`atlas:${a.seed}`), cw = Math.ceil(W.w / C) + 2, ch = Math.ceil(W.h / C) + 2, wob = new Float32Array(cw * ch), bl = new Float32Array(cw * ch);
    for (let j = 0; j < ch; j++) for (let i = 0; i < cw; i++) {
      const x = gx(W.i0 + i * C), y = gy(W.j0 + j * C);
      wob[j * cw + i] = n.fbm(x * 0.0035, y * 0.0035, 3); bl[j * cw + i] = n.fbm(x * 0.006 + 31, y * 0.006 - 7, 3);
    }
    const coarse = (g) => (i, j) => {
      const fi = Math.max(0, i / C), fj = Math.max(0, j / C), i0 = Math.min(cw - 2, Math.floor(fi)), j0 = Math.min(ch - 2, Math.floor(fj)), u = fi - i0, v = fj - j0;
      return (g[j0 * cw + i0] * (1 - u) + g[j0 * cw + i0 + 1] * u) * (1 - v) + (g[(j0 + 1) * cw + i0] * (1 - u) + g[(j0 + 1) * cw + i0 + 1] * u) * v;
    };
    const wobble = coarse(wob);
    each(W, (q, c, i, j) => {
      const dd = Math.sqrt(d[q]) * cell, band = ss(reach + 140, reach + 90, dd) * ss(reach - 110, reach - 60, dd);
      d[q] = dd + LOOK.wander * wobble(i, j) + (band > 0 ? band * crinkAt(c, gx(W.i0 + i), gy(W.j0 + j)) : 0);
    });
    return { W, d, bloom: coarse(bl) };
  });
  // A clearing round every node and name: no border comes nearer than `clear`.
  fields.forEach(({ W, d }, m) => each(W, (q, c) => {
    const k = rawK[c], best = rawBest[c];
    if (k < 0 || k === m || best >= LOOK.clear) return;
    const qk = fields[k].W.at(c);
    if (qk < 0) return;
    const floor = fields[k].d[qk] + 2 * (LOOK.clear - best);
    if (d[q] < floor) d[q] = floor;
  }));
  // The two nearest areas at each cell, in one pass over each window.
  const d1 = new Float32Array(N).fill(Infinity), d2 = new Float32Array(N).fill(Infinity), k1 = new Int16Array(N).fill(-1), k2 = new Int16Array(N).fill(-1);
  fields.forEach(({ W, d }, k) => each(W, (q, c) => {
    const v = d[q];
    if (v < d1[c]) { d2[c] = d1[c]; k2[c] = k1[c]; d1[c] = v; k1[c] = k; } else if (v < d2[c]) { d2[c] = v; k2[c] = k; }
  }));
  // All the land, one landmass: positive inside the coast, negative out on the paper. Each of the two
  // nearest areas claims its own reach, or the corridor it shares with the other; where the second
  // claim nearly reaches the land, the land is lifted, so no sliver of paper is left between them.
  const shared = 2 * reach + LOOK.corridor;
  let land = new Float32Array(N);
  for (let c = 0; c < N; c++) {
    if (d1[c] === Infinity) { land[c] = -reach; continue; }
    const s = (shared - d1[c] - d2[c]) / 2, a = Math.max(reach - d1[c], s), b = Math.max(reach - d2[c], s);
    land[c] = a + LOOK.bridge * ss(-LOOK.bridge, 0, b);
  }
  for (let p = 0; p < LOOK.smooth; p++) land = blur(land, nx, ny);
  // Sea at a grid's edge, so every line traced closes.
  const sea = (f, w, h) => { for (let i = 0; i < w; i++) for (let j = 0; j < h; j++) if (!i || !j || i === w - 1 || j === h - 1) f[j * w + i] = Math.min(f[j * w + i], -1); return f; };
  sea(land, nx, ny);
  // Each area's cell, how much nearer it is than the nearest other area, halved, and its territory,
  // its cell within the land, both within its window.
  const cap = 4 * reach;
  const parts = fields.map(({ W, d }, k) => {
    let ck = new Float32Array(W.n);
    each(W, (q, c) => { ck[q] = k1[c] === k ? Math.min(cap, (d2[c] - d1[c]) / 2) : Math.max(-cap, (d1[c] - d[q]) / 2); });
    for (let p = 0; p < LOOK.smooth; p++) ck = blur(ck, W.w, W.h);
    sea(ck, W.w, W.h);
    const part = new Float32Array(W.n);
    each(W, (q, c) => { part[q] = Math.min(ck[q], land[c]); });
    return { cell: ck, part: sea(part, W.w, W.h) };
  });
  const sampleIn = (g, w, h, fi, fj) => {
    fi = Math.min(w - 1.001, Math.max(0, fi)); fj = Math.min(h - 1.001, Math.max(0, fj));
    const i = Math.floor(fi), j = Math.floor(fj), u = fi - i, v = fj - j, c = j * w + i;
    return (g[c] * (1 - u) + g[c + 1] * u) * (1 - v) + (g[c + w] * (1 - u) + g[c + w + 1] * u) * v;
  };
  const sample = (g, x, y) => sampleIn(g, nx, ny, (x - X0) / cell, (y - Y0) / cell);
  const trace = (f, w, h, i0 = 0, j0 = 0) => contour(f, w, h, (i) => gx(i0 + i), (j) => gy(j0 + j)).map((r) => simplify(smooth(smooth(smooth(r))), 0.15));
  const coast = trace(land, nx, ny);
  const shores = LOOK.waterlines.map(([off, op]) => ({ path: path2d(trace(sea(land.map((v) => v + off), nx, ny), nx, ny), true), colour: mix(paper, COAST, op * 1.25) }));
  const grainy = grainOf(rgb(paper));

  const unknown = new Set(), out = [], ground = rgb(paper);
  areas.forEach((a, k) => {
    const { W, d, bloom } = fields[k], { cell: ck, part: fk } = parts[k], c = inks(a.theme.map.hue, a.theme.map.chroma ?? 1);
    const at = (g, x, y) => sampleIn(g, W.w, W.h, (x - X0) / cell - W.i0, (y - Y0) / cell - W.j0);
    // Its box: its territory, and a margin.
    let i0 = W.w, i1 = -1, j0 = W.h, j1 = -1;
    each(W, (q, cc, i, j) => { if (land[cc] > -1 && ck[q] > -1) { i0 = Math.min(i0, i); i1 = Math.max(i1, i); j0 = Math.min(j0, j); j1 = Math.max(j1, j); } });
    if (i1 < 0) return;
    const box = { x: gx(W.i0 + i0) - 2 * cell, y: gy(W.j0 + j0) - 2 * cell, w: (i1 - i0 + 4) * cell, h: (j1 - j0 + 4) * cell };
    const cellRings = trace(ck, W.w, W.h, W.i0, W.j0);
    if (!cellRings.length) return;

    // Its wash, laid over the map's ground so the land hides the dot grid: the body uneven, thinning to
    // bare paper in patches that keep a faint dried rim, darker where it dried at its border, paler up
    // its history, and stopping a little short of the border here and there, as a hand-laid wash does.
    // It reaches a little past its seams, under its neighbour's, so no ground shows between them.
    const nw = noise(`wash:${a.seed}`), mot = new Float32Array(W.n);
    each(W, (q, cc, i, j) => { if (land[cc] > -2) mot[q] = nw.fbm(gx(W.i0 + i) / LOOK.mottle, gy(W.j0 + j) / LOOK.mottle, 3); });
    const ts = Math.min(LOOK.tintScale, 4096 / box.w, 4096 / box.h), TW = Math.max(1, Math.ceil(box.w * ts)), TH = Math.max(1, Math.ceil(box.h * ts));
    const tint = new OffscreenCanvas(TW, TH), tctx = tint.getContext("2d"), img = tctx.createImageData(TW, TH), px = img.data;
    const fill = rgb(c.fill), wet = rgb(c.wet);
    for (let j = 0, i = 0; j < TH; j++) for (let q = 0; q < TW; q++, i += 4) {
      const x = box.x + (q + 0.5) / ts, y = box.y + (j + 0.5) / ts;
      const cover = ss(-0.8, 0.8, sample(land, x, y)) * ss(-0.8, 0.8, at(ck, x, y) + 3);
      if (cover <= 0) continue;
      const f = at(fk, x, y), short = f < 6 ? 1.2 + 1.2 * nw.n2(x / 40, y / 40) : 0, inside = ss(-0.8, 0.8, f - short), m = at(mot, x, y);
      const body = LOOK.wash * Math.max(0.2, 0.68 + 0.55 * bloom((x - X0) / cell - W.i0, (y - Y0) / cell - W.j0)) * (1 - 0.72 * ss(0.08, 0.3, m));
      const edge = LOOK.wet * Math.exp(-Math.max(0, f - short) / LOOK.wetIn) + 0.14 * Math.exp(-(((m - 0.08) / 0.04) ** 2));
      const A = Math.min(1, (body + edge * (1 - body)) * (0.55 + 0.45 * ss(-1800, -150, y)) * inside), e = edge / (body + edge);
      for (let ch = 0; ch < 3; ch++) px[i + ch] = Math.round(ground[ch] * (1 - A) + (fill[ch] * (1 - e) + wet[ch] * e) * A);
      px[i + 3] = Math.round(255 * cover);
    }
    tctx.putImageData(img, 0, 0);

    // Its chart, in the box from `rise` above the now line to the territory's foot: ink recorded as
    // paths and words in map units, pale tints into the wash, fading up its history faster than the ink.
    const ink = new Map(), texts = [], cx0 = box.x, cy0 = Math.max(box.y, -LOOK.rise), cw = box.w, ch = box.y + box.h - cy0;
    if (cw > 60 && ch > 60) {
      const under = new OffscreenCanvas(Math.ceil(cw * ts), Math.ceil(ch * ts)), uctx = under.getContext("2d");
      const sink = { ox: cx0, oy: cy0, ink, texts, tctx: uctx, ts, calm, unknown };
      const seed = `${a.theme.name}:${a.seed}:chart`;
      a.theme.chart(recorder(new Rec("ink", sink)), {
        w: cw, h: ch, now: -cy0, scale: 1, paper: mix(paper, c.fill, LOOK.wash),
        mass: (x, y) => ss(0, 24, at(fk, x + cx0, y + cy0)),
        calm: (x, y) => calm(x + cx0, y + cy0),
        n: noise(seed), stream: (p) => rng(`${seed}:${p}`), under: recorder(new Rec("tint", sink)), kit,
      });
      const mask = uctx.createImageData(under.width, under.height), md = mask.data;
      for (let j = 0, i = 0; j < under.height; j++) for (let q = 0; q < under.width; q++, i += 4) {
        const x = cx0 + (q + 0.5) / ts, y = cy0 + (j + 0.5) / ts;
        md[i + 3] = Math.round(255 * ss(-1.5, 1.5, at(fk, x, y)) * hist(y) ** 2);
      }
      const mc = new OffscreenCanvas(under.width, under.height);
      mc.getContext("2d").putImageData(mask, 0, 0);
      uctx.setTransform(1, 0, 0, 1, 0, 0);
      uctx.globalCompositeOperation = "destination-in";
      uctx.drawImage(mc, 0, 0);
      tctx.drawImage(under, (cx0 - box.x) * ts, (cy0 - box.y) * ts);
    }

    // The band of tint inside its border, and its seams, each drawn once, by the territory of the two
    // listed later, within the land so each ends on the coast.
    const [inset, width, op] = LOOK.ribbon;
    const band = { path: path2d(trace(fk.map((v) => v - inset), W.w, W.h, W.i0, W.j0), true), colour: c.band, width, op };
    const seams = cellRings.flatMap((r) => runs(r, ([x, y]) => {
      const q = cellOf(x, y);
      if (q < 0 || sample(land, x, y) <= -3) return false;
      const other = k1[q] === k ? k2[q] : k1[q];
      return other >= 0 && other < k;
    }));

    // Its name, lettered up its history where it is clear of the streams and the border: above the
    // chart first; an area too short or narrow for that takes the clearest place it has, and one with no
    // clear run that long has it smaller, up or across its longest clear run.
    const names = [];
    if (a.name && LOOK.letter) {
      const size = LOOK.letter, half = (a.name.length * size * 1.25) / 2 + size, text = a.name.toUpperCase();
      const letter = (x, y, rot, size) => names.push({ text, x, y, rot, size, colour: c.line });
      for (const [above, room0] of [[LOOK.nameAbove, 26], [60, 18]]) {
        if (names.length) break;
        const clear = (i, j) => { const q = j * W.w + i; return d[q] > room0 && fk[q] > 16 && lineD[(W.j0 + j) * nx + W.i0 + i] > 14; };
        let lastY = Infinity;
        for (let j = W.h - 1; j >= 0; j--) {
          const y = gy(W.j0 + j);
          if (lastY - y < LOOK.letterEvery || y > -above) continue;
          let best = null;
          for (let i = 0; i < W.w; i++) {
            if (!clear(i, j)) continue;
            let ok = true;
            for (let dj = -Math.ceil(half / cell); dj <= Math.ceil(half / cell) && ok; dj++) { const jj = j + dj; if (jj < 0 || jj >= W.h || !clear(i, jj)) ok = false; }
            if (!ok) continue;
            const room = Math.min(d[j * W.w + i], fk[j * W.w + i] * 1.6);
            if (!best || room > best.room) best = { i, room };
          }
          if (!best) continue;
          letter(gx(W.i0 + best.i), y, -90, size);
          lastY = y;
        }
      }
      if (!names.length) {
        const ok = (i, j) => { const q = Math.max(0, Math.min(W.h - 1, j)) * W.w + i; return d[q] > 14 && fk[q] > 10 && lineD[(W.j0 + Math.max(0, Math.min(W.h - 1, j))) * nx + W.i0 + i] > 9; };
        let up = null, side = null;
        for (let i = 0; i < W.w; i++) for (let j = 0, run = 0; j < W.h; j++) {
          run = ok(i, j) ? run + 1 : 0;
          if (run && (!up || run > up.run)) up = { run, x: gx(W.i0 + i), y: gy(W.j0 + j - (run - 1) / 2) };
        }
        for (let j = 0; j < W.h; j++) for (let i = 0, run = 0; i < W.w; i++) {
          run = ok(i, j) && ok(i, j - 1) && ok(i, j + 1) ? run + 1 : 0;
          if (run && (!side || run > side.run)) side = { run, x: gx(W.i0 + i - (run - 1) / 2), y: gy(W.j0 + j) };
        }
        const fit = (r) => (r ? Math.min(size, ((r.run - 1) * cell - 12) / (a.name.length * 1.25 + 0.5)) : 0);
        const sUp = fit(up), sSide = fit(side);
        if (sUp >= 8 && sUp >= 0.8 * sSide) letter(up.x, up.y, -90, sUp);
        else if (sSide >= 8) letter(side.x, side.y, 0, sSide);
      }
    }
    out.push({ id: a.id, box, tint, clip: path2d(cellRings, true), band, ink: [...ink.values()], texts, seams: seams.length ? path2d(seams) : null, names });
  });

  // The graph, to knock out of the charts' ink: the streams' segments by their half-widths, and the
  // nodes' boxes.
  const knock = new Map();
  for (const P of lines) for (let i = 1; i < P.length; i++) {
    const hw = Math.round(10 * Math.max(P[i - 1][2] ?? 1.1, P[i][2] ?? 1.1)) / 10;
    const p = knock.get(hw) ?? knock.set(hw, new Path2D()).get(hw);
    p.moveTo(P[i - 1][0], P[i - 1][1]); p.lineTo(P[i][0], P[i][1]);
  }
  return {
    areas: out, land: path2d(coast, true), coast: path2d(coast, true), shores, grainy,
    knock: [...knock], nodes: areas.flatMap((a) => a.marks),
    bounds: { x: X0, y: Y0, w: (nx - 1) * cell, h: (ny - 1) * cell }, unknown: [...unknown],
  };
}

// Part of a drawing, the map units in rect ({ x, y, w, h }), drawn on a canvas W x H pixels. A canvas
// pixel is 1 / dpr of a CSS pixel, for the paper's grain. With washOnly, only the washes.
export function render(d, rect, W, H, { dpr = 1, washOnly = false } = {}) {
  const view = new OffscreenCanvas(W, H), ctx = view.getContext("2d"), s = W / rect.w, at = [s, 0, 0, s, -rect.x * s, -rect.y * s];
  if (!d.bounds) return view;
  const seen = d.areas.filter(({ box: b }) => b.x < rect.x + rect.w && b.x + b.w > rect.x && b.y < rect.y + rect.h && b.y + b.h > rect.y);
  ctx.setTransform(...at);
  ctx.imageSmoothingQuality = "medium";
  // The washes, each within its cell, and the band inside each border.
  for (const a of seen) { ctx.save(); ctx.clip(a.clip, "evenodd"); ctx.drawImage(a.tint, a.box.x, a.box.y, a.box.w, a.box.h); ctx.restore(); }
  if (!washOnly) {
    for (const a of seen) {
      ctx.save(); ctx.clip(a.clip, "evenodd");
      Object.assign(ctx, { strokeStyle: a.band.colour, lineWidth: a.band.width, globalAlpha: a.band.op, lineJoin: "round" });
      ctx.stroke(a.band.path);
      ctx.restore();
    }
    // The paper's grain, the same size however far the map is zoomed.
    ctx.save();
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    const pattern = ctx.createPattern(d.grainy, "repeat"), T = 64 * dpr;
    pattern.setTransform(new DOMMatrix([dpr, 0, 0, dpr, (((-rect.x * s) % T) + T) % T, (((-rect.y * s) % T) + T) % T]));
    ctx.fillStyle = pattern;
    ctx.fillRect(0, 0, W, H);
    ctx.restore();
  }
  // All of it within the land.
  ctx.globalCompositeOperation = "destination-in";
  ctx.fill(d.land, "evenodd");
  ctx.globalCompositeOperation = "source-over";
  if (washOnly) return view;

  // The charts' ink, each within its cell and the land, on a layer of its own: the graph knocked out
  // of it, a gap at least `haloPx` pixels round each stream and node, the ink thinning near the
  // streams beyond, and fading up the history.
  const layer = new OffscreenCanvas(W, H), lc = layer.getContext("2d");
  lc.setTransform(...at);
  for (const a of seen) {
    lc.save(); lc.clip(a.clip, "evenodd");
    for (const op of a.ink) {
      if (op.how === "fill") { lc.fillStyle = op.colour; lc.fill(op.path); continue; }
      Object.assign(lc, { strokeStyle: op.colour, lineWidth: op.lw, lineCap: op.cap, lineJoin: op.join });
      lc.setLineDash(op.dash ?? []);
      lc.stroke(op.path);
    }
    lc.setLineDash([]);
    for (const t of a.texts) {
      lc.save(); lc.transform(...t.m);
      Object.assign(lc, { font: t.font, textAlign: t.align, textBaseline: t.baseline });
      if (t.how === "fill") { lc.fillStyle = t.colour; lc.fillText(t.text, t.x, t.y); } else { lc.strokeStyle = t.colour; lc.lineWidth = t.lw; lc.strokeText(t.text, t.x, t.y); }
      lc.restore();
    }
    lc.restore();
  }
  const gap = Math.max(LOOK.halo, LOOK.haloPx / s);
  Object.assign(lc, { globalCompositeOperation: "destination-out", strokeStyle: "#000", fillStyle: "#000", lineCap: "round", lineJoin: "round" });
  for (const [extra, alpha] of [...LOOK.fade.map((f) => [f, 1 / 6]), [0, 1]]) {
    lc.globalAlpha = alpha;
    for (const [hw, path] of d.knock) { lc.lineWidth = 2 * (hw + gap + extra); lc.stroke(path); }
    const nodes = new Path2D(), r = gap + extra;
    for (const [x0, y0, x1, y1] of d.nodes) nodes.roundRect(x0 - r, y0 - r, x1 - x0 + 2 * r, y1 - y0 + 2 * r, Math.min(x1 - x0, y1 - y0) / 2 + r);
    lc.fill(nodes);
  }
  lc.globalAlpha = 1;
  // The names' halo: the ink breaks round each letter, as an engraver keeps lettering clear.
  for (const a of seen) for (const n of a.names) {
    lc.save(); lc.translate(n.x, n.y); lc.rotate((n.rot * Math.PI) / 180);
    Object.assign(lc, { font: `500 ${n.size}px Literata, Georgia, serif`, letterSpacing: `${n.size * 0.42}px`, textAlign: "center", textBaseline: "middle", lineWidth: 2 * gap });
    lc.strokeText(n.text, 0, 0); lc.fillText(n.text, 0, 0);
    lc.restore();
  }
  lc.globalCompositeOperation = "destination-in";
  const y0 = rect.y, y1 = rect.y + rect.h, fadeY = [];
  for (let i = 0; i <= 8; i++) fadeY.push(-LOOK.rise + (600 * i) / 8, -360 + (330 * i) / 8);
  const ys = [y0, y1, ...fadeY.filter((y) => y > y0 && y < y1)].sort((a, b) => a - b);
  const g = lc.createLinearGradient(0, y0, 0, y1);
  for (const y of ys) g.addColorStop((y - y0) / (y1 - y0), `rgba(0,0,0,${LOOK.ink * hist(y)})`);
  lc.fillStyle = g;
  lc.fill(d.land, "evenodd");
  ctx.save(); ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.drawImage(layer, 0, 0); ctx.restore();

  // The seams, the coast and the water lines, sharp and never too fine to see.
  Object.assign(ctx, { lineCap: "round", lineJoin: "round" });
  ctx.save(); ctx.clip(d.land, "evenodd");
  Object.assign(ctx, { strokeStyle: SEAM, lineWidth: Math.max(LOOK.seam, 0.5 / s), globalAlpha: 0.9 });
  for (const a of seen) if (a.seams) ctx.stroke(a.seams);
  ctx.restore();
  Object.assign(ctx, { strokeStyle: COAST, lineWidth: Math.max(LOOK.coast, 0.75 / s), globalAlpha: 0.85 });
  ctx.stroke(d.coast);
  ctx.globalAlpha = 1;
  for (const w of d.shores) { Object.assign(ctx, { strokeStyle: w.colour, lineWidth: Math.max(0.6, 0.5 / s) }); ctx.stroke(w.path); }

  // The areas' names, in spaced capitals.
  Object.assign(ctx, { textAlign: "center", textBaseline: "middle", globalAlpha: 0.5 });
  for (const a of seen) for (const n of a.names) {
    ctx.save(); ctx.translate(n.x, n.y); ctx.rotate((n.rot * Math.PI) / 180);
    Object.assign(ctx, { font: `500 ${n.size}px Literata, Georgia, serif`, letterSpacing: `${n.size * 0.42}px`, fillStyle: n.colour });
    ctx.fillText(n.text, 0, 0);
    ctx.restore();
  }
  ctx.globalAlpha = 1;
  return view;
}

// Points every `step` map units along each line.
function along(lines, step) {
  const out = [];
  for (const P of lines) {
    for (let i = 1; i < P.length; i++) {
      const [ax, ay] = P[i - 1], [bx, by] = P[i], k = Math.max(1, Math.ceil(Math.hypot(bx - ax, by - ay) / step));
      for (let s = 0; s < k; s++) out.push([ax + ((bx - ax) * s) / k, ay + ((by - ay) * s) / k]);
    }
    if (P.length) out.push(P[P.length - 1]);
  }
  return out;
}

// The paper's grain: a 64-pixel tile of darker specks, softened noise in the map's ground's shade.
function grainOf([r, g, b]) {
  const raw = rng("wash grain"), n = new Float32Array(64 * 64).map(() => raw() * 2 - 1);
  const tile = new OffscreenCanvas(64, 64), tc = tile.getContext("2d"), img = tc.createImageData(64, 64);
  for (let j = 0; j < 64; j++) for (let i = 0; i < 64; i++) {
    let s = 0;
    for (const [di, dj, w] of [[0, 0, 0.5], [1, 0, 0.125], [-1, 0, 0.125], [0, 1, 0.125], [0, -1, 0.125]]) s += w * n[((j + dj) & 63) * 64 + ((i + di) & 63)];
    const p = (j * 64 + i) * 4;
    img.data.set([r * 0.45, g * 0.42, b * 0.38, 255 * LOOK.grain * Math.min(1, Math.max(0, s * 1.6))], p);
  }
  tc.putImageData(img, 0, 0);
  return tile;
}

// Rings or runs of map points as one path; rings closed, and runs closed where they are (`closed`).
function path2d(list, close = false) {
  const p = new Path2D();
  for (const r of list) {
    p.moveTo(r[0][0], r[0][1]);
    for (let i = 1; i < r.length; i++) p.lineTo(r[i][0], r[i][1]);
    if (close || r.closed) p.closePath();
  }
  return p;
}

// ─── The recording context ────────────────────────────────────────────────────────────────────────
// A 2D context a chart draws on as on a canvas, in its box's units. "ink" records strokes, dark fills
// and words in map units: the paths of one style become one path, for `render` to draw at any scale.
// Pale fills and pictures go to the wash; "tint" sends everything to the wash. Members it lacks are
// noted (`unknown`) and do nothing.
const STATE = ["strokeStyle", "fillStyle", "lineWidth", "lineCap", "lineJoin", "font", "textAlign", "textBaseline", "globalAlpha", "globalCompositeOperation"];
class Rec {
  constructor(mode, sink) {
    Object.assign(this, { mode, sink, m: [1, 0, 0, 1, sink.ox, sink.oy], stack: [], path: new Path2D(), dash: [] });
    Object.assign(this, { strokeStyle: "#000", fillStyle: "#000", lineWidth: 1, lineCap: "butt", lineJoin: "miter", font: "10px sans-serif", textAlign: "start", textBaseline: "alphabetic", globalAlpha: 1, globalCompositeOperation: "source-over", imageSmoothingEnabled: true, imageSmoothingQuality: "low", miterLimit: 10, lineDashOffset: 0 });
    this.canvas = { width: sink.tctx.canvas.width, height: sink.tctx.canvas.height };
  }
  P(x, y) { const m = this.m; return [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]]; }
  get k() { const m = this.m; return Math.sqrt(Math.abs(m[0] * m[3] - m[1] * m[2])); }
  save() { this.stack.push({ m: [...this.m], dash: [...this.dash], s: STATE.map((k) => this[k]) }); }
  restore() { const s = this.stack.pop(); if (!s) return; this.m = s.m; this.dash = s.dash; STATE.forEach((k, i) => { this[k] = s.s[i]; }); }
  transform(a, b, c, d, e, f) { const m = this.m; this.m = [m[0] * a + m[2] * b, m[1] * a + m[3] * b, m[0] * c + m[2] * d, m[1] * c + m[3] * d, m[0] * e + m[2] * f + m[4], m[1] * e + m[3] * f + m[5]]; }
  translate(x, y) { this.transform(1, 0, 0, 1, x, y); }
  scale(a, b) { this.transform(a, 0, 0, b, 0, 0); }
  rotate(t) { const c = Math.cos(t), s = Math.sin(t); this.transform(c, s, -s, c, 0, 0); }
  setTransform(a = 1, b = 0, c = 0, d = 1, e = 0, f = 0) { if (typeof a === "object") ({ a, b, c, d, e, f } = a); this.m = [1, 0, 0, 1, this.sink.ox, this.sink.oy]; this.transform(a, b, c, d, e, f); }
  resetTransform() { this.setTransform(); }
  setLineDash(d) { this.dash = [...d]; }
  getLineDash() { return [...this.dash]; }
  // The path is kept in map units: each point taken through the transform as it is added.
  beginPath() { this.path = new Path2D(); }
  moveTo(x, y) { this.path.moveTo(...this.P(x, y)); }
  lineTo(x, y) { this.path.lineTo(...this.P(x, y)); }
  quadraticCurveTo(cx, cy, x, y) { this.path.quadraticCurveTo(...this.P(cx, cy), ...this.P(x, y)); }
  bezierCurveTo(ax, ay, bx, by, x, y) { this.path.bezierCurveTo(...this.P(ax, ay), ...this.P(bx, by), ...this.P(x, y)); }
  arc(x, y, r, a0, a1, ccw = false) { this.ellipse(x, y, r, r, 0, a0, a1, ccw); }
  ellipse(x, y, rx, ry, rot, a0, a1, ccw = false) {
    const [a, b, c, d] = this.m, k = this.k;
    // A turn and an even scale keep an arc an arc; anything else, it is drawn as short lines.
    if (Math.abs(a - d) < 1e-9 * (1 + k) && Math.abs(b + c) < 1e-9 * (1 + k)) return this.path.ellipse(...this.P(x, y), rx * k, ry * k, rot + Math.atan2(b, a), a0, a1, ccw);
    const T = 2 * Math.PI;
    let sweep = a1 - a0;
    if (!ccw) sweep = sweep >= T ? T : ((sweep % T) + T) % T;
    else sweep = -sweep >= T ? -T : -((((a0 - a1) % T) + T) % T);
    const steps = Math.max(8, Math.ceil((Math.abs(sweep) * Math.max(rx, ry) * k) / 1.5)), cr = Math.cos(rot), sr = Math.sin(rot);
    for (let i = 0; i <= steps; i++) {
      const t = a0 + (sweep * i) / steps, ex = rx * Math.cos(t), ey = ry * Math.sin(t);
      this.lineTo(x + ex * cr - ey * sr, y + ex * sr + ey * cr);
    }
  }
  rect(x, y, w, h) { this.moveTo(x, y); this.lineTo(x + w, y); this.lineTo(x + w, y + h); this.lineTo(x, y + h); this.closePath(); }
  closePath() { this.path.closePath(); }
  stroke() { this.paint("stroke"); }
  fill() { this.paint("fill"); }
  fillRect(x, y, w, h) { const kept = this.path; this.beginPath(); this.rect(x, y, w, h); this.paint("fill"); this.path = kept; }
  strokeRect(x, y, w, h) { const kept = this.path; this.beginPath(); this.rect(x, y, w, h); this.paint("stroke"); this.path = kept; }
  clearRect() {}
  clip() { this.sink.unknown.add("clip"); }
  measureText(t) { MEASURE.font = this.font; return MEASURE.measureText(t); }
  createLinearGradient() { return gradient(); }
  createRadialGradient() { return gradient(); }
  createPattern() { this.sink.unknown.add("createPattern"); return "#0000"; }
  drawImage(img, ...a) {
    const { tctx, ts, ox, oy } = this.sink, m = this.m;
    tctx.save();
    tctx.globalAlpha = this.globalAlpha;
    tctx.setTransform(ts * m[0], ts * m[1], ts * m[2], ts * m[3], ts * (m[4] - ox), ts * (m[5] - oy));
    tctx.imageSmoothingEnabled = true;
    tctx.drawImage(img, ...a);
    tctx.restore();
  }
  // One stroke or fill of the current path: into the wash if it is pale, or else into the one path of
  // its style.
  paint(how) {
    const { rgb: hex, a } = colour(how === "stroke" ? this.strokeStyle : this.fillStyle), alpha = a * this.globalAlpha;
    if (alpha <= 0) return;
    const pale = how === "fill" && oklch(hex)[0] >= 0.8;
    if (this.mode === "tint" || pale || this.globalCompositeOperation !== "source-over") return this.raster(how, hex, alpha);
    const lw = how === "stroke" ? this.lineWidth * this.k : 0, dash = how === "stroke" && this.dash.length ? this.dash.map((v) => v * this.k) : null;
    const key = [how, hex, alpha.toFixed(3), lw.toFixed(3), this.lineCap, this.lineJoin, dash?.map((v) => v.toFixed(2)).join(",")].join(" ");
    let op = this.sink.ink.get(key);
    if (!op) this.sink.ink.set(key, (op = { how, colour: `rgba(${rgb(hex).join(",")},${alpha})`, lw, cap: this.lineCap, join: this.lineJoin, dash, path: new Path2D() }));
    op.path.addPath(this.path);
  }
  raster(how, hex, alpha) {
    const { tctx, ts, ox, oy } = this.sink, c = `rgba(${rgb(hex).join(",")},${alpha})`;
    tctx.save();
    tctx.setTransform(ts, 0, 0, ts, -ox * ts, -oy * ts);
    tctx.globalCompositeOperation = this.globalCompositeOperation;
    if (how === "fill") { tctx.fillStyle = c; tctx.fill(this.path); }
    else { Object.assign(tctx, { strokeStyle: c, lineWidth: this.lineWidth * this.k, lineCap: this.lineCap, lineJoin: this.lineJoin }); tctx.setLineDash(this.dash.map((v) => v * this.k)); tctx.stroke(this.path); }
    tctx.restore();
  }
  fillText(t, x, y) { this.text(t, x, y, "fill"); }
  strokeText(t, x, y) { this.text(t, x, y, "stroke"); }
  // A word, left out whole if it would touch the graph's gap or where the chart has faded.
  text(t, x, y, how) {
    if (this.mode === "tint") return;
    const { rgb: hex, a } = colour(how === "fill" ? this.fillStyle : this.strokeStyle), [X, Y] = this.P(x, y), { calm } = this.sink;
    const f = /([\d.]+)px/.exec(this.font), em = (f ? +f[1] : 10) * this.k, wide = em * 0.6 * String(t).length;
    const left = { center: -wide / 2, right: -wide, end: -wide }[this.textAlign] ?? 0, top = { middle: -em / 2, top: 0, hanging: 0 }[this.textBaseline] ?? -em;
    for (const [dx, dy] of [[0, 0], [left, top], [left + wide, top], [left, top + em], [left + wide, top + em]]) if (!calm(X + dx, Y + dy) || hist(Y + dy) < 0.45) return;
    this.sink.texts.push({ text: String(t), m: [...this.m], x, y, font: this.font, align: this.textAlign, baseline: this.textBaseline, how, lw: this.lineWidth, colour: `rgba(${rgb(hex).join(",")},${a * this.globalAlpha})` });
  }
}
const MEASURE = new OffscreenCanvas(1, 1).getContext("2d");
// The recorder behind a proxy, so a member it lacks is noted and does nothing rather than throwing.
function recorder(r) {
  const bound = new Map();
  return new Proxy(r, {
    get(t, k) {
      if (k in t) { const v = t[k]; return typeof v === "function" ? (bound.get(k) ?? bound.set(k, v.bind(t)).get(k)) : v; }
      if (typeof k === "string") t.sink.unknown.add(k);
      return () => undefined;
    },
    set(t, k, v) { t[k] = v; return true; },
  });
}
function gradient() { const g = { stops: [], addColorStop(o, c) { g.stops.push([o, c]); } }; return g; }
// A canvas colour (hex, rgb(a), or a gradient, taken at its first stop) as a hex colour and an alpha.
function colour(style) {
  if (style && typeof style === "object") style = style.stops?.[0]?.[1] ?? "#000";
  const s = String(style).trim();
  const m = /^rgba?\(([^)]+)\)$/.exec(s);
  if (m) { const p = m[1].split(/[ ,/]+/).filter(Boolean).map(Number); return { rgb: toHex(p[0], p[1], p[2]), a: p[3] ?? 1 }; }
  if (s[0] === "#") {
    if (s.length === 4) return { rgb: "#" + [1, 2, 3].map((i) => s[i] + s[i]).join(""), a: 1 };
    if (s.length === 9) return { rgb: s.slice(0, 7), a: parseInt(s.slice(7), 16) / 255 };
    return { rgb: s.slice(0, 7), a: 1 };
  }
  MEASURE.fillStyle = "#000"; MEASURE.fillStyle = s;
  return colour(MEASURE.fillStyle);
}
const toHex = (r, g, b) => "#" + [r, g, b].map((v) => Math.round(v).toString(16).padStart(2, "0")).join("");
// A ring cut into the runs of points that keep(p); a ring kept whole stays closed, and a run that
// wraps past the ring's start is joined to the run it continues.
function runs(ring, keep) {
  const k = ring.map(keep);
  if (k.every(Boolean)) { const r = [...ring]; r.closed = true; return [r]; }
  const start = k.indexOf(false), out = [];
  let cur = [];
  for (let s = 1; s <= ring.length; s++) {
    const i = (start + s) % ring.length;
    if (k[i]) cur.push(ring[i]);
    else { if (cur.length > 1) out.push([ring[(i - cur.length - 1 + ring.length) % ring.length], ...cur, ring[i]]); cur = []; }
  }
  return out;
}

// ─── Geometry ───────────────────────────────────────────────────────────────────────────────────
// A 3x3 box blur, edges clamped.
function blur(f, nx, ny) {
  const out = new Float32Array(f.length);
  for (let j = 0; j < ny; j++) for (let i = 0; i < nx; i++) {
    let s = 0;
    for (let dj = -1; dj <= 1; dj++) { const jj = Math.min(ny - 1, Math.max(0, j + dj)); for (let di = -1; di <= 1; di++) s += f[jj * nx + Math.min(nx - 1, Math.max(0, i + di))]; }
    out[j * nx + i] = s / 9;
  }
  return out;
}
// Squared Euclidean distance (in cells) to the nearest seed: Felzenszwalb and Huttenlocher's two passes.
function edt(seeds, nx, ny) {
  const INF = 1e20, out = new Float32Array(nx * ny), n = Math.max(nx, ny);
  const f = new Float64Array(n), d = new Float64Array(n), v = new Int32Array(n), z = new Float64Array(n + 1);
  const pass = (len) => {
    let k = 0; v[0] = 0; z[0] = -INF; z[1] = INF;
    for (let q = 1; q < len; q++) {
      let s;
      while (true) { const p = v[k]; s = (f[q] + q * q - (f[p] + p * p)) / (2 * q - 2 * p); if (s <= z[k] && k > 0) k--; else break; }
      if (s <= z[k]) { v[0] = q; z[0] = -INF; z[1] = INF; k = 0; continue; }
      k++; v[k] = q; z[k] = s; z[k + 1] = INF;
    }
    k = 0;
    for (let q = 0; q < len; q++) { while (z[k + 1] < q) k++; const p = v[k]; d[q] = (q - p) * (q - p) + f[p]; }
  };
  for (let i = 0; i < nx; i++) { for (let j = 0; j < ny; j++) f[j] = seeds[j * nx + i] ? 0 : INF; pass(ny); for (let j = 0; j < ny; j++) out[j * nx + i] = d[j]; }
  for (let j = 0; j < ny; j++) { for (let i = 0; i < nx; i++) f[i] = out[j * nx + i]; pass(nx); for (let i = 0; i < nx; i++) out[j * nx + i] = d[i]; }
  return out;
}
// Marching squares at f = 0, chained into closed rings of map points.
function contour(f, nx, ny, gx, gy) {
  const H = nx * ny, pts = new Map(), adj = new Map();
  const at = (e) => {
    if (pts.has(e)) return pts.get(e);
    let p;
    if (e < H) { const j = Math.floor(e / nx), i = e % nx, a = f[j * nx + i], b = f[j * nx + i + 1], t = a / (a - b); p = [gx(i + t), gy(j)]; }
    else { const e2 = e - H, j = Math.floor(e2 / nx), i = e2 % nx, a = f[j * nx + i], b = f[(j + 1) * nx + i], t = a / (a - b); p = [gx(i), gy(j + t)]; }
    pts.set(e, p);
    return p;
  };
  const link = (a, b) => { (adj.get(a) ?? adj.set(a, []).get(a)).push(b); (adj.get(b) ?? adj.set(b, []).get(b)).push(a); };
  const PAIRS = { 1: [[0, 1]], 2: [[1, 2]], 3: [[0, 2]], 4: [[3, 2]], 5: [[0, 3], [1, 2]], 6: [[3, 1]], 7: [[0, 3]], 8: [[0, 3]], 9: [[3, 1]], 10: [[0, 1], [3, 2]], 11: [[3, 2]], 12: [[0, 2]], 13: [[1, 2]], 14: [[0, 1]] };
  for (let j = 0; j < ny - 1; j++) for (let i = 0; i < nx - 1; i++) {
    const code = (f[j * nx + i] > 0 ? 8 : 0) | (f[j * nx + i + 1] > 0 ? 4 : 0) | (f[(j + 1) * nx + i + 1] > 0 ? 2 : 0) | (f[(j + 1) * nx + i] > 0 ? 1 : 0);
    if (code === 0 || code === 15) continue;
    // The cell's edges: left, bottom, right, top.
    const e = [H + j * nx + i, (j + 1) * nx + i, H + j * nx + i + 1, j * nx + i];
    for (const [p, q] of PAIRS[code]) link(e[p], e[q]);
  }
  const used = new Set(), rings = [];
  for (const start of adj.keys()) {
    if (used.has(start)) continue;
    const ring = [];
    let prev = null, cur = start;
    while (cur !== undefined && !used.has(cur)) {
      used.add(cur); ring.push(at(cur));
      const next = (adj.get(cur) || []).find((e) => e !== prev && !used.has(e));
      prev = cur; cur = next;
    }
    if (ring.length > 4) rings.push(ring);
  }
  return rings;
}
// Chaikin's corner cutting, once round a closed ring.
function smooth(r) {
  const out = [];
  for (let i = 0; i < r.length; i++) {
    const [ax, ay] = r[i], [bx, by] = r[(i + 1) % r.length];
    out.push([0.75 * ax + 0.25 * bx, 0.75 * ay + 0.25 * by], [0.25 * ax + 0.75 * bx, 0.25 * ay + 0.75 * by]);
  }
  return out;
}
