// Kauri: inside the Waipoua forest, in lit mist. Great kauri trunks stand at every distance: the near
// ones dark and close, their hammered bark catching the light along one edge; the further ones
// narrower and paler, until they are ghosts in the mist behind the lead. Light comes through the
// canopy in shafts, high from one side, straight down through a gap, or low across the trunks; the
// canopy's leaves hang dark along the top; the undergrowth lies in low banks, and the floor is mist.
//
// Who the agent is: its place `at`, planned by the harness, picks the light on a ring of the forest's
// hours: amber evening, dawn gold, green noon, silver rain, blue mist, and back. Agents on screen
// together stand in clearly different light. identity(at) is that light, for the map. The seed picks
// the rest, each part from its own stream: the arrangement (a colonnade, one giant close by, a
// clearing, or a grove looking up), the light's direction, the trunks, the undergrowth and the ferns.
//
// Technique: one fragment shader paints, far to near: the mist (drifting banks, the light's glow, the
// canopy's leaves), far undergrowth, the trunks (each a cylinder leaning to a vanishing point far
// above, with hammered bark: a smooth height of shallow dishes whose slopes catch the light, and soft
// patches of paler fallen flakes, paling toward the mist by
// its distance), nearer undergrowth, the shafts and the floor mist. The near ferns are 2D, drawn
// small and blurred as foliage close to the eye is, each leaflet lit by how much light reaches it.
// Then grain. The lead sits on the mist: near trunks and leaves keep out of its box, and what passes
// behind it is far enough off to be pale.
// No rings and no contour lines, so it never looks like Harbour.

import { oklch, fromOklch } from "./theme.js";

const MAX_TRUNKS = 18;

