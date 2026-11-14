// The map's areas as territories, each drawn as its theme's chart: what regions-paint.js draws for
// regions.js, off the page's thread.
//
// What it depicts: an old atlas, hand-tinted. Every area is a territory, the land within `reach` of
// its nodes, names and streams; where two areas come near each other the land between is shared out
// down the middle, so the areas make one landmass with one coast, cut into territories by seams that
// run to the coast. Each is tinted in a faded watercolour wash of its theme's hue (`map.hue`), darker
// where the wash dried at its edge, grainy where the pigment settled, and paler up its history; the
// land hides the map's dot grid. Inside it, its theme draws its subject as a map in one ink
// (`chart(ctx, scene)`): Harbour's as a nautical chart, Tussock's as a topographic sheet. The chart is
// fullest round the present row and fades up the history, and clears a narrow margin round every
// stream, node and name, as a map clears its linework round a label. Up its history a territory is
// its wash, its border and its name, lettered in spaced capitals.
//
// Technique. One distance field per area on a grid of `cell` map units (Felzenszwalb's transform,
// seeded with its node and name boxes and the points of its streams), wobbled by low noise of its own.
// Each area claims max(reach - d, (2 reach + corridor - d - d_other) / 2): the highest claim is the
// land, and its zero line the coast. Each area's cell is (d_other - d) / 2, positive where it is the
// nearest; the cell's edge inside the land is a seam, the same midline traced from either side. A
// territory is its cell clipped by the land, so a seam meets the coast in a clean T. The whole map is drawn at once, since every border is shared.
// A chart draws on a recording context: every stroke,
// dark fill and letter becomes SVG in map units, so the browser draws it sharp at every zoom and
// nothing is drawn again as the camera moves; every pale fill and picture goes to the area's wash, a
// small picture at `tintScale` pixels per map unit. Each stroke is cut into pieces by how quiet the
// map is where it runs, a few levels of ink, and the pieces of one style become one SVG path, so an
// area is a few dozen paths however much its chart draws.
import { noise, rng, fromOklch, oklch, kit, simplify } from "./theme.js";

const LOOK = {
  reach: 170,      // map units of land round an area's nodes, names and streams
  corridor: 150,   // land narrower than 2 * reach + this between two areas is shared: one border, no bay
  cell: 8,         // the field's grid, in map units
  wander: 30,      // how far a border wanders from the true midline, in map units
  crinkle: 12,     // and how far a coast crinkles, in bays and headlands a few dozen units across,
  jitter: 4,       // and finer still
  smooth: 2,       // passes of a 3x3 blur over each field, which rounds the corners where claims meet
  clear: 34,       // no border nearer a node or a name than this
  bridge: 40,      // a sliver of paper narrower than about twice this between two territories is closed
  ribbon: [2.5, 4, 0.22], // the band of tint just inside the border: inset, width, opacity
  coast: 1.15,     // the coast's line, in map units; a seam is lighter and finer
  seam: 0.8,
  waterlines: [[5, 0.34], [11, 0.22], [19, 0.12]], // the lines round the coast, out on the paper, as an old chart draws its shores: distance and strength
  wash: 0.55,      // the wash's body, as opacity over the map's ground
  wet: 0.42,       // and its dried edge, at the coast, fading over `wetIn` map units inside it
  wetIn: 7,
  grain: 0.22,     // how much the pigment's grain moves the wash's opacity
  letter: 15, letterEvery: 1500, // the area's name: its size, and how often up its history
  rise: 2400,      // how far up its history the chart reaches: full ink to 30 units above the now line, half by 360, gone over its last 600
  nameAbove: 540,  // the area's name is lettered at least this far above the now line, where the chart is quiet
  ink: 0.8,        // the charts' ink, as a share of what they draw
  tintScale: 0.5,  // the wash, in pixels per map unit
  levels: [0.22, 0.42, 0.62, 0.82, 1], // the ink levels a stroke's pieces are sorted into
};
// An area's hue as its wash's body and dried edge, the band inside its border, and its lettering.
const inks = (hue) => ({ fill: fromOklch(0.965, 0.013, hue), wet: fromOklch(0.83, 0.03, hue), band: fromOklch(0.8, 0.04, hue), line: fromOklch(0.5, 0.035, hue) });
// The coast, the seams and the water lines belong to the whole map: one neutral ink.
const COAST = fromOklch(0.46, 0.014, 80), SEAM = fromOklch(0.56, 0.012, 80);
const ss = (a, b, t) => { t = Math.min(1, Math.max(0, (t - a) / (b - a))); return t * t * (3 - 2 * t); };
const f1 = (v) => (Math.round(v * 10) / 10).toString();
const f3 = (v) => (Math.round(v * 1000) / 1000).toString();
const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const attrs = (o) => Object.entries(o).filter(([, v]) => v != null).map(([k, v]) => ` ${k}="${esc(v)}"`).join("");

