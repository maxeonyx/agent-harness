// ─── Blobs: a session's spinner, as metaballs ──────────────────────────────
// A session drawn as one blob of four balls. The centre ball is the session
// itself (its role letter); around it sit the model (the Claude mark), tool
// calls (a gear) and Max (a microphone). The one it waits on grows out of
// the centre and merges with it; going from one to another, the growth
// passes through the centre. Each piece of data sends a soft disk out from
// behind the active ball, at most one each 0.2 s; compactions are dots on a
// track round the blob's edge. A hollow blob, what a task will be once
// the work it waits on is in, has no fill and its outline dashed, in its
// state's colour: DASHES dashes clockwise from twelve o'clock, each an equal
// share of the outline's length, so they stretch as it grows and none
// comes or goes.
//
// Every blob of a layer is drawn in one pass: one WebGL canvas, and one
// instanced square per blob whose fragment shader computes the field per
// pixel. Everything in a blob is sized in its own units, so a blob of any
// size is a scaled copy of every other; below DETAIL css px its icons,
// letter and dots go, and it is just a blob in its state's colour. Its
// outline is never thinner than a css px, so a small stuck blob still shows
// red. While Max dictates, the microphone ball swells with his voice, its
// icon is a stop square, and a ring pulses out from it. The blobs' state
// is struct-of-arrays in typed arrays, and nothing is allocated per
// frame. Only the compaction dots and the dashes need the CPU: marching
// squares traces the track or the outline, and a draw each puts a dot or a
// dash's segment per quad. A blob
// off its canvas is neither animated nor drawn, and while every blob shown
// is still and its springs at rest, nothing runs at all.
//
//   const s = session();               what one session is doing
//   const blobs = layer(host);         null without WebGL2
//   const box = blobs.add(56, s);      an empty box the canvas draws a blob in
//   s.set("streaming"); s.packet();    and the blob follows
//
// The canvas fills `host`, so whatever moves `host` (a scroll, a sticky
// position) moves the blobs with it on the compositor. Each blob is drawn
// in its box as laid out, transforms included: scaling a box scales its
// blob, and `moved()` says a box changed. The blob is centred in its box:
// the box of its balls as its state sets them, wherever its big ball is.
//
// A map draws in the world instead, through a camera, and reads no boxes:
//
//   blobs.camera(x, y, k);             screen = (x, y) + world * k
//   const b = blobs.put(64, s);        a blob in a 64 px box, at k = 1,
//   b.at(wx, wy); b.hide(true);        centred on a point of the world
//   blobs.marks(m, n);                 n plain marks under the blobs (MARK)
//
// `onFrame(step)` runs `step(now)` at the start of each frame, before any
// layer draws, while it returns true: a camera moved there is drawn in the
// same frame.

// Colours: [fill, edge, icon] per state; the whole blob takes the state's,
// as base.css names them: Max's turn orange, an agent at work (or its
// background work) blue, waiting on other agents purple, done green, paused grey, stuck red.
const C = { ink: [20, 20, 19], paper: [250, 249, 245], yours: [212, 105, 28], working: [58, 109, 176], others: [139, 90, 166],
  green: [79, 138, 85], red: [179, 53, 44], faint: [161, 159, 150], muted: [107, 106, 100] };
const tint = (c, k) => C.paper.map((p, i) => p + (c[i] - p) * k);

// The balls, in field units (a blob's box spans -SPAN..SPAN). Satellites sit
// at fixed angles: the model at twelve o'clock, tools at four, Max at eight.
const SATS = { model: -Math.PI / 2, tools: Math.PI / 6, max: 5 * Math.PI / 6 };
const SAT_KEYS = Object.keys(SATS), SAT_ANG = SAT_KEYS.map((k) => SATS[k]);
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
// gap if not IDLE's, and its colours. A `still` state waits on something
// outside the agent: its blob does not wobble, and settles. A `bare` one
// shows no idle icons. Paused, a task rests in the brain: the brain's
// shape, grey, still and bare.
export const STATES = {
  first:     { active: "model", centre: .3, col: [C.working, C.working, C.paper] },
  streaming: { active: "model", centre: .3, col: [C.working, C.working, C.paper] },
  brain:     { active: "centre", centre: .54, gap: -.15, col: [C.working, C.working, C.paper] },
  tools:     { active: "tools", centre: .3, col: [C.working, C.working, C.paper] },
  listening: { active: "tools", centre: .34, size: .32, col: [C.working, C.working, C.paper] },
  children:  { active: "tools", centre: .34, size: .32, col: [C.others, C.others, C.paper] },
  max:       { active: "max", centre: .3, still: true, col: [C.yours, C.yours, C.paper] },
  held:      { active: "model", centre: .32, size: .36, still: true, col: [tint(C.red, .12), C.red, C.red] },
  starting:  { active: null, centre: .4, col: [C.working, C.working, C.paper] },
  paused:    { active: "centre", centre: .54, gap: -.15, still: true, bare: true, col: [[233, 229, 217], C.faint, C.muted] },
  done:      { active: null, centre: .48, merged: true, still: true, col: [C.green, C.green, C.paper] },
};
const satOf = (s) => (STATES[s].active in SATS ? STATES[s].active : null);
// Satellite k's radius and gap as state st sets them.
const satR = (st, k) => (st.active === SAT_KEYS[k] ? st.size || ACTIVE.r : st.merged ? .12 : IDLE.r);
const satG = (st, k) => (st.active === SAT_KEYS[k] ? ACTIVE.g : st.merged ? -.45 : st.gap ?? IDLE.g);
// Each state's hollow colours: no fill, and its icons and letter its edge's.
// And where its balls' box is centred, in blob units: a starting blob's
// satellites bud and fade, so only its centre counts.
for (const [name, st] of Object.entries(STATES)) {
  st.hollowCol = [C.paper, st.col[1], st.col[1]];
  let x0 = -st.centre, x1 = st.centre, y0 = -st.centre, y1 = st.centre;
  if (name !== "starting") for (let k = 0; k < 3; k++) {
    const r = satR(st, k), d = st.centre + r + satG(st, k), x = Math.cos(SAT_ANG[k]) * d, y = Math.sin(SAT_ANG[k]) * d;
    x0 = Math.min(x0, x - r); x1 = Math.max(x1, x + r); y0 = Math.min(y0, y - r); y1 = Math.max(y1, y + r);
  }
  st.mid = [(x0 + x1) / 2, (y0 + y1) / 2];
}