const FOREST = `precision highp float;
uniform vec2 u_res; uniform float u_dpr; uniform vec2 u_size; uniform vec4 u_quiet; uniform float u_seed;
uniform vec3 u_light; uniform vec3 u_shade; uniform vec3 u_haze; uniform vec3 u_bark;
uniform vec4 u_src; uniform vec3 u_beams; uniform float u_floor; uniform vec2 u_vp; uniform float u_calm;
uniform float u_n; uniform vec4 u_t[${MAX_TRUNKS}];
uniform vec4 u_canopy;

float h12(vec2 p){ vec3 p3 = fract(vec3(p.xyx) * .1031); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.x + p3.y) * p3.z); }
vec2 h22(vec2 p){ vec3 p3 = fract(vec3(p.xyx) * vec3(.1031, .1030, .0973)); p3 += dot(p3, p3.yzx + 33.33); return fract((p3.xx + p3.yz) * p3.zy); }
float vn(vec2 p){ vec2 i = floor(p), f = fract(p); vec2 u = f * f * (3. - 2. * f);
  return mix(mix(h12(i), h12(i + vec2(1, 0)), u.x), mix(h12(i + vec2(0, 1)), h12(i + vec2(1, 1)), u.x), u.y); }
float fbm(vec2 p){ float s = 0., a = .5; for (int i = 0; i < 4; i++) { s += a * vn(p); p = p * 2.03 + vec2(17.1, 9.2); a *= .5; } return s / .9375; }
float lum(vec3 c){ return dot(c, vec3(.3, .59, .11)); }

// The light's glow in the mist, round where it comes in.
float glow(vec2 px, float U){ float d = length(px - u_src.xy) / (U * 1000.); return .6 * exp(-d * 1.3) + .3 * exp(-d * 4.); }

// Shafts: near-parallel beams from the light, broken into streaks, fading as they fall. x is the
// beam, y how far the light's cone reaches here (the air between the beams is in their shadow).
vec2 beams(vec2 px, float U, float m){
  vec2 dir = u_src.zw, d = px - u_src.xy;
  float along = dot(d, dir) / (U * 1000.), across = dot(d, vec2(-dir.y, dir.x)) / (U * 1000.);
  if (along < 0. || u_beams.y <= 0.) return vec2(0.);
  float b = .65 * vn(vec2(across * u_beams.x + u_beams.z, 1.3)) + .35 * vn(vec2(across * u_beams.x * 2.7 - u_beams.z, 7.1));
  b = smoothstep(.5, .85, b);
  float cone = exp(-along * .75) * smoothstep(0., .25, along) * smoothstep(1.4, .3, abs(across) / (along + .4));
  return vec2(u_beams.y * b * (.5 + .9 * m) * cone, cone);
}

// The air: lit mist in drifting banks, brighter toward the light, deepening a little low in the
// forest, and the canopy's leaves along the top. calm is 1 in the lead's box.
vec3 air(vec2 px, vec2 uv, float U, float calm, out float m){
  vec2 q = px / (U * 1000.);
  m = fbm(q * vec2(1.1, 4.2) + vec2(u_seed, 0.) + vec2(1.3 * fbm(q * vec2(.5, 1.6) + 3.1), 0.));
  vec3 c = u_haze * (1. + .16 * (m - .5) * (1. - .6 * calm));
  c += u_light * glow(px, U) * .32;
  c *= 1. - .24 * smoothstep(.35, .88, uv.y) * (1. - calm);
  // The canopy (u_canopy: its lowest reach, how full it is, its slope across, how near): clusters
  // of leaves along the top, a far layer pale in the mist and a nearer one darker, thinning where
  // the light breaks through.
  float slope = 1. + u_canopy.z * (uv.x - .5) * 2.;
  if (u_canopy.x <= 0. || px.y > u_canopy.x * 1.8 * max(slope, .2) || calm > .99) return c;
  float edge = u_canopy.x * max(slope, .05) * (.45 + .9 * fbm(vec2(px.x / (U * 260.), u_seed)));
  float open = 1. - .75 * clamp(glow(px, U) * 1.6, 0., 1.);
  float farL = fbm(px / (U * 40.) + u_seed + 3.) * .6 + fbm(px / (U * 130.) - u_seed) * .55;
  float farC = smoothstep(edge * 1.25, edge * .5, px.y) * smoothstep(.46 - .1 * u_canopy.y, .7 - .1 * u_canopy.y, farL);
  c = mix(c, mix(c, u_shade * 2.5, .45), farC * open * (1. - calm));
  float nearL = fbm(px / (U * 26.) + u_seed) * .42 + fbm(px / (U * 95.) - u_seed + 7.) * .72;
  float nearC = smoothstep(edge * .85, edge * .25, px.y) * smoothstep(.52 - .12 * u_canopy.y, .74 - .12 * u_canopy.y, nearL);
  c = mix(c, mix(u_shade * 1.8, c, .3 + .25 * (1. - u_canopy.w)), nearC * open * (1. - calm) * .85);
  return c;
}

// A trunk: a cylinder leaning a little toward the vanishing point far above. T: foot x, width,
// distance 0..1, seed.
vec4 trunk(vec2 px, vec4 T, float U, float calm, vec3 behind, float m, float bm){
  float H = u_size.y, yy = px.y / H;
  float t = (H - px.y) / (H - u_vp.y);
  float cx0 = T.x + (u_vp.x - T.x) * t;
  if (abs(px.x - cx0) > T.y * .75 + 4.) return vec4(0.);
  float cx = cx0 + (vn(vec2(px.y / (420. * U), T.w * 7.)) - .5) * T.y * .05;
  float hw = T.y * .5 * (1. - .1 * (1. - yy)) * (1. + .04 * (vn(vec2(px.y / (50. * U), T.w * 3.)) - .5));
  hw *= 1. + .22 * pow(smoothstep(.8, 1.08, yy), 2.) * (1. - T.z);
  if (abs(px.x - cx) > hw * 1.3 + 3.) return vec4(0.);
  float u = (px.x - cx) / hw;
  float soft = (1. + T.z * T.z * 10. * u_dpr) / (hw * u_dpr);
  float a = 1. - smoothstep(1. - soft, 1., abs(u));
  if (a <= 0.) return vec4(0.);
  u = clamp(u, -1., 1.);
  float nz = sqrt(max(0., 1. - u * u));
  // Which side the light falls on: smooth, so a trunk standing under the light shows no seam where
  // the side would flip.
  float side = clamp((u_src.x - cx) / (hw * 3.), -1., 1.);
  // Bark: soft irregular flakes wrapped round the trunk, some fallen to a paler, bluer grey; each
  // flake a shallow dish whose rim catches the light.
  vec3 n = vec3(u, 0., nz), alb = u_bark;
  if (T.z < .62) {
    float s = asin(u) * hw;
    float cell = clamp(hw * .22, 5. * U, 34. * U);
    vec2 bp = vec2(s / cell, px.y / (cell * 1.3)) + T.w * 31.;
    float detail = clamp(cell * u_dpr / 9. - .3, 0., 1.) * (1. - T.z);
    // Hammered: shallow overlapping dishes, a smooth height whose slope tilts the surface.
    float e = .18;
    float h0 = vn(bp) * .7 + vn(bp * 2.3 + 5.) * .3;
    float hx = vn(bp + vec2(e, 0.)) * .7 + vn((bp + vec2(e, 0.)) * 2.3 + 5.) * .3;
    float hy = vn(bp + vec2(0., e)) * .7 + vn((bp + vec2(0., e)) * 2.3 + 5.) * .3;
    vec2 gr = vec2(hx - h0, hy - h0) / e;
    n = normalize(n - .22 * detail * (gr.x * vec3(nz, 0., -u) + gr.y * vec3(0., 1., 0.)));
    // Tone: flakes fallen to a paler, bluer grey in soft patches, a little darker in the hollows.
    float fall = smoothstep(.48, .7, fbm(vec2(s, px.y) / (cell * 2.4) + T.w * 13.));
    alb = u_bark * (.92 + .16 * h0 * detail);
    alb = mix(alb, u_bark * vec3(1.3, 1.33, 1.4), fall * .8);
    if (yy > .45) {
      float moss = smoothstep(.5, .8, fbm(px / (U * 60.) - T.w * 3.)) * smoothstep(.45, 1., yy);
      alb = mix(alb, vec3(.22, .32, .15), moss * .6);
    }
  }
  // Light: the mist's soft fill, the key from the light's side, a shaft where one falls across it.
  // Both a little greyed on the bark, so warm light leaves it grey and not the brown of a pine.
  vec3 Ld = normalize(vec3(side, -.25, .55));
  float dif = max(0., dot(n, Ld));
  vec3 fill = mix(u_haze, vec3(lum(u_haze)), .6), key = mix(u_light, vec3(lum(u_light)), .5);
  vec3 col = alb * (fill * (.17 + .14 * nz) + key * (dif * .5 + bm * 1.1 * nz));
  col += key * pow(clamp(u * side, 0., 1.), 6.) * .16;
  // Far off, a trunk keeps its shape and loses its bark, paling toward the mist behind it; in the
  // lead's calm it pales further, to a ghost.
  float fog = pow(T.z, .95) * (.92 + .2 * (m - .5)) + .62 * calm * smoothstep(.15, .5, T.z);
  fog += .55 * smoothstep(u_floor, 1.04, yy);
  vec3 far = behind * (.74 + .16 * T.z + .12 * calm) * (.9 + .1 * nz);
  return vec4(mix(col, far, clamp(fog, 0., 1.)), a);
}

void main(){
  vec2 px = vec2(gl_FragCoord.x, u_res.y - gl_FragCoord.y) / u_dpr, uv = px / u_size;
  float U = sqrt(u_size.x * u_size.y) / 1000.;
  // The lead's calm: a rounded pool round its box, so no edge of the box shows.
  vec2 qc = u_quiet.xy + u_quiet.zw * .5, qh = u_quiet.zw * .5 + 24. * U;
  vec2 qd = abs(px - qc) / qh;
  float sd = pow(pow(qd.x, 4.) + pow(qd.y, 4.), .25);
  float calm = 1. - smoothstep(1.05, 1.05 + u_calm / min(qh.x, qh.y), sd);
  float m;
  vec3 airc = air(px, uv, U, calm, m);
  vec2 sh = beams(px, U, m);
  float bm = sh.x * (1. - .8 * calm);
  // Between the shafts the air is in their shadow: a little deeper, so the shafts show as light.
  airc *= 1. - .13 * sh.y * (1. - smoothstep(0., .25, sh.x)) * smoothstep(.15, .55, uv.y) * (1. - calm);
  vec3 col = airc;
  float front = 1.;
  for (int i = 0; i < ${MAX_TRUNKS}; i++) {
    if (float(i) >= u_n) break;
    vec4 T = u_t[i];
    vec4 c = trunk(px, T, U, calm, airc, m, bm);
    if (c.a > 0.) { col = mix(col, c.rgb, c.a); front = mix(front, T.z, c.a); }
  }
  // The shafts hang in the air in front, most of all in the distance; then the floor mist.
  col += u_light * bm * (.16 + .42 * front);
  float fl = 0.;
  if (uv.y > u_floor) fl = clamp(smoothstep(u_floor, 1.05, uv.y) * (.75 + .5 * fbm(px / (U * 1000.) * vec2(2.2, 9.) + u_seed + 5.)), 0., 1.);
  col = mix(col, mix(u_haze, u_light, .2) * 1.05, fl * fl * (3. - 2. * fl) * (.5 + .5 * front));
  gl_FragColor = vec4(col, 1.);
}`;

