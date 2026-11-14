// Puāwai: a native plant in flower, painted close in brush, ink and colour on dyed paper.
//
// What it depicts. One living thing, near, against emptiness, as East Asian bird-and-flower
// painting looks at it: a branch or a clump reaching into the page from an edge or the foot, in
// flower, with most of the sheet left bare. The plants are New Zealand's, each painted in the
// manner its form asks for: kōwhai's zigzag bare wood hung with bunches of gold bells, as plum
// is painted, and most often a tūī come to the flowers; tūrutu's strap leaves swept in single
// strokes, as orchid is, with sprays of blue berries; pōhutukawa's grey limbs spreading low, each
// shoot ending in simple leathery leaves (a few turned to show their white-felted backs) gathered
// under crimson bursts of stamens; kawakawa's jointed stems with broad pale-veined heart leaves (a
// looper's holes in some) and green candles; koromiko's arching stems, its leaves in crossed pairs
// and its soft white-and-lilac spikes drooping from the upper axils; mānuka's wiry stems rising in
// sprays, thick with needle leaves, small white flowers all along them and red-pink buds.
//
// Who each agent is. The agent's place `at` picks its plant, so its bloom colour, its form and its
// paper together: slot 0 kōwhai gold, then tūrutu blue, pōhutukawa crimson, kawakawa green,
// koromiko violet, mānuka sea-green. Each is painted on its own dyed sheet, dipped from the foot in
// a pale tint of its colour (mānuka's is the cool grey-green of mānuka scrub on a hill, so its white
// flowers stand). identity(at) is the plant's colour, for the map; pōhutukawa's is a raspberry
// crimson, bluer than the failed state's brick red, and tūrutu's leans violet, away from the
// working state's blue. The seed picks the rest, each part from its own stream: where the plant
// enters (a side, a bottom corner, hanging from a top corner, or rising from the foot), its growth
// and its blooms.
//
// Technique. Simulated brushwork in three layers, then the paper. Each stroke is a loaded brush
// along a spine that bends at its joints in elbows: the water runs ahead as a pale wet core that
// pools where the brush comes down and spreads into the paper, and the hairs follow, darkest
// where the brush landed, paler along the stroke, breaking into dry streaks as it empties (on a
// broad brush, short staggered gaps between weaving hairs). The old wood is a pale wash with dark
// dry bark down one side, the young wood dark and quick, the branches behind in pale ink. Leaves
// and petals are presses of a brush loaded with colour and tipped with ink, so each is dark along
// one side and glazes where it crosses another; pōhutukawa's are outlined in ink over a wash, as
// the outline-and-colour painters do. Washes go in the wet layer, linework in the dry one, both
// laid down by multiplying; white and pale opaque colour go in a third layer laid on top. One
// shader then lays them on the paper (a sheet of xuan with its formation cloud, long fibres, tooth
// and bark specks, dyed from the foot): a wash carries its pigment to its rim and creeps out along
// the fibres; dry ink feathers a little; the fibres take less dye, so they show as pale strands
// where the sheet is dyed. Ink and pigment combine as absorbance, so they glaze rather than cover.
//
// The page is read as well as looked at. The lead's box is left bare: limbs are aimed to pass it,
// stems stop short of it, and blooms that would reach it are not painted. Below it, the lead's
// column, down to the foot (about half a screen high), is where body text will run: the front
// branches steer clear of it as they do of the lead, no bloom is put in it, and whatever still
// crosses it is laid pale, as the far branches are (the shader's veil, its edges ragged as mist's).
// So the dark strokes and the blooms keep to the margins beside the column and to the foot. A tall
// page is painted at the scale of about one screen, so the plant stays near and the rest is bare
// dyed sheet. Every canvas it makes is an OffscreenCanvas, so the painting can run in a worker.
//
// On the map, were a theme to draw its subject there as a chart in one ink, Puāwai's would be a
// plate from a painting manual, as the Mustard Seed Garden Manual teaches a branch: each area's
// plant in its bone strokes alone, in one grey ink, its blooms as open rings, with no colour and no
// dye.
//
// Tried and dropped. Vector shapes with outlines read as clip-art botany, and an even scatter of
// flowers as a greeting card: blooms are few and clustered, and the plant leaves most of the sheet
// empty. Wood painted as a chain of short strokes looked like a string of sticks, and lifting the
// brush at each twig left white gaps at the joints; each branch is one stroke. A limb that ran at
// the lead and bent round it hooked like a question mark; it is aimed past the lead, and ends if
// it turns too far. A tide line where the dye stops read as a crack across the sheet; the dye
// fades out instead. A turn made at a node in one step left a polyline kink, so the turn is
// spread into an elbow, with a knuckle where the brush paused. Twigs tapered to needles read as
// thorns; they end blunt, as a lifted brush does. Every stroke in one grey made the wood a
// diagram, so it has three inks now (pale for what is behind, dense, and near black for the
// accents). Dry streaks drawn as hairs of lighter ink running a limb's whole length looked combed.
// Opaque leaves read as vector fills, near Kauri's greens; they are transparent glazes tipped with
// ink. A second, paler plant behind the first read as a ghost, not as distance. A deep gold kōwhai
// read as orange on the map, Max's colour, so it is lemon with an olive calyx; kōwhai's flowers
// splayed out from short stalks looked like shuttlecocks, so they hang in bunches of bells. Long
// paper fibres made a busy net (Ledger's fault), so they are few and faint. Kōwhai's pinnate
// leaves looked like ferns (Kauri's), so its wood is bare, as it is when it flowers. Mānuka with
// round white flowers and dark eyes on black zigzag twigs was the stock plum-blossom picture, and
// white on cream or beige gave it no colour of its own: it is wiry sprays thick with needles on a
// sea-green sheet. Its flowers laid as opaque white discs with hard dark eyes, and its buds as
// flat pink circles, read at full size as stickers on barbed wire, and crowded the foot: half as
// many now, the petals thin white over the sheet, the cup a soft wash, the buds dabs of wash, and
// the wood left bare behind each flower so the white never lies grey over ink. Koromiko with narrow leaves and spotted
// spikes on straight stems read as lavender, and solid violet cones as seed pods; the spikes are
// soft white over a lilac wash, furred with stamens. Small red pōhutukawa flowers could read as
// the failed state's dot; its flowers are large airy bursts of stamens, at the shoots' ends, on
// wood that spreads low rather than hooking up. Its leaves drawn as fans of narrow leaflets from
// one point read as cannabis, and were wrong besides: they are single ellipses crowding the end of
// the shoot. A plant painted dark across the middle of a tall page crossed the lines of text over
// it; hence the column and its veil. A pale branch reaching up the page's edge for its whole
// height was considered for tall pages and left out: the bare dyed sheet above the plant is
// calmer. A red seal was the obvious signature, but its vermilion reads as Max's orange or the
// failed red, so there is none. One cream sheet for every plant left identity to small accents,
// hence the dyed sheets.

import { fromOklch } from "./theme.js";

const TAU = Math.PI * 2;
const INK = [24, 21, 18];

// ---- The plants -----------------------------------------------------------------------------

// Each plant: its name, its colour for the map (OKLCh), its sheet (paper, and the colour the dye
// takes it to at the foot), how it enters the page, its painter, and whether it lays opaque colour.
const PLANTS = [
  { name: "kōwhai", id: [0.8, 0.15, 95], paper: [247, 243, 230], dye: [238, 229, 190], enters: ["side", "side", "corner", "hang", "hang"], paint: paintKowhai, gouache: true },
  { name: "tūrutu", id: [0.52, 0.16, 280], paper: [242, 243, 242], dye: [214, 220, 231], enters: ["foot"], paint: paintTurutu, gouache: true },
  { name: "pōhutukawa", id: [0.48, 0.17, 2], paper: [246, 241, 237], dye: [236, 214, 212], enters: ["side", "side", "corner"], paint: paintPohutukawa, gouache: true },
  { name: "kawakawa", id: [0.6, 0.14, 132], paper: [244, 244, 236], dye: [221, 227, 202], enters: ["foot", "corner"], paint: paintKawakawa },
  { name: "koromiko", id: [0.55, 0.16, 322], paper: [243, 239, 242], dye: [222, 208, 228], enters: ["foot", "corner"], paint: paintKoromiko, gouache: true },
  { name: "mānuka", id: [0.6, 0.1, 192], paper: [228, 234, 231], dye: [198, 215, 211], enters: ["corner", "side", "corner"], paint: paintManuka, gouache: true },
];
function plantAt(at) { return PLANTS[Math.floor((((at % 1) + 1) % 1) * PLANTS.length + 1e-6) % PLANTS.length]; }

export default {
  name: "puawai",
  about: "A native plant in flower, painted close in brush, ink and colour on a sheet of dyed paper.",
  display: { family: "Alegreya", files: ["font-alegreya.woff2"], weights: "400 900", bold: 620, axes: null },
  palette: {
    paper: "#f6f3eb", tint: "#fbf9f4", line: "#e5dfd2",
    ink: "#1c1a17", body: "#3d3a34", muted: "#857f74",
    accent: "#3e5573", figure: ["#56693a", "#3e5573", "#9a7a2a", "#8a3a5a"], cover_ink: "#1c1a17", cover_body: "#3d3a34",
  },
  // The six plants, in the order planned: each next as unlike those before as the set allows.
  slots: [0, 1 / 6, 2 / 6, 3 / 6, 4 / 6, 5 / 6],
  map: { tone: "#4a4640", duo: 0 },
  identity(at) { const [L, C, H] = plantAt(at).id; return fromOklch(L, C, H); },
  paint,
};

// ---- The painting ---------------------------------------------------------------------------

function paint(ctx, { w, h, dpr, at, stream, noise, quiet, kit }) {
  const plant = plantAt(at ?? 0);
  // Every canvas the painting makes: an OffscreenCanvas, which a worker has as well as a page.
  const canvas = (cw, ch) => new OffscreenCanvas(cw, ch);
  // The brush's scale, from the page's area, a tall page counted as at most a screen and a bit.
  const S = Math.max(0.25, Math.sqrt(w * Math.min(h, 2.2 * w)) / 1000) * 1.45;
  const q = quiet || { x: w * 0.25, y: h * 0.2, w: w * 0.5, h: h * 0.3 };
  const m = 10 * S + 6;
  const box = { x0: q.x - m, y0: q.y - m, x1: q.x + q.w + m, y1: q.y + q.h + m };
  const rd = reading(w, h, box, S, noise);
  // The layers at up to about a megapixel: on a phone that is nearly its full resolution.
  const k = Math.min(dpr, Math.sqrt(0.95e6 / (w * h)));
  const wet = sheet(canvas, w, h, k), dry = sheet(canvas, w, h, k), top = plant.gouache ? sheet(canvas, w, h, k, true) : null;
  const fr = stream("frame");
  const at0 = entry(fr.pick(plant.enters), fr, w, h, box, S, rd);
  plant.paint({ wet: wet.x, dry: dry.x, top: top?.x, r: stream("near"), parts: stream, f: 1, S, w, h, box, rd }, at0);

  const seed = stream("paper")() * 100;
  const dye = plant.dye.map((v, i) => Math.max(0, -Math.log(v / plant.paper[i])));
  const img = kit.shader(PAPER, wet.c.width, wet.c.height, {
    u_wet: wet.c, u_wets: shrink(canvas, wet.c, 5), u_dry: dry.c, u_drys: shrink(canvas, dry.c, 4), u_top: top ? top.c : blank(canvas), u_veil: veilMap(canvas, rd, w, h),
    u_paper: plant.paper.map((v) => v / 255), u_dye: dye, u_dg: [0.3, 1.32, 0.22 * at0.e, 0], u_size: [w, h], u_seed: seed,
  });
  if (img) ctx.drawImage(img, 0, 0, w, h);
  else {
    ctx.fillStyle = rgba(plant.paper, 1); ctx.fillRect(0, 0, w, h);
    ctx.globalCompositeOperation = "multiply";
    ctx.drawImage(wet.c, 0, 0, w, h); ctx.drawImage(dry.c, 0, 0, w, h);
    if (top) { ctx.globalCompositeOperation = "screen"; ctx.drawImage(top.c, 0, 0, w, h); }
    ctx.globalCompositeOperation = "source-over";
  }
}

