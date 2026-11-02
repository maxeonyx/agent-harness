// Frame-to-frame jitter of a hollow blob's dash ends, as drawn, against the
// true outline. Usage: node measure.mjs PAGE_DIR
// Loads blobs.js's internals, wobbles a "streaming" blob's satellites' gaps
// by WOB at 60 fps for 4 s, and at several on-screen sizes measures where
// each dash visibly starts and ends (its round caps included), as arc length
// along a finely ray-traced true outline, minus where it should (d/16 and
// (d + DUTY)/16 of the way round from twelve o'clock).
import { readFileSync } from "node:fs";
const dir = process.argv[2];
const src = readFileSync(dir + "/blobs.js", "utf8").replace(/^export /gm, "");
const B = new Function(src + "\nreturn { ball, fieldAt, longestLoop, TN, SPAN, EDGE, DASHES, DUTY, typeof_lay: typeof layDashes, lay: typeof layDashes === 'function' ? layDashes : null, placeDashes: typeof placeDashes === 'function' ? placeDashes : null, dashAt, walk: typeof walk === 'function' ? walk : null, outline: typeof outline === 'function' ? outline : null, line: typeof line !== 'undefined' ? line : null };")();
const { ball, fieldAt, SPAN, EDGE, DASHES, DUTY } = B;