// The ring of the forest's light, in order round it: each hour's light, its mist, the canopy's
// shade and the bark.
const HOURS = [
  { name: "amber evening", light: [1.0, 0.72, 0.42], haze: [0.87, 0.77, 0.64], shade: [0.07, 0.08, 0.05], bark: [0.41, 0.39, 0.38] },
  { name: "dawn gold", light: [1.0, 0.88, 0.6], haze: [0.86, 0.84, 0.69], shade: [0.06, 0.09, 0.06], bark: [0.42, 0.41, 0.38] },
  { name: "green noon", light: [0.86, 0.98, 0.6], haze: [0.75, 0.83, 0.67], shade: [0.04, 0.09, 0.05], bark: [0.39, 0.41, 0.37] },
  { name: "silver rain", light: [0.94, 0.97, 0.98], haze: [0.79, 0.82, 0.82], shade: [0.06, 0.08, 0.08], bark: [0.41, 0.42, 0.43] },
  { name: "blue mist", light: [0.75, 0.88, 1.0], haze: [0.7, 0.79, 0.85], shade: [0.04, 0.07, 0.09], bark: [0.38, 0.4, 0.44] },
];
function hourAt(t) {
  const k = (((t % 1) + 1) % 1) * HOURS.length, i = Math.floor(k), f = k - i, a = HOURS[i], b = HOURS[(i + 1) % HOURS.length];
  const e = f * f * (3 - 2 * f);
  const lerp = (x, y) => x.map((v, j) => v + (y[j] - v) * e);
  return { light: lerp(a.light, b.light), haze: lerp(a.haze, b.haze), shade: lerp(a.shade, b.shade), bark: lerp(a.bark, b.bark) };
}
const hex = (c) => "#" + c.map((v) => Math.round(Math.min(1, Math.max(0, v)) * 255).toString(16).padStart(2, "0")).join("");

// The agent's own colour, for its node on the map: its light, deep enough to read as a colour.
function identity(at) {
  const [, C, H] = oklch(hex(hourAt(at).light));
  return fromOklch(0.72, Math.max(C * 1.3, 0.07), H);
}

