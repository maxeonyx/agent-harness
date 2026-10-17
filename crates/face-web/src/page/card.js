// ─── Opening a node ────────────────────────────────────────────────────────
// The URL names the open node, so back closes it and a middle click opens a
// node in a tab of its own. Going from one card to another replaces the
// entry, so back always goes to the map.
const panel = $("#panel"), logEl = $(".log", panel), scrollEl = $(".scroll", panel);
let openId = null, saved = null, moving = Promise.resolve(), stay = false;
// The card's id for a chat not started yet.
const NEW = "new";

// Each entry this document pushes is marked as its own: back from it stays
// in this page. Back from one it did not push (the page reloaded on a card)
// would leave it, so closing that card replaces the entry instead.
const DOC = Math.random().toString(36).slice(2);
function show(id) {
  if (location.hash === `#${id}`) return route();
  if (history.state?.node) history.replaceState({ node: id, doc: history.state.doc }, "", `#${id}`);
  else history.pushState({ node: id, doc: DOC }, "", `#${id}`);
  route();
}
function go(id) {
  clearTimeout(closing);
  if (id !== NEW) id = live(id);
  if (id === openId) return;
  show(id);
}
function closeNode() {
  if (!openId) return;
  if (history.state?.node && history.state.doc === DOC) history.back();
  else { history.replaceState(null, "", location.pathname); route(); }
}
addEventListener("popstate", route);
addEventListener("hashchange", route);

function route() {
  const raw = location.hash.slice(1) || null;
  const id = raw && raw !== NEW ? live(raw) : raw;
  const known = id && (byId[id] || (id === NEW && view));
  moving = moving.then(() => transition(known ? id : id && !view ? id : null));
}

async function transition(to) {
  if (to === openId) return;
  if (to && (!view || (!byId[to] && to !== NEW))) { pendingOpen = to; return; }
  const from = openId;
  if (from) leave();
  hideTip();
  if (!to) {
    // Back to the map as it was: framing all of it, or where Max left it.
    const back = saved?.framed ? framing() : saved?.cam;
    framed = !!saved?.framed;
    const card = cardShown, dot = dotRect(from);
    openId = null; seam = null; saved = null;
    await fly(back || null, { T: 200, cardFrom: card, cardTo: card && dot, box: card });
    if (!openId) { panel.hidden = true; panel.style.clipPath = ""; panel.style.transform = ""; cardShown = null; coverCheck(); }
    nodeEls.get(from)?.classList.remove("selected");
    render();
    return;
  }
  // Unfold a finished project to reach a node in it.
  const comp = graph.comps.find((c) => c.tasks.some((t) => t.id === to));
  if (comp?.folded && comp.root.id !== to) { unfolded.add(comp.root.id); await rebuild(); }
  if (!saved) saved = { cam: { ...cam }, framed };
  const was = cardShown, keep = stay && was;
  stay = false;
  openId = to;
  seam = seamFor(to);
  if (!seam) { openId = null; return; }
  const r = cardRect(seam);
  // Opened from the map, the card holds the node where it was on screen; a
  // task made from the open card (a new chat started, a chat continued, an
  // aside) takes the card's place as it is.
  const at = keep ? { x: r.x + r.w / 2, y: r.y, sx: cam.x + (was.x + was.w / 2) * cam.k, sy: cam.y + was.y * cam.k }
    : to === NEW ? { x: newPoint.x, y: newPoint.y - 14, sx: newPoint.sx, sy: newPoint.sy - 14 } : heldAt(to);
  framed = false;
  panel.hidden = false;
  buildPanel(to);
  $("#empty").hidden = true;
  if (from) nodeEls.get(from)?.classList.remove("selected");
  nodeEls.get(to)?.classList.add("selected");
  await fly(camFor(r, at), { T: 260, cardFrom: from && was ? { ...was } : dotRect(to), cardTo: r, box: r });
  if (openId === to) { panel.style.clipPath = ""; panel.style.transform = ""; }
  if (!matchMedia("(pointer: coarse)").matches) $(".compose textarea", panel)?.focus({ preventScroll: true });
}
let pendingOpen = null;
async function fly(to, opts) {
  const f = glide(to, { ...opts, strong: true });
  flight = f;
  await f;
  if (flight === f) flight = null;
}
// What an open card holds that closing it lets go.
function leave() {
  stopFollowing();
  if (dictation) stopDictation();
  logState = null;
  if (title.isContentEditable) endRename(false);
}

