// The themes an area wears (`Theme` in crates/core/src/design.rs), the kit their grounds and charts
// are drawn with, and how the face dresses a page in one and paints its ground.
//
// A theme is the module theme-<name>.js, whose default export is
//   { name, about, palette, display, slots, map: { hue, chroma }, paint(ctx, scene), chart(ctx, scene) }
// palette holds design.rs's `Palette` fields as hex colours (figure: four); display its `Face`
// ({ family, files, weights, bold, axes }), its files served beside this module as page files; slots the identities'
// places in the theme's range of colour, in [0, 1), in the order checked by eye: slot 0 the area's
// main agent, then each next agent as far from those before as the art allows.
//
// paint draws into a 2D context already scaled to CSS pixels, given
//   scene = { w, h, dpr, seed, slot, at, line, root, rng, stream, noise, quiet, kit }
// where quiet is the box the page lead sits in ({ x, y, w, h }, CSS px), which the art keeps calm.
// A picture with no lead (a thumbnail, the map) gets a small box in the upper middle, so it is
// composed as a cover would be. dpr is the device pixels per CSS pixel, for shaders and hairlines.
// seed, slot, root and line are the task's `Identity`; at is its slot's place (slots[slot], round
// again past the last), which the ground maps onto its own colour. stream(part) is a random source of
// its own for one part of the picture ("sun", "coast"): each part draws from its own stream, so
// changing how one part draws leaves every agent's other parts where they were. rng is one stream for
// the whole picture. line is the task's place in the tree below its furthest ancestor wearing the same
// theme, the child numbers from there down ([] for that agent, [2] for its third child, [2, 0] for that
// one's first), for a ground whose picture of a child is a closer look at its parent's; root is that
// ancestor's seed, which such a ground paints the whole line's picture from.
// paint is synchronous and deterministic in the identity and size, never animated, and may use only
// fonts the page has already loaded (wear waits for the display face).
//
// On the map an area is a territory (territories.js), tinted in a faded wash of the theme's map.hue
// (OKLCH degrees; never 35-75, since orange on the map is Max's alone; an old map's buff, sage, sea
// green, grey-blue, slate), at map.chroma (optional: a share of the wash's usual chroma, for a hue
// that reads too strong at it), and inside it chart draws the
// theme's subject as an old map would: Harbour's a nautical chart, Tussock's a topographic sheet. It
// is the theme's art seen as a map, not its painting faded. chart draws in map units on a recording
// context (strokes, fills, arcs, text, transforms, setLineDash, drawImage), given
//   scene = { w, h, now, scale, paper, mass(x, y), calm(x, y), n, stream, under, kit }
// its box (now is the now line's height in it, with history above), the wash it draws on as `paper`,
// mass the territory (1 inside it, 0 outside, so the chart fills whatever shape it is given and
// never anchors itself to the now line), calm 1 everywhere but 0 within a hair of every stream and
// node (the map's names carry their own halo), n a noise and stream(part) a random source of the area's own, under a
// context for its pale tints, and kit this module's kit, with the chart helpers (grid, iso, chains,
// within, lines, scatter, dots, tint, ok). The rules that keep the map a map Max reads:
// - Inks mid-dark and low in chroma, as a printed map's plates are, and water may have a blue of its
//   own; pale tints drawn on `under`: lines, not paintings. Strokes, dark fills and words are kept
//   as paths in map units, drawn sharp at whatever zoom the map rests at; pale fills go to a small
//   picture.
// - No Path2D, getImageData or clip: the context records, it has no pixels.
// - Line work may run anywhere: the map knocks the graph out of the ink, a gap round every stream
//   and node, thins the ink near the streams and fades it up the history; a word that would touch
//   the gap is left out whole. Keep symbols off the graph with calm, so they are not cut.
// - Deterministic in the area and its shape, and quick: the whole map is worked out again on every
//   change to its layout.