export default {
  name: "kauri",
  about: "Inside the kauri forest: great grey trunks at every distance in lit mist, light falling through the canopy.",
  // Bold at the face's heaviest: at 600, the weight every theme was set in, Cormorant's bold read
  // lighter than the other themes'.
  display: { family: "Cormorant Garamond", files: ["font-cormorant-garamond.woff2"], weights: "500 700", bold: 700, axes: null },
  palette: {
    paper: "#f6f1e4", tint: "#fbf8f0", line: "#e3ddcb",
    ink: "#18261b", body: "#33402f", muted: "#7a826d",
    accent: "#9b7a2f", figure: ["#3f6b45", "#b5782a", "#5d7a8c", "#7a5a48"], cover_ink: "#18261b", cover_body: "#33402f",
  },
  // The five hours of light, each next as far round the ring from those before as it allows.
  slots: [0, 2 / 5, 4 / 5, 1 / 5, 3 / 5],
  map: { hue: 152 },
  chart,
  identity,

  paint(ctx, { w, h, dpr, at, stream, quiet, kit }) {
    const U = Math.sqrt(w * h) / 1000;
    const q = quiet || { x: w * 0.25, y: h * 0.2, w: w * 0.5, h: h * 0.3 };
    const wide = q.w > w * 0.7;
    const hour = hourAt(at ?? 0);
    const lay = stream("layout"), lit = stream("light");
    const layout = lay.pick(["colonnade", "colonnade", "colonnade", "giant", "giant", "clearing", "clearing", "grove", "grove"]);
    const giantSide = lay() < 0.5 ? -1 : 1;
    // The light: high on one side, straight down through a gap, or low from a side.
    const mode = lit.pick(["high", "high", "high", "overhead", "low", "low"]);
    const side = layout === "giant" ? -giantSide : lit() < 0.5 ? -1 : 1;
    let src, ang;
    if (mode === "high") { src = [w / 2 + side * w * lit.range(0.35, 0.6), -h * lit.range(0.08, 0.3)]; ang = Math.PI / 2 + side * lit.range(0.3, 0.6); }
    else if (mode === "overhead") { src = [w * lit.range(0.3, 0.7), -h * lit.range(0.15, 0.3)]; ang = Math.PI / 2 + lit.range(-0.15, 0.15); }
    else { src = [w / 2 + side * w * lit.range(0.55, 0.7), h * lit.range(0.12, 0.38)]; ang = Math.PI / 2 + side * lit.range(0.85, 1.15); }
    const below = q.y + q.h;
    const vp = [w * lay.range(0.38, 0.62), layout === "grove" ? -h * lay.range(1.4, 2.2) : -h * lay.range(5, 9)];
    const floor = Math.max(0.8, below / h + 0.18);
    const calmPx = (wide ? 170 : 90) * U + 30;
    const trunks = placeTrunks({ w, h, U, q, r: stream("trunks"), vp, layout, giantSide, wide, fx: src[0] });
    // The canopy: sometimes open sky through the mist, sometimes full; heavier to one side or level.
    const cr = stream("canopy"), crown = cr.pick(["open", "thin", "full", "full", "side", "side"]);
    const canopy = crown === "open" ? [0, 0, 0, 0]
      : [Math.min(q.y * 0.85, h * cr.range(0.12, 0.2)), crown === "thin" ? -0.6 : crown === "full" ? cr.range(0.4, 1) : cr.range(0, 0.6),
        crown === "side" ? cr.pick([-1, 1]) * cr.range(0.5, 0.9) : cr.range(-0.2, 0.2), cr()];

    // Shafts only where the light comes through leaves; a low light or a clear sky gives a glow.
    const beamy = mode !== "low" && crown !== "open" ? lit.range(0.6, 0.9) : mode === "low" ? lit.range(0.3, 0.5) : 0;
    const uni = {
      u_dpr: 1, u_size: [w, h], u_quiet: [q.x, q.y, q.w, q.h], u_seed: stream("mist")() * 100,
      u_light: hour.light, u_shade: hour.shade, u_haze: hour.haze, u_bark: hour.bark,
      u_src: [src[0], src[1], Math.cos(ang), Math.sin(ang)], u_beams: [lit.range(2.4, 4), beamy, lit() * 50],
      u_floor: floor, u_vp: vp, u_calm: calmPx, u_n: trunks.length, u_canopy: canopy,
    };
    trunks.forEach((t, i) => (uni[`u_t[${i}]`] = t));
    // The mist is soft, so the shader need not run at full device resolution: half a megapixel keeps
    // the bark's hammering, and a phone's paint under the design's budget (it ran 1.1 to 1.3 s at a
    // megapixel, on the check's CPU).
    const k = Math.min(dpr, Math.sqrt(0.5e6 / (w * h)));
    uni.u_dpr = k;
    const g = kit.shader(FOREST, w * k, h * k, uni);
    if (g) ctx.drawImage(g, 0, 0, w, h);
    else {
      const grad = ctx.createLinearGradient(0, 0, 0, h);
      grad.addColorStop(0, hex(hour.haze)); grad.addColorStop(1, hex(hour.haze.map((v) => v * 0.85)));
      ctx.fillStyle = grad; ctx.fillRect(0, 0, w, h);
    }
    drawUndergrowth(ctx, { w, h, U, r: stream("undergrowth"), q, floor, hour, focus: src, trunks, vp });
    drawFerns(ctx, { w, h, U, r: stream("ferns"), q, layout, giantSide, focus: src, floor, hour });
    kit.grain(ctx, w, h, 0.05, `kauri-grain-${uni.u_seed}`);
  },
};

