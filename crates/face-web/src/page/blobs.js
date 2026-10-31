// ─── Blobs: a session's spinner, as metaballs ──────────────────────────────
// A session drawn as one blob of four balls. The centre ball is the session
// itself (its role letter); around it sit the model (the Claude mark), tool
// calls (a gear) and Max (a microphone). The one it waits on grows out of
// the centre and merges with it; going from one to another, the growth
// passes through the centre. Each piece of data sends a soft disk out from
// behind the active ball, at most one each 0.2 s; compactions are dots on a
// track round the blob's edge.
//
// Every blob of a layer is drawn in one pass: one WebGL canvas, and one
// instanced square per blob whose fragment shader computes the field per
// pixel. The blobs' state is struct-of-arrays in typed arrays, and nothing is
// allocated per frame. Only the compaction dots need the CPU: marching
// squares traces the track, and a second draw puts a dot per quad. A blob
// off its canvas is neither animated nor drawn, and while a layer shows no
// blob, nothing runs at all.
//
//   const s = session();               what one session is doing
//   const blobs = layer(host, scroller);
//   const box = blobs.add(56, s);      an empty box the canvas draws a blob in
//   s.set("streaming"); s.packet();    and the blob follows
//
// The canvas lives in `host`, the content `scroller` scrolls natively, so
// the compositor moves it with the text frame for frame: it is as wide as
// `host` and a quarter more than the scroller's height, and each frame it
// moves to an eighth of a viewport above the scroll position and draws
// there. A scroll of up to an eighth of a viewport between two frames stays
// drawn; a faster fling clips a ball at the edge.

// Colours: [fill, edge, icon] per state; the whole blob takes the state's.
const CLAUDE = [217, 119, 87];
const C = { ink: [20, 20, 19], paper: [250, 249, 245], maroon: [122, 31, 34], green: [79, 138, 85], red: [179, 53, 44],
  faint: [161, 159, 150], muted: [107, 106, 100], slate: [52, 66, 84] };
const tint = (c, k) => C.paper.map((p, i) => p + (c[i] - p) * k);

// The balls, in field units (a blob's box spans -SPAN..SPAN). Satellites sit
// at fixed angles: the model at twelve o'clock, tools at four, Max at eight.
const SATS = { model: -Math.PI / 2, tools: Math.PI / 6, max: 5 * Math.PI / 6 };
const SPAN = 1.45;

// Joined or apart. A satellite sits at the centre's radius plus its own plus
// a gap: the distance between their circles. Two circles a gap g apart meet,
// under the smooth minimum, at g/2 - K/4 midway between them, so they join
// while g < K/2. The neck thins as g nears that: it stays at least half the
// smaller ball's diameter while g <= .03. So a joined satellite's gap is at
// most .03 - WOB, and its wobble moves only the gap, by WOB; nothing else
// does, and the gap's spring never overshoots. Every satellite stays joined,
// except while starting: there each one buds off the centre, pinches off and
// fades.
const WOB = .025;
const IDLE = { r: .17, g: 0 }, ACTIVE = { r: .42, g: -.05 };

// A state: which ball is active, the centre's radius, the idle satellites'
// gap if not IDLE's, and its colours.
export const STATES = {
  first:     { active: "model", centre: .3, col: [tint(CLAUDE, .14), CLAUDE, CLAUDE] },
  streaming: { active: "model", centre: .3, col: [tint(CLAUDE, .22), CLAUDE, CLAUDE] },
  brain:     { active: "centre", centre: .54, gap: -.15, col: [tint(C.ink, .1), C.ink, C.ink] },
  tools:     { active: "tools", centre: .3, col: [tint(C.slate, .13), C.slate, C.slate] },
  listening: { active: "tools", centre: .34, size: .32, col: [tint(C.slate, .06), C.faint, C.faint] },
  max:       { active: "max", centre: .3, col: [C.maroon, C.maroon, C.paper] },
  held:      { active: "model", centre: .32, size: .36, still: true, col: [tint(C.red, .12), C.red, C.red] },
  starting:  { active: null, centre: .4, col: [C.paper, C.faint, C.ink] },
  paused:    { active: null, centre: .48, merged: true, col: [[233, 229, 217], C.faint, C.muted] },
  done:      { active: null, centre: .48, merged: true, col: [C.green, C.green, C.paper] },
};
const satOf = (s) => (STATES[s].active in SATS ? STATES[s].active : null);

// A session: its state, when its last packet came, its compactions and its
// role letter, shared by every blob of it. Going from one satellite straight
// to another, it shows the brain for a moment, so the growth passes through
// the centre; `jump` goes straight there, for a blob now showing another
// session.
export function session(state = "starting") {
  return { state, since: performance.now(), via: 0, pips: 0, letter: "?", lastPacket: 0,
    shown(now) { return now < this.via ? "brain" : this.state; },
    set(next, jump = false) {
      if (next === this.state && !jump) return;
      const now = performance.now(), a = satOf(this.shown(now)), b = satOf(next);
      this.via = !jump && a && b && a !== b ? now + 260 : 0;
      this.state = next; this.since = now;
    },
    packet() { this.lastPacket = performance.now(); } };
}