// FNV-1a over the seed string, so an agent's id always gives the same number.
export function hash(str) {
  let h = 2166136261;
  for (const ch of String(str)) { h ^= ch.codePointAt(0); h = Math.imul(h, 16777619); }
  return h >>> 0;
}

// A seeded random source (mulberry32) with a few helpers.
export function rng(seed) {
  let a = typeof seed === "number" ? seed >>> 0 : hash(seed);
  const r = () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  r.range = (lo, hi) => lo + (hi - lo) * r();
  r.int = (lo, hi) => Math.floor(lo + (hi - lo + 1) * r());
  r.pick = (xs) => xs[Math.floor(r() * xs.length)];
  r.chance = (p) => r() < p;
  r.gauss = () => { let u = 0, v = 0; while (!u) u = r(); while (!v) v = r(); return Math.sqrt(-2 * Math.log(u)) * Math.cos(6.283185307 * v); };
  r.fork = (label) => rng(hash(label) ^ Math.floor(r() * 4294967296));
  return r;
}

// Seeded 2D simplex noise in [-1, 1], with fractal sums and domain warping.
export function noise(seed) {
  const r = rng(seed), perm = new Uint8Array(512), p = new Uint8Array(256);
  for (let i = 0; i < 256; i++) p[i] = i;
  for (let i = 255; i > 0; i--) { const j = Math.floor(r() * (i + 1)); [p[i], p[j]] = [p[j], p[i]]; }
  for (let i = 0; i < 512; i++) perm[i] = p[i & 255];
  const G = [[1, 1], [-1, 1], [1, -1], [-1, -1], [1, 0], [-1, 0], [0, 1], [0, -1]];
  const F2 = 0.5 * (Math.sqrt(3) - 1), G2 = (3 - Math.sqrt(3)) / 6;
  function n2(x, y) {
    const s = (x + y) * F2, i = Math.floor(x + s), j = Math.floor(y + s), t = (i + j) * G2;
    const x0 = x - (i - t), y0 = y - (j - t), i1 = x0 > y0 ? 1 : 0, j1 = 1 - i1;
    const x1 = x0 - i1 + G2, y1 = y0 - j1 + G2, x2 = x0 - 1 + 2 * G2, y2 = y0 - 1 + 2 * G2;
    const ii = i & 255, jj = j & 255;
    let n = 0, q;
    let t0 = 0.5 - x0 * x0 - y0 * y0;
    if (t0 > 0) { q = G[perm[ii + perm[jj]] & 7]; t0 *= t0; n += t0 * t0 * (q[0] * x0 + q[1] * y0); }
    let t1 = 0.5 - x1 * x1 - y1 * y1;
    if (t1 > 0) { q = G[perm[ii + i1 + perm[jj + j1]] & 7]; t1 *= t1; n += t1 * t1 * (q[0] * x1 + q[1] * y1); }
    let t2 = 0.5 - x2 * x2 - y2 * y2;
    if (t2 > 0) { q = G[perm[ii + 1 + perm[jj + 1]] & 7]; t2 *= t2; n += t2 * t2 * (q[0] * x2 + q[1] * y2); }
    return 70 * n;
  }
  function fbm(x, y, octaves = 5, lacunarity = 2, gain = 0.5) {
    let sum = 0, amp = 1, norm = 0;
    for (let o = 0; o < octaves; o++) { sum += amp * n2(x, y); norm += amp; amp *= gain; x = x * lacunarity + 17.3; y = y * lacunarity - 9.1; }
    return sum / norm;
  }
  // fbm sampled at a point pushed around by two other fbm fields (Inigo Quilez's domain warp).
  function warp(x, y, strength = 1, octaves = 4) {
    const qx = fbm(x + 1.7, y + 9.2, octaves), qy = fbm(x + 8.3, y + 2.8, octaves);
    return fbm(x + strength * qx, y + strength * qy, octaves);
  }
  return { n2, fbm, warp };
}

