// Pāua: the polished nacre of a pāua shell, close up, under one raking light.
//
// What it depicts. The inside of the shell fills the frame. Its growth layers curve round an apex
// beyond a lower corner, as the shell grew. Polishing has cut down through them, so the nacre shows
// as terraces, one layer each, some of them the shell's dark organic layer, under a clear
// lacquer that holds a reflection of a window. One light falls low across it from that side, so the
// nacre there burns with colour, while the rest lies in shadow: still nacre, dim and smooth, and that
// shadow is where the page's lead sits. The page itself is the shadow's dark.
//
// Who it is. Every pāua shell has its own colouring: blue, indigo, purple, rose, copper, gold, emerald,
// teal. Each is a palette of six interference colours, and an agent's place `at`, planned by the
// harness, picks a place on the ring of them, so agents on screen together wear clearly different
// shells; identity(at) is that shell's signature colour, for its map node. Within one
// shell a second colouring shows in zones along the growth and in the deeper layer, as on a real one.
//
// Technique. One fragment shader through kit.shader, then the kit's grain. A domain-warped noise
// field in growth coordinates (angle and log-radius round the apex), stretched along the growth,
// gives the depth of the layer the polish reached at each point; cut into steps it gives the
// terraces, and its slope gives the surface's relief. Colour runs round the shell's palette by
// terrace, by the slope within it and by tilt, as a thin film's colour shifts with angle; each layer
// has its own brightness, and a few are the dark organic layer. Terraces
// are crisp where the light shows them and melt together in shadow, over a softer deeper layer seen
// through the surface. Fine striations along the growth are the nacre's satin, faded out before they
// would alias. Then a sheen and glints where folds face the light, the window in the lacquer, bent
// by the folds beneath, and now and then a warm flash on a steep fold.
//
// The colour is confined to the light, away from the lead, and everything near the lead is low and
// smooth.

// The shells, round a ring: six interference colours each (deep, then round the loop), and the
// signature colour that names the shell on the map. Each keeps a deep blue or indigo, as all pāua do.
const SHELLS = [
  { name: "blue", sig: "#2a86f0", stops: ["#0d1d79", "#1266ea", "#2a9cf2", "#0aa3b8", "#1a4fd0", "#663dcc"] },
  { name: "indigo", sig: "#6a5af0", stops: ["#170f66", "#4b45e8", "#7a52e6", "#3a7fe0", "#5a2fb3", "#1a8f9e"] },
  { name: "purple", sig: "#b455dc", stops: ["#2a0b55", "#9a3fd6", "#c870e0", "#5a4fe0", "#8a2fa0", "#2a8fc0"] },
  { name: "rose", sig: "#e0609e", stops: ["#30094f", "#c23f9c", "#eb7aa8", "#7a4dd9", "#9c2e8c", "#2a6fb8"] },
  { name: "copper", sig: "#e27a42", stops: ["#2a1030", "#d9682e", "#f0a050", "#e0583a", "#c4467a", "#1e8fa6"] },
  { name: "gold", sig: "#c9b632", stops: ["#1a2a0c", "#94a51f", "#e0c24a", "#4aa84f", "#6f8f2a", "#b8862a"] },
  { name: "emerald", sig: "#2fc27e", stops: ["#04303a", "#14b36e", "#5fdc8c", "#0b8f9c", "#2470d6", "#139a55"] },
  { name: "teal", sig: "#16b9b9", stops: ["#06303f", "#06b0b8", "#3fd6c8", "#0a8f8a", "#1fbfa8", "#5a5fd0"] },
];

const hexRgb = (hex) => { const n = parseInt(hex.slice(1), 16); return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255]; };
const toHex = (c) => "#" + c.map((x) => Math.round(Math.min(1, Math.max(0, x)) * 255).toString(16).padStart(2, "0")).join("");
const lerp = (a, b, t) => a.map((x, i) => x + (b[i] - x) * t);

// The shell at place s in [0, 1) round the ring: the nearest one, blending to the next only in a
// narrow band halfway between, so every agent wears a clear one. The ring runs round the hues, and
// with eight shells the golden-ratio marks 0 to 7 land on eight different ones, each about three
// shells round from the one before: blue, gold, purple, teal, copper, indigo, emerald, rose.
function shellAt(s) {
  const p = (((s % 1) + 1) % 1) * SHELLS.length, i = Math.floor(p), f = p - i;
  const t = Math.min(1, Math.max(0, (f - 0.35) / 0.3)), k = t * t * (3 - 2 * t);
  const A = SHELLS[i], B = SHELLS[(i + 1) % SHELLS.length];
  return { stops: A.stops.map((c, j) => lerp(hexRgb(c), hexRgb(B.stops[j]), k)), sig: lerp(hexRgb(A.sig), hexRgb(B.sig), k) };
}