// Icons, 24x24, as Path2D. The Claude mark is Anthropic's (via Simple Icons);
// the gear and microphone are Lucide's (ISC).
const ICON = {
  model: { fill: true, path: "m4.7144 15.9555 4.7174-2.6471.079-.2307-.079-.1275h-.2307l-.7893-.0486-2.6956-.0729-2.3375-.0971-2.2646-.1214-.5707-.1215-.5343-.7042.0546-.3522.4797-.3218.686.0608 1.5179.1032 2.2767.1578 1.6514.0972 2.4468.255h.3886l.0546-.1579-.1336-.0971-.1032-.0972L6.973 9.8356l-2.55-1.6879-1.3356-.9714-.7225-.4918-.3643-.4614-.1578-1.0078.6557-.7225.8803.0607.2246.0607.8925.686 1.9064 1.4754 2.4893 1.8336.3643.3035.1457-.1032.0182-.0728-.164-.2733-1.3539-2.4467-1.445-2.4893-.6435-1.032-.17-.6194c-.0607-.255-.1032-.4674-.1032-.7285L6.287.1335 6.6997 0l.9957.1336.419.3642.6192 1.4147 1.0018 2.2282 1.5543 3.0296.4553.8985.2429.8318.091.255h.1579v-.1457l.1275-1.706.2368-2.0947.2307-2.6957.0789-.7589.3764-.9107.7468-.4918.5828.2793.4797.686-.0668.4433-.2853 1.8517-.5586 2.9021-.3643 1.9429h.2125l.2429-.2429.9835-1.3053 1.6514-2.0643.7286-.8196.85-.9046.5464-.4311h1.0321l.759 1.1293-.34 1.1657-1.0625 1.3478-.8804 1.1414-1.2628 1.7-.7893 1.36.0729.1093.1882-.0183 2.8535-.607 1.5421-.2794 1.8396-.3157.8318.3886.091.3946-.3278.8075-1.967.4857-2.3072.4614-3.4364.8136-.0425.0304.0486.0607 1.5482.1457.6618.0364h1.621l3.0175.2247.7892.522.4736.6376-.079.4857-1.2142.6193-1.6393-.3886-3.825-.9107-1.3113-.3279h-.1822v.1093l1.0929 1.0686 2.0035 1.8092 2.5075 2.3314.1275.5768-.3218.4554-.34-.0486-2.2039-1.6575-.85-.7468-1.9246-1.621h-.1275v.17l.4432.6496 2.3436 3.5214.1214 1.0807-.17.3521-.6071.2125-.6679-.1214-1.3721-1.9246L14.38 17.959l-1.1414-1.9428-.1397.079-.674 7.2552-.3156.3703-.7286.2793-.6071-.4614-.3218-.7468.3218-1.4753.3886-1.9246.3157-1.53.2853-1.9004.17-.6314-.0121-.0425-.1397.0182-1.4328 1.9672-2.1796 2.9446-1.7243 1.8456-.4128.164-.7164-.3704.0667-.6618.4008-.5889 2.386-3.0357 1.4389-1.882.929-1.0868-.0062-.1579h-.0546l-6.3385 4.1164-1.1293.1457-.4857-.4554.0608-.7467.2307-.2429 1.9064-1.3114Z" },
  tools: { fill: false, path: "M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0" },
  max: { fill: false, path: "M12 19v3M19 10v2a7 7 0 0 1-14 0v-2M9 5a3 3 0 0 1 6 0v7a3 3 0 0 1-6 0z" },
};

const clamp = (x, lo, hi) => Math.min(hi, Math.max(lo, x));
const SAT_KEYS = Object.keys(SATS), SAT_ANG = SAT_KEYS.map((k) => SATS[k]);
const K = .22; // the smooth minimum's reach: balls closer than this melt together
const DISKS = 8, DISK_MS = 1400, DISK_GAP = 200; // at most one disk per 0.2 s
const INST = 44; // floats per blob instance: 11 vec4

