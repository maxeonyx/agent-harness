// Marama: the moon over still water at night, and its path of broken light.
// Marama is te reo Māori for the moon; mārama is light, clear.
//
// Technique: one fragment shader pass (kit.shader) paints the sky's gradation from near-black to the
// night's colour at the horizon, sea mist, the moon (a lit sphere, so any phase from full to a thick
// crescent, with faint seas, a glow hugging the disc and a wide halo), a cloud bank on the horizon or
// broken strips of stratus lit on their moon-facing edge, a pale far range, near land (headlands,
// islands or a bay) with a hairline of light on its ridge, and the water: the sky's light mirrored, a
// slow swell whose bands crowd toward the horizon in perspective, and the land's reflection, broken
// into slivers by the ripples and fading as it nears. Every edge is antialiased in the shader. Canvas
// strokes then lay ripples across the water and the glade: glints as tapered ellipses gathered on the
// faces of the swell, a near-solid band at the horizon breaking up as it nears; long and narrow on
// glassy water, wide and scattered on a ruffled one. A few faint stars, none behind the lead. Grain.
//
// Calm: the horizon sits at least 9% of the height below the lead's box, and the moon is always below
// the lead, so the brightest thing on the screen is never beside the text; the sky behind the text is a
// smooth dark gradient.
//
// Who the agent is: its place `at`, planned by the harness, sets the night's hue, from deep teal
// through prussian, ultramarine and indigo to plum, so agents on screen together wear clearly
// different nights; identity(at) is that hue as the agent's colour on the map.
// Its seed sets the rest, each part from its own stream: the moon's place (rising through the horizon,
// low over it, high in the sky below the lead, or breaking through a bank of cloud), its size and phase, the sky
// (clear, stratus, a bank on the horizon, or a veil over the moon), the shore (open sea, a headland,
// islands, or a bay between two headlands, sometimes a far range) and the water (glassy, calm or ruffled).
//
// What it keeps clear of: a moon that reads as a sticker on a ruled horizon or pulls the eye from
// beside the lead, a reflection that reads as a black slab, a disc showing through a cloud bank over
// a bright path, islands small enough to read as barges, a thin strip of stratus that reads as a
// contrail, and every seed the same postcard: the variety is what keeps a moon over the sea from
// being a stock night image.
import { fromOklch } from "./theme.js";

