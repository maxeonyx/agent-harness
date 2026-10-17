import { $, el, logEl, panel } from "./base.js";
import { byId, graph, live, view } from "./map.js";
import { fit } from "./camera.js";
import { drawMap } from "./draw.js";
import { closeNode, go, logState, mini, NEW, openId, start, title } from "./card.js";
import { compose, drafts, giveBack, newTuning, renderTune, say, takeWords } from "./compose.js";

// ─── A new chat, in Max's own scope ───────────────────────────────────────
// A double click on the ground, or +, opens a small card there on a chat
// that does not exist yet: the same staging box as every chat, beside the
// send its role and a field for each place that role names (a workspace's
// is its directory), its draft and attachments kept like theirs. Sending
// starts it: the words are its mandate, what is attached reaches it with
// them, and the card becomes the chat where it is.
export function buildNew() {
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
  const places = el("span");
  places.style.display = "contents";
  const fill = () => places.replaceChildren(...placesOf(role.value).flatMap(placeField));
  role.onchange = () => { drafts.set("new:role", role.value); fill(); renderTune(null); };
  fill();
  picks.replaceChildren(role, places);
  renderTune(null);
}
const placesOf = (role) => view.environments.find((e) => e.name === role)?.places || [];
// A place's field: a workspace's is a directory, offered from those known.
function placeField(place) {
  const f = el("input", "pick place"); f.dataset.place = place;
  f.placeholder = place === "workspace" ? "directory" : place; f.title = `Its ${place}`;
  f.value = drafts.get(`new:${place}`) || "";
  f.oninput = () => drafts.set(`new:${place}`, f.value);
  if (place !== "workspace") return [f];
  const list = el("datalist"); list.id = `n-${place}`;
  list.append(...view.workspaces.map((w) => Object.assign(el("option"), { value: w.dir })));
  f.setAttribute("list", list.id);
  return [f, list];
}
// A name from a mandate: its first line with words, as plain text (no
// heading, quote or list mark, code ticks, emphasis or link target), to its
// first clause's end, at most six words. A clause ends at punctuation
// before a space, so `./run.sh` and 3.5 stay whole. The test tab's
// `name_from` (crates/brain/examples/device) is the same rule.
const plain = (line) => line
  .replace(/^\s*(#+|>|[-*+]|\d+[.)])\s+/, "")
  .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
  .replace(/`|\*\*|__/g, "")
  .replace(/(^|\s)[*_]+|[*_]+(?=\s|$)/g, "$1")
  .trim();
const nameFrom = (text) => {
  const line = text.split("\n").map(plain).find((l) => l) ?? "";
  const words = line.split(/[.,;:!?](?=\s|$)/)[0].split(/\s+/).filter(Boolean).slice(0, 6).join(" ");
  return words.length > 40 ? `${words.slice(0, 39)}…` : words;
};
export async function startNew(text, files) {
  const env = view.environments.find((e) => e.name === $("#n-role").value);
  if (!env) return say("choose a role", true);
  const given = Object.fromEntries([...compose.querySelectorAll(".place")].map((f) => [f.dataset.place, f.value.trim()]));
  const missing = env.places.find((p) => !given[p]);
  if (missing) return say(`it needs its ${missing}`, true);
  if (!text.trim()) return say("write what it is for", true);
  takeWords(NEW);
  const why = await start((next, out) => {
    const places = {};
    for (const place of env.places) {
      const name = given[place];
      const known = place === "workspace" && view.workspaces.find((w) => w.dir === name);
      const limb = known ? known.id : next();
      if (!known) out.push({ Limb: { id: limb, name, kind: { Plugin: { kind: place, config: JSON.stringify({ dir: name }) } } } });
      places[place] = limb;
    }
    return { name: nameFrom(text), scope: "Max", origin: { Fresh: { environment: env.id, places } }, tuning: newTuning() };
  }, text, files);
  if (why) { giveBack(NEW, text, files); return say(`not started: ${why}`, true); }
}

// ─── Finding a chat ────────────────────────────────────────────────────────
// Ctrl+K, or /: the chats whose name, mandate or words match are listed
// under the box, newest first, each with what matched, and stay lit on the
// map while the rest fades. Enter or a click opens one at the match; the
// arrows choose; Esc lets it go. The words are searched by the face
// (`/find`), which holds every context; the page holds only the open one.
const findEl = $("#find"), foundEl = $("#found");
export let finding = "", found = new Set(), hits = [], chosen = 0, asking = 0;
// The entry to bring into view once its chat's log has it.
let reveal = null;
function startFind() { findEl.hidden = false; findEl.focus(); findEl.select(); }
function endFind() {
  findEl.hidden = foundEl.hidden = true; findEl.value = ""; finding = ""; found = new Set(); hits = [];
  findEl.blur(); if (view) drawMap();
}
findEl.addEventListener("input", () => {
  finding = findEl.value.trim().toLowerCase();
  const q = finding, ask = ++asking;
  // Names and mandates are in the view: they list at once, words follow.
  hits = q ? [...view.tasks].filter((t) => `${t.name}\n${t.mandate || ""}`.toLowerCase().includes(q))
    .map((t) => ({ task: t.id })) : [];
  showFound();
  if (!q) return;
  setTimeout(() => {
    if (ask !== asking) return;
    fetch(`/find?q=${encodeURIComponent(q)}`).then((r) => r.ok ? r.json() : []).catch(() => []).then((said) => {
      if (ask !== asking) return;
      const named = new Map(hits.map((h) => [h.task, h]));
      for (const h of said) named.set(h.task, { ...named.get(h.task), ...h });
      hits = [...named.values()];
      showFound();
    });
  }, 150);
});
function showFound() {
  const seq = (h) => byId[h.task]?.created.seq ?? -1;
  hits = hits.filter((h) => byId[h.task]).sort((a, b) => seq(b) - seq(a));
  chosen = 0;
  found = new Set(hits.map((h) => live(h.task)));
  // A folded project matches through what it folds.
  for (const c of graph.comps) if (c.folded && c.tasks.some((t) => found.has(t.id))) found.add(c.root.id);
  foundEl.replaceChildren(...hits.map((h, i) => {
    const row = el("button", "hit");
    row.type = "button";
    row.append(el("span", "name", byId[h.task].name));
    if (h.hits > 1) row.append(el("span", "count", `${h.hits} places`));
    if (h.snippet) {
      const words = el("span", "said", h.snippet.before);
      words.append(el("mark", "", h.snippet.match), h.snippet.after);
      row.append(words);
    }
    row.onmousedown = (e) => e.preventDefault();
    row.onclick = () => open(i);
    return row;
  }));
  foundEl.hidden = !finding;
  if (finding && !hits.length) foundEl.append(el("div", "none", "Nothing said matches"));
  choose(0);
  drawMap();
}
function choose(i) {
  const rows = foundEl.querySelectorAll(".hit");
  if (!rows.length) return;
  chosen = (i + rows.length) % rows.length;
  rows.forEach((r, k) => r.classList.toggle("chosen", k === chosen));
  rows[chosen].scrollIntoView({ block: "nearest" });
}
function open(i) {
  const h = hits[i];
  endFind();
  if (!h) return;
  reveal = h.entry || null;
  go(h.task);
  if (reveal && logState) revealIn(logState);
}
// Once the open log holds the found entry, it scrolls there and glows.
export function revealIn(s) {
  const at = reveal && $(`[data-entry="${reveal}"]`, logEl);
  if (!at || s !== logState) return;
  reveal = null;
  s.follow = false;
  at.scrollIntoView({ block: "center" });
  at.classList.remove("revealed"); void at.offsetWidth; at.classList.add("revealed");
}
findEl.addEventListener("keydown", (e) => {
  if (e.key === "Enter") { e.preventDefault(); open(chosen); }
  else if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); choose(chosen + (e.key === "ArrowDown" ? 1 : -1)); }
  else if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); endFind(); }
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