// Film grain over what is drawn: a seeded tile of mono noise blended in with `overlay`.
export function grain(ctx, w, h, amount = 0.06, seed = "grain", size = 192) {
  const r = rng(seed), tile = document.createElement("canvas");
  tile.width = tile.height = size;
  const t = tile.getContext("2d"), img = t.createImageData(size, size);
  for (let i = 0; i < img.data.length; i += 4) {
    const v = Math.max(0, Math.min(255, 128 + r.gauss() * 46));
    img.data[i] = img.data[i + 1] = img.data[i + 2] = v; img.data[i + 3] = 255;
  }
  t.putImageData(img, 0, 0);
  ctx.save();
  ctx.globalCompositeOperation = "overlay";
  ctx.globalAlpha = amount;
  ctx.fillStyle = ctx.createPattern(tile, "repeat");
  ctx.fillRect(0, 0, w, h);
  ctx.restore();
}

// One shared WebGL canvas for every ground's shaders, so a page with many grounds holds one context.
// shader(fragmentSource, width, height, uniforms) draws a full-screen pass and returns the canvas:
// drawImage it at once, since the next call reuses it. Uniforms: a number is a float, an array of
// 2 to 4 numbers a vec; an array uniform is set element by element by name (u_t[0], u_t[1], ...).
// A canvas is a sampler2D. u_res (vec2, device pixels) is always set. Returns null without WebGL.
let gl = null, glCanvas = null;
const programs = new Map(), textures = [];
export function shader(frag, w, h, uniforms = {}) {
  if (!glCanvas) {
    glCanvas = document.createElement("canvas");
    gl = glCanvas.getContext("webgl", { preserveDrawingBuffer: true, premultipliedAlpha: false, antialias: false });
    if (gl) {
      // dFdx, dFdy and fwidth, for a shader that writes `#extension GL_OES_standard_derivatives : enable`.
      gl.getExtension("OES_standard_derivatives");
      const buf = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, buf);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
    }
  }
  if (!gl) return null;
  let prog = programs.get(frag);
  if (!prog) {
    const compile = (type, src) => {
      const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error("shader: " + gl.getShaderInfoLog(s));
      return s;
    };
    prog = gl.createProgram();
    gl.attachShader(prog, compile(gl.VERTEX_SHADER, "attribute vec2 p; void main(){ gl_Position = vec4(p, 0., 1.); }"));
    gl.attachShader(prog, compile(gl.FRAGMENT_SHADER, frag));
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error("shader link: " + gl.getProgramInfoLog(prog));
    programs.set(frag, prog);
  }
  glCanvas.width = Math.max(1, Math.round(w)); glCanvas.height = Math.max(1, Math.round(h));
  gl.viewport(0, 0, glCanvas.width, glCanvas.height);
  gl.useProgram(prog);
  const loc = gl.getAttribLocation(prog, "p");
  gl.enableVertexAttribArray(loc);
  gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
  const all = { u_res: [glCanvas.width, glCanvas.height], ...uniforms };
  let unit = 0;
  for (const [name, v] of Object.entries(all)) {
    const u = gl.getUniformLocation(prog, name);
    if (u === null) continue;
    if (typeof v === "number") gl.uniform1f(u, v);
    else if (Array.isArray(v)) gl[`uniform${v.length}fv`](u, v);
    else {
      // A canvas is a texture, its top row at v = 0, linearly filtered and clamped at its edges.
      textures[unit] ??= gl.createTexture();
      gl.activeTexture(gl.TEXTURE0 + unit);
      gl.bindTexture(gl.TEXTURE_2D, textures[unit]);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, v);
      for (const [k, x] of [[gl.TEXTURE_MIN_FILTER, gl.LINEAR], [gl.TEXTURE_MAG_FILTER, gl.LINEAR], [gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE], [gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE]]) gl.texParameteri(gl.TEXTURE_2D, k, x);
      gl.uniform1i(u, unit++);
    }
  }
  gl.drawArrays(gl.TRIANGLES, 0, 3);
  return glCanvas;
}