// Where the trunks stand, far to near: [x at the foot, width, distance 0..1, seed]. The further, the
// narrower and the more of them. Near trunks stand clear of the lead's box; where the box spans the
// width, they show only an edge, in its margin.
function placeTrunks({ w, h, U, q, r, vp, layout, giantSide, wide, fx }) {
  const at = (x, y) => x + (vp[0] - x) * ((h - y) / (h - vp[1]));
  // Clear of the box at its top and its foot, since the trunks lean.
  const clear = (x, wd, margin) => [q.y, q.y + q.h].every((y) => { const c = at(x, y); return c + wd / 2 < q.x - margin || c - wd / 2 > q.x + q.w + margin; });
  const span = Math.min(1.4, w / (U * 1000));
  const out = [];
  const add = (x, wd, z) => out.push([x, wd, z, r() * 50]);
  const nFar = Math.round(span * r.range(6, 9) * (layout === "clearing" ? 0.8 : 1));
  for (let i = 0; i < nFar; i++) { const z = r.range(0.72, 0.94); add(r.range(-0.05, 1.05) * w, U * r.range(8, 30) * (1.5 - z), z); }
  const nMid = Math.max(layout === "clearing" ? 4 : 0, Math.round(span * r.range(2.5, 4.5) * (layout === "grove" ? 1.3 : 1) * (wide ? 1.6 : 1)));
  for (let i = 0, tries = 0; i < nMid && tries < 60; tries++) {
    const z = r.range(0.45, 0.68), wd = U * r.range(26, 70) * (1.4 - z), x = r.range(-0.05, 1.05) * w;
    if (!wide && !clear(x, wd, 0) && r() < 0.6) continue;
    add(x, wd, z); i++;
  }
  const nNearMid = Math.max(1, Math.round(span * r.range(1, 2.2) * (layout === "clearing" ? 0.6 : layout === "grove" ? 1.4 : 1)));
  for (let i = 0, tries = 0; i < nNearMid && tries < 60; tries++) {
    const wd = U * r.range(72, 140), x = r.range(-0.05, 1.05) * w;
    if (!clear(x, wd, 10 * U)) continue;
    add(x, wd, r.range(0.22, 0.38)); i++;
  }
  // A near trunk on edge e (-1 left, 1 right), pushed out until it clears the box; where the box
  // spans the width there may be room for only a sliver at the edge, or none.
  const near = (e, wd, z, inset) => {
    let x = e < 0 ? inset * w : (1 - inset) * w;
    for (let k = 0; k < 80 && !clear(x, wd, 12 * U); k++) x += e * wd * 0.05;
    const inner = e < 0 ? x + wd / 2 : w - (x - wd / 2);
    if (inner >= 18) add(x, wd, z);
  };
  if (layout === "colonnade") {
    const e = r() < 0.5 ? -1 : 1;
    near(e, U * r.range(200, 290), r.range(0.03, 0.08), r.range(0.02, 0.1));
    near(-e, U * r.range(120, 180), r.range(0.12, 0.2), r.range(0.04, 0.14));
  } else if (layout === "giant") {
    near(giantSide, U * r.range(430, 560), r.range(0.01, 0.04), r.range(-0.04, 0.08));
    if (r() < 0.5) near(-giantSide, U * r.range(90, 140), r.range(0.15, 0.24), r.range(0.0, 0.06));
  } else if (layout === "grove") {
    near(-1, U * r.range(170, 250), r.range(0.04, 0.1), r.range(0.04, 0.12));
    near(1, U * r.range(150, 230), r.range(0.06, 0.12), r.range(0.04, 0.12));
  } else {
    near(r() < 0.5 ? -1 : 1, U * r.range(150, 220), r.range(0.06, 0.14), r.range(0.0, 0.06));
  }
  out.sort((a, b) => b[2] - a[2]);
  return out.slice(-MAX_TRUNKS);
}

const mixRGB = (a, b, t) => a.map((x, i) => x + (b[i] - x) * t);
const css = (c, a = 1) => `rgba(${c.map((v) => Math.round(Math.min(1, Math.max(0, v)) * 255)).join(",")},${a})`;