// A session: its state, when its last packet came, its compactions, its
// role letter and whether it is hollow, shared by every blob of it. Going
// from one satellite straight to another, it shows the brain for a moment,
// so the growth passes through the centre; `jump` goes straight there, for
// a blob now showing another session. In a still state its blob settles and stops moving; while Max
// dictates, `voice` is his level from 0 to 1 (null when he is not
// dictating), since `heard`.
export function session(state = "starting") {
  return { state, since: performance.now(), via: 0, pips: 0, letter: "?", hollow: false, lastPacket: 0, voice: null, heard: 0,
    shown(now) { return now < this.via ? "brain" : this.state; },
    set(next, jump = false) {
      if (next === this.state && !jump) return;
      const now = performance.now(), a = satOf(this.shown(now)), b = satOf(next);
      this.via = !jump && a && b && a !== b ? now + 260 : 0;
      this.state = next; this.since = now;
      wake();
    },
    hear(level) {
      if (this.voice === null) this.heard = performance.now();
      this.voice = level; wake();
    },
    packet() { this.lastPacket = performance.now(); },
    // Whether its blob moves even once its springs are at rest.
    moving(now) { return !STATES[this.state].still || this.voice !== null || now - this.lastPacket < DISK_MS; } };
}

// Icons, 24x24, as Path2D. The Claude mark is Anthropic's (via Simple Icons);
// the gear and microphone are Lucide's (ISC).
export const ICON = {
  model: { fill: true, path: "m4.7144 15.9555 4.7174-2.6471.079-.2307-.079-.1275h-.2307l-.7893-.0486-2.6956-.0729-2.3375-.0971-2.2646-.1214-.5707-.1215-.5343-.7042.0546-.3522.4797-.3218.686.0608 1.5179.1032 2.2767.1578 1.6514.0972 2.4468.255h.3886l.0546-.1579-.1336-.0971-.1032-.0972L6.973 9.8356l-2.55-1.6879-1.3356-.9714-.7225-.4918-.3643-.4614-.1578-1.0078.6557-.7225.8803.0607.2246.0607.8925.686 1.9064 1.4754 2.4893 1.8336.3643.3035.1457-.1032.0182-.0728-.164-.2733-1.3539-2.4467-1.445-2.4893-.6435-1.032-.17-.6194c-.0607-.255-.1032-.4674-.1032-.7285L6.287.1335 6.6997 0l.9957.1336.419.3642.6192 1.4147 1.0018 2.2282 1.5543 3.0296.4553.8985.2429.8318.091.255h.1579v-.1457l.1275-1.706.2368-2.0947.2307-2.6957.0789-.7589.3764-.9107.7468-.4918.5828.2793.4797.686-.0668.4433-.2853 1.8517-.5586 2.9021-.3643 1.9429h.2125l.2429-.2429.9835-1.3053 1.6514-2.0643.7286-.8196.85-.9046.5464-.4311h1.0321l.759 1.1293-.34 1.1657-1.0625 1.3478-.8804 1.1414-1.2628 1.7-.7893 1.36.0729.1093.1882-.0183 2.8535-.607 1.5421-.2794 1.8396-.3157.8318.3886.091.3946-.3278.8075-1.967.4857-2.3072.4614-3.4364.8136-.0425.0304.0486.0607 1.5482.1457.6618.0364h1.621l3.0175.2247.7892.522.4736.6376-.079.4857-1.2142.6193-1.6393-.3886-3.825-.9107-1.3113-.3279h-.1822v.1093l1.0929 1.0686 2.0035 1.8092 2.5075 2.3314.1275.5768-.3218.4554-.34-.0486-2.2039-1.6575-.85-.7468-1.9246-1.621h-.1275v.17l.4432.6496 2.3436 3.5214.1214 1.0807-.17.3521-.6071.2125-.6679-.1214-1.3721-1.9246L14.38 17.959l-1.1414-1.9428-.1397.079-.674 7.2552-.3156.3703-.7286.2793-.6071-.4614-.3218-.7468.3218-1.4753.3886-1.9246.3157-1.53.2853-1.9004.17-.6314-.0121-.0425-.1397.0182-1.4328 1.9672-2.1796 2.9446-1.7243 1.8456-.4128.164-.7164-.3704.0667-.6618.4008-.5889 2.386-3.0357 1.4389-1.882.929-1.0868-.0062-.1579h-.0546l-6.3385 4.1164-1.1293.1457-.4857-.4554.0608-.7467.2307-.2429 1.9064-1.3114Z" },
  tools: { fill: false, path: "M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0" },
  max: { fill: false, path: "M12 19v3M19 10v2a7 7 0 0 1-14 0v-2M9 5a3 3 0 0 1 6 0v7a3 3 0 0 1-6 0z" },
  stop: { fill: true, path: "M8.5 6.5h7a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2h-7a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2z" },
};