// Where body text will run over the painting: the lead's column, from the top of the page down to
// its foot (about half a screen high). Ink there is laid pale, as the far branches are, so no dark
// stroke crosses a line of text, and no bloom is put there: the dark strokes and the blooms keep to
// the margins beside the column and to the foot below it.
// The front branches steer clear of the column above the foot (avoid) as they do of the lead.
function reading(w, h, box, S, noise) {
  const F = Math.min(0.5 * h, 430 * Math.sqrt(S)), foot = h - F;
  return { x0: box.x0, x1: box.x1, foot, F, soft: 40 * S, n: noise, avoid: { ...box, y1: Math.max(box.y1, foot - 0.05 * F) } };
}
// How far (x, y) is in the reading column, from 0 (clear of it) to 1, its edges ragged as mist's.
function veil(rd, x, y) {
  const g = rd.n.fbm(x * 0.004, y * 0.004, 3);
  const s = (a, b, v) => { const t = clamp01((v - a) / (b - a)); return t * t * (3 - 2 * t); };
  return s(-0.4 * rd.soft, rd.soft, Math.min(x - rd.x0, rd.x1 - x) + g * rd.soft) * (1 - s(rd.foot - 0.15 * rd.F, rd.foot + 0.15 * rd.F, y + g * 0.12 * rd.F));
}
// The veil as a small map for the shader, a cell to about every 10 pixels.
function veilMap(canvas, rd, w, h) {
  const cw = Math.max(2, Math.ceil(w / 10)), ch = Math.max(2, Math.ceil(h / 10)), c = canvas(cw, ch), x = c.getContext("2d");
  const img = x.createImageData(cw, ch), d = img.data;
  for (let j = 0, i = 0; j < ch; j++) for (let k = 0; k < cw; k++, i += 4) { d[i] = Math.round(255 * veil(rd, ((k + 0.5) * w) / cw, ((j + 0.5) * h) / ch)); d[i + 3] = 255; }
  x.putImageData(img, 0, 0);
  return c;
}

// Where the plant comes into the page: from a side low down, from a bottom corner rising, from a
// top corner hanging beside the lead, or up from the foot. e is the side (-1 left, 1 right), the
// one with more room beside the lead, more often.
function entry(kind, r, w, h, box, S, rd) {
  const roomL = Math.max(0, box.x0), roomR = Math.max(0, w - box.x1), cap = 760 * S;
  let e = r() * (roomL + roomR + 0.4 * w) < roomL + 0.2 * w ? -1 : 1;
  // Hanging needs room beside the lead.
  if (kind === "hang" && Math.max(roomL, roomR) < 0.18 * w) kind = "corner";
  if (kind === "hang" && (e < 0 ? roomL : roomR) < 0.18 * w) e = -e;
  const below = Math.max(box.y1, rd.foot - 0.1 * rd.F);
  // Aimed to pass the lead rather than run at it and bend round it: of a few tries at the angle,
  // the first whose line keeps well clear, or else the clearest.
  const aim = (x, y, lo, hi, len) => {
    let best = null;
    for (let i = 0; i < 6; i++) {
      const a0 = -r.range(lo, hi), a = e < 0 ? a0 : Math.PI - a0;
      let c = Infinity;
      for (let t = 0.15; t <= 0.8; t += 0.13) c = Math.min(c, outside(x + Math.cos(a) * len * t, y + Math.sin(a) * len * t, rd.avoid));
      if (!best || c > best.c) best = { a, c };
      if (c > 40 * S) break;
    }
    return best.a;
  };
  if (kind === "side") {
    const y = below + (h - below) * r.range(0.25, 0.75), x = e < 0 ? -6 : w + 6, len = Math.min(cap, r.range(0.55, 0.85) * Math.max(w, 0.62 * Math.min(h, 2.2 * w)));
    return { e, kind, x, y, a: aim(x, y, 0.08, 0.5, len), len };
  }
  if (kind === "corner") {
    const x = e < 0 ? -8 : w + 8, len = Math.min(cap, r.range(0.6, 0.85) * Math.hypot(w, Math.min(h, 1.4 * w)));
    return { e, kind, x, y: h + 8, a: aim(x, h + 8, 0.35, 1.2, len), len };
  }
  if (kind === "hang") {
    const room = e < 0 ? roomL : roomR, a = r.range(0.75, 1.25);
    const x = r.range(0.1, 0.5) * room;
    return { e, kind, x: e < 0 ? x : w - x, y: -8, a: e < 0 ? a : Math.PI - a, len: Math.min(cap, r.range(0.55, 0.85) * h) };
  }
  // The foot: from below the bottom edge, a little in from a side, leaning in.
  const x = r.range(0.1, 0.42) * Math.min(w, Math.max(roomL, roomR, 0.5 * w));
  return { e, kind: "foot", x: e < 0 ? x : w - x, y: h + 10 * S, a: -Math.PI / 2 - e * r.range(0.1, 0.45), len: Math.min(0.8 * cap, r.range(0.65, 0.95) * h) };
}

// ---- Layers ---------------------------------------------------------------------------------

// A layer at k pixels per CSS pixel, drawn in CSS pixels. Ink and washes go on white, laid down
// by multiplying, as pigment on paper; opaque colour goes on black (the top layer), so each pixel
// holds its colour times its cover, and the cover is its red (opaque colours here are whites,
// creams and pinks, all at full red).
function sheet(canvas, w, h, k, opaque = false) {
  const c = canvas(Math.max(1, Math.round(w * k)), Math.max(1, Math.round(h * k)));
  const x = c.getContext("2d");
  x.fillStyle = opaque ? "#000" : "#fff"; x.fillRect(0, 0, c.width, c.height);
  x.setTransform(k, 0, 0, k, 0, 0);
  x.globalCompositeOperation = opaque ? "source-over" : "multiply";
  x.lineCap = "round"; x.lineJoin = "round";
  return { c, x };
}
function blank(canvas) { const c = canvas(1, 1), x = c.getContext("2d"); x.fillStyle = "#000"; x.fillRect(0, 0, 1, 1); return c; }

// A layer shrunk by f: its blur, for the wash's rim and its bleed.
function shrink(canvas, c, f) {
  const s = canvas(Math.max(1, Math.round(c.width / f)), Math.max(1, Math.round(c.height / f)));
  const x = s.getContext("2d");
  x.imageSmoothingQuality = "high";
  x.drawImage(c, 0, 0, s.width, s.height);
  return s;
}

// The paper, and the layers laid on it. The sheet: its formation (where the fibre lay thicker),
// long fibres bent by the pulp's flow, short fine ones, tooth, and a few flecks of bark. The dye:
// deeper toward the foot and the plant's side, uneven as the formation took it, and taken less by
// the fibres. A wash carries its pigment to its rim as it
// dries (where the layer is darker than its blur) and creeps out along the fibres beyond its edge
// (where its blur reaches but it does not); dry ink feathers a little; both take the paper's
// tooth. Opaque colour sits on top. In the reading column (the veil) all of it is laid pale.
const PAPER = `precision highp float;
uniform vec2 u_res, u_size; uniform float u_seed; uniform vec3 u_paper, u_dye; uniform vec4 u_dg;
uniform sampler2D u_wet, u_wets, u_dry, u_drys, u_top, u_veil;
float h12(vec2 p){ vec3 p3 = fract(vec3(p.xyx) * .1031); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.x + p3.y) * p3.z); }
float vn(vec2 p){ vec2 i = floor(p), f = fract(p); vec2 u = f * f * (3. - 2. * f);
  return mix(mix(h12(i), h12(i + vec2(1, 0)), u.x), mix(h12(i + vec2(0, 1)), h12(i + vec2(1, 1)), u.x), u.y); }
float fbm(vec2 p){ float s = 0., a = .5; for (int i = 0; i < 3; i++) { s += a * vn(p); p = p * 2.07 + 13.7; a *= .5; } return s / .875; }
// In some cells a short fibre at its own angle: how near one runs.
float fibres(vec2 p, float keep){
  vec2 i = floor(p), f = fract(p); float m = 0.;
  for (int y = -1; y <= 1; y++) for (int x = -1; x <= 1; x++) {
    vec2 c = i + vec2(float(x), float(y));
    float a = h12(c * 1.7 + 3.1) * 6.2832;
    vec2 d = vec2(cos(a), sin(a)), o = vec2(float(x), float(y)) + vec2(h12(c), h12(c + 9.4)) - f;
    float t = clamp(-dot(o, d), -.75, .75);
    m = max(m, (1. - smoothstep(.015, .06, length(o + d * t))) * (.3 + .7 * h12(c + 4.2)) * step(keep, h12(c + 1.3)));
  }
  return m;
}
vec3 absorb(vec3 c){ return -log(max(c, vec3(.02))); }
float mean(vec3 a){ return (a.r + a.g + a.b) / 3.; }
void main(){
  vec2 uv = vec2(gl_FragCoord.x / u_res.x, 1. - gl_FragCoord.y / u_res.y), px = uv * u_size;
  float cloud = fbm(px / 170. + u_seed) - .5, flock = fbm(px / 26. - u_seed) - .5, tooth = vn(px * .9 + u_seed * 3.) - .5;
  float fib = fibres(px / 30. + vec2(cloud, flock) * .9 + u_seed, .55), fin = fibres(px / 11. + flock * .8 - u_seed, .65);
  float t = uv.y + u_dg.z * (uv.x - .5) + .12 * cloud + .05 * flock;
  float dye = smoothstep(u_dg.x, u_dg.y, t);
  vec3 aP = u_dye * dye * (1. + .8 * flock + .5 * cloud) * (1. - .3 * fib);
  vec3 aW = absorb(texture2D(u_wet, uv).rgb), aWs = absorb(texture2D(u_wets, uv).rgb);
  vec3 aD = absorb(texture2D(u_dry, uv).rgb), aDs = absorb(texture2D(u_drys, uv).rgb);
  float dW = mean(aW), dWs = mean(aWs), dD = mean(aD), dDs = mean(aDs);
  float rim = clamp((dW - dWs) / (dW + .08), 0., 1.);
  aW *= (1. + 1.1 * rim) * (1. + .6 * flock + .25 * cloud) * (1. + .3 * tooth);
  float beyond = 1. - smoothstep(0., .6 * dWs + .02, dW);
  aW += aWs * beyond * (.15 + 1.4 * fib + .5 * fin) * .6;
  aD *= 1. + .35 * tooth;
  aD += aDs * (1. - smoothstep(0., .7 * dDs + .02, dD)) * (.1 + 1.1 * fib + .4 * fin) * .45;
  float v = texture2D(u_veil, uv).r;
  aW *= 1. - .74 * v; aD *= 1. - .8 * v;
  float speck = step(.9996, h12(floor(px / 3.) + u_seed * 7.)) * (.5 + .5 * h12(floor(px / 3.) + 2.));
  vec3 paper = u_paper * (1. + .03 * cloud + .016 * flock + .014 * tooth + .008 * fib - .006 * fin - .08 * speck);
  vec3 col = paper * exp(-(aP + aW + aD));
  vec3 g = texture2D(u_top, uv).rgb * (1. - .85 * v);
  col = col * (1. - .94 * g.r) + .94 * g * (1. + .03 * tooth);
  gl_FragColor = vec4(col, 1.);
}`;

