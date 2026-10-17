// ─── The preview: the words the map does not show, and what Max can do ────
// A short hover on a node opens it beside the node: its words, which he can
// select, the task it belongs to, and its actions. It stays open while the
// pointer is on the node, on it, or in the triangle between where the
// pointer left the node and its two nearest corners, so he can travel to
// it; a moment after the pointer is in none of them, it closes. A right
// click (a long press, on touch) opens it pinned at the pointer, its
// actions as rows, until Escape or a click elsewhere.
const tip = $("#tip");
let tipFor = null, tipTimer = 0, tipClosing = 0, leftAt = null;
function hoverable(target, id) {
  target.style.webkitTouchCallout = "none";
  target.addEventListener("pointerenter", (e) => {
    if (e.pointerType !== "mouse" || drag?.moved || tipFor?.pinned) return;
    clearTimeout(tipTimer);
    const travelling = tipFor && tipFor.target !== target && inTriangle(e.clientX, e.clientY);
    tipTimer = setTimeout(() => openTip({ id, target }), tip.hidden || travelling ? 300 : 0);
  });
  target.addEventListener("pointerleave", (e) => {
    clearTimeout(tipTimer);
    if (tipFor?.target === target && !tipFor.pinned) leftAt = { x: e.clientX, y: e.clientY };
  });
  target.addEventListener("contextmenu", (e) => { e.preventDefault(); openTip({ id, target, pinned: { x: e.clientX, y: e.clientY } }); });
  // A long press on touch, where the browser sends no contextmenu of its own.
  let press = null;
  target.addEventListener("pointerdown", (e) => {
    if (e.pointerType !== "touch") return;
    const at = { x: e.clientX, y: e.clientY };
    press = { at, timer: setTimeout(() => { press.fired = true; openTip({ id, target, pinned: at }); }, 550) };
  });
  target.addEventListener("pointermove", (e) => { if (press && Math.hypot(e.clientX - press.at.x, e.clientY - press.at.y) > 8) { clearTimeout(press.timer); press = null; } });
  const lift = () => { if (press) clearTimeout(press.timer); };
  target.addEventListener("pointerup", lift);
  target.addEventListener("pointercancel", lift);
  // The click that ends a long press is not a click.
  target.addEventListener("click", (e) => { if (press?.fired) { e.preventDefault(); e.stopImmediatePropagation(); } press = null; }, true);
}
function openTip(f) {
  tipFor = f; leftAt = null;
  clearTimeout(tipClosing); tipClosing = 0;
  tip.replaceChildren();
  tip.classList.toggle("pinned", !!f.pinned);
  fillTip();
  if (tipFor === f) placeTip();
}
function hideTip() { clearTimeout(tipTimer); clearTimeout(tipClosing); tipClosing = 0; tipFor = null; leftAt = null; tip.hidden = true; }

// Is the point in the triangle from where the pointer left the node to the
// preview's two corners nearest it?
function inTriangle(x, y) {
  if (!leftAt || tip.hidden) return false;
  const r = tip.getBoundingClientRect(), p = leftAt;
  const [a, b] = p.y <= r.top ? [[r.left, r.top], [r.right, r.top]] : p.y >= r.bottom ? [[r.left, r.bottom], [r.right, r.bottom]]
    : p.x <= r.left ? [[r.left, r.top], [r.left, r.bottom]] : [[r.right, r.top], [r.right, r.bottom]];
  const side = (q1, q2) => (x - q2[0]) * (q1[1] - q2[1]) - (q1[0] - q2[0]) * (y - q2[1]);
  const o = [p.x, p.y], d1 = side(o, a), d2 = side(a, b), d3 = side(b, o);
  return !((d1 < 0 || d2 < 0 || d3 < 0) && (d1 > 0 || d2 > 0 || d3 > 0));
}
const within = (node, x, y) => { const r = node.getBoundingClientRect(); return x >= r.left && x <= r.right && y >= r.top && y <= r.bottom; };
addEventListener("pointermove", (e) => {
  if (!tipFor || tipFor.pinned || tip.hidden || e.pointerType !== "mouse") return;
  const x = e.clientX, y = e.clientY;
  if (within(tip, x, y) || (tipFor.target.isConnected && within(tipFor.target, x, y)) || inTriangle(x, y)) {
    clearTimeout(tipClosing); tipClosing = 0;
    if (within(tip, x, y)) leftAt = null;
  } else if (!tipClosing) tipClosing = setTimeout(hideTip, 200);
});
addEventListener("pointerdown", (e) => { if (tipFor?.pinned && !tip.contains(e.target)) hideTip(); }, true);
addEventListener("keydown", (e) => { if (e.key === "Escape" && tipFor && !tip.hidden) { e.stopPropagation(); hideTip(); } }, true);

