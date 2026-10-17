// ─── A new chat, in Max's own scope ───────────────────────────────────────
// A double click on the ground, or +, opens a small card there on a chat
// that does not exist yet: the same staging box as every chat, its role and
// directory beside the send, its draft and attachments kept like theirs.
// Sending starts it: the words are its mandate, what is attached reaches it
// with them, and the card becomes the chat where it is.
function buildNew() {
  title.textContent = "New chat";
  title.title = "";
  mini.className = "mini starting";
  $(".ctx", panel).hidden = true;
  const picks = $(".picks", compose);
  if (!picks) return;
  const role = el("select", "pick"); role.id = "n-role"; role.title = "Its role";
  role.append(...view.environments.map((e) => Object.assign(el("option", null, e.name), { value: e.name })));
  role.value = drafts.get("new:role") || (view.environments.some((e) => e.name === "chat") ? "chat" : view.environments[0]?.name);
  role.hidden = view.environments.length < 2;
  const dir = el("input", "pick"); dir.id = "n-dir"; dir.placeholder = "directory"; dir.title = "Where it works";
  dir.value = drafts.get("new:dir");
  dir.setAttribute("list", "n-dirs");
  const dirs = el("datalist"); dirs.id = "n-dirs";
  dirs.append(...view.workspaces.map((w) => Object.assign(el("option"), { value: w.dir })));
  const needs = () => { dir.hidden = !view.environments.find((e) => e.name === role.value)?.places.length; };
  role.onchange = () => { drafts.set("new:role", role.value); needs(); };
  dir.oninput = () => drafts.set("new:dir", dir.value);
  needs();
  picks.replaceChildren(role, dir, dirs);
}
// A name from the first words of a mandate, to its first clause's end.
const nameFrom = (text) => {
  const words = text.trim().split(/[\n.,;:!?]/)[0].split(/\s+/).slice(0, 6).join(" ");
  return words.length > 40 ? `${words.slice(0, 39)}…` : words;
};
async function startNew(text, files) {
  const env = view.environments.find((e) => e.name === $("#n-role").value);
  const dir = $("#n-dir").value.trim();
  if (!env) return say("choose a role", true);
  if (env.places.length && !dir) return say(`it needs a directory for its ${env.places[0]}`, true);
  if (!text.trim()) return say("write what it is for", true);
  takeWords(NEW);
  const why = await start((next, out) => {
    const places = {};
    for (const place of env.places) {
      const known = view.workspaces.find((w) => w.dir === dir);
      const limb = known ? known.id : next();
      if (!known) out.push({ Limb: { id: limb, name: dir, kind: { Plugin: { kind: place, config: JSON.stringify({ dir }) } } } });
      places[place] = limb;
    }
    return { name: nameFrom(text), scope: "Max", origin: { Fresh: { environment: env.id, places } } };
  }, text, files);
  if (why) { giveBack(NEW, text, files); return say(`not started: ${why}`, true); }
}

// ─── Finding a chat ────────────────────────────────────────────────────────
// Ctrl+K, or /: what matches by its name or what it was for stays, the rest
// of the map fades; Enter opens the newest match, Esc lets it go.
const findEl = $("#find");
let finding = "", found = new Set();
function startFind() { findEl.hidden = false; findEl.focus(); findEl.select(); }
function endFind() { findEl.hidden = true; findEl.value = ""; finding = ""; found = new Set(); findEl.blur(); if (view) drawMap(); }
findEl.addEventListener("input", () => {
  finding = findEl.value.trim().toLowerCase();
  found = new Set();
  if (finding) {
    for (const t of view.tasks) if (`${t.name}\n${t.mandate || ""}`.toLowerCase().includes(finding)) found.add(live(t.id));
    // A folded project matches through what it folds.
    for (const c of graph.comps) if (c.folded && c.tasks.some((t) => found.has(t.id))) found.add(c.root.id);
  }
  drawMap();
});
findEl.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    const first = [...view.tasks].sort((a, b) => b.created.seq - a.created.seq).find((t) => found.has(live(t.id)));
    endFind();
    if (first) go(first.id);
  } else if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); endFind(); }
});
findEl.addEventListener("blur", () => { if (!finding) endFind(); });

addEventListener("keydown", (e) => {
  const typing = e.target.closest?.("input, textarea, select, [contenteditable]");
  if (e.key === "Escape") {
    if (!$(".viewer", panel).hidden) { $(".viewer", panel).hidden = true; return; }
    if (!$(".mix", panel).hidden) { $(".mix", panel).hidden = true; return; }
    if (openId) closeNode();
    return;
  }
  if ((e.key === "k" && (e.ctrlKey || e.metaKey)) || (e.key === "/" && !typing)) { e.preventDefault(); startFind(); return; }
  if (!typing && (e.key === "f" || e.key === "F") && !e.ctrlKey && !e.metaKey && !openId) fit(true);
});
// The card fits the window it is in: a phone's keyboard opening shrinks it.
addEventListener("resize", () => {
  const next = openId && seam && seamFor(openId);
  if (next) {
    const was = cardShown;
    seam = next;
    const r = cardRect(seam);
    placeCard(r);
    cardShown = r;
    cam = camFor(r, was && { x: r.x + r.w / 2, y: r.y, sx: cam.x + (was.x + was.w / 2) * cam.k, sy: cam.y + was.y * cam.k });
    applyCamera();
  }
  refresh();
});