// ---- The brush ------------------------------------------------------------------------------

const turn = (a, goal, k) => a + Math.atan2(Math.sin(goal - a), Math.cos(goal - a)) * k;
const rgba = (c, a) => `rgba(${c[0]},${c[1]},${c[2]},${Math.max(0, Math.min(1, a))})`;
const clamp01 = (v) => Math.max(0, Math.min(1, v));
// Smooth noise along a line, in [0, 1], from an integer hash: the same everywhere, always.
function hash1(n) { n = Math.imul(n ^ 0x9e3779b9, 0x85ebca6b); n ^= n >>> 13; n = Math.imul(n, 0xc2b2ae35); n ^= n >>> 16; return (n >>> 0) / 4294967296; }
function wob(x) { const i = Math.floor(x), f = x - i, u = f * f * (3 - 2 * f); return hash1(i) * (1 - u) + hash1(i + 1) * u; }

// Each point's normal, across the spine.
function normals(pts) {
  const n = pts.length;
  for (let i = 0; i < n; i++) {
    const a = pts[Math.max(0, i - 1)], b = pts[Math.min(n - 1, i + 1)];
    const dx = b.x - a.x, dy = b.y - a.y, l = Math.hypot(dx, dy) || 1;
    pts[i].nx = -dy / l; pts[i].ny = dx / l;
  }
  return pts;
}

// A gradient along a spine, from its first point to its last: stops [t, colour, alpha].
function along(c, pts, stops) {
  const A = pts[0], B = pts[pts.length - 1], g = c.createLinearGradient(A.x, A.y, B.x, B.y);
  for (const [t, col, a] of stops) g.addColorStop(t, rgba(col, a));
  return g;
}

// The hairs of a loaded brush along a spine of points {x, y, w} (w its half-width there). Each
// hair weaves a little, comes down and lifts off at its own moment (the outer ones a little later
// and sooner, so the stroke's ends are round). The hairs overlap, so a loaded brush lays a solid
// stroke; as it empties, small clumps of neighbouring hairs run dry together and leave thin white
// streaks, more along the stroke and toward its edges (dry 0 is a wet, solid stroke; 1 mostly
// streaks). Hairs carry ink of five strengths, the darker toward one side (`side`), and the ink
// thins along the stroke as the brush empties, to `end` of its strength. cover is how much each
// hair spreads. span is the part of the brush's width that touches, from -1 to 1.
function bone(c, pts, { ink = INK, a = 0.9, dry = 0.3, r, side = 1, cover = 1.8, span = null, end = null }) {
  if (pts.length < 2) return;
  normals(pts);
  let wmax = 0, len = 0;
  pts.forEach((p, i) => { wmax = Math.max(wmax, p.w); if (i) len += Math.hypot(p.x - pts[i - 1].x, p.y - pts[i - 1].y); });
  const lo = span ? span[0] : -1, hi = span ? span[1] : 1, part = (hi - lo) / 2;
  // The brush's round end, as a share of the stroke: the outer hairs touch down a width later.
  const e0 = Math.min(0.25, wmax / Math.max(1, len));
  const n = Math.max(4, Math.min(30, Math.round(wmax * 1.8 * part + 3))), N = pts.length;
  // Five strengths of ink, each in a fine hair and a thick one.
  const paths = Array.from({ length: 10 }, () => new Path2D()), clump = Math.max(1, Math.round(n / 8));
  const phase = Array.from({ length: Math.ceil(n / clump) }, () => r() * 1000);
  // A broad brush (an old limb) shows its streaks as gaps where hairs ran dry, short and staggered,
  // rather than as hairs of lighter ink running its whole length.
  const broad = clamp01((n - 10) / 12);
  for (let j = 0; j < n; j++) {
    // Unevenly spaced, so the dry streaks are not combed.
    const o0 = lo + ((j + 0.5) / n) * (hi - lo) + (1.8 * r.range(-0.5, 0.5) * (hi - lo)) / n, ao = o0 * o0;
    const g = Math.max(0, Math.min(4, Math.floor(2.5 + 1.6 * o0 * side + r.range(-1.3, 1.3)))) * 2 + (r() < 0.5 ? 1 : 0);
    const t0 = e0 * (ao * 0.9 + r() * 0.3), t1r = r(), thirst = dry * (0.55 + 0.7 * ao) * r.range(0.7, 1.3), ph = phase[Math.floor(j / clump)], own = r() * 1000, sway = r() * 1000;
    // A dry brush's outer hairs give out early, each at its own point.
    const t1 = 1 - e0 * (ao + t1r * 0.4) - dry * ao * 0.35 * hash1(Math.floor(own * 977));
    let on = false;
    for (let i = 0; i < N; i++) {
      const t = i / (N - 1);
      // The clump's streak, and each hair breaking up more as the brush empties.
      const ink = 1 - thirst * (0.2 + 0.9 * t) + 1.1 * dry * (wob(ph + i * 0.045) - 0.5) + dry * (0.3 + 0.8 * t) * (wob(own + i * 0.3) - 0.5) + broad * dry * (1.2 + t) * (wob(own * 1.7 + i * 0.09) - 0.5);
      if (t < t0 || t > t1 || ink <= 0.38) { on = false; continue; }
      // A clump of hairs drifts together across the stroke, so the streaks part and close.
      const q = pts[i], o = o0 + (0.05 + 0.12 * dry) * (wob(ph + 500 + i * 0.03) - 0.5) + (0.05 + 0.12 * broad) * (wob(sway + i * 0.06) - 0.5);
      const x = q.x + q.nx * o * q.w, y = q.y + q.ny * o * q.w;
      if (on) paths[g].lineTo(x, y); else paths[g].moveTo(x, y);
      on = true;
    }
  }
  const base = (2 * wmax * part * cover) / n, tones = [0.4, 0.57, 0.72, 0.87, 1], fin = end ?? 0.85 - 0.4 * dry;
  paths.forEach((p, k) => {
    const tn = tones[k >> 1] + (1 - tones[k >> 1]) * 0.55 * broad;
    c.lineWidth = Math.max(0.45, base * (k & 1 ? 1.3 : 0.7));
    c.strokeStyle = along(c, pts, [[0, ink, a * tn], [1, ink, a * tn * fin]]); c.stroke(p);
  });
}

// The wet body of a stroke: its outline filled in a wash (a colour, or a gradient), the edge a
// little uneven, its ends drawn in round as the brush's are.
function body(c, pts, spread, fill) {
  if (pts.length < 2) return;
  normals(pts);
  const p = new Path2D(), ph = (pts[0].x * 7.1 + pts[0].y * 3.3) | 0, N = pts.length - 1;
  const half = (i, k) => pts[i].w * spread * (0.84 + 0.32 * wob(ph + k + i * 0.45)) * Math.min(1, (Math.min(i, N - i) / Math.max(1, N * 0.1)) ** 0.5);
  pts.forEach((q, i) => { const s = half(i, 0); if (i) p.lineTo(q.x + q.nx * s, q.y + q.ny * s); else p.moveTo(q.x + q.nx * s, q.y + q.ny * s); });
  for (let i = N; i >= 0; i--) { const q = pts[i], s = half(i, 50); p.lineTo(q.x - q.nx * s, q.y - q.ny * s); }
  p.closePath();
  c.fillStyle = fill; c.fill(p);
}

// A whole stroke of the loaded brush: the water runs ahead as a pale wet core, pooled where the
// brush came down and drying out along the stroke; the hairs follow (bone).
function brush(pen, pts, { ink = INK, tone = 0.9, wet = 0.4, dry = 0.3, side = 1, cover = 1.8, span = null }) {
  const { f } = pen;
  if (pts.length < 2) return;
  if (wet > 0) {
    const a = tone * (0.12 + 0.3 * wet) * f;
    body(pen.wet, pts, 0.85 + 0.35 * wet, along(pen.wet, pts, [[0, ink, a], [0.55, ink, a * 0.6], [1, ink, a * 0.15 * (1 - dry)]]));
    const p = pts[0], q = pts[Math.min(3, pts.length - 1)];
    dots(pen.wet, [[p.x, p.y, p.w * 1.3, p.w * 1.05, Math.atan2(q.y - p.y, q.x - p.x)]], ink, tone * 0.32 * wet * f);
  }
  bone(pen.dry, pts, { ink, a: tone * f, dry, r: pen.r, side, cover, span });
}

// One press of a wet brush, a leaf or a petal: from its base at (x, y) along angle ang, len long
// and wid wide at `at` of its length, bending by `bend` along it, its edge a little uneven.
// Added to path p; returns its tip and its middle.
function press(p, x, y, ang, len, wid, { bend = 0, at = 0.45, r, n = 12, blunt = 0 }) {
  const L = [], R = [];
  let px = x, py = y, aa = ang - bend * 0.5, mx = x, my = y;
  for (let i = 0; i <= n; i++) {
    const t = i / n;
    const prof = t < at ? Math.sin(((t / at) * Math.PI) / 2) ** 0.75 : Math.max(blunt, Math.cos((((t - at) / (1 - at)) * Math.PI) / 2)) ** 0.85;
    const hw = wid * 0.5 * prof * (1 + r.range(-0.07, 0.07));
    const nx = -Math.sin(aa), ny = Math.cos(aa);
    L.push(px + nx * hw, py + ny * hw); R.push(px - nx * hw, py - ny * hw);
    if (i === Math.round(n / 2)) { mx = px; my = py; }
    if (i < n) { aa += bend / n; px += (Math.cos(aa) * len) / n; py += (Math.sin(aa) * len) / n; }
  }
  p.moveTo(L[0], L[1]);
  for (let i = 2; i < L.length; i += 2) p.lineTo(L[i], L[i + 1]);
  for (let i = R.length - 2; i >= 0; i -= 2) p.lineTo(R[i], R[i + 1]);
  p.closePath();
  return [px, py, mx, my];
}

// A press of a brush loaded with colour and tipped with ink: dark along one side (sd) of its
// midrib, the colour across to the other. Returns its tip.
function leaf(c, x, y, ang, len, wid, dark, light, a, opts, sd = 1) {
  const p = new Path2D(), [ex, ey, mx, my] = press(p, x, y, ang, len, wid, opts);
  const nx = -Math.sin(ang) * wid * 0.5 * sd, ny = Math.cos(ang) * wid * 0.5 * sd;
  const g = c.createLinearGradient(mx + nx, my + ny, mx - nx, my - ny);
  g.addColorStop(0, rgba(dark, a)); g.addColorStop(0.45, rgba(light, a * 0.9)); g.addColorStop(1, rgba(light, a * 0.7));
  c.fillStyle = g; c.fill(p);
  return [ex, ey];
}

// Fine lines in one colour, each a list of [x, y]: stamens, stalks, veins, outlines.
function lines(c, list, width, col, a) {
  if (!list.length) return;
  const p = new Path2D();
  for (const pts of list) { p.moveTo(pts[0][0], pts[0][1]); for (let i = 1; i < pts.length; i++) p.lineTo(pts[i][0], pts[i][1]); }
  c.lineWidth = width; c.strokeStyle = rgba(col, a); c.stroke(p);
}

