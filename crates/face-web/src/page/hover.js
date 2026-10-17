// ─── Hover: the words the map does not show ───────────────────────────────
const tip = $("#tip");
let tipFor = null, tipTimer = 0;
function hoverable(target, id) {
  target.addEventListener("pointerenter", (e) => {
    if (e.pointerType !== "mouse" || drag?.moved) return;
    clearTimeout(tipTimer);
    tipTimer = setTimeout(() => { tipFor = { id, target }; fillTip(); }, tip.hidden ? 300 : 0);
  });
  target.addEventListener("pointerleave", hideTip);
}
function hideTip() { clearTimeout(tipTimer); tipFor = null; tip.hidden = true; }
function fillTip() {
  if (!tipFor || !tipFor.target.isConnected) return hideTip();
  const id = typeof tipFor.id === "function" ? tipFor.id() : tipFor.id;
  const lines = [];
  if (id?.startsWith("b:")) {
    const n = graph.nodes.get(id);
    if (!n) return hideTip();
    lines.push(el("b", null, n.call.tool), el("div", "now", `started ${ago(n.call.made)} ago`));
    const input = gist(n.call.input);
    if (input) lines.push(el("div", "m", input));
  } else {
    const t = byId[id];
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
  tip.replaceChildren(...lines);
  tip.hidden = false;
  const r = tipFor.target.getBoundingClientRect(), w = tip.offsetWidth, h = tip.offsetHeight;
  let y = r.bottom + 8;
  if (y + h > innerHeight - 8) y = r.top - h - 8;
  tip.style.left = `${Math.max(8, Math.min(innerWidth - w - 8, r.left + r.width / 2 - w / 2))}px`;
  tip.style.top = `${Math.max(8, y)}px`;
}

