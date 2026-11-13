// Tussock: Canterbury and Mackenzie high country in a low sun, seen across.
//
// What it depicts. A pale sky where the lead sits; snow ranges far off in blue-violet, their
// sun-facing faces warm with alpenglow; sometimes a glacial lake, milky turquoise, at their foot
// with the range mirrored in it; an ochre foothill stepping forward through haze; then golden
// tussock, from a far stipple to near clumps, each a fountain of fine blades bent by one wind,
// their tips lit by the sun, some of them copper (red tussock), all casting long shadows away from
// it; soft cloud shadows drifting over the land.
//
// Identity and layout. The agent's place `at` (among the agents wearing Tussock, planned by the
// harness) picks its light from six on a ring: late-afternoon gold, red tussock under an apricot
// evening, rose dusk, lilac after sunset, a winter blue hour, a silver-green morning. Agents
// on screen together wear lights far apart, so they are told apart at a glance;
// `identity(at)` is that light's colour, for the agent's map node and accents. The seed (the
// agent's id) chooses the layout, each part from its own stream: how high the horizon sits under
// the lead, the form of the ranges (one great massif, an uneven chain, or two massifs framing the
// sky), the lake and its shore (long low spits, a shingle line that comes and goes), the foothills,
// where the sun is (low beside the ranges, or below the horizon or off the frame with only its
// glow), and how thick the clumps grow and where they mass. The sun's disc always keeps well clear
// of the lead, and under the lead the ranges are scaled down, keeping their shape, so the sky stays
// calm there; the mass and fine detail sit at the foot and the corners.
//
// Technique. A shin-hanga woodblock (Hiroshi Yoshida, Kawase Hasui): crisp key-block silhouettes,
// each plane filled with bokashi (a graded wipe of colour); inside the ranges and the foothill a
// shaded relief in a print's few soft-edged tones, which dissolves at its foot into the land
// behind it, so no plane ends in a ruled line; sky wood grain, and grain over all. The land is one
// continuous plane under everything. Far tussock is a stipple of short round-capped dabs; near
// tussock blades are quadratic strokes; both are batched by width or length and tone, so thousands
// cost a few dozen draw calls.
//
// Text sits on calm ground.


const TAU = Math.PI * 2;
const clamp = (x, a, b) => (x < a ? a : x > b ? b : x);
const lerp = (a, b, t) => a + (b - a) * t;
const smooth = (a, b, x) => { const t = clamp((x - a) / (b - a), 0, 1); return t * t * (3 - 2 * t); };
const hex = (c) => { const n = parseInt(c.slice(1), 16); return [(n >> 16) & 255, (n >> 8) & 255, n & 255]; };
const mixc = (a, b, t) => { const A = hex(a), B = hex(b); return "#" + A.map((x, i) => Math.round(x + (B[i] - x) * t).toString(16).padStart(2, "0")).join(""); };
const rgba = (c, a) => { const [r, g, b] = hex(c); return `rgba(${r},${g},${b},${a})`; };