// The undergrowth: clumps of ground fern and young growth in two rows across the forest floor, the
// further smaller, paler and softer, with mist lying between the rows, and gaps where the floor
// shows. A clump nearer the light is greener.
function drawUndergrowth(ctx, { w, h, U, r, q, floor, hour, focus, trunks, vp }) {
  const S = Math.max(w, h) * 0.32, span = Math.min(1.4, w / (U * 1000));
  const diag = Math.hypot(w, h);
  const litness = (x, y) => Math.max(0, 1 - Math.hypot(x - focus[0], y - focus[1]) / (diag * 0.9));
  const deep = [0.12, 0.2, 0.1], lit = mixRGB([0.45, 0.6, 0.3], hour.light, 0.35);
  const fy = floor * h;
  const rows = [
    { y: fy - h * r.range(0.04, 0.07), haze: 0.62, size: 0.32, res: 0.35, blur: 1.6, n: Math.round(span * r.range(5, 9)), z: 0.7 },
    { y: fy + h * r.range(0.01, 0.04), haze: 0.4, size: 0.5, res: 0.4, blur: 1.1, n: Math.round(span * r.range(3, 6)), z: 0.42 },
  ];
  // A clump can't stand in front of a trunk nearer than its row.
  const behindTrunk = (x, y, z) => trunks.some(([tx, tw, tz]) => tz < z && Math.abs(x - (tx + (vp[0] - tx) * ((h - y) / (h - vp[1])))) < tw * 0.5);
  for (const row of rows) {
    const L = layer(w, h, row.res);
    // Clumps in groups, with the misty floor showing between the groups.
    const groups = Math.max(1, Math.round(row.n / 3));
    for (let gi = 0; gi < groups; gi++) {
      const gx = r.range(-0.05, 1.05) * w, members = r.int(2, 4), size0 = S * row.size;
      const gy = row.y + r.range(-0.012, 0.02) * h, ext = size0 * members * 0.45;
      const tone = (x, y, s) => mixRGB(mixRGB(deep, lit, Math.min(1, 0.2 + 0.9 * litness(x, y - s * 0.3))), hour.haze, row.haze + r.range(-0.05, 0.05));
      // The ground the clumps stand in: a low soft mound.
      if (behindTrunk(gx, gy, row.z)) continue;
      const mg = L.ctx.createRadialGradient(gx, gy + size0 * 0.1, 0, gx, gy + size0 * 0.1, ext);
      const mc = tone(gx, gy, size0);
      mg.addColorStop(0, css(mc, 0.85)); mg.addColorStop(0.6, css(mc, 0.4)); mg.addColorStop(1, css(mc, 0));
      L.ctx.save(); L.ctx.translate(gx, gy + size0 * 0.1); L.ctx.scale(1, 0.22); L.ctx.translate(-gx, -(gy + size0 * 0.1));
      L.ctx.fillStyle = mg; L.ctx.fillRect(gx - ext, gy + size0 * 0.1 - ext, ext * 2, ext * 2); L.ctx.restore();
      for (let ci = 0; ci < members; ci++) {
        const x = gx + (ci - (members - 1) / 2) * size0 * r.range(0.35, 0.6), y = gy + r.range(-0.01, 0.01) * h;
        const size = size0 * r.range(0.55, 1.15), n = r.int(6, 10);
        if (behindTrunk(x, y, row.z)) continue;
        const col = css(tone(x, y, size), 1);
        for (let k = 0; k < n; k++) {
          const lean = (k / (n - 1) - 0.5) * 2.6 + r.range(-0.25, 0.25);
          const f = { x: x + r.range(-0.15, 0.15) * size, y, head: -Math.PI / 2 + lean * 0.55, len: size * r.range(0.45, 1) * (1 - 0.2 * Math.abs(lean)), bend: lean > 0 ? 1 : -1, curl: r.range(1.6, 2.4) };
          for (let t = 0; t < 6 && reachesBox(f, q); t++) f.len *= 0.75;
          L.ctx.fillStyle = L.ctx.strokeStyle = col;
          frondFill(L.ctx, f, r, 16);
        }
      }
    }
    blit(ctx, L, w, h, row.blur);
    // The mist lying in front of this row, thickening toward the floor.
    const g = ctx.createLinearGradient(0, row.y - h * 0.06, 0, h);
    g.addColorStop(0, css(hour.haze, 0)); g.addColorStop(0.35, css(hour.haze, 0.22)); g.addColorStop(1, css(hour.haze, 0.4));
    ctx.fillStyle = g; ctx.fillRect(0, row.y - h * 0.06, w, h);
  }
}

// A frond in one colour, all its leaflets in one path: for the undergrowth, where there are many.
function frondFill(c, f, r, n) {
  const pts = spine(f, () => r.range(-0.006, 0.006), n);
  c.lineWidth = Math.max(0.8, f.len * 0.012);
  c.beginPath(); c.moveTo(pts[0][0], pts[0][1]);
  for (const [x, y] of pts) c.lineTo(x, y);
  c.stroke();
  const p = new Path2D();
  for (let i = 1; i < n; i++) {
    const t = i / n, [x0, y0, a0] = pts[i];
    const pl = f.len * 0.3 * Math.sin(Math.PI * Math.min(1, 0.1 + t * 0.95)) ** 0.8 * (1 - 0.3 * t);
    for (const s of [-1, 1]) {
      const a = a0 + s * (1.0 + 0.2 * r()) - s * 0.3 * t;
      p.addPath(blade(x0, y0, a, pl * r.range(0.8, 1.05), -s * f.bend, 6));
    }
  }
  c.fill(p);
}