// ─── The atlas, shared by every layer ──────────────────────────────────────
// Signed distance fields of the icons and the role letters, in 128 px cells,
// 8 to a row. A cell is drawn on a 2D canvas and turned into distances by the
// Felzenszwalb transform, with the edge pixels' coverage kept as sub-pixel
// distance (as Mapbox's tiny-sdf does). Each layer uploads it when it grows.
const CELL = 128, COLS = 8, glyphs = new Map();
let atlas = new Float32Array(0), atlasRows = 0, atlasVersion = 0;
function addCell(draw) {
  const n = glyphs.size, row = Math.floor(n / COLS);
  if (row >= atlasRows) {
    const grown = new Float32Array(COLS * CELL * CELL * (row + 1)).fill(1e3);
    grown.set(atlas); atlas = grown; atlasRows = row + 1;
  }
  const c = document.createElement("canvas");
  c.width = c.height = CELL;
  const g = c.getContext("2d", { willReadFrequently: true });
  draw(g);
  const a = g.getImageData(0, 0, CELL, CELL).data, N = CELL * CELL, out = new Float64Array(N), inn = new Float64Array(N);
  for (let j = 0; j < N; j++) {
    const v = a[4 * j + 3] / 255;
    out[j] = v === 1 ? 0 : v === 0 ? 1e20 : Math.max(0, .5 - v) ** 2;
    inn[j] = v === 1 ? 1e20 : v === 0 ? 0 : Math.max(0, v - .5) ** 2;
  }
  edt(out); edt(inn);
  const x0 = (n % COLS) * CELL, y0 = row * CELL;
  for (let y = 0; y < CELL; y++) for (let x = 0; x < CELL; x++) {
    const j = y * CELL + x;
    atlas[(y0 + y) * COLS * CELL + x0 + x] = Math.sqrt(out[j]) - Math.sqrt(inn[j]);
  }
  atlasVersion++;
  return n;
}
// Squared Euclidean distance transform, in place, of a CELL x CELL grid.
const eF = new Float64Array(CELL), eD = new Float64Array(CELL), eV = new Int32Array(CELL), eZ = new Float64Array(CELL + 1);
function edt(grid) {
  for (let pass = 0; pass < 2; pass++) for (let a = 0; a < CELL; a++) {
    const at = (b) => (pass ? a * CELL + b : b * CELL + a);
    for (let b = 0; b < CELL; b++) eF[b] = grid[at(b)];
    let k = 0;
    eV[0] = 0; eZ[0] = -1e20; eZ[1] = 1e20;
    for (let q = 1; q < CELL; q++) {
      let s;
      while ((s = (eF[q] + q * q - eF[eV[k]] - eV[k] * eV[k]) / (2 * q - 2 * eV[k])) <= eZ[k]) k--;
      k++; eV[k] = q; eZ[k] = s; eZ[k + 1] = 1e20;
    }
    k = 0;
    for (let q = 0; q < CELL; q++) { while (eZ[k + 1] < q) k++; eD[q] = (q - eV[k]) ** 2 + eF[eV[k]]; }
    for (let b = 0; b < CELL; b++) grid[at(b)] = eD[b];
  }
}
// The icons take the first cells; a letter gets its cell when first shown.
function glyph(ch) {
  if (!glyphs.size) for (const k of SAT_KEYS) glyphs.set("icon " + k, addCell((g) => {
    const ic = ICON[k], path = new Path2D(ic.path);
    g.setTransform(4, 0, 0, 4, 16, 16);
    if (ic.fill) g.fill(path);
    else { g.lineWidth = 2; g.lineCap = g.lineJoin = "round"; g.stroke(path); }
  }));
  if (!glyphs.has("letter " + ch)) glyphs.set("letter " + ch, addCell((g) => {
    g.font = `600 80px ${getComputedStyle(document.body).getPropertyValue("--serif")}`;
    g.textAlign = "center"; g.textBaseline = "middle";
    g.fillText(ch, CELL / 2, CELL / 2);
  }));
  return glyphs.get("letter " + ch);
}

// ─── The shaders ───────────────────────────────────────────────────────────
const BLOB_VS = `#version 300 es
layout(location = 0) in vec2 corner;
${[...Array(11)].map((_, k) => `layout(location = ${k + 1}) in vec4 a${k};`).join("\n")}
uniform vec2 view;
out vec2 p;
${[...Array(11)].map((_, k) => `flat out vec4 v${k};`).join("\n")}
void main() {
  vec2 dev = a0.xy + corner * a0.z;
  gl_Position = vec4(dev.x / view.x * 2. - 1., 1. - dev.y / view.y * 2., 0., 1.);
  p = (corner * 2. - 1.) * ${SPAN};
  ${[...Array(11)].map((_, k) => `v${k} = a${k};`).join(" ")}
}`;
// v0: device x, y, size, field units per css px. v1: centre radius, active
// ball (0 centre, 1-3 a satellite), letter cell, letter size. v2-v4: the
// satellites' x, y, r and icon alpha. v5-v7: fill, edge and icon colours,
// with the model's and gear's turns and nothing. v8: letter colour. v9-v10:
// the disks' ages, 0-1, or -1.
const BLOB_FS = `#version 300 es
precision highp float;
in vec2 p;
${[...Array(11)].map((_, k) => `flat in vec4 v${k};`).join("\n")}
uniform sampler2D atlas;
uniform vec2 atlasSize;
out vec4 o;
const float K = ${K}, CELL = ${CELL}., COLS = ${COLS}.;
float smin(float f, vec3 b) {
  if (b.z < .005) return f;
  float d = length(p - b.xy) - b.z, h = max(K - abs(f - d), 0.) / K;
  return min(f, d) - h * h * K * .25;
}
vec4 over(vec4 dst, vec3 c, float a) { return vec4(c * a, a) + dst * (1. - a); }
// Signed distance, in atlas pixels, at q in a cell's pixels.
float sdf(float cell, vec2 q) {
  q = clamp(q, .5, CELL - .5);
  return texture(atlas, (vec2(mod(cell, COLS), floor(cell / COLS)) * CELL + q) / atlasSize).r;
}
vec4 disk(vec4 c, vec3 src, float age, float unit) {
  if (age < 0.) return c;
  float e = 1. - pow(1. - age, 3.), d = (length(p - src.xy) - src.z - .34 * e) * unit;
  return over(c, v6.rgb, .14 * pow(1. - age, 1.6) * clamp(.5 - d, 0., 1.));
}
// An icon, 24 units square, centred on its ball and turned by a; the atlas
// holds it at 4 px a unit with a 4-unit margin. A stroked icon's distance is
// from a 2-unit stroke; filled, from its fill.
vec4 icon(vec4 c, vec4 b, float a, float cell, bool filled, vec3 col, float unit, float css) {
  if (b.w <= 0.) return c;
  float s = b.z * 1.15 / 24., cs = cos(a), sn = sin(a);
  vec2 q = mat2(cs, -sn, sn, cs) * (p - b.xy) / s + 12.;
  float d = sdf(cell, (q + 4.) * 4.) / 4., px = 1. / (s * unit);
  float cov = filled ? clamp(.5 - d / px, 0., 1.)
                     : clamp((max(2., 1.4 * css / s) * .5 - (d + 1.)) / px + .5, 0., 1.);
  return over(c, col, b.w * cov);
}
void main() {
  float unit = v0.z / ${2 * SPAN}, css = v0.w, dpr = css * unit;
  float f = smin(smin(smin(length(p) - v1.x, v2.xyz), v3.xyz), v4.xyz);
  float fpx = f / max(length(vec2(dFdx(f), dFdy(f))), 1e-6); // device px

  int act = int(v1.y);
  vec3 src = act == 1 ? v2.xyz : act == 2 ? v3.xyz : act == 3 ? v4.xyz : vec3(0., 0., v1.x);
  vec4 c = vec4(0.);
  c = disk(c, src, v9.x, unit); c = disk(c, src, v9.y, unit); c = disk(c, src, v9.z, unit); c = disk(c, src, v9.w, unit);
  c = disk(c, src, v10.x, unit); c = disk(c, src, v10.y, unit); c = disk(c, src, v10.z, unit); c = disk(c, src, v10.w, unit);

  c = over(c, v5.rgb, clamp(.5 - fpx, 0., 1.));
  c = over(c, v6.rgb, clamp(.65 * dpr + .5 - abs(fpx), 0., 1.));

  c = icon(c, v2, v5.w, 0., true, act == 1 ? v7.rgb : v6.rgb, unit, css);
  c = icon(c, v3, v6.w, 1., false, act == 2 ? v7.rgb : v6.rgb, unit, css);
  c = icon(c, v4, 0., 2., false, act == 3 ? v7.rgb : v6.rgb, unit, css);

  float k = v1.w / 80.; // the letter is drawn 80 px high in its cell
  float d = sdf(v1.z, (p - vec2(0., v1.x * .05)) / k + CELL / 2.) * k * unit;
  o = over(c, v8.rgb, .92 * clamp(.5 - d, 0., 1.));
}`;
const DOT_VS = `#version 300 es
layout(location = 0) in vec2 corner;
layout(location = 1) in vec4 dot; // device x, y, radius, alpha
uniform vec2 view;
out vec2 q;
flat out vec4 d;
void main() {
  q = dot.xy + (corner * 2. - 1.) * (dot.z + 1.);
  gl_Position = vec4(q.x / view.x * 2. - 1., 1. - q.y / view.y * 2., 0., 1.);
  d = dot;
}`;
const DOT_FS = `#version 300 es
precision highp float;
in vec2 q;
flat in vec4 d;
out vec4 o;
void main() { float a = d.w * clamp(d.z + .5 - length(q - d.xy), 0., 1.); o = vec4(${C.ink.map((x) => (x / 255).toFixed(4))} , 1.) * a; }`;