// The agent's light: six lights round a ring, and an agent's place `at` picks its place on the
// ring, so agents on screen together wear lights far apart:
// late-afternoon gold, red tussock under an amber evening, rose dusk, the lilac after sunset, the
// blue hour in winter, and a silver-green morning. Each light colours the whole picture, sky, snow
// and tussock alike, so two agents are told apart at a glance and each is still Tussock. `id` is the
// colour the picture is named by (its map node and accents), `red` the share of red tussock, `disc`
// whether the sun is up, and `glowA` how strong its glow is.
const GOLD = {
  zenith: "#a6c0cf", upper: "#e2e5da", mid: "#f7eacd", low: "#f8d69c", sun: "#fff4d6", glow: "#fbd892",
  haze: "#efdcbf", far: "#a3abcb", farShadow: "#8a90b6", snowLit: "#fdf0e0", snowShadow: "#cbd0e4",
  near: "#8d8fae", nearShadow: "#767a9d", hill1: "#d2a86c", hill2: "#bb8a4c", hill3: "#9d6c35",
  plain: "#bd8f40", plainFar: "#d4ae6a", ground: "#7a5726", core: "#4e3417", mid2: "#a06d2c", tip: "#f6d48a", tipLit: "#fff1c8", shade: "#7a5626", straw: "#d9bf86",
  cCore: "#4a2414", cShade: "#7d3f1f", cMid: "#a75a2c", cTip: "#e9a46a",
  cloudShade: "#6e5a52", lake: "#7fbcb8", lakeFar: "#c3d6c8", shingle: "#c9c2ae",
  id: "#c08a2c", red: 0.15, disc: 1, glowA: 1,
};
const RUST = {
  zenith: "#b4b8c8", upper: "#ecd6c4", mid: "#f8d4b0", low: "#f2b07c", sun: "#fff0d6", glow: "#f6b277",
  haze: "#edcfb2", far: "#aaa1bc", farShadow: "#8e85a9", snowLit: "#fde9d6", snowShadow: "#d0c6da",
  near: "#958a9e", nearShadow: "#7b708d", hill1: "#c98a5a", hill2: "#a8653c", hill3: "#86492a",
  plain: "#a5552e", plainFar: "#c7825a", ground: "#5c2e18", core: "#45200f", mid2: "#9b522c", tip: "#eba068", tipLit: "#ffe0bc", shade: "#73361c", straw: "#d9a878",
  cCore: "#3d170d", cShade: "#6f2a18", cMid: "#9f4226", cTip: "#e88a5e",
  cloudShade: "#5d4040", lake: "#7aaeb0", lakeFar: "#d2d2c4", shingle: "#c6b8a8",
  id: "#a8522b", red: 0.85, disc: 1, glowA: 1,
};
const ROSE = {
  zenith: "#aca5c8", upper: "#e6d6df", mid: "#f7ddd8", low: "#f2b9a8", sun: "#fff0e6", glow: "#f2a28e",
  haze: "#eccdc8", far: "#ab9fc4", farShadow: "#8f84af", snowLit: "#fde1df", snowShadow: "#cfc5e0",
  near: "#9184a8", nearShadow: "#796d95", hill1: "#cf9a86", hill2: "#b17866", hill3: "#8f5a4c",
  plain: "#bb8264", plainFar: "#d6a892", ground: "#6c4234", core: "#482822", mid2: "#9f644e", tip: "#f2bea0", tipLit: "#ffe6da", shade: "#764636", straw: "#dcb3a0",
  cCore: "#44201c", cShade: "#78342e", cMid: "#a44c46", cTip: "#ec988a",
  cloudShade: "#5f4c5e", lake: "#93a9be", lakeFar: "#dccbd2", shingle: "#cdbcbc",
  id: "#c06e62", red: 0.3, disc: 1, glowA: 0.95,
};
const LILAC = {
  zenith: "#9d9dc6", upper: "#dad5e8", mid: "#ecdfe8", low: "#e9c6cf", sun: "#fbeef0", glow: "#dcaac6",
  haze: "#ddcdda", far: "#9b95c2", farShadow: "#7f79ac", snowLit: "#f4e8f3", snowShadow: "#beb9dc",
  near: "#817aa0", nearShadow: "#69638c", hill1: "#ab8c9a", hill2: "#8e6e7f", hill3: "#705264",
  plain: "#a08078", plainFar: "#bfa2a3", ground: "#53373c", core: "#3b2530", mid2: "#87606b", tip: "#dab4b8", tipLit: "#f6e2ec", shade: "#62414c", straw: "#c9aeb6",
  cCore: "#391d26", cShade: "#672f40", cMid: "#8e495a", cTip: "#d68b9c",
  cloudShade: "#4e4560", lake: "#9099c4", lakeFar: "#d4cde2", shingle: "#bdb4c4",
  id: "#8d6fa8", red: 0.2, disc: 0, glowA: 0.7,
};
const BLUE = {
  zenith: "#87a2c8", upper: "#c8d5e8", mid: "#e1e7ef", low: "#eedac8", sun: "#f8f3ec", glow: "#cdd8ee",
  haze: "#d6dde8", far: "#8fa3c9", farShadow: "#7085b3", snowLit: "#f0f4fb", snowShadow: "#b6c5e2",
  near: "#7b8db1", nearShadow: "#63749b", hill1: "#aba494", hill2: "#8f897e", hill3: "#716d68",
  plain: "#a99a78", plainFar: "#c1b9a8", ground: "#4e4a46", core: "#34333a", mid2: "#88806b", tip: "#ded5bd", tipLit: "#f4f1e8", shade: "#615c55", straw: "#ccc5b3",
  cCore: "#3a2a2a", cShade: "#694842", cMid: "#8e685a", cTip: "#c9a090",
  cloudShade: "#4b5470", lake: "#8fb0cf", lakeFar: "#d0dbe7", shingle: "#b8bcc4",
  id: "#5d7eb2", red: 0.1, disc: 0, glowA: 0.55,
};
const SAGE = {
  zenith: "#93adc2", upper: "#d6e1e1", mid: "#eef1e6", low: "#efead2", sun: "#fffdf2", glow: "#eef0c8",
  haze: "#e2e7dc", far: "#98a8c2", farShadow: "#7a8cae", snowLit: "#ffffff", snowShadow: "#c8d4e4",
  near: "#8796a8", nearShadow: "#6c7c92", hill1: "#bdb790", hill2: "#9f9a72", hill3: "#7f7c56",
  plain: "#aaa374", plainFar: "#c8c4a2", ground: "#5e5a3c", core: "#3d3b26", mid2: "#8b8659", tip: "#e4e0bd", tipLit: "#f9f9e8", shade: "#65613f", straw: "#d6d2b2",
  cCore: "#4a3018", cShade: "#7a5028", cMid: "#a07040", cTip: "#d8b080",
  cloudShade: "#55605a", lake: "#72b4b0", lakeFar: "#c5ddd5", shingle: "#c2c6bc",
  id: "#8c9460", red: 0.12, disc: 1, glowA: 0.9,
};
const RING = [GOLD, RUST, ROSE, LILAC, BLUE, SAGE];
function lightAt(u) {
  const x = (((u % 1) + 1) % 1) * RING.length, i = Math.floor(x), t = smooth(0, 1, x - i);
  const A = RING[i], B = RING[(i + 1) % RING.length], P = {};
  for (const k of Object.keys(GOLD)) P[k] = typeof A[k] === "number" ? lerp(A[k], B[k], t) : mixc(A[k], B[k], t);
  return P;
}

// A ridgeline: y for every x step, ridged noise under an envelope of a few summits. Under the
// lead the whole range is scaled down, keeping its shape, so no summit reaches the text.
function ridgeline(noise, w, step, base, amp, peaks, quiet, floorY, off, sharp) {
  const xs = [], hs = [], ins = [];
  for (let x = -step; x <= w + step; x += step) {
    let env = 0.22;
    for (const p of peaks) env = Math.max(env, p.a * Math.exp(-((x - p.x) ** 2) / (2 * p.s * p.s)));
    let r = 0, a = 1, f = 1 / 260, norm = 0;
    for (let o = 0; o < 5; o++) {
      const n = 1 - Math.abs(noise.n2(x * f + off, off * 0.37 + o * 11.1));
      r += a * n ** sharp; norm += a; a *= 0.45; f *= 2.1;
    }
    r /= norm;
    xs.push(x); hs.push(amp * env * (0.5 + 0.5 * r));
    ins.push(smooth(quiet.x - 160, quiet.x + 30, x) * (1 - smooth(quiet.x + quiet.w - 30, quiet.x + quiet.w + 160, x)));
  }
  const cap = Math.max(4, base - floorY);
  let top = 0;
  for (let i = 0; i < hs.length; i++) if (ins[i] > 0.5) top = Math.max(top, hs[i]);
  const k = top > cap ? cap / top : 1;
  return xs.map((x, i) => [x, base - hs[i] * lerp(1, k, ins[i])]);
}

function tracePath(ctx, pts, bottom, w) {
  ctx.beginPath();
  ctx.moveTo(-10, bottom);
  ctx.lineTo(pts[0][0], pts[0][1]);
  for (let i = 1; i < pts.length; i++) ctx.lineTo(pts[i][0], pts[i][1]);
  ctx.lineTo(w + 10, bottom);
  ctx.closePath();
}

