import { $, el, fail, icon, kTokens, panel, parse } from "./base.js";
import { act, can, digest, tab } from "./tab.js";
import { byId, isDone, view } from "./map.js";
import { aside, NEW, resume } from "./card.js";
import { unmovable } from "./log.js";
import { startNew } from "./newchat.js";

// ─── What Max writes ───────────────────────────────────────────────────────
// One box per open node, and one Send. A press with words in the box
// stages them: they move up to the top of the box, above a divider, and
// the box empties for more. An attachment stages the same way. A press with
// the box empty sends everything staged, read with the task's next request
// even mid-turn; the harness tells the model how far into its work he
// wrote. With a keyboard, Enter is the press and Shift+Enter a new line; on
// a phone Enter is a new line. What is staged and what is typed are the
// draft's, kept by the brain, so they survive a reload and follow Max to
// his phone; Send waits, with a spinner, until the brain has every word.
// Dictation streams into the box as edits to a live region, which Max can
// edit too: an edit inside it pins the text up to the end of his edit, and
// the words still coming go after it.
export const compose = $(".compose", panel);
export let dictation = null;
let box = null;
// Until when sending waits, after another device changed the draft.
let lockedUntil = 0;

// Max's drafts, held by the brain: a draft is a message from its first
// pause in typing. Its content is `{staged, text}`: the content blocks
// staged, in order, and the words in the box. This tab keeps what it
// changed until the brain has included it; then the draft's row holds it.
const local = new Map();  // draft id -> { staged, text, v }
const bound = new Map();  // session -> the draft this tab writes to
const saving = new Map(); // session -> pending save timer
const leaving = new Set(); // drafts being sent or thrown away: no longer the box's
const parsed = new Map(); // draft id -> its row's text, parsed
const empty = () => ({ staged: [], text: "" });
const rowsOf = (session) => (view?.drafts || []).filter((d) => d.session === session);
function draftOf(session) {
  const id = bound.get(session);
  if (id && !leaving.has(id) && (local.has(id) || rowsOf(session).some((d) => d.id === id))) return id;
  const open = rowsOf(session).filter((d) => !d.held && !leaving.has(d.id));
  const pick = open.length ? open[open.length - 1].id : null;
  if (pick) bound.set(session, pick); else bound.delete(session);
  return pick;
}
function contentOf(id) {
  if (!id) return empty();
  if (local.has(id)) return local.get(id);
  const raw = (view?.drafts || []).find((d) => d.id === id)?.text || "{}";
  if (parsed.get(id)?.raw === raw) return parsed.get(id).c;
  const said = parse(raw);
  // A held Send's words are the content blocks it sent: staged again.
  const c = Array.isArray(said) ? { staged: said, text: "" } : { ...empty(), ...said };
  parsed.set(id, { raw, c });
  return c;
}
// A staged list as text, once per list: what the box shows is compared by it.
const texts = new WeakMap();
const asText = (staged) => texts.get(staged) ?? texts.set(staged, JSON.stringify(staged)).get(staged);
const keyOf = (session) => draftOf(session) || `new:${session}`;
// A change to the draft, saved at the next pause in typing, or at once.
function edit(session, change, now) {
  const id = keyOf(session);
  const was = contentOf(id.startsWith("new:") ? null : id);
  local.set(id, { ...was, ...local.get(id), ...change, v: (local.get(id)?.v || 0) + 1 });
  clearTimeout(saving.get(session));
  saving.set(session, setTimeout(() => save(session), now ? 0 : 600));
}
// One record per pause: the whole content, or the draft's discarding once
// it is empty.
async function save(session) {
  saving.delete(session);
  let id = keyOf(session);
  const c = local.get(id);
  if (!c) return;
  const value = JSON.stringify({ staged: c.staged, text: c.text });
  const none = !c.text && !c.staged.length;
  if (id.startsWith("new:") && none) { local.delete(id); return renderSaved(session); }
  let raced = null;
  const why = await act((at) => {
    raced = `${at}.0`;
    c.n = Number(at.split(".")[1]);
    c.saved = c.v;
    if (!id.startsWith("new:")) return { Draft: [id, none ? "Discarded" : { Text: digest(value) }] };
    local.delete(id);
    id = `${at}.0`;
    local.set(id, c);
    bound.set(session, id);
    return [{ Draft: [id, { Opened: { session } }] }, { Draft: [id, { Text: digest(value) }] }];
  }, none ? [] : [value]);
  if (!why && none && !local.has(id)) bound.delete(session);
  // Another device edited it first: these words are a draft of their own
  // now, and this box goes on with them.
  if (why && rowsOf(session).some((d) => d.id === raced)) {
    if (local.get(id) === c) { local.delete(id); local.set(raced, c); }
    bound.set(session, raced);
    if (box?.dataset.key === session) syncCompose();
  }
  if (box?.dataset.key === session) renderOthers();
  renderSaved(session);
}
// Forget what this tab changed once the brain holds it: then the draft's
// row is the truth, whichever device changed it last.
export function settleLocal() {
  for (const [id, c] of local) if (c.n && c.saved === c.v && tab.status(c.n) !== "pending") local.delete(id);
  if (box) renderSaved(box.dataset.key);
}
// Whether the brain has everything this tab put in the session's draft.
const unsaved = (session) => !unborn(session) && (saving.has(session) || local.has(keyOf(session)));
function renderSaved(session) {
  if (box?.dataset.key === session) renderComposeState(byId[session]);
}
// After a reload, what this tab changed that the brain has not yet included
// is in its own records: back into the boxes.
export function restoreLocal() {
  for (const r of tab.records()) {
    const said = r.what.Said;
    if (!said) continue;
    const n = Number(r.at.split(".")[1]);
    for (const f of said.facts) {
      const [id, e] = f.Draft || [];
      if (e?.Opened) bound.set(e.Opened.session, id);
      if (e?.Text) {
        const value = said.values.find((v) => typeof v === "string" && digest(v) === e.Text);
        if (value) local.set(id, { ...empty(), ...parse(value), v: 1, saved: 1, n });
      }
    }
  }
}
// A chat not started yet, and its fields, have no session: theirs are kept
// in this tab.
const unborn = (key) => key === NEW || key.startsWith("new:");
const keptHere = (key) => ({ ...empty(), ...parse(sessionStorage.getItem(`draft:${key}`) || "{}") });
const keepHere = (key, change) => {
  try { sessionStorage.setItem(`draft:${key}`, JSON.stringify({ ...keptHere(key), ...change })); return null; }
  catch (e) { fail(`keeping the draft ${key} in this tab`, e); return "too big to keep in this tab"; }
};
const draftContent = (key) => unborn(key) ? keptHere(key) : contentOf(keyOf(key));
// The whole draft changed at once (staging, unstaging), saved now.
function change(key, c) {
  if (unborn(key)) return keepHere(key, c);
  edit(key, c, true);
  return null;
}
export const drafts = {
  get: (session) => draftContent(session).text,
  set: (session, text) => unborn(session) ? keepHere(session, { text }) : edit(session, { text }),
};
// What a send carries, as a new chat's mandate and attachments take it:
// the staged words joined, and the rest.
const words = (blocks) => blocks.filter((b) => b.type === "text").map((b) => b.text).join("\n\n");
const split = (blocks) => ({ text: words(blocks), files: blocks.filter((b) => b.type !== "text").map((block) => ({ block })) });
// The brain's draft in the box, if this tab has nothing of its own unsaved:
// what another device changed.
export function syncCompose() {
  if (!box || dictation || unborn(box.dataset.key)) return;
  const key = box.dataset.key;
  if (local.has(keyOf(key))) return;
  const c = draftContent(key), parts = $(".parts", compose);
  if (box.value === c.text && parts?.shown === asText(c.staged)) return;
  box.value = prev = c.text; autosize(); renderParts();
  // Another device changed the draft: sending waits a moment for Max to
  // see it.
  lockedUntil = Date.now() + 2000;
  setTimeout(() => { if (box?.dataset.key === key) renderComposeState(byId[key]); }, 2000);
  renderComposeState(byId[key]);
}
// The session's other drafts: words held when the session could not take
// them, and drafts opened beside this one (a concurrent Send or edit).
export function renderOthers() {
  const others = $(".others", compose);
  if (!others || !box) return;
  const session = box.dataset.key, mine = draftOf(session);
  others.replaceChildren(...rowsOf(session).filter((d) => d.id !== mine && !leaving.has(d.id)).map((d) => {
    const c = contentOf(d.id), card = el("div", `other${d.held ? " held" : ""}`);
    const gist = (c.text || words(c.staged)).split("\n")[0].slice(0, 160) || `${c.staged.length} staged`;
    card.append(el("span", "said", gist));
    card.append(el("span", "why", d.held ? `Not heard: ${d.held}` : !d.from ? "Another draft" : rowsOf(session).some((o) => o.id === d.from) ? "Typed at the same time as another" : "Typed after another was sent"));
    if (!d.held) { const use = el("button", "btn", "Edit this"); use.onclick = () => { bound.set(session, d.id); box.value = prev = c.text; autosize(); renderParts(); renderOthers(); renderComposeState(byId[session]); }; card.append(use); }
    else {
      const use = el("button", "btn", "Use these words");
      use.onclick = async () => { giveBlocks(session, [...c.staged, ...(c.text ? [{ type: "text", text: c.text }] : [])]); leaving.add(d.id); renderOthers(); await act({ Draft: [d.id, "Discarded"] }); renderOthers(); };
      card.append(use);
    }
    return card;
  }));
  // The box holds an edit of one of his messages: say so, and offer to drop it.
  const row = (view?.drafts || []).find((d) => d.id === mine);
  if (mine && (row?.at || editing.has(mine))) {
    const bar = el("div", "other editing");
    bar.append(el("span", "said", "Editing your message: sending it replaces that message and what followed, which stays as another version"));
    const cancel = el("button", "btn", "Cancel edit");
    cancel.onclick = async () => {
      leaving.add(mine); editing.delete(mine); local.delete(mine);
      box.value = prev = ""; autosize(); renderParts(); renderOthers();
      await act({ Draft: [mine, "Discarded"] });
      renderOthers(); syncCompose();
    };
    bar.append(cancel);
    others.prepend(bar);
  }
}
// Edit one of Max's messages: a draft at the entry before it, holding his
// words, which becomes the box's. Its Send moves the chat back there and
// says them, as one fact; what he was typing stays a draft beside it.
const editing = new Set(); // edit drafts this tab opened, until their rows arrive
export async function editMessage(session, at, text) {
  if (box?.dataset.key !== session || !at) return;
  const blocked = unmovable(byId[session]);
  if (blocked) return say(`Edit: ${blocked}`, true);
  const value = JSON.stringify({ staged: [], text });
  let id = null;
  const why = await act((now) => {
    id = `${now}.0`;
    local.set(id, { staged: [], text, v: 1, saved: 1, n: Number(now.split(".")[1]) });
    editing.add(id);
    bound.set(session, id);
    return [{ Draft: [id, { Opened: { session, at } }] }, { Draft: [id, { Text: digest(value) }] }];
  }, [value]);
  if (why) { local.delete(id); editing.delete(id); bound.delete(session); return say(`Edit: ${why}`, true); }
  box.value = prev = text; autosize(); renderParts(); renderOthers(); renderComposeState(byId[session]);
  box.focus();
}