// ─── The compaction track ──────────────────────────────────────────────────
// Marching squares over an n x n grid of the field at one level, keeping only
// its longest loop. Scratch sized for the finest grid, reused by every layer.
const TN = 64, TM = TN + 1, TE = 2 * TM * TM;
const tVal = new Float32Array(TM * TM), tNb = new Int32Array(2 * TE), tPx = new Float32Array(TE), tPy = new Float32Array(TE), tSeen = new Uint8Array(TE);
let loopA = new Float32Array(2 * TE), loopB = new Float32Array(2 * TE);
const ball = new Float32Array(12); // x, y, r of the traced blob's four balls
function fieldAt(x, y) {
  let f = 1e9;
  for (let b = 0; b < 12; b += 3) {
    const r = ball[b + 2];
    if (r < .005) continue;
    const dx = x - ball[b], dy = y - ball[b + 1], d = Math.sqrt(dx * dx + dy * dy) - r;
    const h = Math.max(K - Math.abs(f - d), 0) / K;
    f = Math.min(f, d) - h * h * K * .25;
  }
  return f;
}
let tstep = 0;
// Edge ids: 2 * vertex + 0 for the edge going right, + 1 for the edge going down.
function cross(e, i, j, a, b, di, dj) {
  const k = a / (a - b);
  tPx[e] = -SPAN + (i + di * k) * tstep; tPy[e] = -SPAN + (j + dj * k) * tstep;
  return e;
}
function link(a, b) { tNb[2 * a + (tNb[2 * a] >= 0)] = b; tNb[2 * b + (tNb[2 * b] >= 0)] = a; }
function loopLength(L, n) {
  let s = 0;
  for (let k = 0; k < n; k++) { const l = (k + 1) % n; s += Math.hypot(L[2 * l] - L[2 * k], L[2 * l + 1] - L[2 * k + 1]); }
  return s;
}
// The longest loop's points into loopB; returns how many.
function longestLoop(n, lv) {
  const m = n + 1, E = 2 * m * m;
  tstep = 2 * SPAN / n;
  // Only within the balls' bounds, grown by the level and the smooth
  // minimum's reach: everywhere else is outside.
  let x0 = SPAN, x1 = -SPAN, y0 = SPAN, y1 = -SPAN;
  for (let b = 0; b < 12; b += 3) {
    const r = ball[b + 2] + lv + K / 4;
    if (ball[b + 2] < .005) continue;
    x0 = Math.min(x0, ball[b] - r); x1 = Math.max(x1, ball[b] + r); y0 = Math.min(y0, ball[b + 1] - r); y1 = Math.max(y1, ball[b + 1] + r);
  }
  const i0 = clamp(Math.floor((x0 + SPAN) / tstep), 0, n), i1 = clamp(Math.ceil((x1 + SPAN) / tstep), 0, n);
  const j0 = clamp(Math.floor((y0 + SPAN) / tstep), 0, n), j1 = clamp(Math.ceil((y1 + SPAN) / tstep), 0, n);
  tVal.fill(1, 0, m * m);
  for (let j = j0; j <= j1; j++) for (let i = i0; i <= i1; i++) tVal[j * m + i] = fieldAt(-SPAN + i * tstep, -SPAN + j * tstep) - lv;
  tNb.fill(-1, 0, 2 * E); tSeen.fill(0, 0, E);
  for (let j = Math.max(0, j0 - 1); j < Math.min(n, j1 + 1); j++) for (let i = Math.max(0, i0 - 1); i < Math.min(n, i1 + 1); i++) {
    const a0 = tVal[j * m + i], a1 = tVal[j * m + i + 1], a2 = tVal[(j + 1) * m + i + 1], a3 = tVal[(j + 1) * m + i];
    const v0 = a0 < 0, v1 = a1 < 0, v2 = a2 < 0, v3 = a3 < 0;
    if (v0 === v1 && v1 === v2 && v2 === v3) continue;
    const e0 = v0 !== v1 ? cross(2 * (j * m + i), i, j, a0, a1, 1, 0) : -1;
    const e1 = v1 !== v2 ? cross(2 * (j * m + i + 1) + 1, i + 1, j, a1, a2, 0, 1) : -1;
    const e2 = v3 !== v2 ? cross(2 * ((j + 1) * m + i), i, j + 1, a3, a2, 1, 0) : -1;
    const e3 = v0 !== v3 ? cross(2 * (j * m + i) + 1, i, j, a0, a3, 0, 1) : -1;
    if (e0 >= 0 && e1 >= 0 && e2 >= 0 && e3 >= 0) {
      // A saddle: the cell's centre decides which corners join.
      if ((a0 + a1 + a2 + a3) / 4 < 0 === v0) { link(e0, e1); link(e2, e3); } else { link(e3, e0); link(e1, e2); }
    } else link(e0 >= 0 ? e0 : e1 >= 0 ? e1 : e2, e3 >= 0 ? e3 : e2 >= 0 ? e2 : e1);
  }
  let best = -1, bestN = 0;
  for (let s = 0; s < E; s++) {
    if (tNb[2 * s] < 0 || tSeen[s]) continue;
    let n2 = 0, prev = -1, cur = s;
    while (!tSeen[cur]) {
      tSeen[cur] = 1;
      loopA[n2++] = tPx[cur]; loopA[n2++] = tPy[cur];
      const next = tNb[2 * cur] !== prev ? tNb[2 * cur] : tNb[2 * cur + 1];
      prev = cur; cur = next;
      if (cur < 0) break;
    }
    if (n2 < 6) continue;
    const len = loopLength(loopA, n2 / 2);
    if (len > best) { best = len; bestN = n2 / 2; const t = loopA; loopA = loopB; loopB = t; }
  }
  return bestN;
}