// The agent's colour among the others wearing Pāua: its shell's signature, for its map node.
function identity(at) { return toHex(shellAt(at).sig); }

const FRAG = `
precision highp float;
uniform vec2 u_res;
uniform vec2 u_apex;
uniform vec4 u_quiet;
uniform vec2 u_light;
uniform float u_light_r;
uniform float u_seed;
uniform float u_tight;
uniform float u_hue;
uniform float u_bands;
uniform float u_gloss;
uniform float u_warm;
uniform float u_zone;
uniform float u_zone_amt;
uniform vec3 u_a0, u_a1, u_a2, u_a3, u_a4, u_a5;
uniform vec3 u_b0, u_b1, u_b2, u_b3, u_b4, u_b5;

vec3 permute(vec3 x){ return mod(((x*34.0)+1.0)*x, 289.0); }
float snoise(vec2 v){
  const vec4 C = vec4(0.211324865405187, 0.366025403784439, -0.577350269189626, 0.024390243902439);
  vec2 i = floor(v + dot(v, C.yy));
  vec2 x0 = v - i + dot(i, C.xx);
  vec2 i1 = (x0.x > x0.y) ? vec2(1.0, 0.0) : vec2(0.0, 1.0);
  vec4 x12 = x0.xyxy + C.xxzz; x12.xy -= i1;
  i = mod(i, 289.0);
  vec3 p = permute(permute(i.y + vec3(0.0, i1.y, 1.0)) + i.x + vec3(0.0, i1.x, 1.0));
  vec3 m = max(0.5 - vec3(dot(x0,x0), dot(x12.xy,x12.xy), dot(x12.zw,x12.zw)), 0.0);
  m = m*m; m = m*m;
  vec3 x = 2.0 * fract(p * C.www) - 1.0;
  vec3 h = abs(x) - 0.5;
  vec3 ox = floor(x + 0.5);
  vec3 a0 = x - ox;
  m *= 1.79284291400159 - 0.85373472095314 * (a0*a0 + h*h);
  vec3 g; g.x = a0.x * x0.x + h.x * x0.y; g.yz = a0.yz * x12.xz + h.yz * x12.yw;
  return 130.0 * dot(m, g);
}
float fbm3(vec2 p){ float s = 0.0, a = 0.5; for (int i = 0; i < 3; i++) { s += a * snoise(p); p = p * 2.07 + vec2(17.0, -9.0); a *= 0.5; } return s / 0.875; }
float box(vec2 p, vec4 b){ vec2 c = 0.5*(b.xy + b.zw), e = 0.5*(b.zw - b.xy); vec2 d = abs(p - c) - e; return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0); }
float hash1(float n){ return fract(sin(n * 91.345 + u_seed) * 43758.5453); }

// Along the growth lines (x) and across them (y), round the apex.
vec2 growth(vec2 uv){
  vec2 d = uv - u_apex;
  return vec2(atan(d.y, d.x) * 1.1, log(length(d)) * u_tight);
}
// The depth of the layer the polish has reached at each point: wavy layers stretched along the
// growth, warped once. The warp is slow, so a nearby point (for the slope) can reuse it.
vec2 warpAt(vec2 g){
  return vec2(fbm3(g * vec2(0.4, 0.7) + u_seed), fbm3(g * vec2(0.4, 0.7) + vec2(5.2, 1.3) - u_seed));
}
float layer(vec2 g, vec2 q){
  return fbm3((g + 0.5 * q) * vec2(0.24, 1.0)) + 0.22 * g.y;
}
// A blend of two colours that keeps their colourfulness, so copper into teal passes through a
// colour rather than grey, as a thin film's does.
vec3 cmix(vec3 a, vec3 b, float t){
  const vec3 W = vec3(0.2126, 0.7152, 0.0722);
  vec3 m = mix(a, b, t);
  float l = dot(m, W), cm = length(m - l);
  float target = mix(length(a - dot(a, W)), length(b - dot(b, W)), t);
  return clamp(l + (m - l) * (target / max(cm, 0.002)), 0.0, 1.0);
}
// The shell's colours round its loop, z leaning toward the second colouring. crisp near 0 gives
// hard steps between stops, 0.5 a smooth blend.
vec3 shell(float t, float crisp, float z){
  t = fract(t) * 6.0;
  vec3 a, b; float f;
  if (t < 1.0) { a = cmix(u_a0, u_b0, z); b = cmix(u_a1, u_b1, z); f = t; }
  else if (t < 2.0) { a = cmix(u_a1, u_b1, z); b = cmix(u_a2, u_b2, z); f = t - 1.0; }
  else if (t < 3.0) { a = cmix(u_a2, u_b2, z); b = cmix(u_a3, u_b3, z); f = t - 2.0; }
  else if (t < 4.0) { a = cmix(u_a3, u_b3, z); b = cmix(u_a4, u_b4, z); f = t - 3.0; }
  else if (t < 5.0) { a = cmix(u_a4, u_b4, z); b = cmix(u_a5, u_b5, z); f = t - 4.0; }
  else { a = cmix(u_a5, u_b5, z); b = cmix(u_a0, u_b0, z); f = t - 5.0; }
  return cmix(a, b, smoothstep(0.5 - crisp, 0.5 + crisp, f));
}
// One cut layer: its own place round the loop and its own brightness, as each layer shows its own
// colour (a step from its neighbour, sometimes a leap). Where the
// light is weak, sharp is low and the layers keep one brightness, so the shadow stays smooth.
vec3 terrace(float i, float fr, float base, float z, float sharp){
  vec3 c = shell(base + i * 0.2 + hash1(i) * 0.22 + fr * 0.16, 0.3, z);
  float b = 0.78 + 0.42 * hash1(i + 3.0);
  c *= b;
  return mix(shell(base + i * 0.2 + hash1(i) * 0.22 + fr * 0.16, 0.3, z), c, sharp);
}

void main(){
  vec2 uv = gl_FragCoord.xy / u_res.y;
  vec2 g = growth(uv);
  float e = 0.006;
  vec2 q = warpAt(g);
  float h0 = layer(g, q);
  float hx = layer(growth(uv + vec2(e, 0.0)), q), hy = layer(growth(uv + vec2(0.0, e)), q);
  vec2 slope = vec2(hx - h0, hy - h0) / e;
  vec3 n = normalize(vec3(-slope * 0.03, 1.0));
  float tilt = 1.0 - n.z;
  // A deeper layer seen through the surface: larger and softer.
  float hd = fbm3((g + vec2(3.1, -1.7) + 0.4 * q) * vec2(0.15, 0.6)) + 0.13 * g.y;

  // The light, and the calm the lead keeps round it.
  vec3 L = normalize(vec3(u_light - uv, 0.55));
  float lobe = exp(-pow(length(uv - u_light) / u_light_r, 2.0));
  float calm = smoothstep(0.0, 0.32, box(uv, u_quiet));
  float lit = lobe * mix(0.12, 1.0, calm);
  float amb = 0.10 + 0.08 * exp(-pow(length(uv - u_light) / (u_light_r * 2.2), 2.0));
  float diff = max(dot(n, L), 0.0);
  float spec = max(dot(reflect(-L, n), vec3(0.0, 0.0, 1.0)), 0.0);

  // The second colouring, in broad zones along the growth.
  float z = smoothstep(0.05, 0.45, fbm3(g * vec2(0.16, 0.55) + u_zone)) * u_zone_amt;

  // Terraces: the polish cut down through the layers. Each is one layer's colour, shading across
  // its width; the step between two is sharp where lit (to a pixel), soft in shadow.
  // The edges are ragged along the growth, as the nacre's tablets break.
  float rag = 0.035 * snoise(vec2(g.x * 9.0, g.y * 2.5) + u_seed) + 0.012 * snoise(vec2(g.x * 31.0, g.y * 7.0) - u_seed);
  float T = h0 * u_bands * 3.0 + rag;
  float idx = floor(T), fr = T - idx;
  float px = length(slope) * u_bands * 3.0 / u_res.y;
  float sharp = smoothstep(0.05, 0.6, lobe);
  float soft = mix(0.6, max(px * 1.2, 0.012), sharp);
  float base = tilt * 1.6 + u_hue;
  vec3 top = mix(terrace(idx - 1.0, 1.0, base, z, sharp), terrace(idx, fr, base, z, sharp), smoothstep(0.0, soft, fr));

  vec3 deep = shell(hd * u_bands * 0.7 + u_hue + 0.33, 0.5, clamp(z + 0.45, 0.0, 1.0));
  vec3 col = mix(deep, top, 0.5 + 0.45 * max(smoothstep(-0.2, 0.6, h0 - hd), lobe));
  // Some steps are lined with a thread of the shell's dark organic layer, a hairline or a little
  // more, coming and going along the step.
  float band = (0.0008 + 0.0024 * hash1(idx + 13.0)) * length(slope) * u_bands * 3.0;
  float along = smoothstep(-0.05, 0.4, snoise(vec2(g.x * 1.6, idx * 1.7 + u_seed)));
  float organic = step(0.65, hash1(idx + 11.0)) * along * (1.0 - smoothstep(band - px, band + px, fr)) * sharp;
  col *= 1.0 - 0.65 * organic;

  // The satin, along the growth, each striation faded out before it is finer than a few pixels.
  float gpx = u_tight / (length(uv - u_apex) * u_res.y);
  float a1 = smoothstep(0.22, 0.08, 90.0 * gpx / 6.2831853), a2 = smoothstep(0.22, 0.08, 34.0 * gpx / 6.2831853);
  float s1 = 0.5 + 0.5 * sin(g.y * 90.0 + h0 * 14.0 + g.x * 2.0);
  float s2 = 0.5 + 0.5 * sin(g.y * 34.0 + h0 * 6.0 + hd * 9.0);
  float stri = mix(0.5, s1, a1) * 0.8 + mix(0.5, s2, a2) * 0.2;

  vec3 c = vec3(0.010, 0.024, 0.036);
  float shade = mix(1.0, 0.8 + 0.2 * fr, sharp);
  // In shadow the shell's light colours (gold, green) would glow brighter than its dark ones: even
  // them out, so every shell's shadow is as quiet.
  float lc = dot(col, vec3(0.2126, 0.7152, 0.0722));
  vec3 colA = col * min(1.0, 0.3 / max(lc, 0.01));
  c += colA * amb * (0.85 + 0.15 * stri * calm) + col * lit * (0.72 + 0.28 * stri) * (0.5 + 0.5 * diff) * shade;
  c += col * col * lit * 0.25;
  // Sheen, and glints along the satin where the folds face the light.
  // The dark organic layers are matt beside the nacre: they take little of it.
  float nacre = smoothstep(0.03, 0.16, lc);
  c += vec3(0.7, 0.9, 0.95) * (pow(spec, 5.0) * 0.20 + pow(spec, 28.0) * smoothstep(0.55, 0.95, stri) * 0.32) * lit * nacre;
  // A window reflected in the lacquer: two narrow panes with hard edges, bent by the folds beneath,
  // added as light so the colour under it stays its own.
  vec2 gd = vec2(cos(u_gloss), sin(u_gloss)), gq = vec2(-gd.y, gd.x);
  vec2 rel = uv - u_light;
  float gp = dot(rel, gd) + 0.42 * dot(n.xy, gd) / max(n.z, 0.2);
  float gs = dot(rel, gq) + 0.42 * dot(n.xy, gq) / max(n.z, 0.2);
  float ew = 1.5 / u_res.y;
  float pane = smoothstep(-0.045 - ew, -0.045, gp) * (1.0 - smoothstep(0.045, 0.045 + ew, gp));
  pane *= 1.0 - (smoothstep(-0.005 - ew, -0.005, gp) * (1.0 - smoothstep(0.005, 0.005 + ew, gp)));
  float reach = exp(-pow(gs / (u_light_r * 0.9), 2.0)) * exp(-pow(length(rel) / (u_light_r * 1.25), 2.0));
  c += vec3(0.62, 0.68, 0.70) * pane * reach * calm * (0.10 + 0.18 * smoothstep(0.3, 0.9, stri)) * (0.4 + 0.6 * nacre);
  // Now and then a warm flash where a fold turns steeply.
  c += vec3(0.55, 0.30, 0.26) * smoothstep(0.10, 0.22, tilt) * lit * u_warm;
  gl_FragColor = vec4(c, 1.0);
}`;