// ─── Staging ───────────────────────────────────────────────────────────────
// What the model reads: images and PDFs as themselves, any other file as
// its text, named.
const IMAGES = ["image/png", "image/jpeg", "image/gif", "image/webp"];
const base64 = (f) => new Promise((ok) => { const r = new FileReader(); r.onload = () => ok(String(r.result).split(",")[1]); r.readAsDataURL(f); });
async function blockOf(f) {
  const name = f.name || "pasted";
  if (IMAGES.includes(f.type)) return { type: "image", source: { type: "base64", media_type: f.type, data: await base64(f) } };
  if (f.type === "application/pdf") return { type: "document", title: name, source: { type: "base64", media_type: f.type, data: await base64(f) } };
  const text = await f.text();
  if (text.includes("\0")) throw new Error(`${name}: the model reads images, PDFs and text, not ${f.type || "this kind of file"}`);
  return { type: "text", text: `[${name}]\n\n${text}` };
}
async function attach(key, files) {
  const blocks = [], wrong = [];
  for (const f of files) {
    try { blocks.push(await blockOf(f)); } catch (e) { wrong.push(e.message); }
  }
  const why = blocks.length ? change(key, { staged: [...draftContent(key).staged, ...blocks] }) : null;
  say([...wrong, ...(why ? [`Not attached: ${why}`] : [])].join("; "), wrong.length > 0 || !!why);
  if (box?.dataset.key === key) { renderParts(); renderComposeState(byId[key]); }
}
// What is in the box, staged: a part of its own, and the box empties.
function stage() {
  const key = box.dataset.key, text = box.value;
  if (!text.trim()) return;
  clearTimeout(saving.get(key));
  const why = change(key, { staged: [...draftContent(key).staged, { type: "text", text }], text: "" });
  if (why) return say(`Not staged: ${why}`, true);
  box.value = prev = "";
  autosize(); renderParts(); renderComposeState(byId[key]);
}
// A staged part out again: words go back into the box, ahead of what is
// there; anything else is removed.
function unstage(i) {
  const key = box.dataset.key, c = draftContent(key), part = c.staged[i];
  const text = part.type === "text" ? (box.value.trim() ? `${part.text}\n\n${box.value}` : part.text) : box.value;
  const why = change(key, { staged: c.staged.filter((_, j) => j !== i), text });
  if (why) return say(`Not changed: ${why}`, true);
  box.value = prev = text;
  autosize(); renderParts(); renderComposeState(byId[key]);
  box.focus();
}
function renderParts() {
  const parts = $(".parts", compose);
  if (!parts || !box) return;
  const staged = draftContent(box.dataset.key).staged;
  parts.shown = asText(staged);
  parts.replaceChildren(...staged.map((b, i) => {
    const part = el("div", `part ${b.type}`);
    if (b.type === "image") { const img = el("img"); img.src = `data:${b.source.media_type};base64,${b.source.data}`; part.append(img); }
    else part.append(el("div", "words", b.type === "text" ? b.text : b.title || b.type));
    const x = el("button", "x"); x.type = "button";
    x.title = b.type === "text" ? "Back into the box" : "Remove";
    x.setAttribute("aria-label", x.title);
    x.onclick = () => unstage(i);
    part.append(x);
    return part;
  }));
}