// Hex colour helpers for painting: mix two colours, or give one an alpha.
export function rgb(hex) { const n = parseInt(hex.slice(1), 16); return [(n >> 16) & 255, (n >> 8) & 255, n & 255]; }
export function mix(a, b, t) { const A = rgb(a), B = rgb(b); return "#" + A.map((x, i) => Math.round(x + (B[i] - x) * t).toString(16).padStart(2, "0")).join(""); }
export function alpha(hex, a) { const [r, g, b] = rgb(hex); return `rgba(${r},${g},${b},${a})`; }

// A hex colour as OKLCH ([lightness 0-1, chroma, hue in degrees]), and back: the space to turn hues in,
// since equal steps there look equal. fromOklch lowers the chroma until the colour fits in sRGB.
const lin = (c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const gam = (c) => (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055);
export function oklch(hex) {
  const [r, g, b] = rgb(hex).map((v) => lin(v / 255));
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  const A = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s, B = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, Math.hypot(A, B), ((Math.atan2(B, A) * 180) / Math.PI + 360) % 360];
}
export function fromOklch(L, C, h) {
  const rad = (h * Math.PI) / 180;
  for (let c = C; ; c = Math.max(0, c - 0.005)) {
    const a = c * Math.cos(rad), b = c * Math.sin(rad);
    const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3, m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3, s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
    const v = [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s, -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s];
    if (c === 0 || v.every((x) => x >= -1e-4 && x <= 1.0001)) return "#" + v.map((x) => Math.round(255 * gam(Math.min(1, Math.max(0, x)))).toString(16).padStart(2, "0")).join("");
  }
}

// An identity's place in its theme's range: its slot's, past the theme's slots round again.
export function placeOf(theme, slot) { return theme.slots[slot % theme.slots.length]; }

export const kit = { hash, rng, noise, grain, shader, rgb, mix, alpha, oklch, fromOklch, ease, grid, iso, chains, within, poly, lines, length, midway, tint, scatter, dots, ok, okRgb };

// ─── The charts' helpers ─────────────────────────────────────────────────────────────────────────
function ease(t) { t = Math.min(1, Math.max(0, t)); return t * t * (3 - 2 * t); }
// An OKLCH colour as a canvas colour with an alpha, and as [r, g, b].
function ok(L, C, h, a = 1) { return alpha(fromOklch(L, C, h), a); }
function okRgb(L, C, h) { return rgb(fromOklch(L, C, h)); }

// A field f(x, y) sampled every `cell` units over w x h, with bilinear reads (at) and its gradient.
function grid(w, h, cell, f) {
  const nx = Math.ceil(w / cell) + 1, ny = Math.ceil(h / cell) + 1, v = new Float32Array(nx * ny);
  for (let j = 0; j < ny; j++) for (let i = 0; i < nx; i++) v[j * nx + i] = f(i * cell, j * cell);
  const at = (x, y) => {
    const gx = Math.min(nx - 1.001, Math.max(0, x / cell)), gy = Math.min(ny - 1.001, Math.max(0, y / cell));
    const i = Math.floor(gx), j = Math.floor(gy), fx = gx - i, fy = gy - j, k = j * nx + i;
    return (v[k] * (1 - fx) + v[k + 1] * fx) * (1 - fy) + (v[k + nx] * (1 - fx) + v[k + nx + 1] * fx) * fy;
  };
  const grad = (x, y) => [(at(x + cell, y) - at(x - cell, y)) / (2 * cell), (at(x, y + cell) - at(x, y - cell)) / (2 * cell)];
  return { nx, ny, cell, v, at, grad };
}