// Dots in one colour, each [x, y, rx, ry, angle].
function dots(c, list, col, a) {
  if (!list.length) return;
  const p = new Path2D();
  for (const [x, y, rx, ry = rx, an = 0] of list) { p.moveTo(x + rx * Math.cos(an), y + rx * Math.sin(an)); p.ellipse(x, y, rx, ry, an, 0, TAU); }
  c.fillStyle = rgba(col, a); c.fill(p);
}

// Leave paper bare in a layer, where something stands in front: as a painter leaves a gap. (On
// the opaque layer, takes its colour off, so ink beneath shows through.)
function reserve(c, list, a = 1, off = false) {
  if (!list.length) return;
  const p = new Path2D();
  for (const [x, y, rx, ry = rx] of list) { p.moveTo(x + rx, y); p.ellipse(x, y, rx, ry, 0, 0, TAU); }
  c.save(); c.globalCompositeOperation = off ? "destination-out" : "source-over"; c.fillStyle = `rgba(255,255,255,${a})`; c.fill(p); c.restore();
}

// A quadratic curve as points: from a, bulging by `bow` (as a share of its length) to b.
function curve(ax, ay, bx, by, bow, n = 6) {
  const mx = (ax + bx) / 2 - (by - ay) * bow, my = (ay + by) / 2 + (bx - ax) * bow, out = [];
  for (let i = 0; i <= n; i++) { const t = i / n, u = 1 - t; out.push([u * u * ax + 2 * u * t * mx + t * t * bx, u * u * ay + 2 * u * t * my + t * t * by]); }
  return out;
}

// ---- Growth ---------------------------------------------------------------------------------

// How far (x, y) is outside the lead's box; negative inside.
function outside(x, y, b) {
  const dx = Math.max(b.x0 - x, 0, x - b.x1), dy = Math.max(b.y0 - y, 0, y - b.y1);
  if (dx > 0 || dy > 0) return Math.hypot(dx, dy);
  return -Math.min(x - b.x0, b.x1 - x, y - b.y0, b.y1 - y);
}

// A branch grown from a start {x, y, a, w, len}: it steps on, wanders and bends, turns at each
// node (mostly to alternate sides, so it zigzags, as kōwhai and plum do) in an elbow about a width
// long, forks at some nodes away from the turn, turns up (or droops) as its habit says, turns away
// from the lead's box, and tapers. Some branches (H.back of them) grow behind, and are painted
// pale. Each branch is one stroke through its nodes, unless H.whole says the brush lifts at each
// joint from that order on (kawakawa's jointed stems).
// Returns {strokes, nodes, tips}.
function grow(pen, start, H) {
  const out = { strokes: [], nodes: [], tips: [], count: 0 };
  branch(pen, start, H, 0, out);
  return out;
}
function branch(pen, { x, y, a, w, len, back = false }, H, order, out) {
  const { r, S, box } = pen;
  const ds = 2.4 * Math.max(0.5, S), look = 70 * S + 20, w0 = w, wEnd = Math.max(H.tip * S, w0 * H.end);
  let pts = [{ x, y, w }], knots = [], seg = r.range(...H.seg) * S, side = r() < 0.5 ? -1 : 1, s = 0, pend = 0;
  const cv = (r.range(-1, 1) * (H.bend ?? 0.25)) / 100, soft = order ? 1 : 0.55, bump = r() * 1000, a0 = a;
  out.count++;
  while (s < len) {
    // The node's turn, taken over the next width or so: an elbow.
    const take = pend * Math.min(0.5, Math.max(0.1, ds / (1.4 * w + ds)));
    a += take + r.gauss() * H.wander + cv * ds; pend -= take;
    // Never curling round on itself: a limb turned past a right angle from where it set out ends.
    if (Math.abs(Math.atan2(Math.sin(a - a0), Math.cos(a - a0))) > (order ? 2.2 : 1.2) && s > 40 * S) break;
    // Toward up (or down, for a droop): turn by the habit's share of the way to vertical.
    a = turn(a, -Math.PI / 2, H.up * 0.012);
    // Away from the lead's box (and, in front, from the reading column): if the way ahead runs into
    // it, turn whichever way clears it sooner.
    const shun = back ? box : pen.rd.avoid, ahead = outside(x + Math.cos(a) * look, y + Math.sin(a) * look, shun);
    if (ahead < 8 * S) {
      const l = outside(x + Math.cos(a - 0.5) * look, y + Math.sin(a - 0.5) * look, shun), rr = outside(x + Math.cos(a + 0.5) * look, y + Math.sin(a + 0.5) * look, shun);
      a += (l > rr ? -1 : 1) * 0.07 * Math.min(1.6, 0.4 + (8 * S - ahead) / look);
    }
    x += Math.cos(a) * ds; y += Math.sin(a) * ds; s += ds;
    w = (w0 + (wEnd - w0) * Math.min(1, s / len) ** 0.7) * (0.92 + 0.16 * wob(bump + s / (14 * S)));
    pts.push({ x, y, w });
    // It stops short of the lead by H.room, room for what grows at its tip.
    if (outside(x, y, box) < (H.room ?? 2) * S) break;
    if ((x < -60 || x > pen.w + 60 || y < -60 || y > pen.h + 60) && s > 60) break;
    seg -= ds;
    if (seg <= 0 && len - s > 14 * S) {
      const lift = order >= (H.whole ?? 3);
      knots.push(pts.length - 1);
      if (lift) out.strokes.push({ pts, order, tip: false, back, knots });
      out.nodes.push({ x, y, a, w, order, side, t: s / len, back });
      if (order < H.orders - 1 && out.count < H.max && r() < H.fork[order]) {
        const ca = a - side * r.range(...H.forkA);
        branch(pen, { x, y, a: ca, w: w * r.range(0.5, 0.72), len: Math.max(30 * S, (len - s) * r.range(...H.child)), back: back || r() < (H.back ?? 0) }, H, order + 1, out);
      }
      pend += side * r.range(...H.kink) * soft;
      if (r() < 0.75) side = -side;
      if (lift) { const bp = pts[Math.max(0, pts.length - 1 - Math.round(w / ds))]; pts = [{ x: bp.x, y: bp.y, w: w * 1.04 }, { x, y, w }]; knots = []; }
      seg = r.range(...H.seg) * S;
    }
  }
  out.strokes.push({ pts, order, tip: true, back, knots });
  out.tips.push({ x, y, a, w, order, back });
}

// Each stroke's pressure: pressed a little where the brush comes down, pressed again at each
// knuckle where it paused, and at a tip lifted off, blunt rather than drawn to a needle.
function pressure(st, S) {
  const n = st.pts.length, ds = 2.4 * Math.max(0.5, S);
  st.pts.forEach((p, i) => {
    const t = i / Math.max(1, n - 1);
    p.w *= 1 + 0.16 * Math.exp(-t * 9);
    if (st.tip) p.w *= Math.max(0.34, 1 - Math.max(0, (t - 0.55) / 0.45) ** 1.5);
  });
  for (const k of st.knots) {
    const rad = Math.max(2, st.pts[k].w / ds);
    for (let i = Math.max(0, Math.floor(k - 2 * rad)); i < Math.min(n, k + 2 * rad); i++) st.pts[i].w *= 1 + 0.2 * Math.exp(-(((i - k) / rad) ** 2));
  }
  return st;
}

// The wood of a grown plant, in three inks: the branches behind in pale wet ink; the old wood in
// a brush of grey ink over a wash, darker down one side and dry, so it breaks into streaks; the
// young wood dense, near black and quick. Then a near-black knot at some joints, and moss dots,
// the painter's dark accents, in small clusters on the old.
function wood(pen, g, { ink = INK, wash, dry = 0.5, moss = 0.5, washA = 0.3 }) {
  const { f, r, S } = pen;
  const side = r() < 0.5 ? -1 : 1;
  for (const st of [...g.strokes].sort((a, b) => b.back - a.back)) {
    pressure(st, S);
    const old = clamp01((st.pts[0].w / S - 2.8) / 5);
    if (st.back) { brush(pen, st.pts, { ink, tone: 0.3, wet: 0.9, dry: 0.35, side }); continue; }
    if (old > 0) body(pen.wet, st.pts, 0.8, along(pen.wet, st.pts, [[0, wash, washA * f * (0.4 + 0.6 * old)], [1, wash, washA * f * 0.3]]));
    brush(pen, st.pts, { ink, tone: st.order ? 0.94 : 0.9 - 0.12 * old, wet: 0.45 * (1 - 0.6 * old), dry: dry * (0.25 + 0.75 * old), side, cover: 1.9 });
  }
  const md = [], knots = [];
  for (const nd of g.nodes) {
    if (nd.back) continue;
    const nx = -Math.sin(nd.a), ny = Math.cos(nd.a);
    if (nd.order < 2 && r() < 0.5) {
      const o = nd.side * nd.w * 0.55;
      knots.push(curve(nd.x + nx * o - Math.cos(nd.a) * nd.w * 0.5, nd.y + ny * o - Math.sin(nd.a) * nd.w * 0.5, nd.x - nx * o * 0.1, nd.y - ny * o * 0.1, 0.2 * nd.side, 3));
    }
    if (nd.order > 1 || r() > moss) continue;
    const k = r.int(1, 3);
    for (let i = 0; i < k; i++) {
      const o = (r() < 0.5 ? -1 : 1) * nd.w * r.range(0.7, 1.15), al = r.range(-6, 6) * S;
      md.push([nd.x + nx * o + Math.cos(nd.a) * al, nd.y + ny * o + Math.sin(nd.a) * al, r.range(1, 2.3) * S, r.range(0.8, 1.5) * S, r() * TAU]);
    }
  }
  lines(pen.dry, knots, 1.6 * S, INK, 0.85 * f);
  dots(pen.wet, md, INK, 0.92 * f);
}

// Whether a part of radius rad at (x, y) keeps clear of the lead and mostly on the page, and, if it
// is a bloom, out of the reading column (leaves may stand there, laid pale).
function clear(pen, x, y, rad, bloom = true) {
  return outside(x, y, pen.box) > rad + 4 * pen.S && x > -rad && x < pen.w + rad && y > -rad && y < pen.h + rad && (!bloom || veil(pen.rd, x, y) < 0.5);
}

// A few of the sites, at least gap apart (and from those already in out): the painter's clusters,
// not a scatter.
function spaced(r, sites, n, gap, out = []) {
  const order = sites.map((s) => [r(), s]).sort((a, b) => a[0] - b[0]).map((x) => x[1]);
  for (const s of order) {
    if (out.length >= n) break;
    if (out.every((o) => Math.hypot(o.x - s.x, o.y - s.y) > gap)) out.push(s);
  }
  return out;
}

// ---- The plants' painters --------------------------------------------------------------------