export default {
  name: "paua",
  about: "Pāua shell, close up: its nacre burns with colour where the light falls, each agent's shell its own, and the page sits in its shadow.",
  // A high-contrast didone for a polished, jewel-like ground; Bodoni Moda is free now Nocturne is gone.
  display: { family: "Bodoni Moda", files: ["font-bodoni-moda.woff2"], weights: "400 900", bold: 600, axes: null },
  palette: {
    paper: "#071219", tint: "#0a1820", ink: "#eef3f1", body: "#b7c8c8", muted: "#7f9799", line: "#183039",
    accent: "#4fc1c0", figure: ["#4fc1c0", "#9a8fe0", "#c9b65a", "#d07aa8"], cover_ink: "#f1f5f3", cover_body: "#c3d3d2",
  },
  // The eight shells, each about three round the ring from the one before: blue, gold, purple, teal,
  // copper, indigo, emerald, rose.
  slots: [0, 5 / 8, 2 / 8, 7 / 8, 4 / 8, 1 / 8, 6 / 8, 3 / 8],
  map: { tone: "#1d6a78", duo: 0.5, below: 0.68, chroma: 0.6 },
  identity,
  paint(ctx, { w, h, dpr, at, stream, quiet, kit }) {
    const aspect = w / h;
    // Where the shell grew from and where the light falls: in units of the height, y up, the apex
    // beyond a lower corner, the light low on that side.
    const place = stream("light"), right = place() < 0.5;
    const apex = [right ? aspect + place.range(0.35, 0.7) : -place.range(0.35, 0.7), -place.range(0.3, 0.6)];
    const light = [right ? aspect * place.range(0.66, 0.95) : aspect * place.range(0.05, 0.34), place.range(0.08, 0.26)];
    const lightR = place.range(0.4, 0.56);
    const layers = stream("layers"), gloss = stream("gloss"), flash = stream("flash"), zones = stream("zones");
    // The agent's shell, from its place; its second colouring a shell or so round the ring, so a painting
    // holds two neighbouring hue families, as a real shell does, and never the whole rainbow.
    const s = at, A = shellAt(s), B = shellAt(s + zones.pick([-1, 1]) * zones.range(0.12, 0.22));
    const q = quiet || { x: w * 0.2, y: h * 0.2, w: w * 0.6, h: h * 0.3 };
    const u = {
      u_apex: apex, u_light: light, u_light_r: lightR,
      u_quiet: [q.x / h, (h - q.y - q.h) / h, (q.x + q.w) / h, (h - q.y) / h],
      u_seed: layers.range(0, 100), u_tight: layers.range(2.2, 3.0), u_bands: layers.range(0.75, 1.1), u_hue: layers.range(0, 0.15),
      u_gloss: gloss.range(0.5, 1.1) * (right ? -1 : 1), u_warm: flash() < 0.4 ? flash.range(0.15, 0.3) : 0,
      u_zone: zones.range(0, 100), u_zone_amt: zones.range(0.55, 0.8),
    };
    A.stops.forEach((c, j) => { u[`u_a${j}`] = c; });
    B.stops.forEach((c, j) => { u[`u_b${j}`] = c; });
    // At most about two thirds of a megapixel, scaled up smoothly: the nacre is soft except at the
    // lit terraces' steps, and a phone's paint stays under the design's budget.
    const k = Math.min(dpr, Math.sqrt(0.65e6 / (w * h)));
    const img = kit.shader(FRAG, w * k, h * k, u);
    if (img) ctx.drawImage(img, 0, 0, w, h);
    else {
      // Without WebGL: the same light as a glow of the shell's colours on its shadow.
      ctx.fillStyle = "#08151c"; ctx.fillRect(0, 0, w, h);
      const x = light[0] * h, y = h - light[1] * h, r = lightR * h * 1.6;
      const glow = ctx.createRadialGradient(x, y, 0, x, y, r), [c1, c2] = [A.stops[2], A.stops[1]].map((c) => c.map((v) => Math.round(v * 255)).join(","));
      glow.addColorStop(0, `rgba(${c1},.7)`); glow.addColorStop(0.45, `rgba(${c2},.35)`); glow.addColorStop(1, "rgba(8,21,28,0)");
      ctx.fillStyle = glow; ctx.fillRect(0, 0, w, h);
    }
    kit.grain(ctx, w, h, 0.05, "paua-grain");
  },
};
