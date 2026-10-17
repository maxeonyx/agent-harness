import { $, el, icon, kTokens, panel, parse } from "./base.js";
import { act, can, digest, tab } from "./tab.js";
import { byId, isDone, view } from "./map.js";
import { aside, NEW, resume } from "./card.js";
import { unmovable } from "./log.js";
import { startNew } from "./newchat.js";

// ─── What Max writes ───────────────────────────────────────────────────────
// One box per open node: a staging area, its text and attachments kept per
// task across tabs and reloads. Nothing reaches the model until Max sends
// it, one of two ways: Send, read with the task's next request even
// mid-turn, or Queue, at the end of its turn, so it does not cut across the
// work. Either way the harness tells the model how far into its work he
// wrote. Each takes two presses: the first stages it, the second sends what
// he has seen staged; any change to the words lets the stage go. With a
// keyboard, Enter stages and sends (Alt+Enter, Queue), and Shift+Enter is a
// new line; on a phone Enter is a new line and the buttons send.
// Dictation streams into the box as edits to a live region, which Max can
// edit too: an edit inside it pins the text up to the end of his edit, and
// the words still coming go after it.
export const compose = $(".compose", panel);
export let box = null, dictation = null;
// The send this tab's box is staged for, "Next" or "TurnEnd"; and until
// when sending waits, after another device changed the words.
let staged = null, lockedUntil = 0;