// Kōwhai: bare zigzag wood, dark at the twigs, hung with loose bunches of gold flowers. Each flower
// hangs on a short stalk from a drooping stem: an olive cup of a calyx, a small petal turned back
// above it, and the long curved bell of the rest, deep gold down its shaded side and lemon down
// its lit one; the last at a bunch's tip still a green bud.
function paintKowhai(pen, at) {
  const { r, S } = pen;
  const g = grow(pen, { ...at, w: r.range(11, 15) * S }, { seg: [26, 60], kink: [0.3, 0.6], wander: 0.01, bend: 0.35, up: at.kind === "hang" ? -0.25 : 0.05, fork: [0.7, 0.55, 0.35], forkA: [0.45, 0.9], child: [0.35, 0.62], orders: 3, tip: 0.9, end: 0.22, max: 26, back: 0.3 });
  wood(pen, g, { wash: [98, 90, 80], dry: 0.6, washA: 0.42 });
  const sites = spaced(r, [...g.tips.filter((t) => t.order > 0), ...g.nodes.filter((n) => n.order > 0 || n.t > 0.4)], r.int(12, 18), 30 * S);
  for (const s of sites) bunch(pen, s, s.back ? 0.45 : 1);
  // Most often a tūī has come to the flowers.
  if (stream(pen, "bird")() < 0.7) tui(pen, g);
}

// A stream of its own for one part of a plant, from the plant's own (so the part can change
// without moving the rest).
function stream(pen, part) { return pen.parts(part); }

// A tūī perched on a front twig that runs level enough, facing into the page: painted as a bird is
// in ink, in a few presses of the brush. A round dark head with a fine curved beak; the back and
// wing one wet press of dark ink, a blue-green sheen glazed over the shoulder, the long flight
// feathers dry strokes; the breast paler; the tail three long strokes; its two white throat tufts
// and the lace at its nape laid in white; the eye a white ring; the feet gripping the twig.
function tui(pen, g) {
  const { wet, dry, top, S } = pen, r = stream(pen, "perch"), L = r.range(82, 96) * S;
  const spots = [];
  for (const st of g.strokes) {
    if (st.back || st.order < 1) continue;
    for (let i = 4; i < st.pts.length - 4; i += 3) {
      const p = st.pts[i], q = st.pts[i + 3], a = Math.atan2(q.y - p.y, q.x - p.x);
      if (Math.abs(Math.sin(a)) < 0.45 && p.w < 5 * S && p.x > 0.06 * pen.w + L * 0.5 && p.x < 0.94 * pen.w - L * 0.5 && clear(pen, p.x, p.y - L * 0.3, L * 0.5)) spots.push({ x: p.x, y: p.y + p.w * 0.3, a });
    }
  }
  if (!spots.length) return;
  const at = spots[Math.floor(r() * spots.length)], d = at.x > pen.w / 2 ? -1 : 1;
  // Local to page: x forward, y up, in the bird's length, from where its feet grip.
  const tilt = r.range(0.12, 0.38), ct = Math.cos(tilt), sn = Math.sin(tilt);
  const P = (lx, ly) => [at.x + d * (lx * ct - ly * sn) * L, at.y - (lx * sn + ly * ct) * L];
  const A = (al) => Math.atan2(-Math.sin(al + tilt), d * Math.cos(al + tilt));
  const pr = (pt, al, len, wid, o) => { const p = new Path2D(); press(p, ...P(...pt), A(al), len * L, wid * L, { r, n: 14, ...o }); return p; };
  const ln = (a0, a1, w0, w1, bow, n = 8) => { const [x0, y0] = P(...a0), [x1, y1] = P(...a1); return curve(x0, y0, x1, y1, bow * d, n).map(([x, y], j) => ({ x, y, w: (w0 + (w1 - w0) * (j / n)) * L })); };
  // The bird stands in front: whatever is behind it is left out.
  const body = [[0.17, 0.45, 0.11], [0.05, 0.33, 0.15], [-0.12, 0.27, 0.12]].map(([lx, ly, rr]) => [...P(lx, ly), rr * L]);
  reserve(wet, body, 0.9); reserve(dry, body, 0.95);
  // The breast in paler ink; over it, the head and back in one wet press, darker; the wing darker
  // still, a blue-green sheen glazed on its shoulder; then the head.
  wet.fillStyle = rgba([34, 38, 40], 0.5); wet.fill(pr([0.25, 0.38], Math.PI + 0.95, 0.42, 0.27, { at: 0.35, bend: -0.4 * d, blunt: 0.15 }));
  wet.fillStyle = rgba([30, 34, 38], 0.78); wet.fill(pr([0.27, 0.5], Math.PI + 0.5, 0.6, 0.3, { at: 0.3, bend: 0.2 * d }));
  wet.fillStyle = rgba([24, 28, 30], 0.82); wet.fill(pr([0.13, 0.39], Math.PI + 0.4, 0.45, 0.17, { at: 0.28, bend: 0.15 * d }));
  wet.fillStyle = rgba([52, 116, 106], 0.42); wet.fill(pr([0.15, 0.42], Math.PI + 0.45, 0.24, 0.15, { at: 0.4 }));
  const [hx, hy] = P(0.2, 0.47);
  dots(wet, [[hx, hy, 0.1 * L, 0.092 * L, A(0.3)]], [22, 25, 28], 0.85);
  // The flight feathers and the tail: dry strokes; the beak, fine and curved down a little.
  for (let i = 0; i < 4; i++) bone(dry, ln([0.02 - 0.035 * i, 0.31 - 0.02 * i], [-0.4 - 0.02 * i, 0.15 - 0.02 * i], 0.024, 0.008, 0.05), { ink: [24, 28, 30], a: 0.9, dry: 0.4, r, cover: 1.6 });
  for (let i = 0; i < 3; i++) { const sp = (i - 1) * 0.13; bone(dry, ln([-0.2, 0.22], [-0.2 - 0.45 * Math.cos(0.72 + sp), 0.22 - 0.45 * Math.sin(0.72 + sp)], 0.036, 0.022, 0.04), { ink: [24, 28, 30], a: 0.9, dry: 0.5, r, cover: 1.7 }); }
  bone(dry, ln([0.28, 0.48], [0.43, 0.43], 0.017, 0.004, 0.08, 6), { ink: INK, a: 0.95, dry: 0, r, cover: 1.6 });
  const feet = [];
  for (const fx of [-0.03, 0.05]) { const [x0, y0] = P(fx, 0.12), [x1, y1] = P(fx + 0.02, 0.0); feet.push([[x0, y0], [x1, y1], [x1 + d * 0.04 * L, y1 + 0.012 * L]]); }
  lines(dry, feet, 0.016 * L, INK, 0.85);
  // The white: the two throat tufts, the lace at the nape, the bar on the wing, the eye's ring.
  const tufts = [[0.272, 0.39, 0.034], [0.29, 0.348, 0.03]].map(([lx, ly, rr]) => { const [x, y] = P(lx, ly); return [x, y, rr * L, rr * L * 0.85, r() * TAU]; });
  const lace = [];
  for (let i = 0; i < 12; i++) { const [x, y] = P(0.13 + r.range(-0.05, 0.06), 0.41 + r.range(-0.04, 0.05)); lace.push([[x, y], [x - d * r.range(0.025, 0.045) * L, y + r.range(0, 0.025) * L]]); }
  dots(top, tufts, [255, 255, 255], 0.92);
  lines(top, lace, 0.005 * L + 0.4, [255, 255, 255], 0.5);
  const bar = new Path2D(); press(bar, ...P(0.03, 0.33), A(Math.PI + 0.4), 0.08 * L, 0.025 * L, { r, n: 6 });
  top.fillStyle = "rgba(255,255,255,0.7)"; top.fill(bar);
  const [ex, ey] = P(0.235, 0.49);
  dots(top, [[ex, ey, 0.014 * L]], [255, 250, 240], 0.75);
  reserve(top, [[ex, ey, 0.009 * L]], 1, true);
}
function bunch(pen, s, k) {
  const { wet, dry, r, S } = pen;
  // The bunch's stem leaves the twig along it and droops in an arc; the flowers hang from it in
  // turn, each on its own stalk, the lower ones further out, so each bell shows.
  const n = r.int(5, 9), A = r.range(18, 32) * S, dir = Math.sign(Math.cos(s.a)) || 1;
  const ex = s.x + dir * A * r.range(0.35, 0.7), ey = s.y + A * r.range(0.7, 0.95);
  if (!clear(pen, ex, ey + 20 * S, 30 * S) || !clear(pen, s.x, s.y, 4 * S)) return;
  const spine = curve(s.x, s.y, ex, ey, dir * 0.3, 8), stalks = [spine], cal = new Path2D(), buds = new Path2D(), keels = [];
  for (let i = 0; i < n; i++) {
    const t = 0.12 + (0.88 * i) / Math.max(1, n - 1), [px, py] = spine[Math.round(t * 8)], sd = i % 2 ? 1 : -1;
    const fa = Math.PI / 2 - dir * r.range(0, 0.2) + sd * r.range(0.12, 0.4), L = r.range(22, 30) * S * (0.85 + 0.2 * r());
    const pl = r.range(3, 8) * S, qx = px + Math.cos(fa - sd * 0.5) * pl, qy = py + Math.sin(fa - sd * 0.5) * pl;
    stalks.push([[px, py], [qx, qy]]);
    if (i === n - 1 ? r() < 0.7 : r() < 0.08) { press(buds, qx, qy, fa, L * 0.5, L * 0.22, { r, at: 0.55, n: 8, bend: sd * 0.3 }); continue; }
    press(cal, qx, qy, fa, L * 0.2, L * 0.26, { r, at: 0.8, blunt: 0.75, n: 6 });
    const cx = qx + Math.cos(fa) * L * 0.14, cy = qy + Math.sin(fa) * L * 0.14;
    // The bell: narrow at the cup, swelling to its curved open end.
    const bend = -sd * r.range(0.3, 0.6), p = new Path2D(), [tx, ty, mx, my] = press(p, cx, cy, fa, L * 0.86, L * 0.36, { r, at: 0.72, blunt: 0.55, bend, n: 10 });
    const nx = -Math.sin(fa) * L * 0.18, ny = Math.cos(fa) * L * 0.18;
    const gr = wet.createLinearGradient(mx + nx * sd, my + ny * sd, mx - nx * sd, my - ny * sd);
    gr.addColorStop(0, rgba([214, 186, 36], 0.92 * k)); gr.addColorStop(0.45, rgba([242, 214, 62], 0.88 * k)); gr.addColorStop(1, rgba([250, 236, 140], 0.82 * k));
    wet.fillStyle = gr; wet.fill(p);
    keels.push(curve(cx + nx * 0.45 * sd, cy + ny * 0.45 * sd, tx + nx * 0.35 * sd, ty + ny * 0.35 * sd, -bend * 0.1, 5));
  }
  wet.fillStyle = rgba([200, 196, 86], 0.85 * k); wet.fill(buds);
  wet.fillStyle = rgba([168, 158, 66], 0.8 * k); wet.fill(cal);
  lines(wet, keels, 0.9 * S, [188, 158, 30], 0.45 * k);
  lines(dry, stalks, 0.9 * S, [70, 64, 34], 0.85 * k);
}