// The compaction dots on the track (the outline offset outward): the first at
// twelve o'clock, then clockwise, each the pitch further along the track and
// at least the pitch away, in a straight line, from every dot before it, so a
// neck in the blob doesn't bunch them. Few dots keep the pitch; many shrink it
// to share the track. Into dotX, dotY; returns how many.
const MAXDOTS = 64, dotX = new Float32Array(MAXDOTS), dotY = new Float32Array(MAXDOTS);
const wX = new Float32Array(TE + 1), wY = new Float32Array(TE + 1), wS = new Float32Array(TE + 1);
let pn = 0, flip = false, seg = 0, qx = 0, qy = 0;
const at = (k) => (flip ? pn - 1 - (k % pn) : k % pn); // clockwise on screen, y down
function along(n, v) {
  while (seg < n - 2 && wS[seg + 1] < v) seg++; // n points walked
  const f = (v - wS[seg]) / (wS[seg + 1] - wS[seg] || 1);
  qx = wX[seg] + (wX[seg + 1] - wX[seg]) * f; qy = wY[seg] + (wY[seg + 1] - wY[seg]) * f;
}
function near(out, p) {
  for (let d = 0; d < out; d++) if (Math.hypot(qx - dotX[d], qy - dotY[d]) < p * .98) return true;
  return false;
}
function placeDots(n, count, pitch) {
  const L = loopB;
  let a = 0;
  for (let k = 0; k < n; k++) { const l = (k + 1) % n; a += L[2 * k] * L[2 * l + 1] - L[2 * l] * L[2 * k + 1]; }
  pn = n; flip = a < 0;
  // Start where the track crosses the upward vertical through the centre.
  let start = 0, sf = 0, top = Infinity;
  for (let k = 0; k < n; k++) {
    const ax = L[2 * at(k)], ay = L[2 * at(k) + 1], bx = L[2 * at(k + 1)], by = L[2 * at(k + 1) + 1];
    if ((ax < 0) !== (bx < 0)) {
      const f = ax / (ax - bx), y = ay + (by - ay) * f;
      if (y < top) { top = y; start = k; sf = f; }
    }
  }
  // Arc length from the start, walking the polyline.
  const x0 = L[2 * at(start)], y0 = L[2 * at(start) + 1];
  let px = x0 + (L[2 * at(start + 1)] - x0) * sf, py = y0 + (L[2 * at(start + 1) + 1] - y0) * sf, s = 0;
  wX[0] = px; wY[0] = py; wS[0] = 0;
  for (let k = 1; k <= n; k++) {
    const x = L[2 * at(start + k)], y = L[2 * at(start + k) + 1];
    s += Math.hypot(x - px, y - py);
    wX[k] = x; wY[k] = y; wS[k] = s;
    px = x; py = y;
  }
  const p = Math.min(pitch, s / count), want = Math.min(count, MAXDOTS);
  let u = 0, out = 0;
  seg = 0;
  along(n + 1, 0); dotX[out] = qx; dotY[out++] = qy;
  while (out < want) {
    u += p; along(n + 1, u);
    while (u < s && near(out, p)) { u += p * .05; along(n + 1, u); }
    if (u >= s - p * .5) break;
    dotX[out] = qx; dotY[out++] = qy;
  }
  return out;
}
// The track and the dots' pitch, in blob units, laid out as on a 260 px blob
// whatever the size: a small blob's dots are a scaled copy of a big one's.
const TRACK_PX = 260, TRACK_LEVEL = .05 + 3 * 2 * SPAN / TRACK_PX, TRACK_PITCH = Math.max(.2, 9 * 2 * SPAN / TRACK_PX);