// Its words, drawn again as they change; a line that has not changed is
// left as it is, so what Max selected in it stays selected.
function fillTip() {
  if (!tipFor || !tipFor.target.isConnected) return hideTip();
  const id = typeof tipFor.id === "function" ? tipFor.id() : tipFor.id;
  const lines = [];
  let t = null;
  if (id?.startsWith("b:")) {
    const n = graph.nodes.get(id);
    if (!n) return hideTip();
    const c = n.call;
    lines.push(el("b", null, c.name ? `${c.name}: ${c.tool}` : c.tool), el("div", "now", `started ${ago(c.made)} ago`));
    if (c.owner === "max" && c.passed) lines.push(el("div", null, `Passed to you by ${byId[c.passed.by]?.name || "an agent"}: ${c.passed.why}`));
    const input = gist(c.input);
    if (input) lines.push(el("div", "m", input));
  } else {
    t = byId[id];
    if (!t) return hideTip();
    const fc = graph.comps.find((c) => c.folded && c.root.id === id);
    lines.push(el("b", null, t.name), el("div", "now", fc ? cardStatus(fc) : trouble(t) || look(t).text));
    const place = Object.values(t.places || {})[0];
    const mode = t.mode === "user-facing" ? "chat" : t.mode === "standing" ? "standing" : null;
    const what = [t.environment, mode !== t.environment && mode].filter(Boolean).join(" ");
    lines.push(el("div", null, [what, place && `in ${place.split("/").filter(Boolean).slice(-2).join("/")}`].filter(Boolean).join(" · ")));
    if (t.tokens && !isDone(t)) lines.push(el("div", null, `${kTokens(t.tokens.context)} of ${kTokens(t.window)} context`));
    if (t.mandate && !fc) lines.push(el("div", "m", t.mandate.slice(0, 240)));
  }
  const words = tip.querySelector(".words") || tip.appendChild(el("div", "words"));
  lines.forEach((line, i) => {
    const old = words.children[i];
    if (!old) words.append(line);
    else if (old.tagName !== line.tagName || old.className !== line.className || old.textContent !== line.textContent) old.replaceWith(line);
  });
  while (words.children.length > lines.length) words.lastChild.remove();
  if (t) fillTipActions(t);
  else fillCallActions(graph.nodes.get(id).call);
  tip.hidden = false;
}
// The task it belongs to, its actions, and Open: drawn again only when
// they change, so an armed action stays armed.
function fillTipActions(t) {
  const parent = byId[parentOf(t)], list = actionsOf(t), pinned = !!tipFor.pinned;
  const key = `${t.id}:${parent?.id}:${parent?.name}:${pinned}:${actionsKey(list)}`;
  let foot = tip.querySelector(".foot");
  if (foot?.dataset.key === key) return;
  if (!foot) tip.append(foot = el("div", "foot"));
  foot.dataset.key = key;
  const why = el("div", "why");
  const report = (text) => { why.textContent = text; };
  const open = el("button", `open${pinned ? " row" : ""}`, "Open");
  open.type = "button";
  open.onclick = () => { hideTip(); go(t.id); };
  const acts = el("div", "acts");
  acts.append(...list.map((a) => actionButton(a, pinned, report)), open);
  const up = parent && el("button", "up", `↑ ${parent.name}`);
  if (up) { up.type = "button"; up.title = `Open ${parent.name}`; up.onclick = () => { hideTip(); go(parent.id); }; }
  foot.replaceChildren(...[up, acts, why].filter(Boolean));
}
// Background work's actions, drawn again only when they change.
function fillCallActions(call) {
  const list = callActionsOf(call), pinned = !!tipFor.pinned;
  const key = `${call.id}:${pinned}:${actionsKey(list)}`;
  let foot = tip.querySelector(".foot");
  if (foot?.dataset.key === key) return;
  if (!foot) tip.append(foot = el("div", "foot"));
  foot.dataset.key = key;
  const why = el("div", "why");
  const acts = el("div", "acts");
  acts.append(...list.map((a) => actionButton(a, pinned, (text) => { why.textContent = text; })));
  foot.replaceChildren(acts, why);
}
// Beside the node, below it if there is room; pinned, at the pointer.
function placeTip() {
  const w = tip.offsetWidth, h = tip.offsetHeight, f = tipFor;
  let x, y;
  if (f.pinned) {
    x = f.pinned.x; y = f.pinned.y;
    if (x + w > innerWidth - 8) x -= w;
    if (y + h > innerHeight - 8) y -= h;
  } else {
    const r = f.target.getBoundingClientRect();
    y = r.bottom + 8;
    if (y + h > innerHeight - 8) y = r.top - h - 8;
    x = r.left + r.width / 2 - w / 2;
  }
  tip.style.left = `${Math.max(8, Math.min(innerWidth - w - 8, x))}px`;
  tip.style.top = `${Math.max(8, Math.min(innerHeight - h - 8, y))}px`;
}