// A relief inside a crisp silhouette. The face below the crest is modelled as the crest's profile
// blurred more the further down it goes, so each summit spreads into a lit flank and a shadowed one,
// as real mountains do. Gully streaks cut it, snow lies above a ragged snowline, and the light
// rakes in from the sun's side. A print's few tones with soft edges, dissolving into haze toward
// the foot, and with `foot` set, fading out entirely there. Depth below the crest is measured from
// a smoothed crest, so its teeth don't stripe the face.
function relief(ctx, { w, pts, base, noise, sunDir, cell, tones, snowAlt, haze, hazeFrom, off, spread, rough = 1, edge = 1, foot = 0 }) {
  let top = Infinity;
  for (const [, y] of pts) top = Math.min(top, y);
  top = Math.floor(top) - 2;
  const H = Math.ceil(base - top) + 2;
  if (H <= 2) return;
  const cw = Math.ceil(w / cell) + 2, ch = Math.ceil(H / cell) + 1;
  const img = new ImageData(cw, ch), d = img.data;
  // Crest height above the base, sampled per cell column.
  const step = pts[1][0] - pts[0][0];
  const crest = new Float32Array(cw);
  for (let i = 0; i < cw; i++) {
    const x = i * cell, k = clamp((x - pts[0][0]) / step, 0, pts.length - 1.001), k0 = Math.floor(k), t = k - k0;
    crest[i] = base - lerp(pts[k0][1], pts[k0 + 1][1], t);
  }
  const pre = new Float64Array(cw + 1);
  for (let i = 0; i < cw; i++) pre[i + 1] = pre[i] + crest[i];
  const blur = (i, rad) => { const a = Math.max(0, i - rad), b = Math.min(cw - 1, i + rad); return (pre[b + 1] - pre[a]) / (b - a + 1); };
  const C = tones.map(hex), hz = hex(haze);
  // Depth is measured from a smoothed crest, so the blur grows smoothly across the face and the
  // crest's small teeth don't run down it as stripes.
  const soft = new Float32Array(cw);
  for (let i = 0; i < cw; i++) soft[i] = blur(i, Math.round(24 / cell));
  const row = new Float32Array(cw);
  for (let j = 0; j < ch; j++) {
    const y = top + j * cell, alt = base - y;
    for (let i = 0; i < cw; i++) row[i] = blur(i, Math.round((6 + Math.max(0, soft[i] - alt) * spread) / cell));
    for (let i = 0; i < cw; i++) {
      const k = (j * cw + i) * 4;
      if (alt > crest[i] + cell) { d[k + 3] = 0; continue; }
      const x = i * cell, depth = Math.max(0, soft[i] - alt);
      const slope = (row[Math.min(cw - 1, i + 1)] - row[Math.max(0, i - 1)]) / (2 * cell);
      const gully = noise.n2(x / 22 + off, depth / 30 + off), gully2 = noise.n2(x / 70 + 4, depth / 70 - off);
      const shade = slope * sunDir * 1.6 + (gully * 0.05 + gully2 * 0.12) * rough;
      // A print's few tones, but with soft edges between them rather than stairs.
      const snowLine = snowAlt == null ? Infinity : snowAlt + gully * 10 + gully2 * 22 - slope * sunDir * 10;
      const snow = snowAlt == null ? 0 : smooth(-3, 3, alt - snowLine);
      const sLit = smooth(-0.07 * edge, 0.0, shade), rLow = smooth(-0.14 - 0.06 * edge, -0.08, shade), rHigh = smooth(0.05, 0.05 + 0.1 * edge, shade);
      const f = smooth(hazeFrom, 1, alt < 0 ? 1 : 1 - alt / Math.max(1, soft[i])) * 0.9;
      for (let q = 0; q < 3; q++) {
        const rock = lerp(lerp(C[4][q], C[3][q], rLow), C[2][q], rHigh), ice = lerp(C[1][q], C[0][q], sLit);
        d[k + q] = lerp(lerp(rock, ice, snow), hz[q], f);
      }
      // Over its last `foot` pixels the plane dissolves into what lies behind it.
      d[k + 3] = foot > 0 ? Math.round(255 * smooth(-2, foot, alt)) : 255;
    }
  }
  const tile = document.createElement("canvas");
  tile.width = cw; tile.height = ch;
  tile.getContext("2d").putImageData(img, 0, 0);
  ctx.save();
  tracePath(ctx, pts, base + 4, w);
  ctx.clip();
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(tile, 0, top, cw * cell, ch * cell);
  ctx.restore();
}