const FRAG = `precision highp float;
uniform vec2 u_res;
uniform float u_hz, u_mx, u_my, u_mr, u_seed, u_phase, u_tilt, u_veil, u_haloK, u_glassy, u_maxLand, u_bankTop, u_vis;
uniform vec3 u_sky0, u_sky1, u_mist, u_sea0, u_sea1, u_halo, u_moon, u_farc, u_near;
uniform vec4 u_l0, u_l1, u_l2, u_far, u_bank, u_c0, u_c1, u_c2;
float hash(vec2 p){ p = fract(p * vec2(123.34, 456.21) + u_seed); p += dot(p, p + 45.32); return fract(p.x * p.y); }
float vn(vec2 p){ vec2 i = floor(p), f = fract(p); vec2 u = f * f * (3. - 2. * f);
  return mix(mix(hash(i), hash(i + vec2(1., 0.)), u.x), mix(hash(i + vec2(0., 1.)), hash(i + vec2(1., 1.)), u.x), u.y); }
float fbm(vec2 p){ float s = 0., a = .5; for (int i = 0; i < 5; i++) { s += a * vn(p); p = p * 2.03 + vec2(17.1, 9.2); a *= .5; } return s; }
float fbm3(vec2 p){ float s = 0., a = .5; for (int i = 0; i < 3; i++) { s += a * vn(p); p = p * 2.03 + vec2(17.1, 9.2); a *= .5; } return s / .875; }

// A piece of land, as its height above the horizon at x. f = (p, q, height, kind):
// kind 0 an island between p and q; 1 a headland from the left edge, its foot at q, falling over p;
// 2 a headland from the right edge, its foot at p, rising over q. Below zero, none.
float feat(vec4 f, float x, float asp){
  if (f.w < -.5) return 0.;
  float detail = .55 + .55 * fbm3(vec2(x * asp * 4. + f.w * 7.3, f.z * 50.)) + .1 * vn(vec2(x * asp * 34., f.w + 3.));
  if (f.w < .5) {
    float u = (x - f.x) / (f.y - f.x);
    if (u <= 0. || u >= 1.) return 0.;
    // One summit off-centre with concave slopes falling to the water, and a shoulder riding on the
    // longer slope, with a broken top: an island, never a pill or a hull.
    float s1 = fract(f.z * 997.), s2 = fract(f.z * 7919.);
    float p1 = s2 < .5 ? mix(.25, .42, s1) : mix(.58, .75, s1);
    float sl = u < p1 ? u / p1 : (1. - u) / (1. - p1);
    // A hummock: flat at the waterline, rounded at the summit, steepest between.
    float b = pow(smoothstep(0., 1., sl), 1.25);
    float p2 = p1 < .5 ? mix(p1 + .18, .85, s2) : mix(.15, p1 - .18, s2);
    b += mix(.15, .3, s2) * exp(-pow((u - p2) / .1, 2.)) * sl;
    float det = 1. + .26 * (fbm3(vec2(u * 6. + f.z * 13., f.z * 31.)) - .5) + .1 * (vn(vec2(u * 30., f.z * 9.)) - .5);
    return f.z * min(b, 1.) * det;
  }
  if (f.w < 1.5) return f.z * detail * pow(1. - smoothstep(f.y - f.x, f.y, x), .7) * (1. - .35 * smoothstep(0., f.y, x));
  return f.z * detail * pow(smoothstep(f.x, f.x + f.y, x), .7) * (1. - .35 * (1. - smoothstep(f.x, 1., x)));
}
float land(float x, float asp){ return max(feat(u_l0, x, asp), max(feat(u_l1, x, asp), feat(u_l2, x, asp))); }
// A far range: low, with soft peaks, between u_far.z and u_far.w.
float farH(float x, float asp){
  if (u_far.x <= 0.) return 0.;
  float taper = smoothstep(u_far.z, u_far.z + .14, x) * (1. - smoothstep(u_far.w - .14, u_far.w, x));
  float ridge = 1. - abs(2. * vn(vec2(x * asp * u_far.y, 9.)) - 1.);
  return u_far.x * taper * (.3 + .5 * fbm3(vec2(x * asp * u_far.y * .45, 11.)) + .35 * ridge * ridge);
}
// A bank of cloud lying on the horizon: a low base (u_bank.x) under a row of rounded billows that
// never rise above u_bankTop.
float bankH(float x, float asp){
  if (u_bank.x <= 0. || x < u_bank.z || x > u_bank.w) return 0.;
  float span = u_bank.w - u_bank.z, cell = span / u_bank.y;
  float k = floor((x - u_bank.z) / cell), top = 0.;
  for (int j = -1; j <= 1; j++) {
    float i = k + float(j);
    float c = u_bank.z + (i + .5 + (hash(vec2(i, 3.)) - .5) * .7) * cell;
    float r = cell * (.6 + .7 * hash(vec2(i, 7.)));
    float e = max(0., 1. - pow(abs(i + .5 - u_bank.y * .5) / (u_bank.y * .5), 2.));
    float q = 1. - (x - c) * (x - c) / (r * r);
    if (q > 0.) top = max(top, sqrt(q) * (.5 + .5 * hash(vec2(i, 11.))) * (.3 + .7 * e) * r / (cell * 1.3));
  }
  // The moon breaking through: a billow rises round it, so its lower half is always behind cloud.
  if (u_vis < .99) top = max(top, .97 * sqrt(max(0., 1. - pow((x - u_mx) * asp / (u_mr * 3.2), 2.))));
  float taper = smoothstep(0., .12, (x - u_bank.z) / span) * smoothstep(0., .12, (u_bank.w - x) / span);
  return taper * (u_bank.x + (u_bankTop - u_bank.x) * top + .04 * u_bankTop * (vn(vec2(x * asp * 60., 6.)) - .5));
}
// A strip of stratus (y, x0, x1, thickness): broken along its length, a little darker than the sky
// it lies on, its moon-facing edge lit, glowing where it crosses the moon.
vec3 cloud(vec3 col, vec4 c, float x, float y, float asp, float lit){
  if (c.w <= 0.) return col;
  float u = (x - c.y) / (c.z - c.y);
  if (u <= 0. || u >= 1. || abs(y - c.x) > c.w * 3.) return col;
  float taper = pow(sin(3.14159 * u), .8);
  float yc = c.x + (fbm3(vec2(u * 2.5, c.x * 50.)) - .5) * c.w * 2.;
  float ht = c.w * taper * (.5 + .8 * fbm3(vec2(x * asp * 6., c.x * 40.)));
  float dy = y - yc;
  float dens = pow(1. - smoothstep(0., ht, abs(dy)), 1.6) * smoothstep(.3, .62, fbm3(vec2(x * asp * 2.2 + c.x * 13., c.x * 31.)));
  dens *= .6 + .4 * smoothstep(.3, .7, fbm3(vec2(x * asp * 10., y * 70.)));
  if (dens <= 0.) return col;
  vec2 d = vec2((x - u_mx) * asp, y - u_my);
  float r2 = dot(d, d);
  float edge = smoothstep(-.2, .9, dy * sign(u_my - yc) / max(ht, 1e-4)) * dens;
  // Away from the moon a strip is darker than the sky; over the moon it is lit, never a grey band.
  col = mix(col, col * .7 + u_mist * .05, dens * .8 * (1. - exp(-r2 / (u_mr * u_mr * 6.))));
  col += u_halo * edge * lit * (.05 + .6 * exp(-r2 * 40.));
  col += u_moon * dens * lit * .2 * exp(-r2 / (u_mr * u_mr * 3.));
  return col;
}
void main(){
  float asp = u_res.x / u_res.y, px = 1. / u_res.y;
  float x = gl_FragCoord.x / u_res.x, y = 1. - gl_FragCoord.y / u_res.y;
  float dx = (x - u_mx) * asp;
  float lit = .5 + .5 * cos(u_phase);
  float glow = .35 + .65 * lit;
  vec3 col;
  if (y < u_hz) {
    float d = u_hz - y, k = clamp(y / u_hz, 0., 1.);
    col = mix(u_sky0, u_sky1, pow(k, 2.));
    col = mix(col, u_mist, smoothstep(.8, 1., k) * .5);
    // The grain of a woodblock: faint horizontal streaks where the ink lay unevenly.
    col *= 1. + .045 * (fbm3(vec2(x * asp * 3., y * 90.)) - .5);
    vec2 m = vec2(dx, y - u_my) / u_mr;
    float md = length(m), r2 = dot(m, m) * u_mr * u_mr;
    // The halo: wide and faint (wider behind a veil of cloud), close and bright, and a glow hugging the disc.
    col += u_halo * glow * u_haloK * (.07 * (1. + 1.5 * u_veil) * exp(-r2 * (7. - 4. * u_veil)) + .15 * exp(-r2 * 110.));
    // The glow hugs only the lit limb, so a gibbous moon never shows an outlined dark side.
    vec3 L = vec3(sin(u_phase) * cos(u_tilt), sin(u_phase) * sin(u_tilt), cos(u_phase));
    vec2 dir = md > 0. ? vec2(m.x, -m.y) / md : vec2(0.);
    float limb = clamp(L.z + length(L.xy) * dot(dir, L.xy / max(length(L.xy), 1e-4)) * 1.2, 0., 1.);
    col += u_moon * glow * .3 * exp(-max(md - 1., 0.) * 3.2) * limb;
    // The moon: a lit sphere, so the seed's phase shades it truly; faint seas; earthshine on the dark side.
    if (md < 1.06 + .12 * u_veil) {
      vec3 n = vec3(m.x, -m.y, sqrt(max(0., 1. - md * md)));
      float l = smoothstep(-.06, .14, dot(n, L));
      float seas = smoothstep(.45, .66, fbm(m * .85 + 4.));
      float foot = clamp(m.y * .5 + .5, 0., 1.) * (1. - smoothstep(0., u_mr * 2.5, u_hz - u_my));
      vec3 face = u_moon * (1.1 - .16 * seas) * (.9 + .1 * n.z) * mix(vec3(1.), vec3(1., .95, .87), foot * foot * .5);
      face *= 1. - .1 * u_veil;
      vec3 disc = mix(col + u_moon * .012, face, l);
      col = mix(col, disc, 1. - smoothstep(1. - 1.5 * px / u_mr - .12 * u_veil, 1. + .5 * px / u_mr + .1 * u_veil, md));
    }
    // A bank of cloud on the horizon, in front of the moon, its top lit where the moon is near.
    float bh = bankH(x, asp);
    if (bh > 0.) {
      float soft = bh * .14 + 1.5 * px;
      float cov = 1. - smoothstep(bh - soft, bh + soft, d);
      float top = cov * pow(1. - smoothstep(0., bh * .25, bh - d), 2.);
      // Darker at its foot, a little lighter toward its billowed top, which the moon lights near it.
      vec3 body = mix(u_sky1, u_mist, .35) * (.42 + .3 * clamp(d / max(bh, 1e-4), 0., 1.));
      body *= 1. + .12 * (fbm3(vec2(x * asp * 14., d * 120.)) - .5);
      col = mix(col, body, cov);
      float near = exp(-(dx * dx + (y - u_my) * (y - u_my)) * 22.);
      col += u_halo * top * glow * (.02 + .3 * near);
      col += u_halo * cov * glow * .05 * near;
    }
    col = cloud(col, u_c0, x, y, asp, glow); col = cloud(col, u_c1, x, y, asp, glow); col = cloud(col, u_c2, x, y, asp, glow);
    if (d < u_maxLand) {
      float fh = farH(x, asp);
      col = mix(col, mix(u_farc, u_mist, .25), smoothstep(-px, px, fh - d) * .88);
      float lh = land(x, asp);
      float cov = smoothstep(-px, px, lh - d);
      // Its ridge catches the moon from behind: a hairline of light, brightest near the moon.
      float rim = cov * exp(-(lh - d) / (1.6 * px)) * (.25 + .75 * exp(-dx * dx * 6.));
      col = mix(col, u_near, cov);
      col += u_halo * rim * glow * .45;
    }
  } else {
    float d = y - u_hz, t = d / (1. - u_hz);
    col = mix(u_sea0, u_sea1, pow(t, .5));
    col = mix(col, u_mist * .8, (1. - smoothstep(0., .05, t)) * .35);
    // The moon's light on the water: a long soft column under it, narrower on glassy water.
    float cw = (.03 + .2 * t) * (1. - .45 * u_glassy);
    col += u_halo * glow * .15 * (.25 + .75 * u_vis) * exp(-dx * dx / (cw * cw)) * (1. - .7 * t);
    float z = 1. / (t + .02);
    float sw = fbm3(vec2(dx * z * .7, z * 2.6)) - .5;
    col *= 1. + .22 * sw * smoothstep(0., .1, t) * (1. - .6 * u_glassy);
    // The land, the far range and the bank mirrored: pushed sideways by the swell, broken into
    // slivers by the ripples, softer and fainter the nearer they come.
    if (d < u_maxLand * 1.25) {
      float br = vn(vec2(x * asp * z * 1.3, z * 8.)) - .5;
      float amp = (.002 + .045 * t) * (1. - .55 * u_glassy) * u_maxLand * 18.;
      float xs = x + sw * (.003 + .03 * t) * (1. - .5 * u_glassy) / asp;
      float e = 1.5 * px + d * .06;
      float fade = 1. - .55 * smoothstep(0., u_maxLand * 1.2, d);
      float lh = land(xs, asp), fh = farH(xs, asp), bh = bankH(xs, asp);
      col = mix(col, mix(col, u_near, .82), smoothstep(-e, e, lh * .97 - d + br * amp) * fade);
      col = mix(col, mix(col, u_farc * .8, .5), smoothstep(-e, e, fh - d + br * amp) * fade * .6);
      col = mix(col, col * .75, smoothstep(-e * 2., e * 2., bh * .8 - d + br * amp) * fade * .5);
    }
  }
  gl_FragColor = vec4(col, 1.);
}`;