// ─── The card: who, how full, what it says, what Max writes ───────────────
let logState = null;
const title = $(".title", panel), mini = $(".mini", panel);
function buildPanel(id) {
  logState = { id, head: undefined, rendered: [], rows: new Map(), group: null, exact: false, follow: true };
  logEl.replaceChildren();
  logEl.classList.remove("exact");
  $(".viewer", panel).hidden = true;
  $(".mix", panel).hidden = true;
  scrollEl.scrollTop = 0;
  panel.classList.toggle("fresh", id === NEW);
  buildCompose(id);
  if (id === NEW) buildNew();
  else renderPanel();
}
$(".x", panel).onclick = closeNode;
hoverable(mini, () => openId);
mini.onclick = refocus;

function renderPanel() {
  const t = byId[openId];
  if (!t) return;
  const l = look(t), needs = (view.queue || []).some((q) => live(q.task) === t.id);
  mini.className = `mini ${l.cls}${needs ? " needs" : ""}`;
  if (!title.isContentEditable && title.textContent !== t.name) title.textContent = t.name;
  title.title = "Double-click to rename";
  const ctx = $(".ctx", panel);
  ctx.hidden = !t.tokens;
  if (t.tokens) {
    const f = fullness(t);
    ctx.className = `ctx${f > .8 ? " high" : ""}`;
    ctx.title = `${kTokens(t.tokens.context)} of ${kTokens(t.window)} context: what it holds`;
    if (!ctx.firstChild) { const i = el("i"); i.append(el("b")); ctx.append(i); }
    $("b", ctx).style.width = `${Math.max(2, Math.round(f * 100))}%`;
  }
  if (!$(".mix", panel).hidden) renderMix(t);
  renderControls(t);
  renderLog(t);
  renderComposeState(t);
}

// The name is renamed where it is: a double click, then Enter.
title.addEventListener("dblclick", () => {
  if (!byId[openId] || !can.act) return;
  title.contentEditable = "plaintext-only";
  title.focus();
  getSelection().selectAllChildren(title);
});
title.addEventListener("keydown", (e) => {
  if (e.key === "Enter") { e.preventDefault(); endRename(true); }
  if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); endRename(false); }
});
title.addEventListener("blur", () => { if (title.isContentEditable) endRename(true); });
async function endRename(keep) {
  const t = byId[openId], name = title.textContent.trim();
  title.removeAttribute("contenteditable");
  if (!t || !keep || !name || name === t.name) { title.textContent = t?.name || ""; return; }
  const why = await act({ Task: [t.id, { Renamed: name }] });
  if (why) { title.textContent = t.name; say(`not renamed: ${why}`, true); }
}

// How full a task's context was at its last request, of its model's window.
const fullness = (t) => t.tokens && t.window ? Math.min(1, t.tokens.context / t.window) : 0;

// What a context holds: its last request's token count, split by what each
// part of the context is, in proportion to its size (an estimate: the
// provider counts only the whole).
const MIX = [
  ["system", "System prompt", "#141413"], ["tools", "Tool definitions", "#6b6a64"],
  ["mandate", "Mandate and brief", "#a19f96"], ["max", "Your words", "#7a1f22"],
  ["notes", "Harness notes", "#c9c4b4"], ["said", "Its words", "#4f6d8a"],
  ["calls", "Its tool calls", "#8aa1b8"], ["results", "Tool results", "#6c8f5e"],
];
function mixOf(chain) {
  const m = Object.fromEntries(MIX.map(([k]) => [k, 0]));
  const size = (b) => b.type === "image" ? 6000 : JSON.stringify(b).length;
  for (const e of chain) {
    if (e.role === "root") { m.system += (e.system || "").length; m.tools += (e.tools || "").length; continue; }
    blocksOf(e).forEach((b, k) => {
      const who = e.role === "user" ? (e.by || [])[k]?.by || "harness" : null;
      const into = b.type === "tool_result" ? "results" : b.type === "tool_use" ? "calls"
        : b.type === "thinking" || b.type === "redacted_thinking" ? null
        : e.role === "assistant" ? "said" : who === "max" || who === "prompt" ? "max"
        : who === "mandate" || who === "brief" ? "mandate" : "notes";
      if (into) m[into] += size(b);
    });
  }
  return m;
}
async function renderMix(t) {
  const box = $(".mix", panel);
  const key = `${t.id}:${t.head}:${t.tokens?.context}:${logState?.exact}`;
  if (box.dataset.key === key) return;
  box.dataset.key = key;
  const chain = t.head ? await contextOf(t.head) : null;
  if (!chain || box.dataset.key !== key) return;
  const m = mixOf(chain), total = Object.values(m).reduce((a, b) => a + b, 0) || 1, tokens = t.tokens?.context || 0;
  const stack = el("div", "stack"), table = el("table");
  for (const [k, label, colour] of MIX) {
    if (!m[k]) continue;
    const b = el("b"); b.style.width = `${(m[k] / total) * 100}%`; b.style.background = colour; b.title = label; stack.append(b);
    const sw = el("i"); sw.style.background = colour;
    const tr = el("tr"), name = el("td"); name.append(sw, label);
    tr.append(name, el("td", "n", `≈${kTokens(Math.round(tokens * m[k] / total))}`), el("td", "n", `${Math.round(m[k] / total * 100)}%`));
    table.append(tr);
  }
  const head = el("div", null, `${kTokens(tokens)} of ${kTokens(t.window)} (${Math.round(fullness(t) * 100)}%) at its last request · ${t.tokens?.output ?? "?"} out`);
  const exact = el("button", `btn${logState?.exact ? " armed" : ""}`, "As sent");
  exact.type = "button";
  exact.title = "The log as the exact context the model was sent";
  exact.onclick = () => {
    const id = logState.id, on = !logState.exact;
    logState = { id, head: undefined, rendered: [], rows: new Map(), group: null, exact: on, follow: true };
    logEl.classList.toggle("exact", on);
    logEl.replaceChildren();
    renderPanel();
  };
  box.replaceChildren(head, stack, table, exact);
}
$(".ctx", panel).onclick = () => { const m = $(".mix", panel); m.hidden = !m.hidden; m.dataset.key = ""; if (!m.hidden && byId[openId]) renderMix(byId[openId]); };

