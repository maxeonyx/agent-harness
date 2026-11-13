// The themes an area wears (`Theme` in crates/core/src/design.rs), the kit their grounds are painted
// with, and how the face dresses a page in one and paints its ground.
//
// A theme is the module theme-<name>.js, whose default export is
//   { name, about, palette, display, slots, identity(at), map, paint(ctx, scene) }
// palette holds design.rs's `Palette` fields as hex colours (figure: four); display its `Face`
// ({ family, files, weights, bold, axes }), its files in face-web/src/fonts, fetched beside this
// module (the face does not serve them yet: its page files are text); slots the identities'
// places in the theme's range of colour, in [0, 1), in the order checked by eye: slot 0 the area's
// main agent, then each next agent as far from those before as the art allows. identity(at) is the
// colour an agent is told apart by, for its node on the map. map says how a dark ground's shadows
// lift as the map fades it (paintRegion): they take the colour `tone` in the share `duo`; a loud one
// may set `below`, a higher floor under the now line, and `chroma`, its own colour gain.
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
    else if (v instanceof HTMLCanvasElement) {
      // A canvas is a texture, its top row at v = 0, linearly filtered and clamped at its edges.
      textures[unit] ??= gl.createTexture();
      gl.activeTexture(gl.TEXTURE0 + unit);
      gl.bindTexture(gl.TEXTURE_2D, textures[unit]);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, v);
      for (const [k, x] of [[gl.TEXTURE_MIN_FILTER, gl.LINEAR], [gl.TEXTURE_MAG_FILTER, gl.LINEAR], [gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE], [gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE]]) gl.texParameteri(gl.TEXTURE_2D, k, x);
      gl.uniform1i(u, unit++);
    } else gl[`uniform${v.length}fv`](u, v);
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

// The colour that says which agent this is among the others wearing its theme, for its node on the map.
export function identity(theme, slot) { return theme.identity(placeOf(theme, slot)); }

export const kit = { hash, rng, noise, grain, shader, rgb, mix, alpha, oklch, fromOklch };

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

// A theme's painting as an area of the map: a frieze along the map's now line, where its agents stand.
// The present row is the horizon: the painting is fullest just below it, where the map has nothing to
// read, fades back toward the map's paper above it, where the lines and names are, and dissolves into
// the history above and into its neighbours at its sides. Each live agent stands in a pool of its
// identity colour on the ground below it, wide enough to hold its live children's. In the map's units:
//   spec = { w, h, now,             the region, and the now line's height in it
//            scale,                 bitmap pixels per map unit
//            paper,                 the map's paper
//            wash }                 how it fades (WASH, the default)
// A dark ground says how its shadows should lift, in `map: { tone, duo }`: they take the colour
// `tone` in the share `duo` (0-1), so a night or a forest fades to a tinted print instead of grey.
// A loud one may set `map.below`, a higher floor for the ground under the now line, and
// `map.chroma`, its own colour gain.
//            pools: [{ x, y, r, slot }] }     each live agent's node, reach and identity slot
// Pools change hue alone ("color" blending), so what is read over them keeps its contrast. Each live
// agent's node stands in a small clearing of paper, so its state mark keeps the contrast it has on
// bare paper: the state is the strongest colour where it stands.
export function paintRegion(ground, who, spec) {
  const { w, h, now, scale, paper, wash = WASH, pools = [] } = spec;
  const art = document.createElement("canvas");
  paintInto(art, ground, who, null, { w, h, dpr: scale });
  const ac = art.getContext("2d");
  ac.setTransform(scale, 0, 0, scale, 0, 0);
  // Faded toward the paper: most above the now line, least below it.
  // Its lightness pressed up into a band that ends at the paper's (OKLab, so hues keep their colour):
  // above the now line no darker than names can be read on at 7:1, below it freer.
  const faded = shader(FADE, art.width, art.height, {
    u_img: art, u_now: (now + 70) / h, u_soft: 90 / h, u_top: oklch(paper)[0],
    u_above: wash.above, u_below: ground.map?.below ?? wash.below, u_chroma: ground.map?.chroma ?? wash.chroma ?? 1.1,
    u_chroma_above: (ground.map?.chroma ?? wash.chroma ?? 1.1) * (wash.chromaAbove ?? 1.1) / (wash.chroma ?? 1.1),
    u_tone: ground.map?.tone ? labOf(ground.map.tone) : [0, 0, 0], u_duo: ground.map?.duo ?? 0,
  });
  if (faded) { ac.save(); ac.setTransform(1, 0, 0, 1, 0, 0); ac.globalCompositeOperation = "copy"; ac.drawImage(faded, 0, 0); ac.restore(); }
  else {
    const veil = ac.createLinearGradient(0, 0, 0, h);
    veil.addColorStop(0, alpha(paper, 0.6)); veil.addColorStop(1, alpha(paper, 0.35));
    ac.fillStyle = veil; ac.fillRect(0, 0, w, h);
  }
  ac.globalCompositeOperation = "color";
  for (const p of [...pools].sort((a, b) => b.r - a.r)) {
    const c = identity(ground, p.slot), y = now + 30;
    ac.save(); ac.translate(p.x, y); ac.scale(1, 0.62);
    const g = ac.createRadialGradient(0, 0, 0, 0, 0, p.r);
    const k = wash.pool ?? 1;
    g.addColorStop(0, alpha(c, 0.62 * k)); g.addColorStop(0.45, alpha(c, 0.42 * k)); g.addColorStop(1, alpha(c, 0));
    ac.fillStyle = g; ac.fillRect(-p.r, -p.r, 2 * p.r, 2 * p.r);
    ac.restore();
  }
  ac.globalCompositeOperation = "source-over";
  for (const p of pools) {
    const g = ac.createRadialGradient(p.x, p.y, 0, p.x, p.y, CLEARING);
    g.addColorStop(0, alpha(paper, 0.97)); g.addColorStop(0.55, alpha(paper, 0.85)); g.addColorStop(1, alpha(paper, 0));
    ac.fillStyle = g; ac.fillRect(p.x - CLEARING, p.y - CLEARING, 2 * CLEARING, 2 * CLEARING);
  }
  // The edges: up into history, down to nothing, and out at the sides, ragged like a wash's.
  const cell = 8, cw = Math.max(2, Math.ceil(w / cell)), ch = Math.max(2, Math.ceil(h / cell));
  const edge = document.createElement("canvas");
  edge.width = cw; edge.height = ch;
  const ec = edge.getContext("2d"), img = ec.createImageData(cw, ch), d = img.data, n = noise(`${ground.name}:${who.seed}:edge`);
  const ease = (t) => { t = Math.min(1, Math.max(0, t)); return t * t * (3 - 2 * t); };
  const side = Math.min(130, w * 0.22), top = now - 40, foot = h - now;
  for (let j = 0, i = 0; j < ch; j++) for (let k = 0; k < cw; k++, i += 4) {
    const x = k * cell, y = j * cell, wob = n.fbm(x * 0.006, y * 0.006, 4);
    const rise = ease((y + 90 * wob) / top);
    const a = ease((Math.min(x, w - x) + 46 * wob) / side) * rise * rise * ease((h - y + 50 * wob) / (foot * 0.75));
    d[i + 3] = Math.round(255 * a);
  }
  ec.putImageData(img, 0, 0);
  ac.globalCompositeOperation = "destination-in";
  ac.setTransform(1, 0, 0, 1, 0, 0);
  ac.drawImage(edge, 0, 0, art.width, art.height);
  ac.globalCompositeOperation = "source-over";
  return art;
}