// The near ferns: fronds in one or two corners, drawn
// small and blurred as foliage close to the eye is, each leaflet lit by how much light reaches it.
function drawFerns(ctx, { w, h, U, r, q, layout, giantSide, focus, floor, hour }) {
  const S = Math.max(w, h) * 0.32;
  const diag = Math.hypot(w, h);
  const litness = (x, y) => Math.max(0, 1 - Math.hypot(x - focus[0], y - focus[1]) / (diag * 1.1));
  const dark = mixRGB([0.08, 0.16, 0.08], hour.shade, 0.3), green = mixRGB([0.52, 0.7, 0.32], hour.light, 0.3);

  // Near fronds: a corner or two, by the arrangement, sometimes none.
  let corners = r() < 0.2 ? [] : [r() < 0.5 ? -1 : 1];
  if (layout === "giant") corners = r() < 0.5 ? [-giantSide] : [];
  if (corners.length && r() < 0.3) corners.push(-corners[0]);
  const near = layer(w, h, 0.4);
  for (const [ci, e] of corners.entries()) {
    const big = ci === 0 ? r.range(0.8, 1.1) : r.range(0.5, 0.75);
    const bx = e < 0 ? w * r.range(-0.1, 0.08) : w * r.range(0.92, 1.1), by = h * r.range(0.94, 1.12);
    const n = r.int(3, 5);
    for (let k = 0; k < n; k++) {
      const head = e < 0 ? -r.range(0.3, 1.1) : -Math.PI + r.range(0.3, 1.1);
      const f = { x: bx + r.range(-0.04, 0.04) * w, y: by, head, len: S * big * r.range(0.75, 1.15), bend: e < 0 ? 1 : -1 };
      for (let tries = 0; tries < 8 && reachesBox(f, q); tries++) f.len *= 0.8;
      frond(near.ctx, f, r, (px, py) => {
        const l = Math.min(1, 0.15 + 1.0 * litness(px, py) ** 1.3 + r.range(-0.06, 0.1));
        return css(mixRGB(dark, green, Math.max(0, l)), 0.95);
      });
    }
  }
  blit(ctx, near, w, h, 0.9);
}

// An offscreen layer at a fraction of the size, to be blurred back up.
function layer(w, h, k) {
  const c = document.createElement("canvas");
  c.width = Math.ceil(w * k); c.height = Math.ceil(h * k);
  const x = c.getContext("2d"); x.scale(k, k);
  return { c, ctx: x };
}
function blit(ctx, { c }, w, h, blur) {
  const b = document.createElement("canvas");
  b.width = c.width; b.height = c.height;
  const bx = b.getContext("2d");
  bx.filter = `blur(${blur}px)`;
  bx.drawImage(c, 0, 0);
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(b, 0, 0, w, h);
}

// Whether a frond would reach into the lead's box.
function reachesBox(f, q) {
  return spine(f, () => 0).some(([x, y]) => x > q.x - 20 && x < q.x + q.w + 20 && y > q.y - 20 && y < q.y + q.h + 40);
}

// The rachis: it rises, then bends over along its length.
function spine({ x, y, head, len, bend, curl = 1 }, jitter, n = 30) {
  const step = len / n, pts = [];
  let a = head;
  for (let i = 0; i <= n; i++) {
    pts.push([x, y, a]);
    const t = i / n;
    a += (bend * curl * (0.004 + 0.07 * t * t) * 30) / n + jitter();
    x += Math.cos(a) * step; y += Math.sin(a) * step;
  }
  return pts;
}

// A frond: leaflets in pairs along the rachis, longest a third of the way out, each a slender blade
// curving toward the tip. colour(x, y) gives each leaflet's fill.
function frond(c, f, r, colour) {
  const pts = spine(f, () => r.range(-0.004, 0.004));
  const n = pts.length - 1;
  c.lineCap = "round";
  c.strokeStyle = colour(pts[n >> 1][0], pts[n >> 1][1]);
  c.lineWidth = Math.max(1, f.len * 0.009);
  c.beginPath(); c.moveTo(pts[0][0], pts[0][1]);
  for (const [x, y] of pts) c.lineTo(x, y);
  c.stroke();
  for (let i = 2; i < n; i++) {
    const t = i / n, [x0, y0, a0] = pts[i];
    const pl = f.len * 0.26 * Math.sin(Math.PI * Math.min(1, 0.1 + t * 0.95)) ** 0.8 * (1 - 0.3 * t);
    for (const s of [-1, 1]) {
      const len = pl * (s > 0 ? 1 : 0.72) * r.range(0.9, 1.08);
      const a = a0 + s * (1.05 + 0.15 * r()) - s * 0.25 * t;
      c.fillStyle = colour(x0 + Math.cos(a) * len * 0.5, y0 + Math.sin(a) * len * 0.5);
      c.fill(blade(x0, y0, a, len, -s * f.bend));
    }
  }
}

// A leaflet: a slender blade, widest a third of the way out, its tip curving.
function blade(x0, y0, a, len, curl, N = 10) {
  const p = new Path2D(), W = len * 0.11, L = [], R = [];
  let x = x0, y = y0, aa = a;
  for (let j = 0; j <= N; j++) {
    const t = j / N, wv = W * Math.sin(Math.PI * Math.min(1, t * 1.25 + 0.05)) ** 0.7 * (1 - t * 0.6);
    const nx = -Math.sin(aa), ny = Math.cos(aa);
    L.push([x + nx * wv, y + ny * wv]); R.push([x - nx * wv, y - ny * wv]);
    aa += (curl * 0.4) / N;
    x += (Math.cos(aa) * len) / N; y += (Math.sin(aa) * len) / N;
  }
  p.moveTo(L[0][0], L[0][1]);
  for (const [px, py] of L) p.lineTo(px, py);
  for (const [px, py] of R.reverse()) p.lineTo(px, py);
  p.closePath();
  return p;
}