const clamp = (x, lo, hi) => Math.min(hi, Math.max(lo, x));
const K = .22; // the smooth minimum's reach: balls closer than this melt together
const DISKS = 8, DISK_MS = 1400, DISK_GAP = 200; // at most one disk per 0.2 s
const INST = 48; // floats per blob instance: 12 vec4
// In blob units: the outline's width (1.3 px on a 64 px blob), the letter's
// size per unit of the centre's radius, and a compaction dot's radius. A
// hollow blob's outline is DASHES dashes, each drawn on DUTY of its share.
const EDGE = 1.3 * 2 * SPAN / 64, LETTER = 1.4, DOT = .05, DASHES = 16, DUTY = .55;
// Below DETAIL css px a blob has no icons, letter or dots: they pop out
// rather than fade, so none lingers faint.
const DETAIL = 32;
// While Max dictates: how much his loudest voice adds to the microphone
// ball's radius, and how often a ring pulses out from it, in ms.
const VOICE = .12, RING_MS = 1400;
// A mark: centre x, y, radius and ring width in world px; fill, then ring,
// as straight rgba 0-1; and how many dashes its ring has (0: solid).
export const MARK = 16;

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
  if (!glyphs.size) for (const k of Object.keys(ICON)) glyphs.set("icon " + k, addCell((g) => {
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
${[...Array(12)].map((_, k) => `layout(location = ${k + 1}) in vec4 a${k};`).join("\n")}
uniform vec2 view;
out vec2 p;
${[...Array(12)].map((_, k) => `flat out vec4 v${k};`).join("\n")}
void main() {
  vec2 dev = a0.xy + corner * a0.z;
  gl_Position = vec4(dev.x / view.x * 2. - 1., 1. - dev.y / view.y * 2., 0., 1.);
  p = (corner * 2. - 1.) * ${SPAN};
  ${[...Array(12)].map((_, k) => `v${k} = a${k};`).join(" ")}
}`;
// v0: device x, y, size, device px per css px. v1: centre radius, active
// ball (0 centre, 1-3 a satellite), letter cell, letter size. v2-v4: the
// satellites' x, y, r and icon alpha. v5-v7: fill, edge and icon colours,
// with the model's and gear's turns and the ring's age, 0-1 (0: no ring,
// and the microphone, not the stop square). v8: letter colour and alpha.
// v9-v10: the disks' ages, 0-1, or -1. v11: the fill's and the outline's
// alpha (a hollow blob's dashes are drawn apart).
const BLOB_FS = `#version 300 es
precision highp float;
in vec2 p;
${[...Array(12)].map((_, k) => `flat in vec4 v${k};`).join("\n")}
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
vec4 ring(vec4 c, vec3 b, float age, float unit) {
  if (age <= 0.) return c;
  float d = (length(p - b.xy) - b.z - .18 * (1. - pow(1. - age, 2.))) * unit;
  return over(c, v6.rgb, .7 * (1. - age) * clamp(max(${EDGE} * unit, v0.w) + .5 - abs(d), 0., 1.));
}
// An icon, 24 units square, centred on its ball and turned by a; the atlas
// holds it at 4 px a unit with a 4-unit margin, as the distance from its
// fill or its 2-unit stroke.
vec4 icon(vec4 c, vec4 b, float a, float cell, vec3 col, float unit) {
  if (b.w <= 0.) return c;
  float s = b.z * 1.15 / 24., cs = cos(a), sn = sin(a);
  vec2 q = mat2(cs, -sn, sn, cs) * (p - b.xy) / s + 12.;
  float d = sdf(cell, (q + 4.) * 4.) / 4.;
  return over(c, col, b.w * clamp(.5 - d * s * unit, 0., 1.));
}
void main() {
  float unit = v0.z / ${2 * SPAN};
  float f = smin(smin(smin(length(p) - v1.x, v2.xyz), v3.xyz), v4.xyz);
  float fpx = f / max(length(vec2(dFdx(f), dFdy(f))), 1e-6); // device px

  int act = int(v1.y);
  vec3 src = act == 1 ? v2.xyz : act == 2 ? v3.xyz : act == 3 ? v4.xyz : vec3(0., 0., v1.x);
  vec4 c = vec4(0.);
  c = disk(c, src, v9.x, unit); c = disk(c, src, v9.y, unit); c = disk(c, src, v9.z, unit); c = disk(c, src, v9.w, unit);
  c = disk(c, src, v10.x, unit); c = disk(c, src, v10.y, unit); c = disk(c, src, v10.z, unit); c = disk(c, src, v10.w, unit);
  c = ring(c, v4.xyz, v7.w, unit);

  c = over(c, v5.rgb, v11.x * clamp(.5 - fpx, 0., 1.));
  c = over(c, v6.rgb, v11.y * clamp(max(${EDGE} * unit, v0.w) * .5 + .5 - abs(fpx), 0., 1.));

  c = icon(c, v2, v5.w, 0., act == 1 ? v7.rgb : v6.rgb, unit);
  c = icon(c, v3, v6.w, 1., act == 2 ? v7.rgb : v6.rgb, unit);
  c = icon(c, v4, 0., v7.w > 0. ? 3. : 2., act == 3 ? v7.rgb : v6.rgb, unit);

  float k = v1.w / 80.; // the letter is drawn 80 px high in its cell
  float d = sdf(v1.z, (p - vec2(0., v1.x * .05)) / k + CELL / 2.) * k * unit;
  o = over(c, v8.rgb, v8.w * clamp(.5 - d, 0., 1.));
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
const SEG_VS = `#version 300 es
layout(location = 0) in vec2 corner;
layout(location = 1) in vec4 ends; // device x, y of each end
layout(location = 2) in vec4 pen; // colour, width in device px
uniform vec2 view;
out vec2 q;
flat out vec4 e, c;
void main() {
  q = mix(min(ends.xy, ends.zw) - pen.w - 1., max(ends.xy, ends.zw) + pen.w + 1., corner);
  gl_Position = vec4(q.x / view.x * 2. - 1., 1. - q.y / view.y * 2., 0., 1.);
  e = ends; c = pen;
}`;
const SEG_FS = `#version 300 es
precision highp float;
in vec2 q;
flat in vec4 e, c;
out vec4 o;
void main() {
  vec2 ab = e.zw - e.xy, aq = q - e.xy;
  float a = clamp(c.w * .5 + .5 - length(aq - ab * clamp(dot(aq, ab) / max(dot(ab, ab), 1e-6), 0., 1.)), 0., 1.);
  o = vec4(c.rgb * a, a);
}`;
const MARK_VS = `#version 300 es
layout(location = 0) in vec2 corner;
layout(location = 1) in vec4 m0; // world x, y, radius, ring width
layout(location = 2) in vec4 m1; // fill
layout(location = 3) in vec4 m2; // ring
layout(location = 4) in vec4 m3; // dashes
uniform vec2 view;
uniform vec4 cam; // x, y, k, device px per css px
out vec2 q;
flat out vec4 c, f, r, d;
void main() {
  float s = cam.z * cam.w;
  c = vec4((cam.xy + m0.xy * cam.z) * cam.w, m0.z * s, max(m0.w * s, 1.));
  q = c.xy + (corner * 2. - 1.) * (c.z + c.w + 1.);
  gl_Position = vec4(q.x / view.x * 2. - 1., 1. - q.y / view.y * 2., 0., 1.);
  f = m1; r = m2; d = m3;
}`;
const MARK_FS = `#version 300 es
precision highp float;
in vec2 q;
flat in vec4 c, f, r, d;
out vec4 o;
void main() {
  vec2 v = q - c.xy;
  float l = length(v), fill = f.a * clamp(c.z + .5 - l, 0., 1.);
  float ring = r.a * clamp(c.w * .5 + .5 - abs(l - c.z), 0., 1.);
  if (d.x > 0. && fract(atan(v.y, v.x) / 6.2831853 * d.x) > .55) ring = 0.;
  o = vec4(f.rgb * fill, fill);
  o = vec4(r.rgb * ring, ring) + o * (1. - ring);
}`;

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

// The compaction dots on the track (the outline offset outward), at most
// MAXDOTS of them: the first at ten o'clock, halfway between the model's
// ball and Max's, then anticlockwise, each the pitch further along the
// track and at least the pitch away, in a straight line, from every dot
// before it, so a neck in the blob doesn't bunch them. When they don't all
// fit so, they share the whole track evenly instead. Into dotX, dotY;
// returns how many.
const MAXDOTS = 64, dotX = new Float32Array(MAXDOTS), dotY = new Float32Array(MAXDOTS);
const wX = new Float32Array(TE + 1), wY = new Float32Array(TE + 1), wS = new Float32Array(TE + 1);
let pn = 0, flip = false, seg = 0, qx = 0, qy = 0;
const at = (k) => (flip ? pn - 1 - (k % pn) : k % pn); // anticlockwise on screen, y down
const START = -5 * Math.PI / 6; // ten o'clock
function along(n, v) {
  while (seg < n - 2 && wS[seg + 1] < v) seg++; // n points walked
  const f = (v - wS[seg]) / (wS[seg + 1] - wS[seg] || 1);
  qx = wX[seg] + (wX[seg + 1] - wX[seg]) * f; qy = wY[seg] + (wY[seg + 1] - wY[seg]) * f;
}
function near(out, p) {
  for (let d = 0; d < out; d++) if (Math.hypot(qx - dotX[d], qy - dotY[d]) < p * .98) return true;
  return false;
}
// Walk loopB's n points, anticlockwise on screen or not, from where it
// crosses the ray from the centre at angle `ang`, furthest out: wX, wY are
// its points and wS the length walked to each. Returns its whole length.
function walk(n, ang, ccw) {
  const L = loopB, SX = Math.cos(ang), SY = Math.sin(ang);
  let a = 0;
  for (let k = 0; k < n; k++) { const l = (k + 1) % n; a += L[2 * k] * L[2 * l + 1] - L[2 * l] * L[2 * k + 1]; }
  pn = n; flip = a > 0 === ccw;
  // `v` is a point's side of the ray's line.
  let start = 0, sf = 0, far = -Infinity;
  for (let k = 0; k < n; k++) {
    const ax = L[2 * at(k)], ay = L[2 * at(k) + 1], bx = L[2 * at(k + 1)], by = L[2 * at(k + 1) + 1];
    const av = ay * SX - ax * SY, bv = by * SX - bx * SY;
    if ((av < 0) !== (bv < 0)) {
      const f = av / (av - bv), u = (ax + (bx - ax) * f) * SX + (ay + (by - ay) * f) * SY;
      if (u > far) { far = u; start = k; sf = f; }
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
  seg = 0;
  return s;
}
function placeDots(n, count, pitch) {
  const s = walk(n, START, true);
  const drawn = Math.min(count, MAXDOTS);
  let u = 0, out = 0;
  along(n + 1, 0); dotX[out] = qx; dotY[out++] = qy;
  while (out < drawn) {
    u += pitch; along(n + 1, u);
    while (u < s && near(out, pitch)) { u += pitch * .05; along(n + 1, u); }
    if (u >= s) break;
    dotX[out] = qx; dotY[out++] = qy;
  }
  if (out === drawn) return out;
  seg = 0;
  for (let d = 0; d < drawn; d++) { along(n + 1, d * s / drawn); dotX[d] = qx; dotY[d] = qy; }
  return drawn;
}
// A hollow blob's outline, loopB's n points, walked clockwise from twelve
// o'clock into `line`: x, y and the length walked to each. Returns how many
// points (n + 1: the last is the first again).
const line = new Float32Array(3 * (TE + 1));
function outline(n) {
  walk(n, -Math.PI / 2, false);
  for (let k = 0; k <= n; k++) { line[3 * k] = wX[k]; line[3 * k + 1] = wY[k]; line[3 * k + 2] = wS[k]; }
  return n + 1;
}
// The dashes on outline L of np points, each pulled in at both ends by r
// along the outline (the round caps the pen draws past them), as segments
// into dashAt, x0, y0, x1, y1 each; returns how many. A dash too short for
// its caps is a dot at its middle.
const dashAt = new Float32Array(4 * (TE + 2 * DASHES));
function layDashes(L, np, r) {
  const s = L[3 * np - 1];
  let m = 0, j = 1;
  for (let d = 0; d < DASHES; d++) {
    let v0 = d * s / DASHES + r, v1 = (d + DUTY) * s / DASHES - r;
    if (v1 < v0) v0 = v1 = (v0 + v1) / 2;
    j = pointAt(L, np, j, v0); dashAt[m] = qx; dashAt[m + 1] = qy;
    // Each point inside the dash ends one segment and starts the next.
    for (; j < np - 1 && L[3 * j + 2] < v1; j++, m += 4) { dashAt[m + 2] = dashAt[m + 4] = L[3 * j]; dashAt[m + 3] = dashAt[m + 5] = L[3 * j + 1]; }
    j = pointAt(L, np, j, v1); dashAt[m + 2] = qx; dashAt[m + 3] = qy; m += 4;
  }
  return m / 4;
}
// The point at length v along outline L of np points, into qx, qy, looking
// from point j on; returns the point its stretch ends at.
function pointAt(L, np, j, v) {
  while (j < np - 1 && L[3 * j + 2] < v) j++;
  const a = 3 * (j - 1), f = (v - L[a + 2]) / (L[a + 5] - L[a + 2] || 1);
  qx = L[a] + (L[a + 3] - L[a]) * f; qy = L[a + 1] + (L[a + 4] - L[a + 1]) * f;
  return j;
}

// The track and the dots' pitch, in blob units, laid out as on a 260 px blob
// whatever the size: a small blob's dots are a scaled copy of a big one's,
// and on a 64 px one they clear the outline by about a pixel.
const TRACK_PX = 260, TRACK_LEVEL = .05 + 8 * 2 * SPAN / TRACK_PX, TRACK_PITCH = Math.max(.2, 9 * 2 * SPAN / TRACK_PX);

// A spring with a little give, so balls overshoot and settle like liquid;
// critically damped (26), it settles without overshooting. Returns whether
// it is still moving.
function pull(v, vel, i, goal, dt, damp, stiff = 170) {
  vel[i] += ((goal - v[i]) * stiff - vel[i] * damp) * dt;
  v[i] += vel[i] * dt;
  return Math.abs(goal - v[i]) > 1e-3 || Math.abs(vel[i]) > 1e-2;
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
// It runs while some layer shows a blob that moves, and stops when none does.
const layers = [], steps = new Set();
let running = false, last = 0;
function wake() {
  if (running) return;
  running = true; last = performance.now();
  requestAnimationFrame(frame);
}
function frame(now) {
  // A frame's time can come before the wake that asked for it, by as long as
  // the task that woke the loop ran: never a step back, which flings the springs.
  const dt = clamp((now - last) / 1000, 0, 1 / 30);
  last = now;
  for (const s of steps) if (!s(now)) steps.delete(s);
  let moving = steps.size > 0;
  for (const l of layers) moving = l.draw(now, dt) || moving;
  if (moving) requestAnimationFrame(frame);
  else running = false;
}
export function onFrame(step) { steps.add(step); wake(); }

// ─── A layer: one canvas, and every blob in it ─────────────────────────────
// Null without WebGL2: the page then shows no blobs. A lost context draws
// nothing until the browser restores it, and then everything is rebuilt.
export function layer(host) {
  const clip = document.createElement("div"), cv = document.createElement("canvas");
  const gl = cv.getContext("webgl2", { antialias: false });
  if (!gl) return null;
  clip.style.cssText = "position: absolute; inset: 0; overflow: clip; pointer-events: none; z-index: 1";
  cv.style.cssText = "position: absolute; left: 0; top: 0; width: 100%; height: 100%";
  clip.append(cv);
  host.append(clip);

  let blobProg, dotProg, segProg, markProg, blobView, blobAtlas, dotView, segView, markView, markCam, instBuf, dotBuf, segBuf, markBuf, blobVao, dotVao, segVao, markVao, atlasTex, uploaded, markSent;
  const build = () => {
    blobProg = program(gl, BLOB_VS, BLOB_FS); dotProg = program(gl, DOT_VS, DOT_FS); segProg = program(gl, SEG_VS, SEG_FS); markProg = program(gl, MARK_VS, MARK_FS);
    blobView = gl.getUniformLocation(blobProg, "view"); blobAtlas = gl.getUniformLocation(blobProg, "atlasSize"); dotView = gl.getUniformLocation(dotProg, "view"); segView = gl.getUniformLocation(segProg, "view");
    markView = gl.getUniformLocation(markProg, "view"); markCam = gl.getUniformLocation(markProg, "cam");
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
    instBuf = gl.createBuffer(); dotBuf = gl.createBuffer(); segBuf = gl.createBuffer(); markBuf = gl.createBuffer();
    blobVao = vao(instBuf, INST / 4); dotVao = vao(dotBuf, 1); segVao = vao(segBuf, 2); markVao = vao(markBuf, MARK / 4);
    markSent = -1;
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    atlasTex = gl.createTexture();
    uploaded = -1;
  };
  build();
  cv.addEventListener("webglcontextlost", (e) => e.preventDefault());
  cv.addEventListener("webglcontextrestored", () => { build(); wake(); });

  // The blobs, struct-of-arrays: per blob, its box, the session it shows,
  // its box's size and place in `host`, a hollow one's outline as last traced
  // (the balls it was traced from, and how many points), its balls'
  // radii and gaps and their velocities, its colours, turns and disks.
  let cap = 0, count = 0;
  const boxes = [], sessions = [], outlines = [];
  const a = {};
  const FIELDS = [["SIZE", Float32Array, 1], ["DOCX", Float32Array, 1], ["DOCY", Float32Array, 1],
    ["R", Float32Array, 4], ["RV", Float32Array, 4], ["G", Float32Array, 3], ["GV", Float32Array, 3],
    ["X", Float32Array, 3], ["Y", Float32Array, 3], ["COL", Float32Array, 9], ["SPIN", Float32Array, 1], ["GEAR", Float32Array, 1],
    ["DISK", Float64Array, DISKS], ["DISKAT", Uint8Array, 1], ["LASTDISK", Float64Array, 1], ["SEEN", Float64Array, 1],
    ["BASE", Float32Array, 1], ["WX", Float32Array, 1], ["WY", Float32Array, 1], ["WORLD", Uint8Array, 1],
    ["MID", Float32Array, 2], ["MIDV", Float32Array, 2], ["TRACED", Float32Array, 12], ["NPTS", Uint16Array, 1]];
  // The world's camera, and its marks.
  let cam = { x: 0, y: 0, k: 1 }, marks = new Float32Array(0), nmarks = 0, marksVersion = 0;
  let inst = new Float32Array(0), dots = new Float32Array(4 * 64), segs = new Float32Array(8 * 64);
  const grow = () => {
    const n = Math.max(8, cap * 2);
    for (const [name, A, per] of FIELDS) { const g = new A(n * per); if (a[name]) g.set(a[name]); a[name] = g; }
    inst = new Float32Array(n * INST);
    cap = n;
  };

  // Where each box is in `host`, and its size, read only when the layout
  // changes. A box not in the page is put far off.
  let dirty = true;
  const resized = new ResizeObserver(() => { dirty = true; wake(); });
  resized.observe(host);
  const measure = () => {
    const h = host.getBoundingClientRect(), k = host.offsetWidth ? h.width / host.offsetWidth : 1;
    for (let i = 0; i < count; i++) {
      if (a.WORLD[i]) continue;
      const r = boxes[i].getBoundingClientRect();
      a.DOCX[i] = r.width ? (r.left - h.left) / k : -1e9; a.DOCY[i] = (r.top - h.top) / k;
      if (r.width) a.SIZE[i] = r.width / k;
    }
    dirty = false;
  };

  // A new blob: its balls idle, in its state's colours and place.
  const start = (i, s) => {
    const st = STATES[s.state];
    a.LASTDISK[i] = -1e9; a.SEEN[i] = s.lastPacket; a.R[4 * i] = .4;
    for (let k = 0; k < 3; k++) { a.R[4 * i + 1 + k] = IDLE.r; a.G[3 * i + k] = IDLE.g; }
    a.COL.set((s.hollow ? st.hollowCol : st.col).flat(), 9 * i);
    a.MID.set(st.mid, 2 * i);
    a.TRACED[12 * i] = NaN; outlines[i] = new Float32Array(0);
  };
  // Blob i's balls into `ball`, for tracing.
  const balls = (i) => {
    const { R, X, Y } = a;
    ball[0] = 0; ball[1] = 0; ball[2] = R[4 * i];
    for (let k = 0; k < 3; k++) { ball[3 + 3 * k] = X[3 * i + k]; ball[4 + 3 * k] = Y[3 * i + k]; ball[5 + 3 * k] = R[4 * i + 1 + k]; }
  };
  const kept = {
    // An empty box of css size px, for the canvas to draw `s`'s blob in.
    add(px, s) {
      const box = document.createElement("div");
      box.className = "blob";
      box.style.width = box.style.height = px + "px";
      if (count === cap) grow();
      const i = count++;
      boxes[i] = box; sessions[i] = s;
      a.SIZE[i] = px; a.WORLD[i] = 0;
      start(i, s);
      kept.moved();
      return box;
    },
    // Forget a box: the last blob takes its place.
    remove(box) {
      const i = boxes.indexOf(box);
      if (i < 0) return;
      const j = --count;
      boxes[i] = boxes[j]; sessions[i] = sessions[j]; outlines[i] = outlines[j]; boxes.length = sessions.length = outlines.length = count;
      for (const [name, , per] of FIELDS) a[name].copyWithin(i * per, j * per, (j + 1) * per);
      kept.moved();
    },
    // Boxes moved without `host` changing size.
    moved() { dirty = true; wake(); },
    // A blob in the world, in a box of px at zoom 1, centred where `at` says.
    put(px, s) {
      const h = { at(x, y) { const i = boxes.indexOf(h); a.WX[i] = x; a.WY[i] = y; wake(); },
        hide(on) { const i = boxes.indexOf(h); a.WORLD[i] = on ? 2 : 1; wake(); },
        set(s2) { sessions[boxes.indexOf(h)] = s2; }, remove() { kept.remove(h); } };
      if (count === cap) grow();
      const i = count++;
      boxes[i] = h; sessions[i] = s;
      a.BASE[i] = px; a.SIZE[i] = px; a.WORLD[i] = 1;
      start(i, s);
      wake();
      return h;
    },
    camera(x, y, k) { cam = { x, y, k }; wake(); },
    // The marks, MARK floats each, as they are now.
    marks(m, n) { marks = m; nmarks = n; marksVersion++; wake(); },
    draw(now, dt) {
      if (gl.isContextLost()) return false;
      if (dirty) measure();
      const dpr = Math.min(3, devicePixelRatio || 1), W = Math.round(host.clientWidth * dpr), H = Math.round(host.clientHeight * dpr);
      if (cv.width !== W || cv.height !== H) { cv.width = W; cv.height = H; }
      let drawn = 0, ndots = 0, nsegs = 0, moving = false;
      const kc = 1 - Math.exp(-dt / .18);
      for (let i = 0; i < count; i++) {
        let x, y;
        if (a.WORLD[i]) {
          if (a.WORLD[i] === 2) continue;
          a.SIZE[i] = a.BASE[i] * cam.k;
          x = (cam.x + a.WX[i] * cam.k - a.SIZE[i] / 2) * dpr; y = (cam.y + a.WY[i] * cam.k - a.SIZE[i] / 2) * dpr;
        } else {
          if (a.DOCX[i] < -1e8) continue;
          x = a.DOCX[i] * dpr; y = a.DOCY[i] * dpr;
        }
        if (sessions[i].moving(now)) moving = true;
        const px = a.SIZE[i], size = px * dpr;
        if (x + size < 0 || y + size < 0 || x > W || y > H) continue;
        const o = drawn++;
        if (this.blob(i, o, now, dt, kc, x, y, size)) moving = true;
        ndots = this.dots(i, ndots, inst[INST * o], inst[INST * o + 1], size);
        nsegs = this.dashes(i, nsegs, inst[INST * o], inst[INST * o + 1], size, inst[INST * o + 3]);
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
      if (nmarks) {
        gl.useProgram(markProg);
        gl.uniform2f(markView, W, H);
        gl.uniform4f(markCam, cam.x, cam.y, cam.k, dpr);
        gl.bindBuffer(gl.ARRAY_BUFFER, markBuf);
        if (markSent !== marksVersion) { gl.bufferData(gl.ARRAY_BUFFER, marks.subarray(0, MARK * nmarks), gl.DYNAMIC_DRAW); markSent = marksVersion; }
        gl.bindVertexArray(markVao);
        gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, nmarks);
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
      if (nsegs) {
        gl.useProgram(segProg);
        gl.uniform2f(segView, W, H);
        gl.bindBuffer(gl.ARRAY_BUFFER, segBuf);
        gl.bufferData(gl.ARRAY_BUFFER, segs.subarray(0, 8 * nsegs), gl.STREAM_DRAW);
        gl.bindVertexArray(segVao);
        gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, nsegs);
      }
      return moving;
    },
    // Blob i, animated a step and written as instance `o`, its box at x, y:
    // centred in it, unless it is in the world; returns whether its springs
    // or colours are still moving.
    blob(i, o, now, dt, kc, x, y, size) {
      const { R, RV, G, GV, X, Y, COL, SPIN, GEAR, DISK, DISKAT, LASTDISK, SEEN, MID, MIDV } = a;
      const s = sessions[i], name = s.shown(now), st = STATES[name], t = (now - s.since) / 1000, px = a.SIZE[i];
      const col = s.hollow ? st.hollowCol : st.col;

      // The balls: each radius and gap springs towards the state's; Max's
      // voice swells the microphone, on a stiffer spring that keeps up with it.
      let stirring = pull(R, RV, 4 * i, st.centre, dt, 17);
      const c0 = R[4 * i];
      for (let k = 0; k < 2; k++) if (pull(MID, MIDV, 2 * i + k, st.mid[k], dt, 26)) stirring = true;
      for (let k = 0; k < 3; k++) {
        let gr = satR(st, k), gg = satG(st, k);
        if (name === "first" && k === 0) gr = .36 + .06 * Math.sin(t * 5); // swelling, in time
        if (name === "starting") { // budding out of the centre in turn, and off it
          const ph = (t * .9 - k / 3) % 1;
          gr = .2 * Math.sin(Math.PI * ph); gg = .25 + .5 * ph - st.centre - gr;
        }
        const r = 4 * i + 1 + k, g = 3 * i + k, an = SAT_ANG[k], heard = SAT_KEYS[k] === "max" && s.voice !== null;
        if (heard) gr += VOICE * s.voice;
        if (heard ? pull(R, RV, r, gr, dt, 28, 400) : pull(R, RV, r, gr, dt, 17)) stirring = true;
        if (pull(G, GV, g, gg, dt, 26)) stirring = true;
        if (R[r] < 0) R[r] = 0;
        const d = c0 + R[r] + G[g] + (st.still ? 0 : WOB * Math.sin(now / 700 + an * 3));
        X[g] = Math.cos(an) * d; Y[g] = Math.sin(an) * d;
      }
      for (let k = 0; k < 9; k++) {
        const off = col[k / 3 | 0][k % 3] - COL[9 * i + k];
        COL[9 * i + k] += off * kc;
        if (Math.abs(off) > .5) stirring = true;
      }
      if (name === "streaming") SPIN[i] += dt * 1.4;
      if (name === "tools" || name === "listening" || name === "children") {
        const period = name === "tools" ? .6 : 2, n = Math.floor(t / period), f = Math.min(1, (t / period - n) * 3);
        GEAR[i] = (n + 1 - Math.pow(1 - f, 3)) * Math.PI / 3; // a tooth a step: the gear has six
      }

      // Liveness: a packet since the last disk starts one, at most every 0.2 s.
      if (s.lastPacket > SEEN[i] && now - LASTDISK[i] >= DISK_GAP) {
        SEEN[i] = s.lastPacket; LASTDISK[i] = now;
        DISK[DISKS * i + DISKAT[i]] = now; DISKAT[i] = (DISKAT[i] + 1) % DISKS;
      }

      const b0 = INST * o, active = SAT_KEYS.indexOf(st.active) + 1, show = +(px >= DETAIL);
      const unit = a.WORLD[i] ? 0 : size / (2 * SPAN);
      inst[b0] = x - MID[2 * i] * unit; inst[b0 + 1] = y - MID[2 * i + 1] * unit; inst[b0 + 2] = size; inst[b0 + 3] = size / px;
      inst[b0 + 4] = c0; inst[b0 + 5] = active; inst[b0 + 6] = glyph(s.letter); inst[b0 + 7] = c0 * LETTER;
      for (let k = 0; k < 3; k++) {
        const r = R[4 * i + 1 + k], b = b0 + 8 + 4 * k;
        inst[b] = X[3 * i + k]; inst[b + 1] = Y[3 * i + k]; inst[b + 2] = r;
        inst[b + 3] = show * (active === k + 1 ? (name === "first" ? .55 : 1) : st.bare ? 0 : .45);
      }
      for (let k = 0; k < 9; k++) inst[b0 + 20 + 4 * (k / 3 | 0) + k % 3] = COL[9 * i + k] / 255;
      inst[b0 + 23] = SPIN[i]; inst[b0 + 27] = GEAR[i];
      inst[b0 + 31] = s.voice === null ? 0 : Math.max(1e-3, ((now - s.heard) / RING_MS) % 1);
      // The role letter: ink on a light blob, paper on a dark one, and in a
      // hollow one its icons' colour.
      const light = (COL[9 * i] * .3 + COL[9 * i + 1] * .59 + COL[9 * i + 2] * .11) / 255;
      const lc = light > .6 ? C.ink : C.paper;
      for (let k = 0; k < 3; k++) inst[b0 + 32 + k] = (s.hollow ? COL[9 * i + 6 + k] : lc[k]) / 255;
      inst[b0 + 35] = .92 * show;
      for (let k = 0; k < DISKS; k++) {
        const t1 = DISK[DISKS * i + (DISKAT[i] + k) % DISKS], age = (now - t1) / DISK_MS; // oldest first
        inst[b0 + 36 + k] = t1 > 0 && age < 1 ? age : -1;
      }
      inst[b0 + 44] = inst[b0 + 45] = s.hollow ? 0 : 1;
      return stirring;
    },
    // Blob i's compaction dots, on the longest loop of its track, written
    // from dot `n`, its quad at x, y; returns the next.
    dots(i, n, x, y, size) {
      const pips = sessions[i].pips;
      if (!(pips > 0) || a.SIZE[i] < DETAIL) return n;
      balls(i);
      const m = longestLoop(TN, TRACK_LEVEL);
      const placed = m ? placeDots(m, pips, TRACK_PITCH) : 0, unit = size / (2 * SPAN);
      if (4 * (n + placed) > dots.length) { const d = new Float32Array(dots.length * 2 + 4 * placed); d.set(dots); dots = d; }
      for (let d = 0; d < placed; d++) {
        const b = 4 * n++;
        dots[b] = x + size / 2 + dotX[d] * unit; dots[b + 1] = y + size / 2 + dotY[d] * unit;
        dots[b + 2] = DOT * unit; dots[b + 3] = Math.max(.22, .7 - d * .07);
      }
      return n;
    },
    // Blob i's dashes, if it is hollow, written from segment `n` in its
    // edge's colour, its quad at x, y, `dpr` device px a css px; traced
    // again only once its balls have moved, and laid on the outline each
    // frame, its round ends inside its length at any zoom, so a small
    // blob's gaps stay open. Returns the next.
    dashes(i, n, x, y, size, dpr) {
      if (!sessions[i].hollow) return n;
      balls(i);
      const T = a.TRACED, t0 = 12 * i;
      let moved = false;
      for (let k = 0; k < 12; k++) if (!(Math.abs(ball[k] - T[t0 + k]) < 1e-4)) moved = true;
      if (moved) {
        T.set(ball, t0);
        const m = longestLoop(TN, 0), np = m ? outline(m) : 0;
        if (outlines[i].length < 3 * np) outlines[i] = new Float32Array(6 * np);
        outlines[i].set(line.subarray(0, 3 * np));
        a.NPTS[i] = np;
      }
      if (a.NPTS[i] < 2) return n;
      const unit = size / (2 * SPAN), cx = x + size / 2, cy = y + size / 2, w = Math.max(EDGE * unit, dpr), col = a.COL;
      const placed = layDashes(outlines[i], a.NPTS[i], w / 2 / unit);
      if (8 * (n + placed) > segs.length) { const g = new Float32Array(segs.length * 2 + 8 * placed); g.set(segs); segs = g; }
      for (let d = 0; d < placed; d++) {
        const b = 8 * n++;
        for (let k = 0; k < 4; k++) segs[b + k] = (k & 1 ? cy : cx) + dashAt[4 * d + k] * unit;
        for (let k = 0; k < 3; k++) segs[b + 4 + k] = col[9 * i + 3 + k] / 255;
        segs[b + 7] = w;
      }
      return n;
    },
  };
  layers.push(kept);
  return { add: kept.add, remove: kept.remove, moved: kept.moved, put: kept.put, camera: kept.camera, marks: kept.marks };
}
