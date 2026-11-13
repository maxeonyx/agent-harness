// The map's lanes checked over a whole view: every project laid out as the
// page lays it out, folded ones too, and every crossing of two lives' lines
// that no routing could avoid told from one it could. Fails on a crossing
// that is not forced, and on a line passing through a node not its own.
//
//   curl -sk https://localhost:PORT/view > view.json
//   node dev/lanes-check.mjs view.json
//
// The page's modules run here with just enough of a document to load:
// labels are measured at 7 px a letter.
const stub = () => new Proxy(function () {}, { get: (_, k) => (k === Symbol.toPrimitive ? () => "" : k === "measureText" ? (t) => ({ width: 7 * t.length }) : stub()), apply: () => stub() });
Object.assign(globalThis, { document: stub(), getComputedStyle: () => stub(), window: globalThis });
const fs = await import("node:fs");
const map = await import("../crates/face-web/src/page/map.js");
const { lanes, setVariant } = await import("../crates/face-web/src/page/layout.js");
// SPIKE: VARIANT=name picks layout.js's variant; width is summed over projects.
if (process.env.VARIANT) setVariant(process.env.VARIANT);

map.takeView(JSON.parse(fs.readFileSync(process.argv[2], "utf8")));
map.derive(null);
let forced = 0, bad = 0, lines = 0, width = 0;
for (const c of map.graph.comps) {
  const ns = map.nodesOf(c), l = lanes(ns), by = new Map(ns.map((n) => [n.id, n]));
  lines += l.edges.length;
  const xs = [...l.at.values()].map((p) => p.x);
  width += Math.max(...xs) - Math.min(...xs);
  const name = c.root.name;
  for (const x of l.cuts) {
    if (x.forced) { forced++; continue; }
    bad++;
    const [a, b] = [l.edges[x.over], l.edges[x.under]];
    console.log(`${name}: unforced crossing at ${x.at.map(Math.round)}: ${a.kind} ${a.from}>${a.to} over ${b.kind} ${b.from}>${b.to}`);
  }
  // A line through a node: within the node's radius and half the line's
  // width of its centre, at a node it doesn't end at.
  for (const e of l.edges) {
    for (const [id, p] of l.at) {
      if (id === e.from || id === e.to) continue;
      const n = by.get(id), reach = (n.big ? 13 : n.r) + 1.1;
      for (let i = 1; i < e.P.length; i++) {
        if (dist(p, e.P[i - 1], e.P[i]) < reach) { bad++; console.log(`${name}: ${e.kind} ${e.from}>${e.to} runs through ${id}`); break; }
      }
    }
  }
}
console.log(`${map.graph.comps.length} projects, ${lines} lines: ${forced} forced crossings, ${bad} faults, width ${Math.round(width)}`);
process.exit(bad ? 1 : 0);

function dist(p, a, b) {
  const vx = b[0] - a[0], vy = b[1] - a[1], L = vx * vx + vy * vy || 1;
  const t = Math.max(0, Math.min(1, ((p.x - a[0]) * vx + (p.y - a[1]) * vy) / L));
  return Math.hypot(a[0] + t * vx - p.x, a[1] + t * vy - p.y);
}