// A grid's iso-line at `level`, as segments [x0, y0, x1, y1, ...] (marching squares).
function iso(g, level) {
  const { nx, ny, cell, v } = g, out = [];
  for (let j = 0; j < ny - 1; j++) for (let i = 0; i < nx - 1; i++) {
    const k = j * nx + i, a = v[k], b = v[k + 1], c = v[k + nx + 1], d = v[k + nx];
    const idx = (a > level ? 8 : 0) | (b > level ? 4 : 0) | (c > level ? 2 : 0) | (d > level ? 1 : 0);
    if (idx === 0 || idx === 15) continue;
    const x = i * cell, y = j * cell, t = (p, q) => (level - p) / (q - p);
    const T = [x + cell * t(a, b), y], R = [x + cell, y + cell * t(b, c)], B = [x + cell * t(d, c), y + cell], L = [x, y + cell * t(a, d)];
    const seg = (p, q) => out.push(p[0], p[1], q[0], q[1]);
    const mid = (a + b + c + d) / 4 > level;
    switch (idx) {
      case 1: case 14: seg(L, B); break;
      case 2: case 13: seg(B, R); break;
      case 3: case 12: seg(L, R); break;
      case 4: case 11: seg(T, R); break;
      case 6: case 9: seg(T, B); break;
      case 7: case 8: seg(L, T); break;
      case 5: if (mid) { seg(L, T); seg(B, R); } else { seg(L, B); seg(T, R); } break;
      case 10: if (mid) { seg(T, R); seg(L, B); } else { seg(L, T); seg(B, R); } break;
    }
  }
  return out;
}

// Segments joined end to end into polylines [x, y, x, y, ...], so a line can be smoothed and dashed.
function chains(segs) {
  const key = (x, y) => `${Math.round(x * 32)},${Math.round(y * 32)}`;
  const n = segs.length / 4, used = new Uint8Array(n), ends = new Map();
  const add = (k, i) => { const l = ends.get(k); if (l) l.push(i); else ends.set(k, [i]); };
  for (let i = 0; i < n; i++) { add(key(segs[4 * i], segs[4 * i + 1]), i); add(key(segs[4 * i + 2], segs[4 * i + 3]), i); }
  const out = [];
  for (let s = 0; s < n; s++) {
    if (used[s]) continue;
    used[s] = 1;
    const head = [segs[4 * s], segs[4 * s + 1]], tail = [segs[4 * s + 2], segs[4 * s + 3]], fwd = [], back = [];
    for (const [arr, start] of [[fwd, tail], [back, head]]) {
      let [x, y] = start;
      for (;;) {
        const next = (ends.get(key(x, y)) || []).find((i) => !used[i]);
        if (next === undefined) break;
        used[next] = 1;
        const ax = segs[4 * next], ay = segs[4 * next + 1], bx = segs[4 * next + 2], by = segs[4 * next + 3];
        [x, y] = key(ax, ay) === key(x, y) ? [bx, by] : [ax, ay];
        arr.push(x, y);
      }
    }
    const line = [];
    for (let i = back.length - 2; i >= 0; i -= 2) line.push(back[i], back[i + 1]);
    line.push(...head, ...tail, ...fwd);
    out.push(line);
  }
  return out;
}

// Polylines cut to the runs where test(x, y) holds.
function within(list, test) {
  const out = [];
  for (const p of list) {
    let cur = [];
    for (let i = 0; i < p.length; i += 2) {
      if (test(p[i], p[i + 1])) cur.push(p[i], p[i + 1]);
      else { if (cur.length >= 4) out.push(cur); cur = []; }
    }
    if (cur.length >= 4) out.push(cur);
  }
  return out;
}