// Max's drafts, held by the brain: a draft is a message from its first
// pause in typing, so the phone shows what the PC typed and either can send
// it. Its content is `{text, attached}`. This tab keeps what it typed until
// the brain has included it; then the draft's row holds it.
const local = new Map();  // draft id -> { text, attached, v }
const bound = new Map();  // session -> the draft this tab writes to
const saving = new Map(); // session -> pending save timer
const leaving = new Set(); // drafts being sent or thrown away: no longer the box's
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
  if (!id) return { text: "", attached: [] };
  if (local.has(id)) return local.get(id);
  const row = (view?.drafts || []).find((d) => d.id === id);
  const said = parse(row?.text || "{}");
  // A held Send's words are the content blocks it sent.
  if (Array.isArray(said)) return {
    text: said.filter((b) => b.type === "text").map((b) => b.text).join("\n\n"),
    attached: said.filter((b) => b.type !== "text").map((block) => ({ name: block.type, block })),
  };
  return { text: "", attached: [], ...said };
}
function edit(session, change) {
  const id = draftOf(session) || `new:${session}`;
  const was = contentOf(id === `new:${session}` ? null : id);
  local.set(id, { ...was, ...local.get(id), ...change, v: (local.get(id)?.v || 0) + 1 });
  clearTimeout(saving.get(session));
  saving.set(session, setTimeout(() => save(session), 600));
}
// One record per pause: the whole content, or the draft's discarding once
// it is empty.
async function save(session) {
  saving.delete(session);
  let id = draftOf(session) || `new:${session}`;
  const c = local.get(id);
  if (!c) return;
  const value = JSON.stringify({ text: c.text, attached: c.attached });
  const empty = !c.text && !c.attached.length;
  if (id.startsWith("new:") && empty) return local.delete(id);
  let raced = null;
  const why = await act((at) => {
    raced = `${at}.0`;
    c.n = Number(at.split(".")[1]);
    c.saved = c.v;
    if (!id.startsWith("new:")) return { Draft: [id, empty ? "Discarded" : { Text: digest(value) }] };
    local.delete(id);
    id = `${at}.0`;
    local.set(id, c);
    bound.set(session, id);
    return [{ Draft: [id, { Opened: { session } }] }, { Draft: [id, { Text: digest(value) }] }];
  }, empty ? [] : [value]);
  if (!why && empty && !local.has(id)) bound.delete(session);
  // Another device edited it first: these words are a draft of their own
  // now, and this box goes on with them.
  if (why && rowsOf(session).some((d) => d.id === raced)) {
    if (local.get(id) === c) { local.delete(id); local.set(raced, c); }
    bound.set(session, raced);
    if (box?.dataset.key === session) syncCompose();
  }
  if (box?.dataset.key === session) renderOthers();
}
// Forget what this tab typed once the brain holds it: then the draft's row
// is the truth, whichever device typed last.
export function settleLocal() {
  for (const [id, c] of local) if (c.n && c.saved === c.v && tab.status(c.n) !== "pending") local.delete(id);
}
// After a reload, what this tab typed that the brain has not yet included
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
        if (value) local.set(id, { text: "", attached: [], ...parse(value), v: 1, saved: 1, n });
      }
    }
  }
}
// A chat not started yet, and its fields, have no session: theirs are kept
// in this tab.
const unborn = (key) => key === NEW || key.startsWith("new:");
const keptHere = (key) => ({ text: "", attached: [], ...parse(sessionStorage.getItem(`draft:${key}`) || "{}") });
const keepHere = (key, change) => {
  try { sessionStorage.setItem(`draft:${key}`, JSON.stringify({ ...keptHere(key), ...change })); return null; }
  catch { return "too big to keep in this tab"; }
};
export const drafts = {
  get: (session) => unborn(session) ? keptHere(session).text : contentOf(draftOf(session) || `new:${session}`).text,
  set: (session, text) => unborn(session) ? keepHere(session, { text }) : edit(session, { text }),
};
const attached = {
  get: (session) => unborn(session) ? keptHere(session).attached : contentOf(draftOf(session) || `new:${session}`).attached,
  set: (session, list) => { if (unborn(session)) return keepHere(session, { attached: list }); edit(session, { attached: list }); return null; },
};
// The brain's text for the draft in the box, if this tab has nothing of
// its own unsaved: what another device typed.
export function syncCompose() {
  if (!box || dictation || unborn(box.dataset.key)) return;
  const id = draftOf(box.dataset.key);
  if (id && local.has(id)) return;
  const text = drafts.get(box.dataset.key);
  if (box.value === text) return;
  box.value = prev = text; autosize(); renderTray();
  // Another device changed the words: what was staged is not what is here,
  // and sending waits a moment for Max to see them.
  staged = null;
  lockedUntil = Date.now() + 2000;
  const key = box.dataset.key;
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
    card.append(el("span", "said", c.text.split("\n")[0].slice(0, 160) || `${c.attached.length} attached`));
    card.append(el("span", "why", d.held ? `not heard: ${d.held}` : !d.from ? "another draft" : rowsOf(session).some((o) => o.id === d.from) ? "typed at the same time as another" : "typed after another was sent"));
    if (!d.held) { const use = el("button", "btn", "Edit this"); use.onclick = () => { bound.set(session, d.id); box.value = prev = c.text; unstage(); autosize(); renderTray(); renderOthers(); }; card.append(use); }
    else {
      const use = el("button", "btn", "Use these words");
      use.onclick = async () => { giveBack(session, c.text, c.attached); leaving.add(d.id); renderOthers(); await act({ Draft: [d.id, "Discarded"] }); renderOthers(); };
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
      box.value = prev = ""; autosize(); renderTray(); renderOthers();
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
  const value = JSON.stringify({ text, attached: [] });
  let id = null;
  const why = await act((now) => {
    id = `${now}.0`;
    local.set(id, { text, attached: [], v: 1, saved: 1, n: Number(now.split(".")[1]) });
    editing.add(id);
    bound.set(session, id);
    return [{ Draft: [id, { Opened: { session, at } }] }, { Draft: [id, { Text: digest(value) }] }];
  }, [value]);
  if (why) { local.delete(id); editing.delete(id); bound.delete(session); return say(`Edit: ${why}`, true); }
  box.value = prev = text; unstage(); autosize(); renderTray(); renderOthers();
  box.focus();
}

const IMAGES = ["image/png", "image/jpeg", "image/gif", "image/webp"];
async function attach(key, files) {
  const list = attached.get(key);
  for (const f of files) {
    if (IMAGES.includes(f.type)) {
      const data = await new Promise((ok) => { const r = new FileReader(); r.onload = () => ok(String(r.result).split(",")[1]); r.readAsDataURL(f); });
      list.push({ name: f.name || "pasted image", block: { type: "image", source: { type: "base64", media_type: f.type, data } } });
    } else {
      list.push({ name: f.name, block: { type: "text", text: `[${f.name}]\n\n${await f.text()}` } });
    }
  }
  const why = attached.set(key, list);
  if (why) say(`not attached: ${why}`, true);
  if (box?.dataset.key === key) unstage();
  renderTray();
}
function renderTray() {
  const tray = $(".tray", compose);
  if (!tray || !box) return;
  const key = box.dataset.key;
  tray.replaceChildren(...attached.get(key).map((a, i) => {
    const chip = el("span", "att");
    if (a.block.type === "image") { const img = el("img"); img.src = `data:${a.block.source.media_type};base64,${a.block.source.data}`; chip.append(img); }
    const x = el("button", "x"); x.type = "button"; x.title = "Remove"; x.setAttribute("aria-label", "Remove");
    x.onclick = () => { const list = attached.get(key); list.splice(i, 1); attached.set(key, list); unstage(); renderTray(); };
    chip.append(el("span", null, a.name), x);
    return chip;
  }));
}

export function buildCompose(id) {
  compose.replaceChildren();
  box = null;
  staged = null;
  const t = byId[id];
  if (id !== NEW && !t) return;
  if (!can.act) { compose.append(el("div", "closed", "This face only watches.")); return; }
  const wrap = el("div", "box");
  const tray = el("div", "tray");
  box = el("textarea");
  box.rows = id === NEW ? 2 : 1;
  box.dataset.key = id;
  box.spellcheck = true;
  box.value = drafts.get(id);
  // Beside the box, the two Max presses most: dictate and send, large.
  const side = el("div", "side");
  const mic = icon("mic", "mic", "Dictate"); mic.hidden = !can.dictation;
  const queue = goButton("queue", "queue", "Queue", "It reads this when its turn ends, without cutting across its work (Alt+Enter, twice)");
  const send = goButton("send", "send", "Send", "Send (Enter, twice)");
  side.append(mic, queue, send);
  const line = el("div", "line");
  line.append(box, side);
  const row = el("div", "row2");
  const sample = el("button", "later sample", "Dictate the sample"); sample.type = "button"; sample.hidden = !can.sample;
  row.append(sample, el("span", "picks"), el("span", "status"), el("span", "tune"));
  wrap.append(el("div", "others"), tray, line, row);
  // What Max can do to the task itself floats outside the card, beside it.
  compose.append(wrap, el("div", "acts"));
  // What is pasted or dropped in is attached.
  box.addEventListener("paste", (e) => { const fs = [...(e.clipboardData?.files || [])]; if (fs.length) { e.preventDefault(); attach(id, fs); } });
  wrap.addEventListener("dragover", (e) => { e.preventDefault(); wrap.classList.add("drop"); });
  wrap.addEventListener("dragleave", () => wrap.classList.remove("drop"));
  wrap.addEventListener("drop", (e) => { e.preventDefault(); wrap.classList.remove("drop"); attach(id, [...e.dataTransfer.files]); });
  mic.onclick = () => dictate(false);
  sample.onclick = () => dictate(true);
  send.onclick = () => press("Next");
  queue.onclick = () => press("TurnEnd");
  box.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && staged) { e.preventDefault(); e.stopPropagation(); return unstage(); }
    if (e.key !== "Enter" || e.shiftKey || e.isComposing || e.keyCode === 229) return;
    if (matchMedia("(pointer: coarse)").matches) return;
    e.preventDefault();
    press(e.altKey && !queue.hidden ? "TurnEnd" : "Next");
  });
  box.addEventListener("input", onInput);
  autosize();
  renderTray();
  renderOthers();
  renderComposeState(t);
}
// ─── Model and effort ──────────────────────────────────────────────────────
// The pill under the send: the model and effort the chat's next request
// runs at, "Opus · high". Each is a choice of its own; choosing its role's
// leaves it to the role. While its cache is warm, each other model says what
// it costs: the whole context read again without the cache. Effort is not in
// the cached prefix, so changing it costs nothing. A new
// chat's choice is kept with its draft and goes into the fact that starts
// it.
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
    el("span", "sep"),
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