// Pōhutukawa: grey limbs reaching out, each shoot's tip a cluster of leaves gathered under a head of
// crimson flowers. Each leaf is simple, elliptic and leathery, painted as the outline-and-wash
// painters do it: a grey-green wash, deeper down one side, outlined in ink while wet, a pale midrib;
// a few turned to show their white-felted backs. Each flower is a burst of long stamens tipped with
// gold pollen; grey felted buds beside them.
function paintPohutukawa(pen, at) {
  const { wet, dry, top, f, r, S } = pen;
  const g = grow(pen, { ...at, w: r.range(13, 18) * S }, { seg: [40, 80], kink: [0.1, 0.28], wander: 0.02, bend: 0.22, up: 0, fork: [0.8, 0.6, 0.25], forkA: [0.4, 0.8], child: [0.4, 0.7], orders: 3, tip: 1.8, end: 0.3, max: 16, back: 0.25, room: 30 });
  wood(pen, g, { ink: [46, 43, 41], wash: [126, 120, 112], dry: 0.7, washA: 0.5 });
  const edges = new Path2D(), felts = new Path2D(), ribs = [], backs = [], heads = [], buds = [];
  // The heads stand at the shoots' ends, and only then at a node.
  for (const t of spaced(r, g.nodes.filter((n) => n.order > 0 || n.t > 0.45), 12, 50 * S, spaced(r, g.tips, 12, 50 * S))) {
    if (!clear(pen, t.x, t.y, 30 * S, false)) continue;
    // The leaves crowd the last of the shoot, in turn on either side, the older ones further back and
    // spread wider, all drooping a little: under the head. A few are seen end-on, short and broad.
    const kk = t.back ? 0.45 : 1, n = r.int(5, 8);
    for (let i = 0; i < n; i++) {
      const u = i / (n - 1), sd = i % 2 ? 1 : -1, back = (1 - u) * r.range(10, 22) * S, short = r() < 0.25;
      const la = turn(t.a + sd * (0.3 + 1.3 * (1 - u)) + r.range(-0.2, 0.2), Math.PI / 2, 0.3), L = r.range(26, 38) * S * (short ? 0.65 : 1), W = L * (short ? 0.52 : r.range(0.36, 0.44));
      const bx = t.x - Math.cos(t.a) * back + Math.cos(la) * 2 * S, by = t.y - Math.sin(t.a) * back + Math.sin(la) * 2 * S;
      if (!clear(pen, bx + Math.cos(la) * L, by + Math.sin(la) * L, 4 * S, false)) continue;
      const felt = r() < 0.22, lit = r() < 0.5 ? -1 : 1, p = new Path2D();
      const [ex, ey, mx, my] = press(p, bx, by, la, L, W, { bend: r.range(-0.25, 0.25), at: 0.5, blunt: 0.25, r, n: 12 });
      const nx = -Math.sin(la) * W * 0.5 * lit, ny = Math.cos(la) * W * 0.5 * lit, gr = wet.createLinearGradient(mx + nx, my + ny, mx - nx, my - ny);
      if (felt) { gr.addColorStop(0, rgba([164, 168, 154], 0.5 * kk)); gr.addColorStop(1, rgba([212, 212, 198], 0.4 * kk)); }
      else { gr.addColorStop(0, rgba([56, 72, 60], 0.66 * kk)); gr.addColorStop(0.5, rgba([94, 112, 92], 0.55 * kk)); gr.addColorStop(1, rgba([136, 150, 126], 0.45 * kk)); }
      wet.fillStyle = gr; wet.fill(p);
      if (t.back) continue;
      (felt ? felts : edges).addPath(p);
      (felt ? backs : ribs).push(curve(bx, by, ex, ey, r.range(-0.04, 0.04), 5));
    }
    if (t.back) continue;
    const up = Math.atan2(Math.sin(t.a) - 1.2, Math.cos(t.a)), nb = r.int(1, 3);
    for (let i = 0; i < nb; i++) {
      const R = r.range(22, 32) * S * (i ? 0.75 : 1), ba = up + r.range(-0.8, 0.8), d = (i ? r.range(12, 20) : r.range(2, 6)) * S;
      const bx = t.x + Math.cos(ba) * d, by = t.y + Math.sin(ba) * d;
      if (clear(pen, bx + Math.cos(up) * R * 0.5, by + Math.sin(up) * R * 0.5, R * 1.1)) heads.push({ x: bx, y: by, R, a: up + r.range(-0.3, 0.3) });
    }
    if (r() < 0.6) for (let i = r.int(2, 5); i > 0; i--) {
      const ba = up + Math.PI * r.range(0.3, 0.7) * (r() < 0.5 ? -1 : 1), d = r.range(10, 22) * S;
      buds.push([t.x + Math.cos(ba) * d, t.y + Math.sin(ba) * d, r.range(2.6, 4) * S]);
    }
  }
  // The outlines, in ink with the brush lifting here and there; the felted backs whitened, their
  // outline and midrib grey; the upper sides' midribs pale.
  dry.setLineDash([16 * S, 2.4 * S, 7 * S, 1.8 * S]);
  dry.lineWidth = 0.8 * S; dry.strokeStyle = rgba([34, 42, 36], 0.72 * f); dry.stroke(edges);
  dry.strokeStyle = rgba([116, 120, 106], 0.5 * f); dry.stroke(felts);
  dry.setLineDash([]);
  top.fillStyle = rgba([255, 252, 242], 0.4 * f); top.fill(felts);
  lines(top, ribs, 0.9 * S, [255, 252, 238], 0.45 * f);
  lines(wet, backs, 0.8 * S, [120, 124, 110], 0.6 * f);
  dots(wet, buds, [168, 172, 152], 0.85 * f);
  dots(dry, buds.map(([x, y, rr]) => [x + rr * 0.3, y + rr * 0.35, rr * 0.45]), [92, 96, 80], 0.5 * f);
  // The heads in front of the leaves and the wood: a crimson core, then the stamens.
  const core = heads.map((b) => [b.x + Math.cos(b.a) * b.R * 0.35, b.y + Math.sin(b.a) * b.R * 0.35, b.R * 0.45]);
  reserve(wet, core, 0.6); reserve(dry, core, 0.8);
  const stamens = [], deep = [], anthers = [];
  for (const b of heads) {
    const cx = b.x + Math.cos(b.a) * b.R * 0.35, cy = b.y + Math.sin(b.a) * b.R * 0.35;
    const gr = wet.createRadialGradient(cx, cy, 0, cx, cy, b.R * 0.7);
    gr.addColorStop(0, rgba([178, 22, 54], 0.6 * f)); gr.addColorStop(0.55, rgba([196, 40, 70], 0.35 * f)); gr.addColorStop(1, rgba([196, 40, 70], 0));
    wet.fillStyle = gr; wet.beginPath(); wet.arc(cx, cy, b.R * 0.7, 0, TAU); wet.fill();
    // Several flowers to a head, each its own burst.
    for (let j = r.int(4, 6); j > 0; j--) {
      const ox = b.x + r.range(-0.3, 0.3) * b.R, oy = b.y + r.range(-0.1, 0.3) * b.R, fa = b.a + r.range(-0.7, 0.7);
      for (let i = Math.round((b.R * 1.1) / Math.max(0.5, S)); i > 0; i--) {
        const sa = fa + r.gauss() * 0.75, L = b.R * r.range(0.5, 0.95), ex = ox + Math.cos(sa) * L, ey = oy + Math.sin(sa) * L;
        (r() < 0.35 ? deep : stamens).push(curve(ox, oy, ex, ey, r.range(-0.12, 0.12), 3));
        if (r() < 0.55) anthers.push([ex, ey, r.range(0.6, 0.9) * S]);
      }
    }
  }
  lines(dry, stamens, 0.75 * S, [206, 26, 58], 0.85 * f);
  lines(dry, deep, 0.75 * S, [146, 14, 44], 0.85 * f);
  dots(top, anthers, [255, 206, 120], 0.85 * f);
}

// Mānuka: a wiry shrub-branch rising in sprays, every stem set thick with tiny sharp leaves (dark
// green, a few turned bronze), and small flowers here and there along the stems in the leaves'
// axils: five round petals laid in thin white, full at the rim and thinning to the base so the
// sheet shows through, some flushed pink beneath, round a soft green-gold cup and a few stamens;
// the buds dabs of red-pink wash. Its sheet is the cool grey-green of mānuka scrub on a hill.
function paintManuka(pen, at) {
  const { wet, dry, top, f, r, S } = pen;
  // Two or three stems from one place, fanning up and out.
  const g = { strokes: [] }, stems = r.int(2, 3), e = at.e || 1;
  for (let kk = 0; kk < stems; kk++) {
    const st = { ...at, x: at.x + r.range(-16, 16) * S, a: at.a + e * (kk - (stems - 1) / 2) * r.range(0.25, 0.4), len: at.len * (kk ? r.range(0.55, 0.8) : 1), w: r.range(4, 5.5) * S * (kk ? 0.8 : 1) };
    const gk = grow(pen, st, { seg: [24, 46], kink: [0.03, 0.12], wander: 0.014, bend: 0.3, up: 0.32, fork: [0.85, 0.65, 0.4], forkA: [0.22, 0.5], child: [0.45, 0.8], orders: 3, tip: 0.55, end: 0.3, max: kk ? 6 : 10, back: 0.3, whole: 4 });
    wood(pen, gk, { ink: [58, 40, 34], wash: [112, 96, 88], dry: 0.5, moss: 0, washA: 0.35 });
    g.strokes.push(...gk.strokes);
  }
  const leaves = new Path2D(), bronze = new Path2D(), pale = new Path2D(), ds = 2.4 * Math.max(0.5, S), flowers = [];
  for (const st of g.strokes) {
    const n = st.pts.length, from = st.order ? 1 : Math.floor(n * 0.3);
    for (let i = from, gap = 0; i < n - 1; i += Math.max(1, Math.round((r.range(2.8, 4.4) * S) / ds))) {
      const sd = r() < 0.5 ? -1 : 1, p = st.pts[i], q = st.pts[i - 1], a = Math.atan2(p.y - q.y, p.x - q.x), nx = -Math.sin(a) * sd, ny = Math.cos(a) * sd;
      const L = r.range(3.5, 8) * S * (i > n - 6 ? 0.6 : 1), bx = p.x + nx * p.w * 0.6, by = p.y + ny * p.w * 0.6;
      if (clear(pen, bx, by, 6 * S, false)) press(st.back ? pale : r() < 0.12 ? bronze : leaves, bx, by, a + sd * r.range(0.25, 0.95), L, L * r.range(0.2, 0.3), { r, at: 0.3, n: 5, bend: r.range(-0.3, 0.3) });
      // A flower in an axil now and then, all along the stem.
      gap -= ds * Math.max(1, Math.round((2.7 * S) / ds));
      if (st.back || (st.order === 0 && i < n * 0.5) || gap > 0 || r() < 0.4) continue;
      gap = r.range(12, 22) * S;
      const R = r.range(5, 7.5) * S, bud = r() < 0.28, o = p.w + R * 0.75, x = p.x + nx * o, y = p.y + ny * o;
      if (clear(pen, x, y, R * 1.5)) flowers.push({ x, y, R: bud ? R * 0.5 : R, rot: r() * TAU, bud, pink: r() < (bud ? 0.85 : 0.25), a: Math.atan2(ny, nx) });
    }
  }
  wet.fillStyle = rgba([34, 50, 36], 0.85 * f); wet.fill(leaves);
  wet.fillStyle = rgba([104, 62, 46], 0.75 * f); wet.fill(bronze);
  wet.fillStyle = rgba([40, 54, 38], 0.3 * f); wet.fill(pale);
  // Every second flower is left out, so the sprays stay open. The flowers' own brushwork draws on a
  // stream of its own, so it leaves the stems and leaves as they are.
  const fr = stream(pen, "flowers"), flush = [], cups = [], hearts = [], stamens = [], stalks = [], gaps = [];
  for (const fl of flowers.filter((_, i) => i % 2 === 0)) {
    if (fl.bud) {
      // A bud: one dab of red-pink wash from a short stalk, deep on its shaded side.
      const R = fl.R * fr.range(1, 1.25), a = fl.a + fr.range(-0.4, 0.4), bx = fl.x - Math.cos(a) * R * 1.1, by = fl.y - Math.sin(a) * R * 1.1;
      stalks.push([[bx - Math.cos(a) * R * 0.7, by - Math.sin(a) * R * 0.7], [bx, by]]);
      const sd = fr() < 0.5 ? -1 : 1;
      leaf(wet, bx, by, a, R * 2.2, R * 1.75, [168, 44, 74], [236, 150, 168], 0.78 * f, { r: fr, at: 0.55, blunt: 0.2, n: 9, bend: fr.range(-0.25, 0.25) }, sd);
      cups.push([bx + Math.cos(a) * R * 0.25, by + Math.sin(a) * R * 0.25, R * 0.42, R * 0.32, a]);
      continue;
    }
    // A flower, tilted a little to the eye: the flush of a pink one and its cup in wash beneath;
    // then the petals in white, thin at the base and full at the rim, laid in two passes so each
    // overlaps the next.
    const R = fl.R * fr.range(1, 1.2), tilt = fr.range(0.62, 1), ax = fr() * TAU;
    // The wood and leaves behind it left bare, so the thin white never lies grey over ink.
    gaps.push([fl.x, fl.y, R * 0.8 * tilt]);
    if (fl.pink) flush.push([fl.x, fl.y, R * 0.8, R * 0.8 * tilt, ax]);
    hearts.push([fl.x, fl.y, R * 0.26, R * 0.26 * tilt, ax]);
    for (let i = fr.int(5, 8); i > 0; i--) {
      const sa = fr() * TAU, c = Math.cos(sa), s = Math.sin(sa), d = R * fr.range(0.3, 0.42);
      const ex = c * Math.cos(ax) - s * tilt * Math.sin(ax), ey = c * Math.sin(ax) + s * tilt * Math.cos(ax);
      stamens.push([[fl.x + ex * R * 0.12, fl.y + ey * R * 0.12], [fl.x + ex * d, fl.y + ey * d]]);
    }
    top.save(); top.translate(fl.x, fl.y); top.rotate(ax); top.scale(1, tilt);
    const g = top.createRadialGradient(0, 0, R * 0.08, 0, 0, R * 1.05), col = fl.pink ? [255, 226, 234] : [255, 255, 255];
    g.addColorStop(0, rgba(col, 0.1)); g.addColorStop(0.45, rgba(col, 0.46)); g.addColorStop(1, rgba(col, 0.9));
    top.fillStyle = g;
    for (const pass of [0, 1]) {
      const p = new Path2D();
      for (let i = pass; i < 5; i += 2) press(p, 0, 0, fl.rot + (i * TAU) / 5 + fr.range(-0.14, 0.14), R * fr.range(0.9, 1.08), R * fr.range(0.82, 1), { r: fr, at: 0.62, blunt: 0.5, n: 8, bend: fr.range(-0.25, 0.25) });
      top.fill(p);
    }
    top.restore();
  }
  reserve(wet, gaps); reserve(dry, gaps);
  dots(wet, flush, [232, 150, 170], 0.42 * f);
  dots(wet, hearts, [150, 136, 66], 0.55 * f);
  dots(wet, cups, [120, 112, 58], 0.6 * f);
  lines(dry, stalks, 0.8 * S, [70, 54, 40], 0.8 * f);
  lines(dry, stamens, 0.55 * S, [168, 128, 64], 0.55 * f);
}