// On the map: a forest survey, as a surveyor draws one. Each stand of bush outlined, its type told by
// its texture: kauri stands a close field of small round crowns, podocarp-hardwood a crown stipple,
// and mānuka scrub sparse dots on the margins inside a dotted line. The giant kauri each drawn to
// scale, their crown outlined round a numbered trunk. A track winds through on the dry ground.
function chart(ctx, s) {
  const { w, h, mass, calm, n } = s, { grid, iso, chains, within, poly, lines, length, scatter, dots, tint, ok, okRgb, ease } = s.kit;
  const INK = [0.44, 0.045, 150];
  const forest = grid(w, h, 4, (x, y) => mass(x, y) * (0.5 + 0.5 * n.fbm(x * 0.0055 + 2, y * 0.007 - 1, 4)));
  const kind = grid(w, h, 6, (x, y) => n.fbm(x * 0.0042 - 7, y * 0.0052 + 3, 3));
  const EDGE = 0.36, SCRUB = EDGE - 0.1, KAURI = 0.1;
  const isKauri = (x, y) => kind.at(x, y) > KAURI;
  // The bush in a pale sage, its kauri stands a step deeper.
  const BUSH = okRgb(0.952, 0.02, 140), KST = okRgb(0.938, 0.026, 148);
  tint(s.under, w, h, 2, (x, y) => {
    const f = forest.at(x, y);
    return f <= EDGE ? null : [...(isKauri(x, y) ? KST : BUSH), Math.round(215 * ease((f - EDGE) / 0.03))];
  });
  // The giant kauri first, so nothing else is drawn inside their crowns.
  const gr = s.stream("giants"), giants = [];
  for (const [x, y] of scatter(gr, w, h, 70, (x, y) => (forest.at(x, y) > EDGE + 0.07 && kind.at(x, y) > KAURI + 0.06 ? 0.6 * calm(x, y) : 0))) {
    const r = gr.range(7.5, 11.5), k = gr.int(10, 13), ph = gr.range(0, 6.28), p = [];
    for (let i = 0; i <= k; i++) { const a = ph + (i / k) * 6.283, q = r * (0.8 + 0.28 * n.n2(x * 0.3 + Math.cos(a), y * 0.3 + Math.sin(a))); p.push(x + Math.cos(a) * q, y + Math.sin(a) * q); }
    giants.push({ x, y, r, p });
  }
  const inGiant = (x, y) => giants.some((g) => Math.hypot(g.x - x, g.y - y) < g.r * 1.1);
  // Crowns, kept off the streams and away from the nodes: kauri stands a field of small open rings,
  // the rest of the bush a stipple of crowns.
  const rings = [], stip = [], pick = s.stream("crownkind");
  for (const [x, y] of scatter(s.stream("canopy"), w, h, 3.3, (x, y) => (forest.at(x, y) > EDGE + 0.012 ? 0.8 * calm(x, y) : 0))) {
    if (inGiant(x, y)) continue;
    const r = pick();
    if (isKauri(x, y)) { if (r < 0.7) rings.push([x, y, 0.85 + 0.5 * pick()]); }
    else stip.push([x, y, 0.28 + 0.42 * r * r]);
  }
  dots(ctx, stip, ok(...INK, 0.5));
  ctx.beginPath();
  for (const [x, y, r] of rings) { ctx.moveTo(x + r, y); ctx.arc(x, y, r, 0, 6.283); }
  ctx.strokeStyle = ok(...INK, 0.5); ctx.lineWidth = 0.3; ctx.stroke();
  // Scrub: sparse dots between the scrub line and the bush.
  dots(ctx, scatter(s.stream("scrub"), w, h, 5, (x, y) => { const f = forest.at(x, y); return f > SCRUB && f < EDGE ? 0.5 * calm(x, y) : 0; }).map(([x, y]) => [x, y, 0.32]), ok(...INK, 0.42));
  // The boundaries: the bush's edge solid, the scrub's dotted, a kauri stand's dashed within the bush.
  lines(ctx, chains(iso(forest, EDGE)), ok(...INK, 0.62), 0.5, null, 14);
  lines(ctx, chains(iso(forest, SCRUB)), ok(...INK, 0.4), 0.4, [0.35, 1.8], 24);
  lines(ctx, within(chains(iso(kind, KAURI)), (x, y) => forest.at(x, y) > EDGE + 0.03), ok(...INK, 0.45), 0.4, [1.6, 1.3], 12);
  // The giants: crown outlines round a trunk dot, each numbered as a survey of notable trees lists them.
  ctx.beginPath();
  for (const g of giants) poly(ctx, g.p);
  ctx.strokeStyle = ok(...INK, 0.66); ctx.lineWidth = 0.5; ctx.stroke();
  dots(ctx, giants.map((g) => [g.x, g.y, 0.9]), ok(...INK, 0.75));
  ctx.fillStyle = ok(...INK, 0.66); ctx.font = "4.6px Georgia, serif"; ctx.textAlign = "left"; ctx.textBaseline = "middle";
  giants.forEach((g, i) => { if (calm(g.x + g.r + 4, g.y) > 0.6) ctx.fillText(String(i + 1), g.x + g.r + 1.5, g.y - g.r * 0.45); });
  // A track along the dry ground: the longest runs of one slow field's zero line, dashed.
  const tf = grid(w, h, 5, (x, y) => n.fbm(x * 0.0021 + 21, y * 0.0016 - 4, 2));
  lines(ctx, within(chains(iso(tf, 0)), (x, y) => mass(x, y) > 0.25 && !inGiant(x, y)).filter((p) => length(p) > 120), ok(...INK, 0.62), 0.55, [2.6, 1.5]);
}