// Send or Queue: its icon, and the word it shows once staged.
function goButton(name, cls, word, tip) {
  const b = icon(name, `go ${cls}`, tip);
  b.dataset.word = word;
  b.append(el("span", "word"));
  return b;
}
// Mid-turn, words can go now or wait for the turn's end; between turns, in
// response to its question, or to a finished chat, there is only now.
const midTurn = (t) => ["model", "calls", "harness", "held", "limited"].includes(t.waiting?.kind);
export function renderComposeState(t) {
  if (!box) return;
  const key = box.dataset.key, has = !!box.value.trim() || attached.get(key).length > 0;
  const send = $(".send", compose), queue = $(".queue", compose);
  const mine = t && key === t.id;
  if (key === NEW) box.placeholder = "New chat";
  else if (mine) box.placeholder = isDone(t) ? "Continue this chat" : "Reply";
  queue.hidden = !mine || !midTurn(t) || isDone(t);
  if (!has || (staged === "TurnEnd" && queue.hidden)) staged = null;
  const locked = lockedUntil > Date.now();
  for (const [b, read] of [[send, "Next"], [queue, "TurnEnd"]]) {
    b.disabled = !has || locked;
    b.classList.toggle("staged", staged === read);
    $(".word", b).textContent = staged === read ? `${b.dataset.word}?` : "";
  }
  send.classList.toggle("locked", locked);
  send.title = locked ? "Changed on another device a moment ago" : "Send (Enter, twice)";
  $(".box", compose).classList.toggle("staged", !!staged);
  renderTune(t);
}
// The first press stages, the second sends; the other button restages as
// itself. A press while dictating ends the dictation and stages, and a
// second press sends once its last words are in.
function press(read) {
  if (!box) return;
  if (dictation) {
    if (!dictation.stopping) { stopDictation(); staged = read; }
    else if (staged === read) dictation.then = read;
    else staged = read;
    return renderComposeState(byId[box.dataset.key]);
  }
  if (staged === read) return submit(read);
  staged = read;
  renderComposeState(byId[box.dataset.key]);
}
function unstage() {
  if (!staged || !box) return;
  staged = null;
  if (dictation) dictation.then = null;
  renderComposeState(byId[box.dataset.key]);
}
// Only what went wrong is said; what went right shows as itself.
export const say = (text, bad) => { const s = $(".status", compose); if (s) { s.textContent = bad ? text : ""; s.title = text; s.className = `status${bad ? " bad" : ""}`; } };
function autosize() {
  if (!box) return;
  box.style.height = "auto";
  const h = Math.min(box.scrollHeight, innerHeight * .4);
  box.style.height = `${h}px`;
}