// A polyline added to the path, smoothed through its midpoints, its nearly straight runs thinned a
// stretch of at most `span` units at a time (a chart on the map is recorded as SVG, so every point
// costs).
function poly(ctx, p, tol = 0.15, span = 12) {
  const pts = [[p[0], p[1]]], kept = [];
  let length = 0;
  const flush = () => { for (const q of simplify(pts, tol).slice(kept.length ? 1 : 0)) kept.push(q[0], q[1]); };
  for (let i = 2; i < p.length; i += 2) {
    length += Math.hypot(p[i] - p[i - 2], p[i + 1] - p[i - 1]);
    pts.push([p[i], p[i + 1]]);
    if (length > span) { flush(); pts.splice(0, pts.length - 1); length = 0; }
  }
  if (pts.length > 1) flush();
  p = kept;
  ctx.moveTo(p[0], p[1]);
  for (let i = 2; i < p.length - 2; i += 2) ctx.quadraticCurveTo(p[i], p[i + 1], (p[i] + p[i + 2]) / 2, (p[i + 1] + p[i + 3]) / 2);
  ctx.lineTo(p[p.length - 2], p[p.length - 1]);
}
// A polyline's points, [[x, y], ...], with each run nearer than `tol` to a straight line cut to its
// ends (Douglas and Peucker's algorithm).
export function simplify(r, tol) {
  if (r.length < 8) return r;
  const keep = new Uint8Array(r.length); keep[0] = keep[r.length - 1] = 1;
  const stack = [[0, r.length - 1]];
  while (stack.length) {
    const [a, b] = stack.pop(), [ax, ay] = r[a], [bx, by] = r[b], dx = bx - ax, dy = by - ay, L = Math.hypot(dx, dy) || 1;
    let far = -1, best = tol;
    for (let i = a + 1; i < b; i++) { const e = Math.abs((r[i][0] - ax) * dy - (r[i][1] - ay) * dx) / L; if (e > best) { best = e; far = i; } }
    if (far > 0) { keep[far] = 1; stack.push([a, far], [far, b]); }
  }
  return r.filter((_, i) => keep[i]);
}
// Polylines stroked in one path, those shorter than `min` left out.
function lines(ctx, list, colour, width, dash = null, min = 0) {
  ctx.beginPath();
  for (const p of list) if (p.length >= 4 && length(p) >= min) poly(ctx, p);
  ctx.strokeStyle = colour; ctx.lineWidth = width; ctx.lineCap = "round"; ctx.lineJoin = "round";
  ctx.setLineDash(dash || []); ctx.stroke(); ctx.setLineDash([]);
}
function length(p) { let s = 0; for (let i = 2; i < p.length; i += 2) s += Math.hypot(p[i] - p[i - 2], p[i + 1] - p[i - 1]); return s; }
// The point `at` of the way along a polyline, and the line's direction there.
function midway(p, at = 0.5) {
  const want = length(p) * at;
  let run = 0;
  for (let i = 2; i < p.length; i += 2) {
    const d = Math.hypot(p[i] - p[i - 2], p[i + 1] - p[i - 1]);
    if (run + d >= want) return [p[i], p[i + 1], Math.atan2(p[i + 1] - p[i - 1], p[i] - p[i - 2])];
    run += d;
  }
  return [p[p.length - 2], p[p.length - 1], 0];
}

// A soft tint, f(x, y) giving [r, g, b, a] or null every `cell` units, drawn smooth over w x h: the
// lines over it carry the sharp edges.
function tint(ctx, w, h, cell, f) {
  const cw = Math.ceil(w / cell), ch = Math.ceil(h / cell), c = new OffscreenCanvas(cw, ch);
  const cc = c.getContext("2d"), img = cc.createImageData(cw, ch), d = img.data;
  for (let j = 0, i = 0; j < ch; j++) for (let k = 0; k < cw; k++, i += 4) {
    const v = f((k + 0.5) * cell, (j + 0.5) * cell);
    if (v) { d[i] = v[0]; d[i + 1] = v[1]; d[i + 2] = v[2]; d[i + 3] = v[3]; }
  }
  cc.putImageData(img, 0, 0);
  ctx.save(); ctx.imageSmoothingEnabled = true; ctx.drawImage(c, 0, 0, cw * cell, ch * cell); ctx.restore();
}