// Kawakawa: jointed stems rising from the foot, swollen at each node, a broad heart leaf at each
// (two presses of an ink-tipped brush, one each side of the midrib, veined while wet, a looper
// caterpillar's holes in some), and slender green-gold flower spikes standing up like candles.
function paintKawakawa(pen, at) {
  const { wet, dry, f, r, S } = pen;
  const stems = r.int(2, 3);
  const leaves = [], knots = [], spikes = [];
  for (let kk = 0; kk < stems; kk++) {
    const st = { ...at, x: at.x + (at.e || 1) * r.range(-10, 40) * S, a: at.a + (kk - (stems - 1) / 2) * r.range(0.3, 0.5) + r.range(-0.1, 0.1), len: at.len * r.range(0.6, 0.85), w: r.range(4.5, 6) * S };
    const g = grow(pen, st, { seg: [48, 86], kink: [0.12, 0.32], wander: 0.006, up: 0.3, fork: [0.5, 0.25], forkA: [0.5, 0.9], child: [0.4, 0.65], orders: 2, tip: 1.4, end: 0.55, max: 6, whole: 0, back: 0.3 });
    wood(pen, g, { ink: [34, 44, 30], wash: [96, 116, 80], dry: 0.3, moss: 0, washA: 0.2 });
    for (const n of g.nodes) {
      knots.push(n);
      if (r() < 0.75) leaves.push({ x: n.x, y: n.y, a: n.a + n.side * r.range(0.8, 1.4), L: r.range(46, 70) * S, back: n.back });
      if (r() < 0.35) spikes.push({ x: n.x, y: n.y, a: n.a - n.side * r.range(0.15, 0.45), L: r.range(22, 38) * S });
    }
    for (const t of g.tips) leaves.push({ x: t.x, y: t.y, a: t.a + r.range(-0.4, 0.4), L: r.range(32, 48) * S, back: t.back });
  }
  const pet = [], veins = [], holes = [];
  for (const lf of leaves) {
    // The leaf hangs a little from its stalk.
    const pa = lf.a + 0.25 * Math.cos(lf.a), pl = r.range(7, 13) * S;
    const bx = lf.x + Math.cos(pa) * pl, by = lf.y + Math.sin(pa) * pl, la = pa + r.range(0.05, 0.35) * Math.sign(Math.cos(pa) || 1);
    const L = lf.L, cx = bx + Math.cos(la) * L * 0.4, cy = by + Math.sin(la) * L * 0.4;
    if (!clear(pen, cx, cy, L * 0.62, false)) continue;
    pet.push([[lf.x, lf.y], [bx, by]]);
    const kk = lf.back ? 0.45 : 1, tone = r(), nx = -Math.sin(la), ny = Math.cos(la), W = L * r.range(0.82, 0.95);
    for (const s of [-1, 1]) {
      const p = new Path2D();
      heartHalf(p, bx, by, la, L, W, s, 0.5 * S);
      const mx = bx + Math.cos(la) * L * 0.35, my = by + Math.sin(la) * L * 0.35;
      const gr = wet.createLinearGradient(mx, my, mx + nx * s * W * 0.5, my + ny * s * W * 0.5);
      gr.addColorStop(0, rgba(s < 0 ? [52, 72, 42] : [72, 96, 54], 0.62 * kk)); gr.addColorStop(0.5, rgba([116 + 20 * tone, 142, 78], 0.52 * kk)); gr.addColorStop(1, rgba([156 + 16 * tone, 172, 104], 0.44 * kk));
      wet.fillStyle = gr; wet.fill(p);
    }
    if (lf.back) continue;
    const tipx = bx + Math.cos(la) * L, tipy = by + Math.sin(la) * L;
    veins.push(curve(bx, by, tipx, tipy, 0.02, 6));
    for (const s of [-1, 1]) for (const [u0, u1, v] of [[0.02, 0.8, 0.3], [0.05, 0.55, 0.36], [0.3, 0.75, 0.2]]) {
      const ax = bx + Math.cos(la) * L * u0, ay = by + Math.sin(la) * L * u0, ex = bx + Math.cos(la) * L * u1 + nx * s * L * v, ey = by + Math.sin(la) * L * u1 + ny * s * L * v;
      veins.push(curve(ax, ay, ex, ey, -s * 0.12, 5));
    }
    if (r() < 0.45) for (let i = r.int(1, 3); i > 0; i--) {
      const u = r.range(0.3, 0.75), v = r.range(-0.3, 0.3);
      holes.push([bx + Math.cos(la) * L * u + nx * v * L, by + Math.sin(la) * L * u + ny * v * L, r.range(1.4, 3.2) * S, r.range(1.2, 2.6) * S]);
    }
  }
  reserve(wet, holes, 0.92);
  // The veins drawn into the wet wash, so they show pale.
  wet.save(); wet.globalCompositeOperation = "source-over"; lines(wet, veins, 1.1 * S, [255, 255, 250], 0.42 * f); wet.restore();
  lines(dry, pet, 1.3 * S, [40, 52, 34], 0.85 * f);
  // The swollen joints: a short dark stroke across the stem.
  const kn = [];
  for (const n of knots) {
    if (n.back) continue;
    const nx = -Math.sin(n.a), ny = Math.cos(n.a), wd = n.w * 1.5;
    kn.push(curve(n.x - nx * wd, n.y - ny * wd, n.x + nx * wd, n.y + ny * wd, 0.25, 4));
  }
  lines(dry, kn, 2 * S, INK, 0.85 * f);
  const spd = [];
  for (const s of spikes) {
    const ex = s.x + Math.cos(s.a) * s.L, ey = s.y + Math.sin(s.a) * s.L;
    if (!clear(pen, ex, ey, 6 * S)) continue;
    const pts = curve(s.x, s.y, ex, ey, r.range(-0.08, 0.08), 10).map(([x, y], i) => ({ x, y, w: 2.2 * S * (i < 2 ? 0.45 : 1 - 0.35 * (i / 10)) }));
    body(wet, pts, 1, along(wet, pts, [[0, [150, 156, 70], 0.7 * f], [1, [196, 190, 84], 0.8 * f]]));
    for (let i = 3; i < pts.length; i++) spd.push([pts[i].x + r.range(-1, 1) * S, pts[i].y + r.range(-1, 1) * S, 0.65 * S]);
  }
  dots(dry, spd, [92, 94, 34], 0.6 * f);
}

// Half a heart leaf, side s of the midrib: from its stalk round a lobe, out to its widest, and in to
// a drawn-out point; set off from the midrib by gap, as two presses of a brush leave a pale vein.
function heartHalf(p, x, y, a, L, W, s, gap) {
  const c = Math.cos(a), sn = Math.sin(a), H = W / 2;
  const P = (u, v) => [x + u * c - (v * s + gap * s) * sn, y + u * sn + (v * s + gap * s) * c];
  p.moveTo(...P(0, 0));
  p.bezierCurveTo(...P(-0.17 * L, 0.3 * H), ...P(-0.08 * L, 1.05 * H), ...P(0.3 * L, H));
  p.bezierCurveTo(...P(0.62 * L, 0.98 * H), ...P(0.78 * L, 0.2 * H), ...P(L, 0));
  p.closePath();
}