export function buildCompose(id) {
  compose.replaceChildren();
  box = null;
  const t = byId[id];
  if (id !== NEW && !t) return;
  if (!can.act) { compose.append(el("div", "closed", "This face only watches.")); return; }
  const wrap = el("div", "box");
  box = el("textarea");
  box.rows = id === NEW ? 2 : 1;
  box.dataset.key = id;
  box.spellcheck = true;
  box.value = prev = drafts.get(id);
  // Beside the box, what Max presses most: attach, dictate and send.
  const side = el("div", "side");
  const file = el("input"); file.type = "file"; file.multiple = true; file.hidden = true;
  const clip = icon("attach", "clip", "Attach");
  const mic = icon("mic", "mic", "Dictate"); mic.hidden = !can.dictation;
  const send = icon("send", "go send");
  send.append(el("span", "word"));
  side.append(file, clip, mic, send);
  // What is staged and what is typed scroll as one, under the buttons.
  const field = el("div", "field");
  field.append(el("div", "parts"), box);
  const line = el("div", "line");
  line.append(field, side);
  // Its height follows its words at the width it is shown at.
  let width = 0;
  new ResizeObserver(([e]) => { if (e.contentRect.width !== width) { width = e.contentRect.width; autosize(); } }).observe(field);
  const row = el("div", "row2");
  const sample = el("button", "later sample", "Dictate the sample"); sample.type = "button"; sample.hidden = !can.sample;
  row.append(el("span", "picks"), el("span", "status"), sample, el("span", "tune"));
  wrap.append(el("div", "others"), line, row);
  // What Max can do to the task itself floats outside the card, beside it.
  compose.append(wrap, el("div", "acts"));
  // What is chosen, pasted or dropped in is attached.
  clip.onclick = () => file.click();
  file.onchange = () => { attach(id, [...file.files]); file.value = ""; };
  box.addEventListener("paste", (e) => { const fs = [...(e.clipboardData?.files || [])]; if (fs.length) { e.preventDefault(); attach(id, fs); } });
  wrap.addEventListener("dragover", (e) => { e.preventDefault(); wrap.classList.add("drop"); });
  wrap.addEventListener("dragleave", () => wrap.classList.remove("drop"));
  wrap.addEventListener("drop", (e) => { e.preventDefault(); wrap.classList.remove("drop"); attach(id, [...e.dataTransfer.files]); });
  mic.onclick = () => dictate(false);
  sample.onclick = () => dictate(true);
  // Send never takes focus: the box keeps it, so the keyboard stays up and
  // the phone's layout doesn't shift under the finger between taps.
  send.onpointerdown = (e) => e.preventDefault();
  send.onclick = press;
  box.addEventListener("keydown", (e) => {
    if (e.key !== "Enter" || e.shiftKey || e.isComposing || e.keyCode === 229) return;
    if (matchMedia("(pointer: coarse)").matches) return;
    e.preventDefault();
    press();
  });
  box.addEventListener("input", onInput);
  autosize();
  renderParts();
  renderOthers();
  renderComposeState(t);
}
// ─── Model and effort ──────────────────────────────────────────────────────
// The bottom edge of the box, at its right: the model and effort the chat's
// next request runs at, each a section of its own. Choosing its role's
// leaves it to the role. While its cache is warm, each other model says what
// it costs: the whole context read again without the cache. Effort is not in
// the cached prefix, so changing it costs nothing. A new chat's choice is
// kept with its draft and goes into the fact that starts it.
export const modelName = (id) => view.models?.find((m) => m.id === id)?.name || id;
function tuneOf(t) {
  if (t) return { env: view.environments.find((e) => e.name === t.environment), ...t.tuning };
  const env = view.environments.find((e) => e.name === $("#n-role")?.value);
  return { env, model: drafts.get("new:model") || env?.model, effort: drafts.get("new:effort") || env?.effort, warm: null };
}
export function renderTune(t) {
  const pill = $(".tune", compose);
  if (!pill || (t && isDone(t))) return pill?.replaceChildren();
  const now = tuneOf(t);
  if (!now.env || !now.model) return pill.replaceChildren();
  const warm = now.warm && now.warm.until > Date.now() ? now.warm.tokens : 0;
  const key = `${now.model}|${now.effort}|${warm}|${now.env.name}`;
  if (pill.dataset.key === key) return;
  pill.dataset.key = key;
  const cost = warm ? `rereads ${kTokens(warm)}` : "";
  const pick = (list, chosen, name, field, cost = "") => {
    const b = el("button", "pick", name(chosen)); b.type = "button";
    b.onclick = () => menu(b, list.map((v) => ({ v, name: name(v), on: v === chosen, cost: v === chosen ? "" : cost })), (v) => retune(t, now, field, v));
    return b;
  };
  const models = view.models.map((m) => m.id);
  if (!models.includes(now.model)) models.push(now.model);
  pill.title = warm ? `Changing the model reads the whole context again without the cache: ${kTokens(warm)} tokens` : "The model and effort its next request runs at";
  pill.replaceChildren(
    pick(models, now.model, modelName, "model", cost),
    pick(view.efforts, now.effort, (e) => e, "effort"),
  );
}
// A choice from a short list: a menu over the control that opens it, which
// goes at a choice, Escape, or a press anywhere else; its control again
// closes it.
let closeMenu = null;
export function menu(opener, items, picked) {
  const again = closeMenu?.opener === opener;
  closeMenu?.();
  if (again) return;
  const list = el("div", "menu");
  list.setAttribute("role", "menu");
  for (const it of items) {
    const b = el("button", it.on ? "on" : null, it.name); b.type = "button";
    if (it.cost) b.append(el("span", "cost", it.cost));
    b.onclick = () => { close(); if (!it.on) picked(it.v); };
    list.append(b);
  }
  const away = (e) => { if (!list.contains(e.target) && e.target !== opener) close(); };
  const keys = (e) => {
    const all = [...list.children], i = all.indexOf(document.activeElement);
    if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); close(); opener.focus(); }
    else if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); all[(i + (e.key === "ArrowDown" ? 1 : all.length - 1)) % all.length].focus(); }
  };
  function close() {
    closeMenu = null;
    list.remove(); opener.removeAttribute("aria-expanded");
    removeEventListener("pointerdown", away, true); removeEventListener("keydown", keys, true); removeEventListener("resize", close);
  }
  closeMenu = Object.assign(close, { opener });
  opener.setAttribute("aria-expanded", "true");
  document.body.append(list);
  // Above the control, where the composer has room; its left edge on the
  // control's, kept on the screen.
  const r = opener.getBoundingClientRect(), m = list.getBoundingClientRect();
  list.style.left = `${Math.max(6, Math.min(innerWidth - m.width - 6, r.left))}px`;
  list.style.bottom = `${innerHeight - r.top + 6}px`;
  addEventListener("pointerdown", away, true); addEventListener("keydown", keys, true); addEventListener("resize", close);
  (list.querySelector(".on") || list.firstChild).focus();
}
// The whole setting at once, with what is its role's left to the role.
async function retune(t, now, field, value) {
  const next = { model: now.model, effort: now.effort, [field]: value };
  if (!t) { drafts.set(`new:${field}`, value === now.env[field] ? "" : value); return renderTune(null); }
  const tuning = {};
  if (next.model !== now.env.model) tuning.model = next.model;
  if (next.effort !== now.env.effort) tuning.effort = next.effort;
  const why = await act({ Task: [t.id, { Tuned: tuning }] });
  if (why) say(`not changed: ${why}`, true);
}
// What a new chat's Created says it runs at.
export function newTuning() {
  const now = tuneOf(null), tuning = {};
  if (now.model && now.model !== now.env?.model) tuning.model = now.model;
  if (now.effort && now.effort !== now.env?.effort) tuning.effort = now.effort;
  return tuning;
}