// What Max can do to the task itself, quietly under the box: pause it and
// what is under it, cancel it and what it started, and, in a chat, say it
// is done. A cancel asks once more; a second while it tidies up ends it now.
function renderControls(t) {
  const row = $(".acts", compose);
  if (!row) return;
  const holder = heldBy(t);
  const key = `${t.id}:${isDone(t)}:${holder?.id}:${t.mode}:${!!t.stop}:${t.stop?.hard}:${can.act}`;
  if (row.dataset.key === key) return;
  row.dataset.key = key;
  row.replaceChildren();
  if (!can.act || isDone(t)) return;
  const action = (label, tip, fact, confirm) => {
    const b = el("button", null, label);
    b.type = "button";
    b.title = tip;
    b.onclick = async () => {
      if (confirm && !b.classList.contains("armed")) { b.classList.add("armed"); b.textContent = `${label}?`; setTimeout(() => { b.classList.remove("armed"); b.textContent = label; }, 4000); return; }
      b.classList.remove("armed"); b.textContent = label;
      const why = await act(fact);
      if (why) say(`${label}: ${why}`, true);
    };
    row.append(b);
  };
  if (holder) action("Resume", holder === t ? "It and everything under it carry on" : `Paused with ${holder.name}: it and everything under it carry on`, () => ({ Task: [holder.id, { Held: false }] }));
  else action("Pause", "It and everything under it finish what is running, then wait", () => ({ Task: [t.id, { Held: true }] }));
  if (!t.stop) action("Cancel", "It and everything it started stop, tidy up and report", () => ({ Task: [t.id, { Drain: { Stop: { by: "Max", within: 120000 } } }] }), true);
  else if (!t.stop.hard) action("Cancel now", "End it and everything it started now, with no tidy-up", () => ({ Task: [t.id, { Drain: { Stop: { by: "Max", within: 0 } } }] }), true);
  if (t.mode === "user-facing") action("Done", "It writes its result and finishes", (at) => ({ Delivery: { id: `${at}.0`, to: t.id, content: { Prompt: "Done" } } }));
}

// A task Max starts from what he wrote: its mandate his words, what he
// attached delivered with them. A new chat, a finished chat continued, or
// an aside: a fork of a task waiting on agents it started, to talk with
// while they work, as one more of them. What the record creates is named
// by its time and the fact that creates it; `how` may make facts of its
// own first (a new chat's limbs), and `after` rides along.
async function start(how, text, files, after = []) {
  const mandate = digest(text), words = files.length ? JSON.stringify(files.map((a) => a.block)) : null;
  let id;
  const why = await act((at) => {
    const out = [], next = () => `${at}.${out.length}`;
    const h = typeof how === "function" ? how(next, out) : how;
    id = next();
    out.push({ Task: [id, { Created: {
      name: h.name, by: null, scope: h.scope, origin: h.origin,
      mode: "UserFacing", mandate: { text: mandate, brief: null, quotes: [] },
    } }] });
    if (words) out.push({ Delivery: { id: next(), to: id, content: { Words: digest(words) }, read: "Next" } });
    return [...out, ...after];
  }, words ? [text, words] : [text]);
  if (!why) { pendingOpen = id; stay = true; }
  return why;
}
const aside = (t, text, files, after) => start({ name: `${t.name}-aside`, scope: { Spawn: t.scoped }, origin: { Aside: { from: t.id } } }, text, files, after);
const resume = (t, text, files, after) => start({ name: t.name, scope: "Max", origin: { Resume: { from: t.id } } }, text, files, after);