// areas: [{ id, theme (the module), seed, name, marks: [[x0, y0, x1, y1]] }], each mark a node's or a
// name's box in map units, the now line at y = 0 and history above it. lines: the map's lines, each
// [[x, y], ...]. paper: the map's ground. Returns, for each area with land, { id, box: { x, y, w, h },
// svg (an <svg> whose units are map units, for an element at box), tint (an OffscreenCanvas of its
// wash, stretched over box) }, and `unknown`: the context members a chart used that the recorder lacks.
export function territories(areas, lines, paper, look = LOOK) {
  const { reach, cell } = look;
  let X0 = Infinity, Y0 = Infinity, X1 = -Infinity, Y1 = -Infinity;
  for (const a of areas) for (const [x0, y0, x1, y1] of a.marks) { X0 = Math.min(X0, x0); Y0 = Math.min(Y0, y0); X1 = Math.max(X1, x1); Y1 = Math.max(Y1, y1); }
  if (X0 === Infinity) return { areas: [], unknown: [] };
  // Room round the marks for every claim: a shared corridor reaches reach + corridor / 2 out, a bridge
  // lifts it further, and the wobble moves it about.
  const pad = reach + look.corridor / 2 + look.bridge + 1.8 * look.wander + 1.5 * look.crinkle + look.waterlines.at(-1)[0] + 3 * cell;
  X0 = Math.floor((X0 - pad) / cell) * cell; Y0 = Math.floor((Y0 - pad) / cell) * cell;
  const nx = Math.ceil((X1 + pad - X0) / cell) + 1, ny = Math.ceil((Y1 + pad - Y0) / cell) + 1, N = nx * ny;
  const gx = (i) => X0 + i * cell, gy = (j) => Y0 + j * cell;
  const cellOf = (x, y) => { const i = Math.round((x - X0) / cell), j = Math.round((y - Y0) / cell); return i >= 0 && j >= 0 && i < nx && j < ny ? j * nx + i : -1; };
  const markSeeds = (a) => {
    const seeds = new Uint8Array(N);
    for (const [x0, y0, x1, y1] of a.marks)
      for (let j = Math.max(0, Math.floor((y0 - Y0) / cell)); j <= Math.min(ny - 1, Math.ceil((y1 - Y0) / cell)); j++)
        for (let i = Math.max(0, Math.floor((x0 - X0) / cell)); i <= Math.min(nx - 1, Math.ceil((x1 - X0) / cell)); i++) seeds[j * nx + i] = 1;
    return seeds;
  };
  // Each area's true distance to its nodes and names: for the clearing round them, and to give each
  // stream's points to the area whose nodes are nearest.
  const raw = areas.map((a) => edt(markSeeds(a), nx, ny).map((v) => Math.sqrt(v) * cell));
  const owner = (c) => { let k = 0; for (let m = 1; m < areas.length; m++) if (raw[m][c] < raw[k][c]) k = m; return k; };
  const lineSeeds = new Uint8Array(N), own = areas.map(() => []);
  for (const [x, y] of along(lines, cell)) { const c = cellOf(x, y); if (c >= 0) { lineSeeds[c] = 1; own[owner(c)].push(c); } }
  const lineD = edt(lineSeeds, nx, ny).map((v) => Math.sqrt(v) * cell);
  const near = new Float32Array(N).fill(Infinity);
  for (const r of raw) for (let c = 0; c < N; c++) if (r[c] < near[c]) near[c] = r[c];

  // Each area's distance to its nodes, names and streams, wobbled by its own noise; and its wash's
  // bloom, the slow unevenness of a wash laid by hand, on the same coarse grid.
  const C = 4, cx = Math.ceil(nx / C) + 2, cy = Math.ceil(ny / C) + 2;
  const coarse = (w) => (i, j) => {
    const fi = i / C, fj = j / C, i0 = Math.floor(fi), j0 = Math.floor(fj), u = fi - i0, v = fj - j0;
    return (w[j0 * cx + i0] * (1 - u) + w[j0 * cx + i0 + 1] * u) * (1 - v) + (w[(j0 + 1) * cx + i0] * (1 - u) + w[(j0 + 1) * cx + i0 + 1] * u) * v;
  };
  const blooms = [];
  const dist = areas.map((a, k) => {
    const seeds = markSeeds(a);
    for (const c of own[k]) seeds[c] = 1;
    const d = edt(seeds, nx, ny);
    const n = noise(`atlas:${a.seed}`), w = new Float32Array(cx * cy), b = new Float32Array(cx * cy);
    for (let j = 0; j < cy; j++) for (let i = 0; i < cx; i++) { w[j * cx + i] = n.fbm(gx(i * C) * 0.0035, gy(j * C) * 0.0035, 3); b[j * cx + i] = n.fbm(gx(i * C) * 0.006 + 31, gy(j * C) * 0.006 - 7, 3); }
    const wob = coarse(w);
    blooms.push(coarse(b));
    for (let j = 0; j < ny; j++) for (let i = 0; i < nx; i++) {
      const dd = Math.sqrt(d[j * nx + i]) * cell;
      // The coast's crinkle, only where a coast can run: it is the dearest noise here.
      const shoreward = ss(reach + 130, reach + 80, dd);
      d[j * nx + i] = dd + look.wander * wob(i, j) + (shoreward > 0 ? shoreward * (look.crinkle * n.fbm(gx(i) / 70, gy(j) / 70, 2) + look.jitter * n.n2(gx(i) / 24, gy(j) / 24)) : 0);
    }
    return d;
  });
  // A clearing round every node and name: no border comes nearer than `clear`.
  for (let c = 0; c < N; c++) {
    let k = -1, best = look.clear;
    for (let m = 0; m < areas.length; m++) if (raw[m][c] < best) { best = raw[m][c]; k = m; }
    if (k < 0) continue;
    for (let m = 0; m < areas.length; m++) if (m !== k && dist[m][c] < dist[k][c] + 2 * (look.clear - best)) dist[m][c] = dist[k][c] + 2 * (look.clear - best);
  }
  // Each area's claim on the land: its own reach, or the corridor it shares with its nearest
  // neighbour; and its cell, how much nearer it is than that neighbour, halved (the midline between
  // them is the seam). Each smoothed, which rounds the corners where claims meet.
  const claims = [], cells = [], across = areas.map(() => new Int16Array(N));
  areas.forEach((_, k) => {
    let claim = new Float32Array(N), cellK = new Float32Array(N);
    const dk = dist[k];
    for (let c = 0; c < N; c++) {
      let other = Infinity, om = -1;
      for (let m = 0; m < areas.length; m++) if (m !== k && dist[m][c] < other) { other = dist[m][c]; om = m; }
      across[k][c] = om;
      claim[c] = other === Infinity ? reach - dk[c] : Math.max(reach - dk[c], (2 * reach + look.corridor - dk[c] - other) / 2);
      cellK[c] = other === Infinity ? 1e6 : (other - dk[c]) / 2;
    }
    for (let p = 0; p < look.smooth; p++) { claim = blur(claim, nx, ny); cellK = blur(cellK, nx, ny); }
    claims.push(claim); cells.push(cellK);
  });
  // Sea at the grid's edge, so every line traced closes.
  const sea = (f) => { for (let i = 0; i < nx; i++) for (let j = 0; j < ny; j++) if (!i || !j || i === nx - 1 || j === ny - 1) f[j * nx + i] = Math.min(f[j * nx + i], -1); return f; };
  // All the land, one landmass: positive inside the coast, negative out on the paper. Where two
  // claims nearly meet, the land is lifted, so no sliver of paper is left between them.
  const land = new Float32Array(N);
  for (let c = 0; c < N; c++) {
    let a = -Infinity, b = -Infinity;
    for (const f of claims) { const v = f[c]; if (v > a) { b = a; a = v; } else if (v > b) b = v; }
    land[c] = a + look.bridge * ss(-look.bridge, 0, b);
  }
  sea(land);
  cells.forEach(sea);
  // Each territory: how far inside it, to its coast or to a seam, which it shares exactly with its
  // neighbour (both are traced from the same midline), so a seam is one line.
  const part = cells.map((ck) => sea(ck.map((v, c) => Math.min(v, land[c]))));
  const trace = (f) => contour(f, nx, ny, gx, gy).map((r) => simplify(smooth(smooth(smooth(r))), 0.15));
  // The water lines round the coast, each run drawn by the area whose cell it lies in.
  const shores = look.waterlines.map(([off, op]) => [trace(sea(land.map((v) => v + off))), op]);
  const sample = (g, x, y) => {
    const fi = Math.min(nx - 1.001, Math.max(0, (x - X0) / cell)), fj = Math.min(ny - 1.001, Math.max(0, (y - Y0) / cell)), i = Math.floor(fi), j = Math.floor(fj), u = fi - i, v = fj - j, c = j * nx + i;
    return (g[c] * (1 - u) + g[c + 1] * u) * (1 - v) + (g[c + nx] * (1 - u) + g[c + nx + 1] * u) * v;
  };
  // How much ink the map lets a chart put down: none in a narrow margin round every stream, node and
  // name, all of it a few units out, and, up its history, fading out.
  const calm = (x, y) => ss(2.5, 6, sample(lineD, x, y)) * ss(4, 9, sample(near, x, y));
  const hist = (y) => (y > -30 ? 1 : 0.5 + 0.5 * ss(-360, -30, y)) * ss(-look.rise, -look.rise + 600, y);
  const grain = grainTile();

  const unknown = new Set(), out = [], ground = rgb(paper);
  const border = { fill: "none", "stroke-linejoin": "round", "stroke-linecap": "round" };
  areas.forEach((a, k) => {
    const c = inks(a.theme.map?.hue ?? 90), fk = part[k], ck = cells[k];
    // Its box: its territory, and a margin.
    let i0 = nx, i1 = -1, j0 = ny, j1 = -1;
    for (let j = 0; j < ny; j++) for (let i = 0; i < nx; i++) if (land[j * nx + i] > -1 && ck[j * nx + i] > -1) { i0 = Math.min(i0, i); i1 = Math.max(i1, i); j0 = Math.min(j0, j); j1 = Math.max(j1, j); }
    if (i1 < 0) return;
    const box = { x: gx(i0) - 2 * cell, y: gy(j0) - 2 * cell, w: (i1 - i0 + 4) * cell, h: (j1 - j0 + 4) * cell };
    const inBox = (x, y) => Math.min(x - box.x, box.x + box.w - x, y - box.y, box.y + box.h - y);
    const boxF = new Float32Array(N);
    for (let j = 0; j < ny; j++) for (let i = 0; i < nx; i++) boxF[j * nx + i] = inBox(gx(i), gy(j));
    // The land and its cell within the box, as rings: the territory is the one clipped by the other.
    const shore = trace(land.map((v, q) => Math.min(v, boxF[q]))), cellRings = trace(ck.map((v, q) => Math.min(v, boxF[q])));
    if (!shore.length || !cellRings.length) return;
    const id = String(a.id).replace(/[^\w-]/g, "_"), L = `land-${id}`, T = `cell-${id}`;
    const svg = [`<svg xmlns="http://www.w3.org/2000/svg" viewBox="${box.x} ${box.y} ${box.w} ${box.h}" width="${box.w}" height="${box.h}" style="position: absolute; left: 0; top: 0; overflow: visible; pointer-events: none">`,
      `<defs><clipPath id="${L}"><path clip-rule="evenodd" d="${rings2d(shore)}"/></clipPath><clipPath id="${T}"><path clip-rule="evenodd" d="${rings2d(cellRings)}"/></clipPath></defs>`,
      `<g clip-path="url(#${L})"><g clip-path="url(#${T})">`];

    // Its wash, laid over the map's ground so the land hides the dot grid: the body faintly uneven,
    // darker where it dried at its border, grainy, paler up its history, and stopping a little short
    // of the border here and there, as a hand-laid wash does. It reaches a little past its seams, under
    // its neighbour's, so no ground shows between them.
    const ts = Math.min(look.tintScale, 4096 / box.w, 4096 / box.h), W = Math.max(1, Math.ceil(box.w * ts)), H = Math.max(1, Math.ceil(box.h * ts));
    const tint = new OffscreenCanvas(W, H), tctx = tint.getContext("2d"), img = tctx.createImageData(W, H), px = img.data;
    const fill = rgb(c.fill), wet = rgb(c.wet), bloom = blooms[k], n = noise(`wash:${a.seed}`);
    for (let j = 0, i = 0; j < H; j++) for (let q = 0; q < W; q++, i += 4) {
      const x = box.x + (q + 0.5) / ts, y = box.y + (j + 0.5) / ts;
      const cover = ss(-0.8, 0.8, sample(land, x, y)) * ss(-0.8, 0.8, sample(ck, x, y) + 3);
      if (cover <= 0) continue;
      const f = sample(fk, x, y), short = f < 6 ? 1.2 + 1.2 * n.n2(x / 40, y / 40) : 0, inside = ss(-0.8, 0.8, f - short);
      const body = look.wash * (0.82 + 0.18 * bloom((x - X0) / cell, (y - Y0) / cell)), edge = look.wet * Math.exp(-Math.max(0, f - short) / look.wetIn);
      const A = Math.min(1, (body + edge * (1 - body)) * (1 + look.grain * grain[(q & 63) + 64 * (j & 63)]) * (0.55 + 0.45 * ss(-1800, -150, y)) * inside);
      const e = edge / (body + edge);
      for (let ch = 0; ch < 3; ch++) px[i + ch] = Math.round(ground[ch] * (1 - A) + (fill[ch] * (1 - e) + wet[ch] * e) * A);
      px[i + 3] = Math.round(255 * cover);
    }
    tctx.putImageData(img, 0, 0);

    // The band of tint inside the border.
    const [inset, width, op] = look.ribbon;
    const inner = trace(fk.map((v) => v - inset));
    if (inner.length) svg.push(`<path${attrs({ d: rings2d(inner), fill: "none", stroke: c.band, "stroke-width": width, "stroke-opacity": op, "stroke-linejoin": "round" })}/>`);

    // Its chart, in the box from `rise` above the now line to the territory's foot, clipped to it.
    const cx0 = box.x, cy0 = Math.max(box.y, -look.rise), cw = box.w, ch = box.y + box.h - cy0;
    if (a.theme.chart && cw > 60 && ch > 60) {
      const under = new OffscreenCanvas(Math.ceil(cw * ts), Math.ceil(ch * ts)), uctx = under.getContext("2d");
      const sink = { ox: cx0, oy: cy0, paths: new Map(), texts: [], tctx: uctx, ts, quiet: (x, y) => calm(x, y) * hist(y), levels: look.levels, ink: look.ink, unknown };
      const ctx = recorder(new Rec("ink", sink)), tinted = recorder(new Rec("tint", sink)), seed = `${a.theme.name}:${a.seed}:chart`;
      a.theme.chart(ctx, {
        w: cw, h: ch, now: -cy0, scale: 1, paper: mix(paper, c.fill, look.wash),
        mass: (x, y) => ss(0, 24, sample(fk, x + cx0, y + cy0)),
        calm: (x, y) => calm(x + cx0, y + cy0),
        n: noise(seed), stream: (p) => rng(`${seed}:${p}`), under: tinted, kit,
      });
      for (const [style, d] of sink.paths) svg.push(`<path d="${d}"${style}/>`);
      svg.push(...sink.texts);
      // The chart's own tints, inside the territory, fading up its history faster than its ink, and
      // clear of the nodes and names as its ink is.
      const mask = uctx.createImageData(under.width, under.height), md = mask.data;
      for (let j = 0, i = 0; j < under.height; j++) for (let q = 0; q < under.width; q++, i += 4) {
        const x = cx0 + (q + 0.5) / ts, y = cy0 + (j + 0.5) / ts;
        md[i + 3] = Math.round(255 * ss(-1.5, 1.5, sample(fk, x, y)) * hist(y) ** 2 * ss(4, 9, sample(near, x, y)));
      }
      const mc = new OffscreenCanvas(under.width, under.height);
      mc.getContext("2d").putImageData(mask, 0, 0);
      uctx.setTransform(1, 0, 0, 1, 0, 0);
      uctx.globalCompositeOperation = "destination-in";
      uctx.drawImage(mc, 0, 0);
      tctx.drawImage(under, (cx0 - box.x) * ts, (cy0 - box.y) * ts);
    }
    svg.push("</g>");

    // Its seams, each drawn once, by the territory of the two listed later, within the land so each
    // ends on the coast; then its stretches of the coast, and of the water lines off it.
    const mine = (x, y) => inBox(x, y) > 1 && sample(ck, x, y) >= 0;
    const seams = cellRings.flatMap((r) => runs(r, ([x, y]) => { const q = cellOf(x, y); return inBox(x, y) > 1 && sample(land, x, y) > -3 && q >= 0 && across[k][q] < k; }));
    if (seams.length) svg.push(`<path${attrs({ d: runs2d(seams), stroke: SEAM, "stroke-width": look.seam, ...border })}/>`);
    svg.push("</g>");
    const coast = shore.flatMap((r) => runs(r, ([x, y]) => mine(x, y)));
    if (coast.length) svg.push(`<path${attrs({ d: runs2d(coast), stroke: COAST, "stroke-width": look.coast, ...border })}/>`);
    for (const [rings, op] of shores) {
      const lines = rings.flatMap((r) => runs(r, ([x, y]) => sample(ck, x, y) >= 0));
      if (lines.length) svg.push(`<path${attrs({ d: runs2d(lines), stroke: mix(paper, COAST, op * 1.25), "stroke-width": 0.6, ...border })}/>`);
    }

    // Its name, lettered up its history where it is clear of the streams and the border: above the
    // chart first; an area too short or narrow for that takes the clearest place it has.
    if (a.name && look.letter) {
      const size = look.letter, half = (a.name.length * size * 1.25) / 2 + size;
      let placed = 0;
      for (const [above, room0] of [[look.nameAbove, 26], [60, 18]]) {
        if (placed) break;
        const clear = (i, j) => { const c2 = j * nx + i; return dist[k][c2] > room0 && fk[c2] > 16 && lineD[c2] > 14; };
        let lastY = Infinity;
        for (let j = ny - 1; j >= 0; j--) {
          const y = gy(j);
          if (lastY - y < look.letterEvery || y > -above) continue;
          let best = null;
          for (let i = 0; i < nx; i++) {
            if (!clear(i, j)) continue;
            let ok = true;
            for (let dj = -Math.ceil(half / cell); dj <= Math.ceil(half / cell) && ok; dj++) { const jj = j + dj; if (jj < 0 || jj >= ny || !clear(i, jj)) ok = false; }
            if (!ok) continue;
            const room = Math.min(dist[k][j * nx + i], fk[j * nx + i] * 1.6);
            if (!best || room > best.room) best = { i, room };
          }
          if (!best) continue;
          svg.push(`<text${attrs({ transform: `translate(${f1(gx(best.i))} ${f1(y)}) rotate(-90)`, "text-anchor": "middle", "dominant-baseline": "central", fill: c.line, "fill-opacity": 0.5, "font-size": size, "letter-spacing": size * 0.42, "font-family": "Literata, Georgia, serif", "font-weight": 500 })}>${esc(a.name.toUpperCase())}</text>`);
          lastY = y;
          placed++;
        }
      }
    }
    svg.push("</svg>");
    out.push({ id: a.id, box, svg: svg.join(""), tint });
  });
  return { areas: out, unknown: [...unknown] };
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

// The pigment's grain: a 64-pixel tile of softened noise in [-1, 1], the same for every wash.
let tile = null;
function grainTile() {
  if (tile) return tile;
  const r = rng("wash grain"), raw = new Float32Array(64 * 64).map(() => r() * 2 - 1);
  tile = new Float32Array(64 * 64);
  for (let j = 0; j < 64; j++) for (let i = 0; i < 64; i++) {
    let s = 0;
    for (const [di, dj, w] of [[0, 0, 0.5], [1, 0, 0.125], [-1, 0, 0.125], [0, 1, 0.125], [0, -1, 0.125]]) s += w * raw[((j + dj) & 63) * 64 + ((i + di) & 63)];
    tile[j * 64 + i] = s * 1.6;
  }
  return tile;
}

// ─── The recording context ────────────────────────────────────────────────────────────────────────
// A 2D context a chart draws on as on a canvas, in its box's units. "ink" turns strokes, dark fills
// and letters into SVG in map units, each stroke cut into pieces by the quiet where they run; pale fills
// and pictures go to the wash. "tint" sends everything to the wash. Members it lacks are noted
// (`unknown`) and do nothing.
const STATE = ["strokeStyle", "fillStyle", "lineWidth", "lineCap", "lineJoin", "font", "textAlign", "textBaseline", "globalAlpha", "globalCompositeOperation"];
class Rec {
  constructor(mode, sink) {
    Object.assign(this, { mode, sink, m: [1, 0, 0, 1, sink.ox, sink.oy], stack: [], subs: [], cur: null, dash: [] });
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
  beginPath() { this.subs = []; this.cur = null; }
  moveTo(x, y) { const p = this.P(x, y); this.cur = { start: p, at: p, segs: [] }; this.subs.push(this.cur); }
  lineTo(x, y) { if (!this.cur) return this.moveTo(x, y); const p = this.P(x, y); this.cur.segs.push(["L", p]); this.cur.at = p; }
  quadraticCurveTo(cx, cy, x, y) { if (!this.cur) this.moveTo(cx, cy); const c = this.P(cx, cy), p = this.P(x, y); this.cur.segs.push(["Q", p, c]); this.cur.at = p; }
  bezierCurveTo(ax, ay, bx, by, x, y) {
    if (!this.cur) this.moveTo(ax, ay);
    const [sx, sy] = apply(invert(this.m), this.cur.at);
    for (let i = 1; i <= 10; i++) { const t = i / 10, u = 1 - t; this.lineTo(u * u * u * sx + 3 * u * u * t * ax + 3 * u * t * t * bx + t * t * t * x, u * u * u * sy + 3 * u * u * t * ay + 3 * u * t * t * by + t * t * t * y); }
  }
  arc(x, y, r, a0, a1, ccw = false) { this.ellipse(x, y, r, r, 0, a0, a1, ccw); }
  ellipse(x, y, rx, ry, rot, a0, a1, ccw = false) {
    let sweep = a1 - a0;
    const T = 2 * Math.PI;
    if (!ccw) sweep = sweep >= T ? T : ((sweep % T) + T) % T;
    else sweep = -sweep >= T ? -T : -((((a0 - a1) % T) + T) % T);
    const steps = Math.max(8, Math.ceil((Math.abs(sweep) * Math.max(rx, ry) * this.k) / 1.5));
    const cr = Math.cos(rot), sr = Math.sin(rot);
    for (let i = 0; i <= steps; i++) {
      const t = a0 + (sweep * i) / steps, ex = rx * Math.cos(t), ey = ry * Math.sin(t), px = x + ex * cr - ey * sr, py = y + ex * sr + ey * cr;
      if (i === 0 && !this.cur) this.moveTo(px, py); else this.lineTo(px, py);
    }
  }
  rect(x, y, w, h) { this.moveTo(x, y); this.lineTo(x + w, y); this.lineTo(x + w, y + h); this.lineTo(x, y + h); this.closePath(); }
  closePath() { if (!this.cur) return; const s = this.cur.start; this.cur.segs.push(["L", s]); this.cur = { start: s, at: s, segs: [] }; this.subs.push(this.cur); }
  stroke() { this.paint("stroke"); }
  fill() { this.paint("fill"); }
  fillRect(x, y, w, h) { const saved = [this.subs, this.cur]; this.beginPath(); this.rect(x, y, w, h); this.paint("fill"); [this.subs, this.cur] = saved; }
  strokeRect(x, y, w, h) { const saved = [this.subs, this.cur]; this.beginPath(); this.rect(x, y, w, h); this.paint("stroke"); [this.subs, this.cur] = saved; }
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
  // One stroke or fill of the current path: into the wash if it is pale, or else into the SVG path
  // of its style and its level of ink.
  paint(how) {
    const style = how === "stroke" ? this.strokeStyle : this.fillStyle, { rgb, a } = colour(style), alpha = a * this.globalAlpha;
    if (alpha <= 0 || !this.subs.length) return;
    const pale = how === "fill" && oklch(rgb)[0] >= 0.8;
    if (this.mode === "tint" || pale || this.globalCompositeOperation !== "source-over") return this.raster(how, rgb, alpha);
    const { quiet, levels, ink } = this.sink;
    if (how === "fill") {
      // Each shape at the quiet of where it lies, so one call filling many small shapes (dots, leaves)
      // thins each where it is rather than all of them at their mean.
      const shapes = levels.map(() => "");
      for (const s of this.subs) {
        if (!s.segs.length) continue;
        let d = move(s.start), prev = s.start, sx = 0, sy = 0;
        for (const g of s.segs) { d += seg(g, prev); prev = g[1]; sx += g[1][0]; sy += g[1][1]; }
        const lv = level(quiet(sx / s.segs.length, sy / s.segs.length), levels);
        if (lv >= 0) shapes[lv] += d + "Z";
      }
      shapes.forEach((d, lv) => { if (d) this.put(attrs({ fill: rgb, "fill-opacity": f3(alpha * levels[lv] * ink) }), d); });
      return;
    }
    const buckets = levels.map(() => ({ d: "", last: null }));
    for (const s of this.subs) {
      let prev = s.start;
      for (const g of thin(s)) {
        const end = g[1], mx = g[0] === "Q" ? (prev[0] + 2 * g[2][0] + end[0]) / 4 : (prev[0] + end[0]) / 2, my = g[0] === "Q" ? (prev[1] + 2 * g[2][1] + end[1]) / 4 : (prev[1] + end[1]) / 2;
        const lv = level(quiet(mx, my), levels);
        if (lv >= 0) {
          const b = buckets[lv];
          if (!b.last || b.last[0] !== prev[0] || b.last[1] !== prev[1]) b.d += move(prev);
          b.d += seg(g, prev); b.last = end;
        }
        prev = end;
      }
    }
    const dash = this.dash.length ? this.dash.map((v) => f3(v * this.k)).join(" ") : null;
    buckets.forEach((b, lv) => {
      if (b.d) this.put(attrs({ fill: "none", stroke: rgb, "stroke-opacity": f3(alpha * levels[lv] * ink), "stroke-width": f3(this.lineWidth * this.k), "stroke-linecap": this.lineCap, "stroke-linejoin": this.lineJoin, "stroke-dasharray": dash }), b.d);
    });
  }
  // A piece of path, added to the one path of its style.
  put(style, d) { const p = this.sink.paths; p.set(style, (p.get(style) ?? "") + d); }
  raster(how, rgb, alpha) {
    const { tctx, ts, ox, oy } = this.sink;
    tctx.save();
    tctx.setTransform(ts, 0, 0, ts, -ox * ts, -oy * ts);
    tctx.globalCompositeOperation = this.globalCompositeOperation;
    tctx.beginPath();
    for (const s of this.subs) { tctx.moveTo(...s.start); for (const g of s.segs) g[0] === "Q" ? tctx.quadraticCurveTo(...g[2], ...g[1]) : tctx.lineTo(...g[1]); }
    const c = `rgba(${hexRgb(rgb).join(",")},${alpha})`;
    if (how === "fill") { tctx.fillStyle = c; tctx.fill(); }
    else { tctx.strokeStyle = c; tctx.lineWidth = this.lineWidth * this.k; tctx.lineCap = this.lineCap; tctx.lineJoin = this.lineJoin; tctx.setLineDash(this.dash.map((v) => v * this.k)); tctx.stroke(); }
    tctx.restore();
  }
  fillText(t, x, y) { this.text(t, x, y, "fill"); }
  strokeText(t, x, y) { this.text(t, x, y, "stroke"); }
  text(t, x, y, how) {
    const { rgb, a } = colour(how === "fill" ? this.fillStyle : this.strokeStyle), [X, Y] = this.P(x, y), { quiet, levels, ink } = this.sink;
    const q = quiet(X, Y);
    if (q < 0.45 || this.mode === "tint") return;
    const f = /([\d.]+)px\s+(.+)$/.exec(this.font) || [0, "10", "sans-serif"];
    const weight = /\b(\d{3}|bold)\b/.exec(this.font)?.[1];
    const look = { x: f1(X), y: f1(Y), "font-size": f3(+f[1] * this.k), "font-family": f[2], "font-weight": weight, "text-anchor": { center: "middle", right: "end", end: "end" }[this.textAlign], "dominant-baseline": { middle: "central", top: "hanging", hanging: "hanging", bottom: "text-after-edge" }[this.textBaseline] };
    if (how === "fill") Object.assign(look, { fill: rgb, "fill-opacity": f3(a * this.globalAlpha * levels[level(q, levels)] * ink) });
    else Object.assign(look, { fill: "none", stroke: rgb, "stroke-opacity": f3(a * this.globalAlpha), "stroke-width": f3(this.lineWidth * this.k), "stroke-linejoin": "round" });
    this.sink.texts.push(`<text${attrs(look)}>${esc(t)}</text>`);
  }
}
const MEASURE = new OffscreenCanvas(1, 1).getContext("2d");
// The recorder behind a proxy, so a member it lacks is noted and does nothing rather than throwing.
function recorder(r) {
  return new Proxy(r, {
    get(t, k) {
      if (k in t) { const v = t[k]; return typeof v === "function" ? v.bind(t) : v; }
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
const hexRgb = (h) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
const rgb = hexRgb;
const mix = (a, b, t) => toHex(...hexRgb(a).map((v, i) => v + (hexRgb(b)[i] - v) * t));
const level = (q, levels) => { if (q < 0.1) return -1; let best = 0; for (let i = 1; i < levels.length; i++) if (Math.abs(levels[i] - q) < Math.abs(levels[best] - q)) best = i; return best; };
// Path data, short: each piece starts at an absolute point and goes on by steps from it, each step
// taken between rounded points so that the rounding never adds up along a line.
const r1 = (v) => Math.round(v * 10) / 10;
const num = (v) => r1(v).toString().replace(/^(-?)0\./, "$1.");
const nums = (vs) => vs.map(num).reduce((d, t, i) => d + (i && t[0] !== "-" ? " " : "") + t, "");
const move = (p) => "M" + nums(p);
const step = (to, from) => [r1(to[0]) - r1(from[0]), r1(to[1]) - r1(from[1])];
const seg = (g, from) => (g[0] === "Q" ? "q" + nums([...step(g[2], from), ...step(g[1], from)]) : "l" + nums(step(g[1], from)));
const line = (r) => move(r[0]) + r.slice(1).map((p, i) => "l" + nums(step(p, r[i]))).join("");
const invert = ([a, b, c, d, e, f]) => { const k = a * d - b * c; return [d / k, -b / k, -c / k, a / k, (c * f - d * e) / k, (b * e - a * f) / k]; };
const apply = (m, [x, y]) => [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];
const rings2d = (rings) => rings.map((r) => line(r) + "Z").join("");
const runs2d = (list) => list.map((r) => line(r) + (r.closed ? "Z" : "")).join("");
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
// Ramer-Douglas-Peucker: drop the points within `tol` map units of the line through their neighbours.
// A stroke's segments with its nearly straight runs of lines thinned, a stretch of at most `span` map
// units at a time, so a thinned segment is never long enough to cross a clearing at one level of ink.
function thin(s, tol = 0.12, span = 10) {
  const out = [];
  let run = [s.start], length = 0;
  const flush = () => { if (run.length > 1) for (const p of simplify(run, tol).slice(1)) out.push(["L", p]); };
  for (const g of s.segs) {
    if (g[0] !== "L") { flush(); out.push(g); run = [g[1]]; length = 0; continue; }
    length += Math.hypot(g[1][0] - run.at(-1)[0], g[1][1] - run.at(-1)[1]);
    run.push(g[1]);
    if (length > span) { flush(); run = [g[1]]; length = 0; }
  }
  flush();
  return out;
}