// The one button: with words in the box it stages them; with the box
// empty it sends what is staged, and says so. While the brain does not yet
// have every word, it spins, and cannot send.
export function renderComposeState(t) {
  if (!box) return;
  const key = box.dataset.key, send = $(".send", compose);
  const typed = !!box.value.trim(), staged = draftContent(key).staged.length > 0;
  const mine = t && key === t.id;
  if (key === NEW) box.placeholder = "New chat";
  else if (mine) box.placeholder = isDone(t) ? "Continue this chat" : "Reply";
  const saving = unsaved(key), locked = lockedUntil > Date.now();
  const sends = !typed && staged;
  send.disabled = !typed && (!staged || saving || locked);
  send.classList.toggle("armed", sends && !saving);
  send.classList.toggle("saving", saving);
  send.classList.toggle("locked", locked);
  $(".word", send).textContent = sends && !saving ? "Send" : "";
  const tip = locked ? "Changed on another device a moment ago" : saving ? "Saving the draft" : sends ? "Send what is staged (Enter)" : "Stage (Enter)";
  send.title = tip; send.setAttribute("aria-label", tip);
  renderTune(t);
}
// A press while dictating ends the dictation, and stages once its last
// words are in.
function press() {
  if (!box) return;
  if (dictation) {
    if (!dictation.stopping) stopDictation();
    dictation.then = stage;
    return;
  }
  if (box.value.trim()) return stage();
  if (!unsaved(box.dataset.key) && lockedUntil <= Date.now()) submit();
}
// Only what went wrong is said; what went right shows as itself.
export const say = (text, bad) => { const s = $(".status", compose); if (s) { s.textContent = bad ? text : ""; s.title = text; s.className = `status${bad ? " bad" : ""}`; } };
function autosize() {
  if (!box) return;
  box.style.height = "auto";
  box.style.height = `${box.scrollHeight}px`;
  // Typing at the end keeps the end in view.
  if (box.selectionEnd === box.value.length) box.parentElement.scrollTop = box.parentElement.scrollHeight;
}