// Koromiko: two or three stems arching up and out, bare below, leafy toward the tip in crossed
// pairs (one pair spread, the next seen end-on and short), the tip a tight rosette; from the upper
// pairs' axils long furred spikes arch out and droop, laid in white and flushed lilac, violet and
// in tight bud at the tip, their stamens standing out as fur.
function paintKoromiko(pen, at) {
  const { wet, top, f, r, S } = pen;
  const stems = r.int(2, 3), e = at.e || 1;
  const ribs = [], spikes = [];
  for (let kk = 0; kk < stems; kk++) {
    const st = { ...at, x: at.x + r.range(-24, 24) * S, a: at.a - e * r.range(0.1, 0.3) + (kk - (stems - 1) / 2) * r.range(0.3, 0.45), len: at.len * r.range(0.6, 0.9), w: r.range(3.4, 4.6) * S };
    const g = grow(pen, st, { seg: [40, 70], kink: [0.04, 0.14], wander: 0.008, bend: 0.3, up: 0.1, fork: [0.5, 0], forkA: [0.35, 0.7], child: [0.4, 0.6], orders: 2, tip: 1.2, end: 0.45, max: 3, whole: 2, back: 0.35 });
    wood(pen, g, { ink: [52, 50, 38], wash: [120, 116, 90], dry: 0.35, moss: 0.15, washA: 0.2 });
    for (const stk of g.strokes) {
      const pts = stk.pts, ds = 2.4 * Math.max(0.5, S), step = Math.max(3, Math.round((r.range(12, 16) * S) / ds)), kk2 = stk.back ? 0.4 : 1;
      const from = Math.max(4, Math.floor(pts.length * (stk.tip ? 0.42 : 0.6)));
      for (let i = from, pair = 0; i < pts.length - 1; i += step, pair++) {
        const p = pts[i], q = pts[i - 1], a = Math.atan2(p.y - q.y, p.x - q.x), u = (i - from) / Math.max(1, pts.length - from);
        const end = pair % 2 ? 0.42 : 1, size = 1 - 0.55 * u * u;
        for (const sd of [-1, 1]) {
          const la = a + sd * (pair % 2 ? r.range(0.2, 0.4) : r.range(0.55, 0.9)) * (1 - 0.4 * u), L = r.range(28, 40) * S * end * size;
          if (!clear(pen, p.x + Math.cos(la) * L, p.y + Math.sin(la) * L, 3 * S, false)) continue;
          const tip = leaf(wet, p.x, p.y, la, L, L * 0.3, [30, 50, 32], [92, 132, 76], (pair % 2 ? 0.78 : 0.66) * kk2 * f, { bend: 0.3 * Math.cos(la) * (Math.sin(la) < 0 ? 1 : 0.4), at: 0.38, r, n: 8 }, sd);
          ribs.push([[p.x, p.y], tip]);
        }
        if (!stk.back && u > 0.1 && u < 0.75 && pair % 2 === 0 && r() < 0.8) for (const sd of [-1, 1]) {
          if (r() < 0.25) continue;
          spikes.push({ x: p.x, y: p.y, a: a + sd * r.range(0.6, 1.1), L: r.range(48, 80) * S });
        }
      }
    }
  }
  lines(wet, ribs, 0.6 * S, [28, 46, 30], 0.5 * f);
  const florets = [[]], eyes = [], fuzz = [], anth = [], tips = [];
  for (const s of spikes) {
    const ex = s.x + Math.cos(s.a) * s.L, ey = s.y + Math.sin(s.a) * s.L * 0.6 + s.L * 0.35;
    if (!clear(pen, (s.x + ex) / 2, (s.y + ey) / 2, s.L * 0.55)) continue;
    // Arched over and drooping: bulging up from its chord.
    const bow = -Math.sign(ex - s.x || 1) * r.range(0.12, 0.24), sp = curve(s.x, s.y, ex, ey, bow, 14), W0 = r.range(5.5, 7.5) * S;
    const pts = sp.map(([x, y], i) => ({ x, y, w: W0 * (i < 2 ? 0.4 + 0.3 * i : 1 - 0.75 * (i / 14) ** 1.3) }));
    // A soft cone of lilac, violet toward the tip; flecked with the tiny flowers' violet eyes; its
    // stamens standing out past the edge as a fur; and laid over it in white, thinning to the tip,
    // so the eyes show through and the buds at the tip are left violet.
    body(wet, pts, 1.1, along(wet, pts, [[0, [214, 200, 224], 0.35 * f], [0.55, [176, 128, 198], 0.5 * f], [1, [112, 58, 128], 0.8 * f]]));
    body(top, pts, 0.95, along(top, pts, [[0, [255, 255, 255], 0.72 * f], [0.5, [255, 246, 252], 0.5 * f], [0.82, [255, 240, 250], 0]]));
    const n = Math.round(s.L / (1.2 * S));
    for (let i = 0; i < n; i++) {
      const t = 0.05 + 0.93 * (i / n), j = Math.min(13, Math.floor(t * 14)), [x0, y0] = sp[j], [x1, y1] = sp[j + 1];
      const a = Math.atan2(y1 - y0, x1 - x0), u = r.range(-1, 1), wd = W0 * (1 - 0.75 * t), nx = -Math.sin(a), ny = Math.cos(a);
      const x = x0 + nx * u * wd * 0.8, y = y0 + ny * u * wd * 0.8;
      eyes.push([x, y, (0.75 - 0.25 * t) * S]);
      if (t < 0.8 && Math.abs(u) > 0.55 && r() < 0.5) florets[0].push([x + nx * Math.sign(u) * wd * 0.3, y + ny * Math.sign(u) * wd * 0.3, (1.5 - 0.6 * t) * S]);
      if (t < 0.85 && r() < 0.55) {
        const sd = r() < 0.5 ? -1 : 1, o = wd * r.range(1, 1.3) + r.range(1.5, 4) * S, ax = x0 + nx * sd * o + r.range(-1.5, 1.5) * S, ay = y0 + ny * sd * o + r.range(-1.5, 1.5) * S;
        fuzz.push([[x0 + nx * sd * wd * 0.6, y0 + ny * sd * wd * 0.6], [ax, ay]]);
        anth.push([ax, ay, 0.5 * S]);
      }
    }
    tips.push([ex, ey, 1.5 * S]);
  }
  lines(pen.dry, fuzz, 0.45 * S, [150, 104, 172], 0.6 * f);
  dots(pen.dry, anth, [96, 52, 112], 0.75 * f);
  dots(pen.dry, eyes, [118, 62, 138], 0.75 * f);
  dots(top, florets[0], [255, 250, 252], 0.75 * f);
  dots(pen.dry, tips, [78, 36, 90], 0.8 * f);
}

// Tūrutu: a clump at the foot, its strap leaves each swept in one stroke as orchid leaves are (thin
// where the brush enters, swelling, pinched where the leaf turns, and drawn out to a point), the
// ones behind in pale wet ink; above them wiry stalks branching into sprays of glossy blue berries.
function paintTurutu(pen, at) {
  const { wet, dry, top, f, r, S, box } = pen;
  const n = r.int(7, 11), x0 = at.x, y0 = at.y;
  const reach = Math.min(at.len, (pen.h - Math.max(0, Math.min(pen.h * 0.5, box.y1))) * 1.2 + 0.25 * pen.h);
  const leaves = [];
  for (let i = 0; i < n; i++) {
    const lean = (i / Math.max(1, n - 1) - 0.5) * 1.3 + r.range(-0.15, 0.15);
    let a = at.a + lean, x = x0 + r.range(-10, 10) * S, y = y0;
    const L = reach * r.range(0.45, 0.95), arch = ((lean >= 0 ? 1 : -1) * r.range(0.6, 1.8)) / L, W = r.range(4, 6.5) * S, pinch = r.range(0.35, 0.6);
    const pts = [], ds = 3 * Math.max(0.5, S);
    for (let s = 0; s < L; s += ds) {
      const t = s / L;
      let wd = W * Math.min(1, t / 0.06) * (1 - 0.72 * Math.exp(-(((t - pinch) / 0.045) ** 2)));
      if (t > 0.6) wd *= Math.max(0.05, ((1 - t) / 0.4) ** 0.9);
      pts.push({ x, y, w: wd });
      a += arch * ds * (0.4 + t);
      const ahead = outside(x + Math.cos(a) * 40 * S, y + Math.sin(a) * 40 * S, box);
      if (ahead < 6 * S) a += (Math.cos(a) > 0 ? 1 : -1) * 0.12;
      x += Math.cos(a) * ds; y += Math.sin(a) * ds;
      if (outside(x, y, box) < 2 * S) break;
    }
    leaves.push({ pts, back: r() < 0.4 });
  }
  for (const lf of leaves.sort((a, b) => b.back - a.back)) {
    if (lf.back) { body(wet, lf.pts, 1.1, along(wet, lf.pts, [[0, [54, 70, 58], 0.34 * f], [1, [54, 70, 58], 0.18 * f]])); bone(dry, lf.pts, { ink: [36, 48, 38], a: 0.3 * f, dry: 0.5, r }); }
    else bone(dry, lf.pts, { ink: [30, 42, 32], a: 0.92 * f, dry: 0.22, r, end: 0.55 });
  }
  // The stalks and their sprays.
  const stalks = [], twigs = [], berries = [[], [], []], hi = [], calyx = [];
  const ns = r.int(2, 4);
  for (let kk = 0; kk < ns; kk++) {
    let a = at.a + r.range(-0.35, 0.35), x = x0 + r.range(-8, 8) * S, y = y0;
    const L = reach * r.range(0.7, 1.05), bend = r.range(-0.5, 0.5) / L, pts = [[x, y]], ds = 4 * Math.max(0.5, S);
    for (let s = 0; s < L; s += ds) {
      a += bend * ds;
      if (outside(x + Math.cos(a) * 50 * S, y + Math.sin(a) * 50 * S, box) < 10 * S) a += (Math.cos(a) > 0 ? 1 : -1) * 0.1;
      // and back into the page from its edges, where the berries would be lost.
      if (x < 40 * S) a = turn(a, -Math.PI / 2 + 0.6, 0.08); else if (x > pen.w - 40 * S) a = turn(a, -Math.PI / 2 - 0.6, 0.08);
      x += Math.cos(a) * ds; y += Math.sin(a) * ds; pts.push([x, y]);
      if (outside(x, y, box) < 14 * S) break;
    }
    stalks.push(pts);
    const tp = pts.slice(Math.floor(pts.length * 0.55)), nb = r.int(3, 6);
    for (let j = 0; j < nb; j++) {
      const [bx, by] = tp[Math.floor((j / nb) * tp.length)], sd = j % 2 ? 1 : -1;
      const ba = a + sd * r.range(0.5, 1.1), bl = r.range(14, 36) * S, ex = bx + Math.cos(ba) * bl, ey = by + Math.sin(ba) * bl + 6 * S;
      const tw = curve(bx, by, ex, ey, sd * 0.15, 5);
      twigs.push(tw);
      const nbr = r.int(3, 7);
      for (let mm = 0; mm < nbr; mm++) {
        const [px, py] = tw[r.int(2, 5)], pa = r.range(0, TAU), pl = r.range(3, 9) * S, R = r.range(3.2, 5) * S;
        const cx = px + Math.cos(pa) * pl, cy = py + Math.sin(pa) * pl + 2 * S;
        if (!clear(pen, cx, cy, R)) continue;
        twigs.push([[px, py], [cx, cy]]);
        const ripe = r();
        berries[ripe < 0.7 ? 0 : ripe < 0.86 ? 1 : 2].push([cx, cy, R]);
        hi.push([cx - R * 0.35, cy - R * 0.38, R * 0.26, R * 0.2, -0.6]);
        calyx.push([cx + Math.cos(pa + Math.PI) * R * 0.85, cy + Math.sin(pa + Math.PI) * R * 0.85, 0.7 * S]);
      }
    }
  }
  lines(dry, stalks, 1.25 * S, [66, 50, 58], 0.85 * f);
  lines(dry, twigs, 0.75 * S, [74, 56, 66], 0.8 * f);
  dots(wet, berries[0], [54, 66, 168], 0.92 * f);
  dots(wet, berries[1], [70, 50, 138], 0.92 * f);
  dots(wet, berries[2], [196, 206, 172], 0.85 * f);
  dots(top, hi, [255, 255, 255], 0.75 * f);
  dots(dry, calyx, [40, 34, 40], 0.8 * f);
}