// Points on a jittered grid over w x h, each kept with probability keep(x, y).
function scatter(r, w, h, step, keep) {
  const out = [];
  for (let y = step / 2; y < h; y += step) for (let x = step / 2; x < w; x += step) {
    const px = x + (r() - 0.5) * step * 0.9, py = y + (r() - 0.5) * step * 0.9;
    if (r() < keep(px, py)) out.push([px, py]);
  }
  return out;
}

// Small round marks [x, y, r], all in one path.
function dots(ctx, pts, colour) {
  ctx.beginPath();
  for (const [x, y, r] of pts) { ctx.moveTo(x + r, y); ctx.arc(x, y, r, 0, 6.283); }
  ctx.fillStyle = colour; ctx.fill();
}

// Paint a theme's ground into a canvas sized to its element, at up to 2x device pixels, for a task's
// identity ({ slot, seed, root, line }, as `Identity` in crates/core/src/design.rs).
// size ({ w, h, dpr }) paints a canvas that is not on the page, at that size.
// Returns how long the paint took, in ms.
export function paintInto(canvas, theme, who, quiet, size = null) {
  const { slot, seed, root = seed, line = [] } = who;
  const w = size?.w ?? canvas.clientWidth, h = size?.h ?? canvas.clientHeight, dpr = size?.dpr ?? Math.min(2, window.devicePixelRatio || 1);
  canvas.width = Math.round(w * dpr); canvas.height = Math.round(h * dpr);
  const ctx = canvas.getContext("2d");
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  quiet ??= { x: w * 0.25, y: h * 0.2, w: w * 0.5, h: h * 0.3 };
  const t0 = performance.now();
  const stream = (part) => rng(`${theme.name}:${seed}:${part}`);
  theme.paint(ctx, { w, h, dpr, seed, slot, at: placeOf(theme, slot), line, root, rng: rng(`${theme.name}:${seed}`), stream, noise: noise(`${theme.name}:${seed}`), quiet, kit });
  return performance.now() - t0;
}

// The themes, by name: each `theme-<name>.js`, whose default export is the theme.
export const THEMES = ["tussock", "harbour", "kauri", "paua", "marama"];
export async function load(name) { return (await import(`./theme-${name}.js`)).default; }

// Dress an element (the page's root) in a theme: its tokens as the language's CSS custom properties,
// and its display face, loaded from the face's files. Resolves once the face is ready, since a ground
// that letters (Harbour's soundings) paints in it.
export async function wear(el, theme) {
  const p = theme.palette, d = theme.display, s = el.style;
  for (const k of ["paper", "ink", "body", "muted", "line", "tint", "accent"]) s.setProperty(`--${k}`, p[k]);
  p.figure.forEach((c, i) => s.setProperty(`--fig-${i + 1}`, c));
  s.setProperty("--cover-ink", p.cover_ink);
  s.setProperty("--cover-body", p.cover_body);
  s.setProperty("--display", `"${d.family}", Georgia, serif`);
  s.setProperty("--display-bold", String(d.bold));
  s.setProperty("--display-var", d.axes ?? "normal");
  if (![...document.fonts].some((f) => f.family.replace(/"/g, "") === d.family)) {
    for (const file of d.files) document.fonts.add(new FontFace(d.family, `url(${new URL(`./${file}`, import.meta.url)})`, { weight: d.weights }));
  }
  // A face that fails to load leaves the fallback: the page never waits on it.
  await document.fonts.load(`${d.bold} 34px "${d.family}"`).catch(() => {});
}

// paintInto's time is the script's alone: the browser rasters the canvas, and runs shaders, later and
// off the page's thread. settle(canvas) waits for that by reading one pixel back, and returns how
// long it waited. Only for measuring: in a page it would block the thread the raster was spared.
export function settle(canvas) {
  const t0 = performance.now();
  canvas.getContext("2d").getImageData(0, 0, 1, 1);
  return performance.now() - t0;
}