// Sending what is staged: to a task working or between turns, the draft's
// Send; to a task waiting on agents it started, an aside that talks now;
// to a finished chat, the chat continued.
async function submit() {
  if (!box) return;
  const key = box.dataset.key, t = byId[key];
  const blocks = draftContent(key).staged;
  if (!blocks.length) return;
  const { text, files } = split(blocks);
  if (!t) return startNew(text, files);
  say("");
  const continues = isDone(t) || t.scoped;
  if (continues && !text.trim()) return say("Write something to go with what is attached", true);
  let draft = takeWords(key);
  // Words to a task Max paused carry it on.
  const release = t.pause?.by === t.id && !isDone(t) ? [{ Task: [t.id, { Pause: "Off" }] }] : [];
  const discard = [...(draft ? [{ Draft: [draft, "Discarded"] }] : []), ...release];
  // A new task whose mandate is these words; the draft goes with them.
  if (continues) {
    const why = isDone(t) ? await resume(t, text, files, discard) : await aside(t, text, files, discard);
    if (why) { returnWords(key, draft, blocks); return say(`Not sent: ${why}`, true); }
    local.delete(draft);
    return;
  }
  const sent = JSON.stringify(blocks), id = digest(sent);
  let as = null;
  const why = await act((at) => {
    as = `${at}.0`;
    const facts = [];
    if (!draft) { draft = as; facts.push({ Draft: [draft, { Opened: { session: t.id } }] }); }
    facts.push({ Draft: [draft, { Send: { text: id } }] }, ...release);
    return facts;
  }, [sent]);
  if (why) { returnWords(key, draft === as ? null : draft, blocks); return say(`Not sent: ${why}`, true); }
  local.delete(draft);
  const rows = view.drafts;
  const successor = rows.find((d) => d.id === as && d.from === draft);
  const held = rows.find((d) => d.id === draft && d.held);
  if (successor) {
    returnWords(key, successor.id, blocks);
    return say("Another device sent this draft first: your words are a new draft, beside it", true);
  }
  if (held) leaving.delete(draft);
  renderOthers();
  if (held) return say(`Held, not heard: ${held.held}`, true);
  renderComposeState(t);
}
// What is staged leaves the box as it is sent: what Max types while it
// goes is a draft of its own. The draft it was in is the send's.
export function takeWords(key) {
  clearTimeout(saving.get(key));
  saving.delete(key);
  const draft = unborn(key) ? null : draftOf(key);
  if (draft) leaving.add(draft);
  bound.delete(key);
  local.delete(`new:${key}`);
  if (unborn(key)) keepHere(key, empty());
  if (box?.dataset.key === key) { box.value = prev = ""; autosize(); renderParts(); renderOthers(); renderComposeState(byId[key]); }
  return draft;
}
// Refused, what was staged comes back to its draft, before anything
// staged since.
function returnWords(key, draft, blocks) {
  if (draft) { leaving.delete(draft); bound.set(key, draft); }
  giveBlocks(key, blocks);
}
export const giveBack = (key, text, files) => giveBlocks(key, [...files.map((a) => a.block), ...(text.trim() ? [{ type: "text", text }] : [])]);
function giveBlocks(key, blocks) {
  const c = draftContent(key);
  change(key, { staged: [...blocks, ...c.staged], text: c.text });
  if (box?.dataset.key === key) { box.value = prev = c.text; autosize(); renderParts(); renderOthers(); renderComposeState(byId[key]); }
}