// A spring with a little give, so balls overshoot and settle like liquid;
// critically damped (26), it settles without overshooting.
function pull(v, vel, i, goal, dt, damp) {
  vel[i] += ((goal - v[i]) * 170 - vel[i] * damp) * dt;
  v[i] += vel[i] * dt;
}

function program(gl, vs, fs) {
  const p = gl.createProgram();
  for (const [type, src] of [[gl.VERTEX_SHADER, vs], [gl.FRAGMENT_SHADER, fs]]) {
    const s = gl.createShader(type);
    gl.shaderSource(s, src); gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
    gl.attachShader(p, s);
  }
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
  return p;
}

// ─── The frame loop, shared by every layer ─────────────────────────────────
// It runs while some layer shows a blob, and stops when none does.
const layers = [];
let running = false, last = 0;
function wake() {
  if (running) return;
  running = true; last = performance.now();
  requestAnimationFrame(frame);
}
function frame(now) {
  const dt = Math.min(1 / 30, (now - last) / 1000);
  last = now;
  let shown = false;
  for (const l of layers) shown = l.draw(now, dt) || shown;
  if (shown) requestAnimationFrame(frame);
  else running = false;
}

// ─── A layer: one canvas, and every blob in it ─────────────────────────────
export function layer(host, scroller) {
  const clip = document.createElement("div"), cv = document.createElement("canvas");
  clip.style.cssText = "position: absolute; inset: 0; overflow: clip; pointer-events: none; z-index: 1";
  cv.style.cssText = "position: absolute; left: 0; top: 0";
  clip.append(cv);
  host.append(clip);
  const gl = cv.getContext("webgl2", { antialias: false });

  const blobProg = program(gl, BLOB_VS, BLOB_FS), dotProg = program(gl, DOT_VS, DOT_FS);
  const blobView = gl.getUniformLocation(blobProg, "view"), blobAtlas = gl.getUniformLocation(blobProg, "atlasSize"), dotView = gl.getUniformLocation(dotProg, "view");
  const quad = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, quad);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([0, 0, 1, 0, 0, 1, 1, 1]), gl.STATIC_DRAW);
  // A vertex array per program: the quad's corners, then per-instance vec4s.
  const vao = (buf, vec4s) => {
    const v = gl.createVertexArray();
    gl.bindVertexArray(v);
    gl.bindBuffer(gl.ARRAY_BUFFER, quad);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    for (let k = 0; k < vec4s; k++) {
      gl.enableVertexAttribArray(k + 1);
      gl.vertexAttribPointer(k + 1, 4, gl.FLOAT, false, vec4s * 16, k * 16);
      gl.vertexAttribDivisor(k + 1, 1);
    }
    return v;
  };
  const instBuf = gl.createBuffer(), dotBuf = gl.createBuffer();
  const blobVao = vao(instBuf, INST / 4), dotVao = vao(dotBuf, 1);
  gl.enable(gl.BLEND);
  gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
  const atlasTex = gl.createTexture();
  let uploaded = -1;

  // The blobs, struct-of-arrays: per blob, its box, the session it shows,
  // where its box is in `host`, its balls' radii and gaps and their
  // velocities, its colours, turns and disks.
  let cap = 0, count = 0;
  const boxes = [], sessions = [];
  const a = {};
  const FIELDS = [["SIZE", Float32Array, 1], ["DOCX", Float32Array, 1], ["DOCY", Float32Array, 1],
    ["R", Float32Array, 4], ["RV", Float32Array, 4], ["G", Float32Array, 3], ["GV", Float32Array, 3],
    ["X", Float32Array, 3], ["Y", Float32Array, 3], ["COL", Float32Array, 9], ["SPIN", Float32Array, 1], ["GEAR", Float32Array, 1],
    ["DISK", Float64Array, DISKS], ["DISKAT", Uint8Array, 1], ["LASTDISK", Float64Array, 1], ["SEEN", Float64Array, 1]];
  let inst = new Float32Array(0), dots = new Float32Array(4 * 64);
  const grow = () => {
    const n = Math.max(8, cap * 2);
    for (const [name, A, per] of FIELDS) { const g = new A(n * per); if (a[name]) g.set(a[name]); a[name] = g; }
    inst = new Float32Array(n * INST);
    cap = n;
  };

  // Where each box is in `host`, read only when the layout changes. A box
  // not in the page is put far off.
  let dirty = true, hostTop = 0;
  const resized = new ResizeObserver(() => { dirty = true; wake(); });
  resized.observe(host);
  const measure = () => {
    const h = host.getBoundingClientRect(), k = host.offsetWidth ? h.width / host.offsetWidth : 1;
    hostTop = (h.top - scroller.getBoundingClientRect().top) / k + scroller.scrollTop;
    for (let i = 0; i < count; i++) {
      const r = boxes[i].getBoundingClientRect();
      a.DOCX[i] = r.width ? (r.left - h.left) / k : -1e9; a.DOCY[i] = (r.top - h.top) / k;
    }
    dirty = false;
  };

  let cvTop = -1, cvW = 0, cvH = 0;
  const kept = {
    // An empty box of css size px, for the canvas to draw `s`'s blob in.
    add(px, s) {
      const box = document.createElement("div");
      box.className = "blob";
      box.style.width = box.style.height = px + "px";
      if (count === cap) grow();
      const i = count++;
      boxes[i] = box; sessions[i] = s;
      a.SIZE[i] = px; a.LASTDISK[i] = -1e9; a.SEEN[i] = s.lastPacket; a.R[4 * i] = .4;
      for (let k = 0; k < 3; k++) { a.R[4 * i + 1 + k] = IDLE.r; a.G[3 * i + k] = IDLE.g; }
      a.COL.set(STATES[s.state].col.flat(), 9 * i);
      kept.moved();
      return box;
    },
    // Forget a box: the last blob takes its place.
    remove(box) {
      const i = boxes.indexOf(box);
      if (i < 0) return;
      const j = --count;
      boxes[i] = boxes[j]; sessions[i] = sessions[j]; boxes.length = sessions.length = count;
      for (const [name, , per] of FIELDS) a[name].copyWithin(i * per, j * per, (j + 1) * per);
      kept.moved();
    },
    // Boxes moved without `host` changing size.
    moved() { dirty = true; wake(); },
    draw(now, dt) {
      if (dirty) measure();
      const w = host.clientWidth, h = Math.round(1.25 * scroller.clientHeight);
      if (w !== cvW || h !== cvH) { cvW = w; cvH = h; cv.style.width = w + "px"; cv.style.height = h + "px"; }
      const top = Math.max(0, Math.round(scroller.scrollTop - hostTop - scroller.clientHeight / 8));
      if (top !== cvTop) { cvTop = top; cv.style.transform = `translateY(${top}px)`; }
      const dpr = Math.min(3, devicePixelRatio || 1), W = Math.round(cvW * dpr), H = Math.round(cvH * dpr);
      if (cv.width !== W || cv.height !== H) { cv.width = W; cv.height = H; }
      let drawn = 0, ndots = 0, shown = false;
      const kc = 1 - Math.exp(-dt / .18);
      for (let i = 0; i < count; i++) {
        if (a.DOCX[i] < -1e8) continue;
        shown = true;
        const px = a.SIZE[i], x = a.DOCX[i] * dpr, y = (a.DOCY[i] - top) * dpr, size = px * dpr;
        if (x + size < 0 || y + size < 0 || x > W || y > H) continue;
        drawn = this.blob(i, drawn, now, dt, kc, x, y, size);
        ndots = this.dots(i, ndots, x, y, size);
      }
      gl.viewport(0, 0, W, H);
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      if (uploaded !== atlasVersion) {
        gl.bindTexture(gl.TEXTURE_2D, atlasTex);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.R16F, COLS * CELL, atlasRows * CELL, 0, gl.RED, gl.FLOAT, atlas);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
        uploaded = atlasVersion;
      }
      if (drawn) {
        gl.useProgram(blobProg);
        gl.uniform2f(blobView, W, H);
        gl.uniform2f(blobAtlas, COLS * CELL, atlasRows * CELL);
        gl.bindBuffer(gl.ARRAY_BUFFER, instBuf);
        gl.bufferData(gl.ARRAY_BUFFER, inst.subarray(0, INST * drawn), gl.STREAM_DRAW);
        gl.bindVertexArray(blobVao);
        gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, drawn);
      }
      if (ndots) {
        gl.useProgram(dotProg);
        gl.uniform2f(dotView, W, H);
        gl.bindBuffer(gl.ARRAY_BUFFER, dotBuf);
        gl.bufferData(gl.ARRAY_BUFFER, dots.subarray(0, 4 * ndots), gl.STREAM_DRAW);
        gl.bindVertexArray(dotVao);
        gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, ndots);
      }
      return shown;
    },
    // Blob i, animated a step and written as instance `o`; returns the next.
    blob(i, o, now, dt, kc, x, y, size) {
      const { R, RV, G, GV, X, Y, COL, SPIN, GEAR, DISK, DISKAT, LASTDISK, SEEN } = a;
      const s = sessions[i], name = s.shown(now), st = STATES[name], t = (now - s.since) / 1000, px = a.SIZE[i];

      // The balls: each radius and gap springs towards the state's.
      pull(R, RV, 4 * i, st.centre, dt, 17);
      const c0 = R[4 * i];
      for (let k = 0; k < 3; k++) {
        let gr = IDLE.r, gg = st.gap ?? IDLE.g;
        if (st.merged) { gr = .12; gg = -.45; }
        if (st.active === SAT_KEYS[k]) { gr = st.size || ACTIVE.r; gg = ACTIVE.g; }
        if (name === "first" && k === 0) gr = .36 + .06 * Math.sin(t * 5); // swelling, in time
        if (name === "starting") { // budding out of the centre in turn, and off it
          const ph = (t * .9 - k / 3) % 1;
          gr = .2 * Math.sin(Math.PI * ph); gg = .25 + .5 * ph - st.centre - gr;
        }
        const r = 4 * i + 1 + k, g = 3 * i + k, an = SAT_ANG[k];
        pull(R, RV, r, gr, dt, 17); pull(G, GV, g, gg, dt, 26);
        if (R[r] < 0) R[r] = 0;
        const d = c0 + R[r] + G[g] + (st.still ? 0 : WOB * Math.sin(now / 700 + an * 3));
        X[g] = Math.cos(an) * d; Y[g] = Math.sin(an) * d;
      }
      for (let k = 0; k < 9; k++) COL[9 * i + k] += (st.col[k / 3 | 0][k % 3] - COL[9 * i + k]) * kc;
      if (name === "streaming") SPIN[i] += dt * 1.4;
      if (name === "tools" || name === "listening") {
        const period = name === "tools" ? .6 : 2, n = Math.floor(t / period), f = Math.min(1, (t / period - n) * 3);
        GEAR[i] = (n + 1 - Math.pow(1 - f, 3)) * Math.PI / 4;
      }

      // Liveness: a packet since the last disk starts one, at most every 0.2 s.
      if (s.lastPacket > SEEN[i] && now - LASTDISK[i] >= DISK_GAP) {
        SEEN[i] = s.lastPacket; LASTDISK[i] = now;
        DISK[DISKS * i + DISKAT[i]] = now; DISKAT[i] = (DISKAT[i] + 1) % DISKS;
      }

      const b0 = INST * o, onePx = 2 * SPAN / px, active = SAT_KEYS.indexOf(st.active) + 1;
      inst[b0] = x; inst[b0 + 1] = y; inst[b0 + 2] = size; inst[b0 + 3] = onePx;
      inst[b0 + 4] = c0; inst[b0 + 5] = active; inst[b0 + 6] = glyph(s.letter); inst[b0 + 7] = Math.max(c0 * 1.25, 9 * onePx);
      for (let k = 0; k < 3; k++) {
        const r = R[4 * i + 1 + k], show = clamp((r * px / (2 * SPAN) - 5) / 6, 0, 1), b = b0 + 8 + 4 * k;
        inst[b] = X[3 * i + k]; inst[b + 1] = Y[3 * i + k]; inst[b + 2] = r;
        inst[b + 3] = show * (active === k + 1 ? (name === "first" ? .55 : 1) : .45);
      }
      for (let k = 0; k < 9; k++) inst[b0 + 20 + 4 * (k / 3 | 0) + k % 3] = COL[9 * i + k] / 255;
      inst[b0 + 23] = SPIN[i]; inst[b0 + 27] = GEAR[i]; inst[b0 + 31] = 0;
      // The role letter: ink on a light blob, paper on a dark one.
      const light = (COL[9 * i] * .3 + COL[9 * i + 1] * .59 + COL[9 * i + 2] * .11) / 255, lc = light > .6 ? C.ink : C.paper;
      inst[b0 + 32] = lc[0] / 255; inst[b0 + 33] = lc[1] / 255; inst[b0 + 34] = lc[2] / 255; inst[b0 + 35] = 0;
      for (let k = 0; k < DISKS; k++) {
        const t1 = DISK[DISKS * i + (DISKAT[i] + k) % DISKS], age = (now - t1) / DISK_MS; // oldest first
        inst[b0 + 36 + k] = t1 > 0 && age < 1 ? age : -1;
      }
      return o + 1;
    },
    // Blob i's compaction dots, on the longest loop of its track, written
    // from dot `n`; returns the next.
    dots(i, n, x, y, size) {
      const pips = sessions[i].pips;
      if (!(pips > 0)) return n;
      const { R, X, Y } = a, onePx = 2 * SPAN / a.SIZE[i];
      ball[0] = 0; ball[1] = 0; ball[2] = R[4 * i];
      for (let k = 0; k < 3; k++) { ball[3 + 3 * k] = X[3 * i + k]; ball[4 + 3 * k] = Y[3 * i + k]; ball[5 + 3 * k] = R[4 * i + 1 + k]; }
      const m = longestLoop(TN, TRACK_LEVEL);
      const placed = m ? placeDots(m, pips, TRACK_PITCH) : 0, unit = size / (2 * SPAN);
      if (4 * (n + placed) > dots.length) { const d = new Float32Array(dots.length * 2 + 4 * placed); d.set(dots); dots = d; }
      for (let d = 0; d < placed; d++) {
        const b = 4 * n++;
        dots[b] = x + size / 2 + dotX[d] * unit; dots[b + 1] = y + size / 2 + dotY[d] * unit;
        dots[b + 2] = Math.max(1.4 * onePx, .035) * unit; dots[b + 3] = Math.max(.22, .7 - d * .07);
      }
      return n;
    },
  };
  layers.push(kept);
  return { add: kept.add, remove: kept.remove, moved: kept.moved };
}