// The night's hue, from the agent's place: deep teal (210°) through prussian, ultramarine and indigo
// to plum (325°), in OKLCH.
const HUE0 = 210, HUE1 = 325;
const hueOf = (at) => HUE0 + (HUE1 - HUE0) * at;
function night(at) {
  const hue = hueOf(at), c = (L, C) => fromOklch(L, C, hue);
  return { sky0: c(0.13, 0.022), sky1: c(0.33, 0.092), mist: c(0.42, 0.09), sea0: c(0.29, 0.082), halo: c(0.8, 0.058), ripple: c(0.53, 0.095) };
}
// The agent's colour on the map: its night's hue, light enough to read on the map's paper.
function identity(at) { return fromOklch(0.6, 0.12, hueOf(at)); }

const hex3 = (h) => { const n = parseInt(h.slice(1), 16); return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255]; };
const OFF = [0, 0, 0, -1];

export default {
  name: "marama",
  about: "The moon over still water at night, and its path of light.",
  display: { family: "Newsreader", files: ["font-newsreader.woff2"], weights: "200 800", bold: 620, axes: '"opsz" 48' },
  palette: {
    paper: "#0b111d", tint: "#0f1625", ink: "#ecebe4", body: "#b8bfcc", muted: "#7d879a", line: "#1f2a3b",
    accent: "#d8c48e", figure: ["#d8c48e", "#7fb0c8", "#a99ad8", "#8fc0a0"], cover_ink: "#ecebe4", cover_body: "#b8bfcc",
  },
  // The night's hue, from teal to plum (hueOf): the two ends, then the middles.
  slots: [0, 1, 1 / 2, 1 / 4, 3 / 4],
  map: { tone: "#41507f", duo: 0.6 },
  identity,
  paint(ctx, { w, h, dpr, at, stream, noise, quiet, kit }) {
    const N = night(at);
    const portrait = h > w;
    // A thumbnail has no lead: compose as if one sat where a page puts it.
    const q = quiet || (portrait ? { x: w * 0.04, y: h * 0.2, w: w * 0.92, h: h * 0.38 } : { x: w * 0.27, y: h * 0.2, w: w * 0.46, h: h * 0.3 });
    const qb = (q.y + q.h) / h;
    // The horizon sits clear below the lead, so the moon and its haze never reach the text.
    const hr = stream("horizon");
    const hz = Math.min(0.86, Math.max(qb + (portrait ? 0.12 : 0.09), portrait ? hr.range(0.62, 0.74) : hr.range(0.6, 0.84)));

    // The moon: rising through the horizon, low over it, or high in the sky where the lead leaves room
    // below it; never beside the lead. Its place across is free, since it is always below the text.
    const mo = stream("moon");
    const right = mo.chance(0.5);
    const mr = mo.range(0.017, 0.029) * (portrait ? 0.8 : 1);
    const floor = qb + (portrait ? 0.035 : 0.07) + mr;   // the highest the moon may stand: clear below the lead
    const pose = mo();
    let mx = portrait ? (right ? mo.range(0.6, 0.78) : mo.range(0.22, 0.4)) : (right ? mo.range(0.58, 0.85) : mo.range(0.15, 0.42));
    let my;
    if (pose < 0.4) my = hz - mr * mo.range(-0.35, 0.7);
    else if (pose < 0.72 || hz - floor < mr * 6) my = Math.max(floor, hz - mr * mo.range(1.6, 4.5));
    else my = mo.range(floor, hz - mr * 5);
    my = Math.min(my, hz + mr * 0.35);      // never set: at least two thirds of the disc above the horizon
    const phaseDraw = mo(), phaseRange = mo.range(0.45, 1.35), tiltSign = mo.chance(0.5) ? 1 : -1, tiltK = mo.range(0.25, 1.1);
    const hideDraw = mo();

    // The sky: clear, strips of stratus, a bank on the horizon (the moon clear above it, or wholly behind
    // it, lighting its top), or a veil across the moon.
    const sk = stream("sky");
    const zoneTop = Math.max(qb + 0.03, hz - 0.2), zoneBot = hz - 0.015;
    const strips = [];
    let bank = [0, 0, 0, 0], bankTop = 0, veil = 0, vis = 1;
    const thick = portrait ? 0.8 : 1;
    const strip = (yy, x0, x1, th) => strips.push([yy, x0, x1, th * thick]);
    const skyKind = sk();
    if (skyKind < 0.25) {
      // Clear.
    } else if (skyKind < 0.58) {
      for (let i = sk.int(1, 3); i > 0; i--) {
        const yy = sk.range(zoneTop + 0.025, zoneBot), x0 = sk.range(-0.25, 0.65);
        strip(yy, x0, x0 + sk.range(0.4, 0.9), sk.range(0.013, 0.026) * (0.6 + 0.4 * (yy - zoneTop) / (zoneBot - zoneTop + 1e-6)));
      }
    } else if (skyKind < 0.84) {
      // A bank of billows on the horizon, low enough to stay well clear of the lead.
      bankTop = Math.min(sk.range(0.035, 0.075), (hz - qb) * 0.45);
      const base = bankTop * sk.range(0.22, 0.35);
      const x0 = Math.min(sk.range(-0.3, 0.15), mx - 0.2), x1 = Math.max(x0 + sk.range(0.8, 1.3), mx + 0.2);
      bank = [base, sk.int(16, 28), x0, x1];
      const peekTop = Math.min(bankTop, (hz - floor) / 0.97);
      if (hideDraw < 0.3 && peekTop >= 0.022) {
        // Breaking through it: a full moon, its lower half behind a billow whose top it lights, and a
        // path dimmed to the share of it that shows.
        bankTop = peekTop; bank[0] = Math.min(bank[0], bankTop * 0.3);
        my = hz - bankTop * 0.97 + mr * mo.range(0.05, 0.35); vis = 0.6;
      } else {
        // Clear above it: where the lead leaves too little room, the bank lies lower, or not at all.
        bankTop = Math.min(bankTop, (hz - floor - mr * 1.25) / 1.05);
        if (bankTop < 0.015) { bank = [0, 0, 0, 0]; bankTop = 0; }
        else { bank[0] = Math.min(bank[0], bankTop * 0.35); my = Math.min(my, hz - bankTop * 1.05 - mr * 1.25); }
      }
      if (sk.chance(0.4)) { const xs = sk.range(-0.2, 0.55); strip(sk.range(zoneTop + 0.025, Math.max(zoneTop + 0.03, hz - bankTop - 0.03)), xs, xs + sk.range(0.4, 0.7), sk.range(0.012, 0.018)); }
    } else {
      veil = sk.range(0.55, 1);
      if (my > hz - mr * 1.5) my = Math.max(floor, hz - mr * 2.2);
      strip(my + sk.range(-0.3, 0.3) * mr, mx - sk.range(0.22, 0.4), mx + sk.range(0.22, 0.45), mr * sk.range(1.6, 2.2));
    }
    // A thin strip across the moon reads as a ring round it: move it clear, above or below the disc.
    for (const c of strips) {
      if (veil || c[1] > mx || c[2] < mx) continue;
      const gap = mr * 1.6 + c[3] * 2;
      if (Math.abs(c[0] - my) < gap) c[0] = my + (c[0] < my && my - gap - c[3] * 3 > qb + 0.01 ? -gap : gap);
      if (c[0] > hz - 0.008) c[3] = 0;
    }
    const C = (i) => (strips[i] && strips[i][0] - strips[i][3] * 3 > qb + 0.01 ? strips[i] : [0, 0, 0, 0]);

    // Full in the horizon, as a rising moon is; once risen, full or gibbous to just past half (a thin
    // crescent reads as an icon); the lit limb faces down toward the sun under the horizon.
    const risen = hz - my > mr * 1.3 && vis === 1;
    const phase = !risen || phaseDraw < 0.35 ? phaseRange * 0.15 : phaseRange;
    const tilt = -Math.PI / 2 + tiltSign * tiltK;
    const lit = 0.5 + 0.5 * Math.cos(phase);

    // The shore: open sea, a headland, islands, or a bay between two headlands; sometimes a far range.
    const sh = stream("shore");
    const lands = [];
    const scale = portrait ? 0.75 : 1;
    const headland = (fromRight, foot) => {
      const fw = sh.range(0.05, 0.16), ht = sh.range(0.04, 0.085) * scale;
      lands.push(fromRight ? [foot, fw, ht, 2] : [fw, foot, ht, 1]);
    };
    // An island's width and height go together, so a small one is a hummock, never a long low pill.
    // It never stands in front of a low moon: it moves to the side it was nearer.
    const island = (x0, width) => {
      if (x0 < mx + 0.05 && x0 + width > mx - 0.05) x0 = x0 + width / 2 < mx ? mx - 0.05 - width : mx + 0.05;
      lands.push([x0, x0 + width, width * (w / h) * sh.range(0.09, 0.16), 0]);
    };
    const shore = sh();
    if (shore < 0.22) {
      if (sh.chance(0.5)) island(sh.range(0.05, 0.85), sh.range(0.05, 0.1));
    } else if (shore < 0.52) {
      // Mostly on the side away from the moon, its foot well clear of it.
      const away = sh.chance(0.75);
      const fromRight = away ? !right : right;
      const foot = fromRight ? Math.max(sh.range(0.55, 0.82), mx + 0.1) : Math.min(sh.range(0.18, 0.45), mx - 0.1);
      headland(fromRight, foot);
    } else if (shore < 0.74) {
      for (let i = sh.int(1, 2); i > 0; i--) island(sh.range(-0.05, 0.85), sh.range(0.1, 0.28));
    } else {
      headland(false, Math.min(sh.range(0.12, 0.3), mx - 0.08));
      headland(true, Math.max(sh.range(0.7, 0.88), mx + 0.08));
      for (const l of lands) l[2] *= 0.75;
    }
    for (const l of lands) l[2] = Math.min(l[2], (hz - qb) * 0.5);
    let far = [0, 0, 0, 0];
    if (sh.chance(0.45)) { const x0 = sh.range(-0.3, 0.5); far = [sh.range(0.007, 0.02) * scale, sh.range(3, 9), x0, x0 + sh.range(0.45, 1.1)]; }
    const maxLand = Math.max(0.004, ...lands.map((l) => l[2] * 1.3), far[0] * 1.3, bankTop * 1.1) + 0.004;
    const L = (i) => lands[i] || OFF;

    // The water: glassy, calm or ruffled.
    const wa = stream("water");
    const glassy = wa.pick([1, 0.5, 0.5, 0]);

    // At most about half a megapixel, scaled up smoothly: the sky and the water are soft, the
    // ripples and stars are drawn after at full resolution, and a phone's paint stays under budget.
    const res = Math.min(dpr, Math.sqrt(0.45e6 / (w * h)));
    const g = kit.shader(FRAG, w * res, h * res, {
      u_hz: hz, u_mx: mx, u_my: my, u_mr: mr, u_seed: wa() * 10, u_phase: phase, u_tilt: tilt, u_veil: veil, u_haloK: 1,
      u_glassy: glassy, u_maxLand: maxLand, u_bankTop: bankTop, u_vis: vis,
      u_sky0: hex3(N.sky0), u_sky1: hex3(N.sky1), u_mist: hex3(N.mist), u_sea0: hex3(N.sea0), u_sea1: hex3("#02050b"),
      u_halo: hex3(N.halo), u_moon: hex3("#eef2f8"), u_farc: hex3(kit.mix(N.sky1, "#000000", 0.4)), u_near: hex3("#03060d"),
      u_l0: L(0), u_l1: L(1), u_l2: L(2), u_far: far, u_bank: bank, u_c0: C(0), u_c1: C(1), u_c2: C(2),
    });
    if (g) ctx.drawImage(g, 0, 0, w, h);
    else {
      const sky = ctx.createLinearGradient(0, 0, 0, hz * h);
      sky.addColorStop(0, N.sky0); sky.addColorStop(1, N.sky1);
      ctx.fillStyle = sky; ctx.fillRect(0, 0, w, hz * h);
      const sea = ctx.createLinearGradient(0, hz * h, 0, h);
      sea.addColorStop(0, N.sea0); sea.addColorStop(1, "#02050b");
      ctx.fillStyle = sea; ctx.fillRect(0, hz * h, w, h - hz * h);
    }

    const H0 = hz * h, cx = mx * w, cy = my * h, span = Math.max(w, h * 0.8);
    // Strokes are sized to the picture, so a thumbnail reads like the full piece.
    const k = Math.min(1.2, Math.max(0.25, span / 1200));
    const glint = "#f2f0e8", bright = (0.35 + 0.65 * lit) * (1 - 0.45 * veil) * vis;

    ctx.save();
    ctx.globalCompositeOperation = "lighter";
    ctx.lineCap = "round";
    // Ripples: the sky's light caught on the swell, across the whole water, crowding toward the horizon.
    // Each row is three paths, one per strength, so a phone strokes hundreds of paths, not thousands.
    const rp = stream("ripples");
    ctx.strokeStyle = N.ripple;
    let y = H0 + 2;
    while (y < h) {
      const t = (y - H0) / (h - H0);
      const n = Math.round((w / ((60 + 140 * t) * k)) * (1 - 0.55 * glassy));
      const rows = [new Path2D(), new Path2D(), new Path2D()];
      for (let i = 0; i < n; i++) {
        const x = rp() * w, len = (4 + 40 * t) * (0.3 + rp()) * k * (1 + 0.8 * glassy);
        const row = rows[Math.floor(rp() * 3)];
        row.moveTo(x, y); row.lineTo(x + len, y);
      }
      ctx.lineWidth = (0.5 + 0.9 * t) * Math.max(0.6, k);
      rows.forEach((row, b) => { ctx.globalAlpha = (0.07 + 0.04 * b) * (1 - 0.6 * t); ctx.stroke(row); });
      y += Math.max(0.8, (1 + 7 * t * t + rp() * (1 + 3 * t)) * k);
    }
    // The glade: the moon's glints, a near-solid band at the horizon breaking into strokes as they near.
    // Glassy water draws it long and narrow, a ruffled one wide and scattered; a high moon widens it.
    // Tiny far glints are rects; only the large near ones carry a soft halo.
    const gl = stream("glade");
    ctx.fillStyle = glint;
    const width = (1.45 - 0.95 * glassy) * (1 + Math.min(1, Math.max(0, hz - my) / 0.25) * 0.5);
    y = H0 + 1.5;
    while (bright > 0.05 && y < h) {
      const t = (y - H0) / (h - H0);
      const spreadX = (0.006 + 0.085 * t) * span * width;
      const n = Math.round((8 + 20 * (1 - t) + 16 * t * gl()) * (0.8 + 0.4 * width));
      for (let i = 0; i < n; i++) {
        const off = gl.gauss() * spreadX;
        // Glints gather on the faces of the swell: a field stretched along the crests, widening as it nears.
        const crest = noise.n2(off / ((30 + 90 * t) * k), (y - H0) / ((3 + 26 * t) * k));
        if (crest < -0.2 - 0.9 * (1 - t) ** 3) continue;
        const fall = Math.exp(-((off / spreadX) ** 2) * 0.6);
        const len = (2 + 30 * t) * (0.25 + gl() * gl() * 1.8) * (0.4 + fall) * k * (1 + 0.6 * glassy);
        const a = bright * (0.15 + 0.7 * gl() ** 2) * fall * (0.45 + 0.55 * (1 - t)) * Math.min(1, 0.55 + crest + 0.5 * (1 - t) ** 3);
        const thickG = (0.5 + 1.6 * t) * Math.max(0.6, k) * (0.6 + 0.8 * gl()) * (1 - 0.3 * glassy);
        if (a < 0.02) continue;
        const gx = cx + off;
        if (len < 3) { ctx.globalAlpha = a * 0.8; ctx.fillRect(gx - len / 2, y - thickG / 2, len, thickG); continue; }
        const tl = (gl() - 0.5) * 0.08;
        if (t > 0.18 && len > 6) {
          ctx.globalAlpha = a * 0.18;
          ctx.beginPath(); ctx.ellipse(gx, y, len * 0.62, thickG * 2.4, tl, 0, 6.2832); ctx.fill();
        }
        ctx.globalAlpha = a;
        ctx.beginPath(); ctx.ellipse(gx, y, len / 2, thickG / 2, tl, 0, 6.2832); ctx.fill();
      }
      y += Math.max(0.7, (0.8 + 8 * t * t + gl() * (0.8 + 4 * t)) * k);
    }
    ctx.globalAlpha = 1;
    // A few stars, faint and high, away from the moon, and none behind the lead.
    const st = stream("stars");
    for (let i = 0; i < Math.round((w * h) / 16000); i++) {
      const x = st() * w, sy = st() ** 1.3 * H0 * 0.8, m = st() ** 3;
      if (x > q.x - 8 && x < q.x + q.w + 8 && sy > q.y - 8 && sy < q.y + q.h + 8) continue;
      const away = Math.min(1, Math.hypot((x - cx) / span, (sy - cy) / span) * 2.2);
      ctx.fillStyle = kit.alpha(m > 0.6 ? "#fff6e6" : "#d8e4ff", (0.06 + 0.45 * m) * away * (1 - (sy / H0) * 0.6) * (1 - 0.6 * veil));
      ctx.beginPath(); ctx.arc(x, sy, 0.45 + 0.8 * m, 0, 6.2832); ctx.fill();
    }
    ctx.restore();
    kit.grain(ctx, w, h, 0.05, "marama-grain");
  },
};