// Truth: 4000 rays from the centre, bisected.
const RAYS = 4000, tp = new Float64Array(2 * (RAYS + 1)), tl = new Float64Array(RAYS + 1);
function truth() {
  for (let i = 0; i <= RAYS; i++) {
    const a = -Math.PI / 2 + i / RAYS * 2 * Math.PI, dx = Math.cos(a), dy = Math.sin(a);
    let lo = 0, hi = SPAN;
    for (let j = 0; j < 40; j++) { const m = (lo + hi) / 2; fieldAt(dx * m, dy * m) < 0 ? lo = m : hi = m; }
    tp[2 * i] = dx * lo; tp[2 * i + 1] = dy * lo;
    if (i) tl[i] = tl[i - 1] + Math.hypot(tp[2 * i] - tp[2 * i - 2], tp[2 * i + 1] - tp[2 * i - 1]);
  }
  return tl[RAYS];
}
// Arc length (from twelve o'clock, clockwise on screen) of the outline point at the angle of (x, y).
function arcAt(x, y) {
  let a = Math.atan2(y, x) + Math.PI / 2; a = ((a % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI);
  const f = a / (2 * Math.PI) * RAYS, i = Math.min(RAYS - 1, Math.floor(f));
  return tl[i] + (tl[i + 1] - tl[i]) * (f - i);
}

// Before: the layer's dashes() as it was, segments in device px about the centre.
function drawnBefore(unit, dpr) {
  const m = B.longestLoop(B.TN, 0), placed = m ? B.placeDashes(m) : 0, O = B.dashAt, out = [];
  const w = Math.max(EDGE * unit, dpr);
  let cur = null;
  for (let d = 0; d < placed; d++) {
    const x0 = O[5 * d] * unit, y0 = O[5 * d + 1] * unit, x1 = O[5 * d + 2] * unit, y1 = O[5 * d + 3] * unit;
    const len = Math.hypot(x1 - x0, y1 - y0), ends = O[5 * d + 4];
    const cut = Math.min(w / 2, ends === 3 ? len / 2 : len) / (len || 1), c0 = ends & 1 ? cut : 0, c1 = ends & 2 ? cut : 0;
    if (ends & 1) out.push(cur = []);
    cur.push([x0 + (x1 - x0) * c0, y0 + (y1 - y0) * c0, x1 - (x1 - x0) * c1, y1 - (y1 - y0) * c1]);
  }
  return { dashes: out, w };
}
// After: whatever the page exposes; set below once it exists.
let drawnAfter = null;
if (B.lay) drawnAfter = (unit, dpr) => {
  const w = Math.max(EDGE * unit, dpr);
  const m = B.longestLoop(B.TN, 0);
  const np = B.outline(m);
  const L = B.line, out = [];
  const n = B.lay(L, np, w / 2 / unit);
  let cur = null;
  for (let k = 0; k < n; k++) {
    const o = 4 * k, D = B.dashAt;
    // A dash's segments join end to start; a gap starts the next dash.
    if (!k || D[o] !== D[o - 2] || D[o + 1] !== D[o - 1]) out.push(cur = []);
    cur.push([D[o] * unit, D[o + 1] * unit, D[o + 2] * unit, D[o + 3] * unit]);
  }
  return { dashes: out, w };
};

// Visible ends of a dash: its first and last points pushed out by half the width along the line.
function ends(segs, w) {
  const first = segs.find((s) => Math.hypot(s[2] - s[0], s[3] - s[1]) > 1e-9) || segs[0];
  const last = [...segs].reverse().find((s) => Math.hypot(s[2] - s[0], s[3] - s[1]) > 1e-9) || segs.at(-1);
  const s0 = segs[0], s1 = segs.at(-1);
  const u = (s) => { const l = Math.hypot(s[2] - s[0], s[3] - s[1]); return l > 1e-9 ? [(s[2] - s[0]) / l, (s[3] - s[1]) / l] : null; };
  const u0 = u(first), u1 = u(last);
  // A dash that is one dot: its caps reach w/2 either side along the outline.
  return { a: [s0[0], s0[1], u0], b: [s1[2], s1[3], u1] };
}

const SIZES = [[20, 3], [32, 3], [48, 3], [64, 3], [120, 3], [64, 1]]; // css px, dpr
function run(drawn, label) {
  console.log(`\n${label}: per size, error of each visible dash end vs where it should be, device px`);
  console.log("css px  dpr | mean |err|  max |err| | frame-to-frame |Δerr| mean  p99   max | dash length min..max");
  for (const [css, dpr] of SIZES) {
    const size = css * dpr, unit = size / (2 * SPAN);
    let prev = null, sumE = 0, maxE = 0, nE = 0, dj = [], lmin = Infinity, lmax = 0;
    for (let f = 0; f < 240; f++) {
      const t = f / 60;
      // streaming: centre .3, the model active (r .42, gap -.05), tools and Max idle (r .17), every gap wobbling by WOB.
      const R = [.3, .42, .17, .17], G = [0, -.05, 0, 0], ANG = [0, -Math.PI / 2, Math.PI / 6, 5 * Math.PI / 6];
      ball[0] = ball[1] = 0; ball[2] = R[0];
      for (let k = 1; k < 4; k++) {
        const g = G[k] - .025 * (.5 + .5 * Math.sin(2 * Math.PI * (t / 1.3 + k * .31))), d = R[0] + R[k] + g;
        ball[3 * k] = Math.cos(ANG[k]) * d; ball[3 * k + 1] = Math.sin(ANG[k]) * d; ball[3 * k + 2] = R[k];
      }
      const S = truth() * unit;
      const { dashes, w } = drawn(unit, dpr);
      const errs = [];
      dashes.forEach((segs, d) => {
        const { a, b } = ends(segs, w);
        // Arc position of each visible end: the point's arc, less or plus half the width.
        const sa = arcAt(a[0], a[1]) * unit - w / 2, sb = arcAt(b[0], b[1]) * unit + w / 2;
        const ea = sa - d * S / DASHES, eb = sb - (d + DUTY) * S / DASHES;
        errs.push(ea, eb);
        lmin = Math.min(lmin, sb - sa); lmax = Math.max(lmax, sb - sa);
      });
      if (process.env.DEBUG && label === "after" && css === +process.env.DEBUG) errs.forEach((e, k) => Math.abs(e) > .3 && console.log("frame", f, "dash", k >> 1, k & 1 ? "end" : "start", e.toFixed(2), "segs", dashes[k >> 1].length));
      for (const e of errs) { sumE += Math.abs(e); maxE = Math.max(maxE, Math.abs(e)); nE++; }
      if (prev && prev.length === errs.length) errs.forEach((e, k) => dj.push(Math.abs(e - prev[k])));
      prev = errs;
    }
    dj.sort((x, y) => x - y);
    const mean = dj.reduce((x, y) => x + y, 0) / dj.length;
    console.log(`${String(css).padStart(6)} ${String(dpr).padStart(4)} | ${(sumE / nE).toFixed(3).padStart(9)} ${maxE.toFixed(3).padStart(10)} | ${mean.toFixed(3).padStart(30)} ${dj[Math.floor(dj.length * .99)].toFixed(3).padStart(5)} ${dj.at(-1).toFixed(3).padStart(5)} | ${lmin.toFixed(2)}..${lmax.toFixed(2)}`);
  }
}
if (B.placeDashes) run(drawnBefore, "before (grid-cut ends)");
if (drawnAfter) run(drawnAfter, "after");
// Cost of a retrace: the per-frame work while a planned blob wobbles.
{
  const t0 = performance.now(); let n = 0;
  for (let r = 0; r < 2000; r++) { ball[2] = .3 + r * 1e-6; const m = B.longestLoop(B.TN, 0); n += B.placeDashes ? B.placeDashes(m) : B.outline(m); }
  console.log(`\nretrace: ${((performance.now() - t0) / 2000).toFixed(4)} ms`);
  if (B.lay) {
    const m = B.longestLoop(B.TN, 0), np = B.outline(m), t1 = performance.now();
    for (let r = 0; r < 20000; r++) n += B.lay(B.line, np, .02 + r * 1e-7);
    console.log(`lay dashes on the traced outline: ${((performance.now() - t1) / 20000).toFixed(4)} ms`);
  }
}