// Tussock clumps in one depth tier, batched: every blade is cut into three lengths and each length
// goes in one path per tone, so a tier is a handful of strokes however many blades it has.
function clumps(ctx, list, { wind, sunX, tone, scale, noise, shadow, shadowOnly }) {
  const T = [[], [], []], L = [[], [], []];
  const cores = [];
  for (const c of list) {
    const S = c.s, n = Math.round(clamp(S * 1.3, 14, 150));
    cores.push(c);
    for (let b = 0; b < n; b++) {
      const r = c.r;
      const spread = r.gauss() * 0.42;
      const a0 = -Math.PI / 2 + spread;
      const len = S * (0.55 + 0.45 * r()) * (1 - 0.35 * Math.abs(spread));
      const bx = c.x + spread * S * 0.22 + r.gauss() * S * 0.05, by = c.y + r() * S * 0.06;
      const flow = noise.n2(c.x / 180, c.y / 180) * 0.35;
      const lean = (wind + flow) * len * (0.55 + 0.3 * r());
      const droop = len * (0.15 + 0.5 * Math.abs(spread)) * (0.6 + 0.4 * r());
      const dx = Math.cos(a0) * len, dy = Math.sin(a0) * len;
      const at = (t) => [bx + dx * t + lean * t * t, by + dy * t + droop * t * t * t];
      // A blade that leans toward the sun catches it.
      const lit = (bx + lean - c.x) * Math.sign(sunX - c.x) > 0 || r() < 0.25;
      const ts = [0, 0.4, 0.72, 1];
      for (let k = 0; k < 3; k++) {
        const p0 = at(ts[k]), pm = at((ts[k] + ts[k + 1]) / 2), p1 = at(ts[k + 1]);
        const cx = 2 * pm[0] - (p0[0] + p1[0]) / 2, cy = 2 * pm[1] - (p0[1] + p1[1]) / 2;
        (lit ? L : T)[k].push(p0[0], p0[1], cx, cy, p1[0], p1[1]);
      }
    }
  }
  // Long shadows thrown away from the low sun, flattened by the view.
  if (shadow) {
    ctx.fillStyle = shadow;
    ctx.beginPath();
    for (const c of cores) {
      const away = -Math.sign(sunX - c.x) || 1, len = c.s * (1.1 + 0.9 * Math.min(1, Math.abs(sunX - c.x) / 900));
      ctx.moveTo(c.x + away * len * 1.05 + len * 0.55, c.y + c.s * 0.03);
      ctx.ellipse(c.x + away * len * 0.5, c.y + c.s * 0.03, len * 0.55, c.s * 0.085, 0, 0, TAU);
    }
    ctx.fill();
    if (shadowOnly) return;
  }
  // Each clump's dark core first, so the blades rise out of shadow.
  ctx.fillStyle = tone.core;
  ctx.beginPath();
  for (const c of cores) { ctx.moveTo(c.x + c.s * 0.3, c.y); ctx.ellipse(c.x, c.y, c.s * 0.3, c.s * 0.12, 0, 0, TAU); }
  ctx.fill();
  const widths = [1.5, 1.0, 0.6].map((x) => x * scale);
  const shadowCols = [tone.core, tone.shade, tone.mid];
  const litCols = [tone.shade, tone.mid, tone.tip];
  ctx.lineCap = "round";
  for (let k = 0; k < 3; k++) {
    for (const [set, col] of [[T[k], shadowCols[k]], [L[k], litCols[k]]]) {
      ctx.strokeStyle = col;
      ctx.lineWidth = widths[k];
      ctx.beginPath();
      for (let i = 0; i < set.length; i += 6) { ctx.moveTo(set[i], set[i + 1]); ctx.quadraticCurveTo(set[i + 2], set[i + 3], set[i + 4], set[i + 5]); }
      ctx.stroke();
    }
  }
  // The brightest tips: the last few pixels of lit blades, in the sun's colour.
  ctx.strokeStyle = tone.tipLit;
  ctx.lineWidth = widths[2] * 0.8;
  ctx.globalAlpha = 0.85;
  ctx.beginPath();
  const tips = L[2];
  for (let i = 0; i < tips.length; i += 6) { ctx.moveTo(tips[i + 2], tips[i + 3]); ctx.lineTo(tips[i + 4], tips[i + 5]); }
  ctx.stroke();
  ctx.globalAlpha = 1;
}

// Scatter clumps in a band, sized by depth, as dense as dens(x, y) says, never overlapping much.
function scatter(r, w, y0, y1, sMin, sMax, count, dens) {
  const out = [];
  for (let tries = 0; out.length < count && tries < count * 8; tries++) {
    const x = r.range(-0.05, 1.05) * w, t = r(), y = lerp(y0, y1, t);
    if (r() > dens(x, y)) continue;
    const s = lerp(sMin, sMax, t) * (0.7 + 0.5 * r());
    if (out.some((o) => Math.abs(o.x - x) < (o.s + s) * 0.3 && Math.abs(o.y - y) < (o.s + s) * 0.1)) continue;
    out.push({ x, y, s, r: r.fork(`${x}`) });
  }
  return out.sort((a, b) => a.y - b.y);
}

// Tussock seen from far off: flat dark dabs (each clump's shadow) of every size and shape, in
// patches, with a fleck of light above some of them. Each dab is a short round-capped stroke (an
// ellipse at this size, far cheaper to build), sorted into a few widths, so it all takes five paths.
const DAB_W = [0.8, 1.5, 2.6, 4.4];
function stipple(ctx, r, noise, { x0, x1, y0, y1, n, sizeAt, dark, light, off, wind }) {
  const flecks = [], dabs = DAB_W.map(() => []);
  for (let i = 0; i < n; i++) {
    const x = lerp(x0, x1, r()), y = lerp(y0, y1, r());
    if (noise.fbm(x / 150 + off, y / 60 - off, 2) < r.range(-0.5, 0.15)) continue;
    const s = sizeAt(y) * (0.45 + r() * r() * 1.6), e = s * r.range(0.22, 0.45), tilt = r.range(-0.12, 0.12);
    const k = e < 0.55 ? 0 : e < 1 ? 1 : e < 1.7 ? 2 : 3, half = Math.max(0.3, s - DAB_W[k] / 2);
    dabs[k].push(x - half, y - half * tilt, x + half, y + half * tilt);
    if (r() < 0.6) flecks.push(x + r.range(-0.4, 0.4) * s, y - e * 1.1, s);
  }
  ctx.strokeStyle = dark;
  ctx.lineCap = "round";
  dabs.forEach((d, k) => {
    ctx.lineWidth = DAB_W[k];
    ctx.beginPath();
    for (let i = 0; i < d.length; i += 4) { ctx.moveTo(d[i], d[i + 1]); ctx.lineTo(d[i + 2], d[i + 3]); }
    ctx.stroke();
  });
  ctx.strokeStyle = light;
  ctx.lineWidth = 0.7;
  ctx.beginPath();
  for (let i = 0; i < flecks.length; i += 3) {
    const x = flecks[i], y = flecks[i + 1], l = flecks[i + 2] * r.range(0.6, 1.4);
    ctx.moveTo(x, y); ctx.lineTo(x + wind * l * r.range(0.2, 0.9), y - l * r.range(0.4, 1));
  }
  ctx.stroke();
}