let prev = "";
function onInput() {
  const now = box.value;
  if (dictation && dictation.key === box.dataset.key) {
    let x = 0;
    while (x < prev.length && x < now.length && prev[x] === now[x]) x++;
    let tail = 0;
    while (tail < prev.length - x && tail < now.length - x && prev[prev.length - 1 - tail] === now[now.length - 1 - tail]) tail++;
    const removed = prev.length - x - tail, inserted = now.length - x - tail;
    const l = dictation;
    const end = l.start + l.T.length - l.P;
    if (x + removed <= l.start && !(x === l.start && removed === 0 && l.T.length === l.P)) l.start += inserted - removed;
    else if (x <= end) { l.P = Math.min(l.T.length, l.P + Math.max(0, x + removed - l.start)); l.start = x + inserted; }
  }
  prev = now;
  drafts.set(box.dataset.key, now);
  autosize();
  renderComposeState(byId[box.dataset.key]);
}


// The endpoint's edits, in the transcript's own offsets, last first so the
// earlier offsets still hold. If Max has gone to another node meanwhile,
// they land in the draft of the node he was dictating to.
function applyEdits(edits) {
  const l = dictation;
  const shown = box && box.dataset.key === l.key;
  const target = shown ? box : Object.assign(document.createElement("textarea"), { value: drafts.get(l.key) });
  const end = () => l.start + l.T.length - l.P;
  const following = shown && target.selectionStart === target.selectionEnd && target.selectionEnd === end();
  for (const e of [...edits].reverse()) {
    if (e.at >= l.P) { const from = l.start + e.at - l.P; target.setRangeText(e.put, from, from + e.cut, "preserve"); }
    else if (e.at + e.cut <= l.P) l.P += e.put.length - e.cut;
    else { target.setRangeText(e.put, l.start, l.start + e.at + e.cut - l.P, "preserve"); l.P = e.at; }
    l.T = l.T.slice(0, e.at) + e.put + l.T.slice(e.at + e.cut);
  }
  if (following) target.setSelectionRange(end(), end());
  drafts.set(l.key, target.value);
  if (shown) { prev = box.value; autosize(); renderComposeState(byId[l.key]); }
}