// How the map fades a region. Above the now line, behind the streams Max reads, the painting keeps
// lightness no lower than `above` (OKLab) and `chromaAbove` of its colour ("the background behind the
// streams on the map needs to be a little bit more faded", 13 Nov); below it, where nothing is read,
// `below` and `chroma`. Each live agent's pool of colour at `pool` strength.
export const WASH = { above: 0.9, below: 0.56, chroma: 1.1, chromaAbove: 0.35, pool: 0.5 };

// How far a live agent's clearing reaches, in map units: past its mark and onto its name's start.
const CLEARING = 50;

// A hex colour as OKLab [L, a, b].
function labOf(hex) { const [L, C, h] = oklch(hex), r = (h * Math.PI) / 180; return [L, C * Math.cos(r), C * Math.sin(r)]; }

// The map's fade: each pixel's OKLab lightness pressed into [floor, the paper's], the floor u_above
// above the now line (u_now, from the top) and u_below beneath it, its colour scaled by u_chroma.
const FADE = `precision highp float;
uniform sampler2D u_img; uniform vec2 u_res;
uniform float u_now, u_soft, u_top, u_above, u_below, u_chroma, u_chroma_above, u_duo;
uniform vec3 u_tone;
vec3 lin(vec3 c) { return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(0.04045, c)); }
vec3 gam(vec3 c) { return mix(12.92 * c, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(0.0031308, c)); }
void main() {
  vec2 uv = vec2(gl_FragCoord.x / u_res.x, 1.0 - gl_FragCoord.y / u_res.y);
  vec3 c = lin(texture2D(u_img, uv).rgb);
  vec3 lms = pow(max(mat3(0.4122214708, 0.2119034982, 0.0883024619, 0.5363325363, 0.6806995451, 0.2817188376, 0.0514459929, 0.1073969566, 0.6299787005) * c, 0.0), vec3(1.0 / 3.0));
  vec3 lab = mat3(0.2104542553, 1.9779984951, 0.0259040371, 0.7936177850, -2.4285922050, 0.7827717662, -0.0040720468, 0.4505937099, -0.8086757660) * lms;
  float below = smoothstep(u_now - u_soft, u_now + u_soft, uv.y);
  float floor_ = mix(u_above, u_below, below), t = clamp(lab.x, 0.0, 1.0);
  lab.x = floor_ + (u_top - floor_) * t;
  // A dark painting's shadows take the theme's tone as they lift, rather than going grey.
  lab.yz = mix(lab.yz * mix(u_chroma_above, u_chroma, below), u_tone.yz * (1.0 - t), u_duo);
  vec3 m = mat3(1.0, 1.0, 1.0, 0.3963377774, -0.1055613458, -0.0894841775, 0.2158037573, -0.0638541728, -1.2914855480) * lab;
  vec3 o = mat3(4.0767416621, -1.2684380046, -0.0041960863, -3.3077115913, 2.6097574011, -0.7034186147, 0.2309699292, -0.3413193965, 1.7076147010) * (m * m * m);
  gl_FragColor = vec4(gam(clamp(o, 0.0, 1.0)), 1.0);
}`;