export default {
  name: "tussock",
  about: "Golden tussock under snow ranges, in a low sun.",
  display: { family: "Fraunces", files: ["font-fraunces.woff2"], weights: "100 900", bold: 640, axes: '"SOFT" 100, "opsz" 72' },
  palette: {
    paper: "#faf4e6", tint: "#fdf9f0", ink: "#2c1f0d", body: "#574329", muted: "#937c58", line: "#ecdcb9",
    accent: "#9a5b13", figure: ["#a8691c", "#5f6a9a", "#6f7d4a", "#8a5a6e"], cover_ink: "#2c1f0d", cover_body: "#574329",
  },
  // The six lights round the ring, each next one as far round from those before as the ring allows.
  slots: [0, 3 / 6, 1 / 6, 4 / 6, 2 / 6, 5 / 6],
  // The colour this agent is told apart by: the colour its light is named by.
  identity(at) { return lightAt(at).id; },
  paint(ctx, { w, h, at, stream, noise, quiet, kit }) {
    // Without a lead (a thumbnail, a swatch), keep the upper sky clear as if one sat there.
    quiet ||= { x: w / 2, y: 0, w: 0, h: h * 0.42 };
    const portrait = h > w, m = Math.min(w, h);
    const qb = quiet.y + quiet.h, qx0 = quiet.x, qx1 = quiet.x + quiet.w;

    // The light comes from the agent's place; everything else, from its seed, a stream per part.
    const P = lightAt(at);
    const fr = stream("frame");
    const side = fr() < 0.5 ? -1 : 1;
    const wind = (fr() < 0.7 ? -side : side) * fr.range(0.25, 0.55);
    // The horizon sits under the lead, closer or further: a big sky or a big land.
    const H0 = clamp(qb + h * fr.range(portrait ? 0.05 : 0.04, portrait ? 0.14 : 0.2), h * 0.5, h * (portrait ? 0.8 : 0.78));
    const space = h - H0;
    const off = fr() * 100;
    const step = Math.max(2, w / 600);

    // Sky: bokashi from a pale steel zenith to the warm low sky.
    const sky = ctx.createLinearGradient(0, 0, 0, H0);
    sky.addColorStop(0, P.zenith); sky.addColorStop(0.2, P.upper); sky.addColorStop(0.55, P.mid); sky.addColorStop(1, P.low);
    ctx.fillStyle = sky;
    ctx.fillRect(0, 0, w, h);

    // The sun: low on one side, well clear of the lead. Where there is no room for it (a phone's
    // lead spans the width), it is off the frame or under the horizon, and only its glow shows.
    const sr = m * 0.035 + 8;
    const gap = Math.max(110, m * 0.16) + sr;
    // Clear means well away from the lead, and beside its lines only when far to the side, so the
    // disc never sits where the eye stops reading.
    const clear = (x, y) => {
      const dx = Math.max(qx0 - x, 0, x - qx1), dy = Math.max(quiet.y - y, 0, y - qb);
      return Math.hypot(dx, dy) >= gap && (y > qb + sr || dx >= w * 0.2);
    };
    // After sunset (the lilac and the blue hour) it is under the horizon on its side.
    const sn = stream("sun");
    let sunX = 0, sunY = 0, found = false;
    if (P.disc < 0.5) { sunX = w * (side < 0 ? sn.range(0.05, 0.35) : sn.range(0.65, 0.95)); sunY = H0 + sr * 2; found = true; }
    else if (sn() < 0.82) {
      for (let i = 0; i < 16 && !found; i++) {
        sunX = w * (side < 0 ? sn.range(0.03, 0.3) : sn.range(0.7, 0.97));
        sunY = lerp(H0 + sr * 0.3, quiet.y + quiet.h * 0.3, sn() ** 1.7);
        found = clear(sunX, sunY);
      }
    }
    if (!found) { sunX = side < 0 ? -w * 0.05 : w * 1.05; sunY = Math.max(H0 - space * 0.08, qb + gap); }
    const sunDir = Math.sign(sunX - w / 2) || 1;
    const glowR = Math.max(w, h) * 0.7;
    let g = ctx.createRadialGradient(sunX, sunY, 0, sunX, sunY, glowR);
    g.addColorStop(0, rgba(P.glow, 0.95 * P.glowA)); g.addColorStop(0.12, rgba(P.glow, 0.6 * P.glowA)); g.addColorStop(0.4, rgba(P.glow, 0.16 * P.glowA)); g.addColorStop(1, rgba(P.glow, 0));
    ctx.fillStyle = g;
    ctx.fillRect(0, 0, w, h);
    // Its light spreads sideways along the haze at the horizon.
    ctx.save();
    ctx.translate(sunX, Math.min(sunY, H0) + space * 0.04);
    ctx.scale(1, 0.12);
    g = ctx.createRadialGradient(0, 0, 0, 0, 0, w * 0.9);
    g.addColorStop(0, rgba(P.sun, 0.75 * P.glowA)); g.addColorStop(0.3, rgba(P.glow, 0.35 * P.glowA)); g.addColorStop(1, rgba(P.glow, 0));
    ctx.fillStyle = g;
    ctx.fillRect(-w * 2, -w, w * 4, w * 2);
    ctx.restore();

    // Wood grain in the sky, as the block prints it: long faint streaks.
    const r = stream("sky");
    ctx.lineWidth = 1;
    for (let i = 0; i < Math.round(H0 / 3); i++) {
      const y0 = r() * H0, a = 0.018 + 0.03 * r();
      ctx.strokeStyle = rgba(r() < 0.5 ? P.upper : P.low, a * 1.6);
      ctx.beginPath();
      const ph = r() * 10, len = w * r.range(0.3, 1.2), x0 = r.range(-0.2, 1) * w;
      for (let x = x0; x < x0 + len; x += 24) ctx.lineTo(x, y0 + Math.sin(x / 300 + ph) * 3 + noise.n2(x / 400, y0 / 50) * 4);
      ctx.stroke();
    }

    // Long soft cloud bands, lit from below, low in the sky.
    for (let i = 0; i < 5; i++) {
      const cy = lerp(qb + 4, H0 - space * 0.04, r() ** 0.7), cx = r() * w, cw = w * r.range(0.2, 0.55), ch = r.range(3, 9);
      ctx.save();
      ctx.translate(cx, cy);
      ctx.scale(1, ch / cw);
      const cg = ctx.createRadialGradient(0, 0, 0, 0, 0, cw);
      cg.addColorStop(0, rgba(P.sun, 0.5)); cg.addColorStop(0.5, rgba(P.sun, 0.2)); cg.addColorStop(1, rgba(P.sun, 0));
      ctx.fillStyle = cg;
      ctx.fillRect(-cw, -cw, cw * 2, cw * 2);
      ctx.restore();
    }

    // The sun's disc, half lost in haze; the ranges and the land hide it when it sits low.
    g = ctx.createRadialGradient(sunX, sunY, 0, sunX, sunY, sr);
    g.addColorStop(0, rgba(P.sun, 1)); g.addColorStop(0.6, rgba(P.sun, 0.92)); g.addColorStop(1, rgba(P.sun, 0));
    ctx.fillStyle = g;
    ctx.beginPath();
    ctx.arc(sunX, sunY, sr, 0, TAU);
    ctx.fill();

    // The far snow range: one great massif, a long serrated chain, or two massifs framing the sky.
    const rg = stream("range");
    const form = rg.pick(["massif", "chain", "twin"]);
    const wide = portrait ? 2 : 1, edgeX = () => (rg() < 0.5 ? rg.range(0.02, 0.17) : rg.range(0.83, 0.98));
    let peaks, sharp;
    if (form === "massif") {
      peaks = [{ x: edgeX() * w, a: 1, s: w * rg.range(0.06, 0.1) * wide }, { x: rg() * w, a: rg.range(0.3, 0.5), s: w * 0.12 * wide }];
      sharp = 2.2;
    } else if (form === "chain") {
      // Uneven spacing and heights, and summits in clusters, so the chain never reads as a saw.
      const n = portrait ? 3 : rg.int(4, 7);
      peaks = [];
      let x = rg.range(-0.05, 0.12);
      for (let i = 0; i < n; i++) {
        peaks.push({ x: x * w, a: rg.range(0.35, 0.95) ** 0.8, s: w * rg.range(0.03, 0.08) * wide });
        x += (rg() < 0.35 ? rg.range(0.04, 0.08) : rg.range(0.14, 0.3)) * (portrait ? 2 : 1);
      }
      sharp = 2.6;
    } else {
      peaks = [{ x: rg.range(0.02, 0.18) * w, a: rg.range(0.8, 1), s: w * rg.range(0.08, 0.13) * wide }, { x: rg.range(0.82, 0.98) * w, a: rg.range(0.8, 1), s: w * rg.range(0.08, 0.13) * wide }];
      sharp = 2.0;
    }
    const farAmp = (portrait ? space * 0.45 : Math.max(space * 0.5, H0 - qb + h * 0.1)) * rg.range(0.8, 1.12) * (form === "massif" ? 1.15 : form === "chain" ? 0.8 : 1);
    const floorY = qb + 10;
    const far = ridgeline(noise, w, step, H0, farAmp, peaks, quiet, floorY, off, sharp);
    relief(ctx, { w, pts: far, base: H0, noise, sunDir, cell: 1.5, off, spread: 0.9, hazeFrom: 0.35, haze: P.haze, rough: 0.45, edge: 0.4,
      tones: [P.snowLit, P.snowShadow, P.far, mixc(P.far, P.farShadow, 0.5), P.farShadow], snowAlt: farAmp * 0.42 });

    // Sometimes a glacial lake at the range's foot, milky turquoise, with the range in it.
    const lk = stream("lake");
    const lake = lk() < 0.45;
    const lakeH = lake ? space * lk.range(0.09, 0.17) : 0, slant = lk.range(-0.6, 0.6);
    // The near shore, seen low across the plain: nearly level, ragged at every scale, slanting so
    // the water narrows toward one side, and broken by a spit or two of land: long and low, as
    // perspective flattens them, tapering out into the water on one side.
    const points = Array.from({ length: lk.int(1, 2) }, () => ({ x: lk() * w, s: w * lk.range(0.06, 0.16), a: lk.range(0.16, 0.36), k: lk.pick([lk.range(0.3, 0.6), lk.range(1.7, 3.3)]) }));
    const shoreAt = (x) => {
      let f = 0.74 + 0.2 * noise.fbm(x / 120 + off, 3.3, 5, 2.1, 0.6) + slant * (x / w - 0.5);
      for (const p of points) { const d = (x - p.x) / p.s; f -= p.a * Math.exp(-Math.abs(d) * (d > 0 ? p.k : 1 / p.k)); }
      return H0 + lakeH * clamp(f, 0.08, 1);
    };
    const shore = [];
    for (let x = -step * 2; x <= w + step * 2; x += step) shore.push([x, lake ? shoreAt(x) : H0]);
    const landTop = lake ? Math.min(...shore.map(([, y]) => y)) : H0;
    if (lake) {
      const lh = lakeH * 1.3;
      const lg = ctx.createLinearGradient(0, H0, 0, H0 + lakeH);
      lg.addColorStop(0, mixc(P.lakeFar, P.haze, 0.3)); lg.addColorStop(1, P.lake);
      ctx.fillStyle = lg;
      ctx.fillRect(0, H0 - 1, w, lh);
      ctx.save();
      ctx.beginPath(); ctx.rect(0, H0, w, lh); ctx.clip();
      ctx.translate(0, 2 * H0); ctx.scale(1, -1);
      tracePath(ctx, far, H0, w);
      const rg2 = ctx.createLinearGradient(0, H0, 0, H0 - lakeH * 0.9);
      rg2.addColorStop(0, rgba(mixc(P.far, P.lake, 0.35), 0.6)); rg2.addColorStop(1, rgba(mixc(P.snowShadow, P.lake, 0.4), 0.45));
      ctx.fillStyle = rg2;
      ctx.fill();
      ctx.restore();
      // Wind lanes on the water: long thin lines of sky, closer together with distance.
      ctx.lineWidth = 1;
      ctx.strokeStyle = rgba(P.lakeFar, 0.6);
      ctx.beginPath();
      for (let i = 0; i < 46; i++) {
        const t = lk() ** 1.5, y = lerp(H0 + 1, H0 + lakeH, t), x = lk.range(-0.1, 1) * w, l = w * lk.range(0.05, 0.3) * (0.4 + t);
        ctx.moveTo(x, y); ctx.lineTo(x + l, y);
      }
      ctx.stroke();
      // The sun's path on the water.
      if (sunX > 0 && sunX < w && sunY < H0) {
        ctx.strokeStyle = rgba(P.sun, 0.85);
        ctx.beginPath();
        for (let i = 0; i < 40; i++) {
          const t = lk(), y = lerp(H0 + 1, H0 + lakeH, t), x = sunX + lk.gauss() * sr * lerp(0.4, 1.1, t), l = lk.range(2, 9) * (0.5 + t);
          ctx.moveTo(x - l / 2, y); ctx.lineTo(x + l / 2, y);
        }
        ctx.stroke();
      }
    }

    // The land: one plane from the horizon or the shore to the foot, hazy far off, darker near.
    const spaceL = h - landTop;
    const lgr = ctx.createLinearGradient(0, landTop, 0, h);
    lgr.addColorStop(0, mixc(P.plainFar, P.haze, 0.5)); lgr.addColorStop(0.2, P.plainFar); lgr.addColorStop(0.5, P.plain); lgr.addColorStop(1, P.ground);
    tracePath(ctx, shore, h + 10, w);
    ctx.fillStyle = lgr;
    ctx.fill();
    if (lake) {
      // Grey shingle along the water, wider in places and gone in others, and a darker wet line
      // where the water meets it, instead of one even rim.
      ctx.fillStyle = rgba(P.shingle, 0.8);
      ctx.beginPath();
      for (const [x, y] of shore) {
        const wd = Math.max(0, 2.6 * noise.fbm(x / 40 + off, 17.5, 3) + 1.1);
        if (wd > 0.3) ctx.rect(x, y, step + 0.5, wd);
      }
      ctx.fill();
      ctx.strokeStyle = rgba(mixc(P.lake, P.nearShadow, 0.45), 0.45);
      ctx.lineWidth = 0.8;
      ctx.beginPath();
      shore.forEach(([x, y], i) => (i ? ctx.lineTo(x, y - 0.4) : ctx.moveTo(x, y - 0.4)));
      ctx.stroke();
    }

    // A nearer, lower range, darker, with snow only on its tops, when there is no lake.
    if (!lake) {
      const nr = stream("near");
      const nearBase = H0 + space * 0.05;
      const nearPeaks = [{ x: nr() * w, a: 1, s: w * nr.range(0.1, 0.2) }, { x: nr() * w, a: 0.7, s: w * 0.08 }];
      const nearAmp = farAmp * nr.range(0.35, 0.5);
      const near = ridgeline(noise, w, step, nearBase, nearAmp, nearPeaks, quiet, Math.max(floorY, H0 - farAmp * 0.25), off + 37, 1.6);
      relief(ctx, { w, pts: near, base: nearBase, noise, sunDir, cell: 2, off: off + 9, spread: 1.1, hazeFrom: 0.2, haze: P.haze, rough: 0.5, edge: 0.5, foot: space * 0.05,
        tones: [mixc(P.snowLit, P.near, 0.15), mixc(P.snowShadow, P.near, 0.35), P.near, mixc(P.near, P.nearShadow, 0.5), P.nearShadow], snowAlt: nearAmp * 0.8 });
    }

    // Haze where the land meets the ranges.
    g = ctx.createLinearGradient(0, H0 - space * 0.06, 0, H0 + space * 0.14);
    g.addColorStop(0, rgba(P.haze, 0)); g.addColorStop(0.5, rgba(P.haze, lake ? 0.45 : 0.75)); g.addColorStop(1, rgba(P.haze, 0));
    ctx.fillStyle = g;
    ctx.fillRect(0, H0 - space * 0.06, w, space * 0.2);

    // A foothill rising to one side, lit on the sun's side, its foot dissolving into the plain.
    const hillRidge = (base, amp, o, freq, tilt) => {
      const pts = [];
      for (let x = -step; x <= w + step; x += step) {
        const across = clamp(tilt > 0 ? x / w : 1 - x / w, 0, 1), n = 0.5 + 0.5 * noise.fbm(x * freq + o, o * 0.7, 3, 2, 0.4);
        pts.push([x, base - (amp * (0.2 + 0.8 * Math.abs(tilt) * across ** 1.5 + 0.55 * n)) / (1 + Math.abs(tilt) * 0.5)]);
      }
      return pts;
    };
    // Sometimes a second, nearer and lower, rising from the other side, warmer and darker.
    const hs = stream("hills"), hills = [];
    if (hs() < 0.85) hills.push({ base: landTop + spaceL * 0.16, amp: spaceL * hs.range(0.22, 0.36), tilt: -sunDir * hs.range(0.8, 1.4), warm: 0 });
    if (hs() < 0.45) hills.push({ base: landTop + spaceL * hs.range(0.3, 0.38), amp: spaceL * hs.range(0.14, 0.22), tilt: sunDir * hs.range(0.9, 1.5), warm: 0.35 });
    hills.forEach((hl, i) => {
      const pts = hillRidge(hl.base, hl.amp, off + 3 + i * 17, 1 / 420, hl.tilt);
      const c1 = mixc(P.hill1, P.hill2, 0.2 + hl.warm), c2 = mixc(P.hill2, P.hill3, 0.2 + hl.warm);
      relief(ctx, { w, pts, base: hl.base, noise, sunDir, cell: 3, off: off + 2 + i * 5, spread: 1.6, rough: 0.55, hazeFrom: 0.1, haze: mixc(P.haze, c1, 0.3 + hl.warm), foot: hl.amp * 0.24,
        tones: [c1, c1, mixc(c1, P.haze, 0.25 - hl.warm * 0.5), mixc(c1, c2, 0.5), mixc(c2, P.nearShadow, 0.35)], snowAlt: null });
      // Its tussock, seen from far off.
      let top = Infinity;
      for (const [, y] of pts) top = Math.min(top, y);
      ctx.save();
      tracePath(ctx, pts, hl.base + 4, w);
      ctx.clip();
      stipple(ctx, hs, noise, { x0: 0, x1: w, y0: top, y1: hl.base, n: (w * (hl.base - top)) / 55, off: off + 5 + i, wind,
        sizeAt: (y) => (0.8 + 1.8 * clamp((y - top) / (hl.base - top), 0, 1)) * (1 + i * 0.5), dark: rgba(mixc(P.hill3, P.core, 0.25), 0.42), light: rgba(P.tip, 0.6) });
      ctx.restore();
    });

    // Short grass over the whole land: tiny strokes, finer and paler with distance, so the land
    // reads as a golden cover and the clumps as its accents.
    ctx.lineCap = "round";
    const gs = stream("grass");
    const grassN = (w * spaceL) / 55;
    // Over a lake the land starts at a ragged shore: the grass starts below its lowest point, so
    // none of it lands on the water (a clip would be dearer to raster).
    const grassTop = lake ? Math.max(...shore.map(([, y]) => y)) : landTop;
    for (const [col, a, lw] of [[P.shade, 0.42, 0.9], [P.mid2, 0.4, 0.8], [P.tip, 0.55, 0.7]]) {
      ctx.strokeStyle = rgba(col, a);
      ctx.lineWidth = lw;
      ctx.beginPath();
      for (let i = 0; i < grassN / 3; i++) {
        const t = gs() ** 1.15, y = lerp(grassTop + 2, h, t), x = gs() * w, sz = 0.6 + t * t * 9.4;
        ctx.moveTo(x, y); ctx.quadraticCurveTo(x + wind * sz * 0.3, y - sz * 0.6, x + wind * sz * 0.9, y - sz * 1.1);
      }
      ctx.stroke();
    }

    // Cloud shadows: big soft patches of shade drifting over the land.
    const cs = stream("shadows");
    for (let i = 0; i < 3; i++) {
      const cx = cs() * w, cy = landTop + spaceL * cs.range(0.22, 0.75), rw = w * cs.range(0.15, 0.4), rh = spaceL * cs.range(0.05, 0.12);
      ctx.save();
      ctx.translate(cx, cy); ctx.scale(1, rh / rw);
      const cg = ctx.createRadialGradient(0, 0, 0, 0, 0, rw);
      cg.addColorStop(0, rgba(P.cloudShade, 0.38)); cg.addColorStop(0.6, rgba(P.cloudShade, 0.22)); cg.addColorStop(1, rgba(P.cloudShade, 0));
      ctx.fillStyle = cg;
      ctx.fillRect(-rw, -rw, rw * 2, rw * 2);
      ctx.restore();
    }

    // The clumps: far, middle and near tiers, in patches, as thick and as big as the seed grows
    // them, massed where it puts them.
    const cl = stream("clumps");
    const mass = [cl(), cl()], thick = cl.range(0.6, 1.35), big = cl.range(0.85, 1.2);
    const dens = (x, y) => {
      const mm = Math.max(...mass.map((c) => Math.exp(-((x / w - c) ** 2) / 0.06)));
      const patch = smooth(-0.35, 0.35, noise.fbm(x / 260 + off, y / 90, 3));
      return 0.12 + 0.88 * (0.45 * mm + 0.55 * patch);
    };
    const area = w / 1000;
    const tiers = [
      { y0: 0.24, y1: 0.42, s0: 0.035, s1: 0.06, n: 170 * area + 40, scale: 0.65, haze: 0.3 },
      { y0: 0.36, y1: 0.62, s0: 0.065, s1: 0.12, n: 95 * area + 24, scale: 0.85, haze: 0.12 },
      { y0: 0.55, y1: 0.98, s0: 0.15, s1: 0.3, n: 34 * area + 9, scale: 1.2, haze: 0 },
    ];
    // Each clump is golden, copper (red tussock) or pale straw.
    const hues = { gold: [P.core, P.shade, P.mid2, P.tip], copper: [P.cCore, P.cShade, P.cMid, P.cTip], straw: [P.shade, P.mid2, P.straw, P.tipLit] };
    // Red tussock comes with the light: everywhere under the rust light, a little elsewhere.
    const copperShare = clamp(P.red + cl.range(-0.1, 0.15), 0, 0.92), copperAt = (c) => smooth(0.15, 0.45, noise.fbm(c.x / 300 - off, c.y / 120 + 7, 2)) * copperShare;
    for (const t of tiers) {
      const list = scatter(cl, w, landTop + spaceL * t.y0, landTop + spaceL * t.y1, spaceL * t.s0 * big, spaceL * t.s1 * big, Math.round(t.n * thick), dens);
      clumps(ctx, list, { wind, sunX, tone: {}, scale: t.scale, noise, shadow: rgba(mixc(P.core, P.haze, t.haze), 0.22), shadowOnly: true });
      const groups = { gold: [], copper: [], straw: [] };
      for (const c of list) { const u = c.r(); groups[u < copperAt(c) ? "copper" : u > 0.85 ? "straw" : "gold"].push(c); }
      for (const [name, cs] of Object.entries(groups)) {
        const [core, shade, mid, tip] = hues[name];
        const tone = { core: mixc(core, P.haze, t.haze), shade: mixc(shade, P.haze, t.haze), mid: mixc(mid, P.haze, t.haze), tip: mixc(tip, P.haze, t.haze * 0.5), tipLit: P.tipLit };
        clumps(ctx, cs, { wind, sunX, tone, scale: t.scale, noise });
      }
    }

    // Big clumps against the light at the foot's corners, to give the picture its near edge.
    const cn = stream("corners"), corner = [];
    for (const cx of [cn.range(-0.04, 0.14), cn.range(0.86, 1.04)]) if (cn() < 0.85) corner.push({ x: cx * w, y: h * 0.99, s: spaceL * cn.range(0.4, 0.55), r: cn.fork(`c${cx}`) });
    clumps(ctx, corner, { wind, sunX, tone: { core: P.core, shade: P.core, mid: mixc(P.core, P.shade, 0.5), tip: P.mid2, tipLit: P.tip }, scale: 1.5, noise });

    // The low sun washes over everything on its side.
    ctx.save();
    ctx.globalCompositeOperation = "screen";
    g = ctx.createRadialGradient(sunX, sunY, 0, sunX, sunY, glowR);
    g.addColorStop(0, rgba(P.glow, 0.55 * P.glowA)); g.addColorStop(0.3, rgba(P.glow, 0.18 * P.glowA)); g.addColorStop(1, rgba(P.glow, 0));
    ctx.fillStyle = g;
    ctx.fillRect(0, 0, w, h);
    ctx.restore();

    kit.grain(ctx, w, h, 0.07, "tussock");
  },
};