// The microphone as 16-bit PCM, in chunks of a tenth of a second.
const WORKLET = URL.createObjectURL(new Blob([`
registerProcessor("pcm", class extends AudioWorkletProcessor {
  constructor() { super(); this.buf = new Int16Array(sampleRate / 10); this.n = 0; }
  process([input]) {
    const ch = input[0];
    if (!ch) return true;
    for (let i = 0; i < ch.length; i++) {
      const s = Math.max(-1, Math.min(1, ch[i]));
      this.buf[this.n++] = s < 0 ? s * 0x8000 : s * 0x7fff;
      if (this.n === this.buf.length) {
        this.port.postMessage(this.buf.buffer, [this.buf.buffer]);
        this.buf = new Int16Array(sampleRate / 10); this.n = 0;
      }
    }
    return true;
  }
});`], { type: "application/javascript" }));

async function dictate(sample) {
  if (dictation) return stopDictation();
  if (!box) return;
  let rate = 16000, mic = null;
  try {
    if (!sample) {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true } });
      const ctx = new AudioContext();
      await ctx.audioWorklet.addModule(WORKLET);
      const node = new AudioWorkletNode(ctx, "pcm");
      ctx.createMediaStreamSource(stream).connect(node);
      rate = ctx.sampleRate;
      mic = { stream, ctx, node };
    }
  } catch (e) { fail("opening the microphone", e); return say(`no microphone: ${e.message}`, true); }
  // Dictation goes in at the caret, a space from the word before it.
  let at = box.selectionEnd;
  if (at > 0 && !/\s/.test(box.value[at - 1])) { box.setRangeText(" ", at, at, "end"); at += 1; prev = box.value; }
  const ws = new WebSocket(`wss://${location.host}/dictation?rate=${rate}${sample ? "&sample" : ""}`);
  ws.binaryType = "arraybuffer";
  dictation = { ws, mic, key: box.dataset.key, T: "", P: 0, start: at };
  if (mic) mic.node.port.onmessage = (m) => { if (ws.readyState === 1) ws.send(m.data); };
  const b = $(".mic", compose); if (b) { b.classList.add("on"); b.title = "Stop dictating"; }
  say("connecting");
  const l = dictation;
  ws.onmessage = (m) => {
    const d = JSON.parse(m.data);
    if (d.listening) say(sample ? "dictating the sample" : "listening");
    if (d.edits && dictation === l) applyEdits(d.edits);
    if (d.error) { fail("dictating", d.error); say(d.error, true); }
    if (d.final) finishDictation(l);
  };
  ws.onclose = () => finishDictation(l);
  box.focus();
}
function releaseMic(l) {
  if (!l.mic) return;
  l.mic.stream.getTracks().forEach((t) => t.stop());
  l.mic.ctx.close();
  l.mic = null;
}
export function stopDictation() {
  if (!dictation) return;
  dictation.stopping = true;
  releaseMic(dictation);
  if (dictation.ws.readyState === 1) dictation.ws.send(JSON.stringify({ type: "stop" }));
  say("finishing");
}
function finishDictation(l) {
  if (dictation !== l) return;
  releaseMic(l);
  dictation = null;
  l.ws.close();
  const b = $(".mic", compose); if (b) { b.classList.remove("on"); b.title = "Dictate"; }
  if (!$(".status", compose)?.classList.contains("bad")) say("");
  if (l.then && box?.dataset.key === l.key) l.then();
}