// Sending: to a task working or between turns, the draft's Send; to a task
// waiting on agents it started, an aside that talks now; to a finished
// chat, the chat continued.
async function submit(read) {
  if (!box) return;
  const key = box.dataset.key, t = byId[key];
  const text = box.value, files = attached.get(key);
  if (!text.trim() && !files.length) return;
  staged = null;
  renderComposeState(t);
  if (!t) return startNew(text, files);
  say("");
  const continues = isDone(t) || (t.scoped && read === "Next");
  if (continues && !text.trim()) return say("write something to go with what is attached", true);
  let draft = takeWords(key);
  // Words to a task Max paused carry it on.
  const release = t.pause?.by === t.id && !isDone(t) ? [{ Task: [t.id, { Pause: "Off" }] }] : [];
  const discard = [...(draft ? [{ Draft: [draft, "Discarded"] }] : []), ...release];
  // A new task whose mandate is these words; the draft goes with them.
  if (continues) {
    const why = isDone(t) ? await resume(t, text, files, discard) : await aside(t, text, files, discard);
    if (why) { returnWords(key, draft, text, files); return say(`not sent: ${why}`, true); }
    local.delete(draft);
    return;
  }
  const blocks = [...files.map((a) => a.block), ...(text.trim() ? [{ type: "text", text }] : [])];
  const words = JSON.stringify(blocks), id = digest(words);
  let sent = null;
  const why = await act((at) => {
    sent = `${at}.0`;
    const facts = [];
    if (!draft) { draft = sent; facts.push({ Draft: [draft, { Opened: { session: t.id } }] }); }
    facts.push({ Draft: [draft, { Send: { text: id, read } }] }, ...release);
    return facts;
  }, [words]);
  if (why) { returnWords(key, draft === sent ? null : draft, text, files); return say(`not sent: ${why}`, true); }
  local.delete(draft);
  const rows = view.drafts;
  const successor = rows.find((d) => d.id === sent && d.from === draft);
  const held = rows.find((d) => d.id === draft && d.held);
  if (successor) {
    returnWords(key, successor.id, text, files);
    return say("another device sent this draft first: your words are a new draft, beside it", true);
  }
  if (held) leaving.delete(draft);
  renderOthers();
  if (held) return say(`held, not heard: ${held.held}`, true);
  renderComposeState(t);
}
// What is in the box leaves it as it is sent: the box is empty at once, so
// a second press sends nothing, and what Max types while it goes is a draft
// of its own. The draft the words were in is the send's.
export function takeWords(key) {
  clearTimeout(saving.get(key));
  saving.delete(key);
  const draft = unborn(key) ? null : draftOf(key);
  if (draft) leaving.add(draft);
  bound.delete(key);
  local.delete(`new:${key}`);
  if (unborn(key)) { drafts.set(key, ""); attached.set(key, []); }
  if (box?.dataset.key === key) { box.value = prev = ""; autosize(); renderTray(); renderOthers(); }
  return draft;
}
// Refused, the words come back to their draft, before anything typed since.
function returnWords(key, draft, text, files) {
  if (draft) { leaving.delete(draft); bound.set(key, draft); }
  giveBack(key, text, files);
}
export function giveBack(key, text, files) {
  const now = drafts.get(key), more = attached.get(key);
  const back = now.trim() ? `${text}\n\n${now}` : text;
  drafts.set(key, back);
  attached.set(key, [...files, ...more]);
  if (box?.dataset.key === key) { box.value = prev = back; autosize(); renderTray(); renderOthers(); }
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
  staged = null;
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
  // Words still coming after Max pressed were said before he pressed.
  if (shown) { prev = box.value; if (!l.stopping) staged = null; autosize(); renderComposeState(byId[l.key]); }
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
  } catch (e) { return say(`no microphone: ${e.message}`, true); }
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
    if (d.error) say(d.error, true);
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
  if (l.then && staged === l.then && box?.dataset.key === l.key) submit(l.then);
}

