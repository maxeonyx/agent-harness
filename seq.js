// Renders a sequence diagram into an SVG: one lifeline per actor, one row per step.
// sequence(el, { actors: [{id, label}], steps: [{from, to, text, seq?, tip?, style?} | {group: "label", steps: [...]}] })
// A step with from === to is a note on that lifeline. `seq` marks a journal record, drawn as a badge at the left.
function sequence(el, spec) {
  const NS = "http://www.w3.org/2000/svg";
  const colW = spec.colW ?? 190, left = 64, top = 44, rowH = 34;
  const x = new Map(spec.actors.map((a, i) => [a.id, left + colW / 2 + i * colW]));
  const rows = [];
  const flat = (steps, depth) => {
    for (const s of steps) {
      if (s.group) { rows.push({ groupStart: s.group, depth }); flat(s.steps, depth + 1); rows.push({ groupEnd: true, depth }); }
      else rows.push({ ...s, depth });
    }
  };
  flat(spec.steps, 0);
  const W = left + colW * spec.actors.length + 10, H = top + rows.length * rowH + 30;
  const svg = document.createElementNS(NS, "svg");
  svg.setAttribute("viewBox", `0 0 ${W} ${H}`); svg.setAttribute("width", W); svg.classList.add("dg", "sq");
  const add = (tag, attrs, text) => {
    const e = document.createElementNS(NS, tag);
    for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
    if (text != null) e.textContent = text;
    svg.append(e); return e;
  };
  add("defs", {}).innerHTML = `<marker id="sq-${el.id}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto"><path d="M0,0 L10,5 L0,10 z" class="arrowhead"/></marker>`;
  for (const a of spec.actors) {
    add("line", { x1: x.get(a.id), x2: x.get(a.id), y1: top - 6, y2: H - 10, class: "life" });
    add("rect", { x: x.get(a.id) - colW / 2 + 8, y: 6, width: colW - 16, height: 28, rx: 6, class: "leaf actor" });
    add("text", { x: x.get(a.id), y: 25, "text-anchor": "middle", class: "nlabel" }, a.label);
  }
  const tip = document.createElement("div");
  tip.className = "dgtip";
  const opens = [];
  rows.forEach((r, i) => {
    const y = top + i * rowH + rowH / 2;
    if (r.groupStart) { opens.push({ y: y - rowH / 2 + 4, label: r.groupStart, depth: r.depth }); return; }
    if (r.groupEnd) {
      const o = opens.pop();
      const g = add("rect", { x: left + 4 + o.depth * 6, y: o.y, width: W - left - 14 - o.depth * 12, height: y - o.y, rx: 6, class: "sqgroup" });
      svg.insertBefore(g, svg.children[1]);
      add("text", { x: left + 12 + o.depth * 6, y: o.y + 13, class: "glabel" }, o.label);
      return;
    }
    const g = document.createElementNS(NS, "g"); g.classList.add("step"); svg.append(g);
    const put = (tag, attrs, text) => { const e = document.createElementNS(NS, tag); for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v); if (text != null) e.textContent = text; g.append(e); return e; };
    put("rect", { x: 0, y: y - rowH / 2, width: W, height: rowH, class: "hitrow" });
    if (r.seq != null) {
      put("rect", { x: 6, y: y - 10, width: 46, height: 20, rx: 10, class: "badge" });
      put("text", { x: 29, y: y + 4, "text-anchor": "middle", class: "badgetext" }, "#" + r.seq);
    }
    const x1 = x.get(r.from), x2 = x.get(r.to);
    if (r.from === r.to) {
      const t = put("text", { x: x1 + 8, y: y + 4, class: "sqnote" }, r.text);
      put("rect", { x: x1 - 3, y: y - 8, width: 6, height: 16, rx: 2, class: "act" });
      void t;
    } else {
      put("line", { x1, x2: x2 + (x2 > x1 ? -2 : 2), y1: y + 6, y2: y + 6, class: "edge msg " + (r.style ?? ""), "marker-end": `url(#sq-${el.id})` });
      put("text", { x: Math.min(x1, x2) + 6, y: y, class: "sqlabel" }, r.text);
    }
    const say = () => { svg.querySelectorAll(".step.on").forEach((s) => s.classList.remove("on")); g.classList.add("on"); tip.textContent = (r.seq != null ? `#${r.seq} ` : "") + r.text + (r.tip ? ": " + r.tip : ""); };
    g.addEventListener("mouseenter", say); g.addEventListener("click", say);
  });
  const wrap = document.createElement("div"); wrap.className = "dgwrap"; wrap.append(svg);
  el.replaceChildren(wrap, tip);
  fit(svg, W);
}
