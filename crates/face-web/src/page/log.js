import { $, clock, el, getJson, gist, kTokens, logEl, markdown, panel, parse, scrollEl } from "./base.js";
import { act } from "./tab.js";
import { byId, glyph, isDone, look, order, shortName, view } from "./map.js";
import { trouble } from "./draw.js";
import { go, logState } from "./card.js";
import { editMessage, modelName, say } from "./compose.js";
import { revealIn } from "./newchat.js";

// ─── The log: the context, marked up, or exactly as sent ─────────────────
const contexts = new Map();
export async function contextOf(head) {
  if (!contexts.has(head)) {
    contexts.set(head, getJson(`/context/${head}`, `loading the context ending at ${head}`, null)
      .then((c) => { if (!c) contexts.delete(head); return c; }));
    while (contexts.size > 20) contexts.delete(contexts.keys().next().value);
  }
  return contexts.get(head);
}

// Where the task's context has other versions, read again with each head
// and each request that ends, since any request can add one, and a retry
// can end where it began.
const versionsOf = (task) => getJson(`/versions/${task}`, `loading the versions of task ${task}`, {});

export async function renderLog(t) {
  const s = logState;
  if (!s || s.id !== t.id) return;
  const seen = `${t.head} ${t.ended}`;
  if (s.seen !== seen) {
    s.seen = seen;
    const head = t.head, moved = s.head !== head;
    const [chain, versions] = await Promise.all([moved && head ? contextOf(head) : [], versionsOf(t.id)]);
    if (logState !== s || s.seen !== seen) return;
    if (!chain) { s.seen = undefined; return; }
    if (moved) { s.head = head; stick(() => appendChain(s, chain, t)); }
    s.versions = versions;
    stick(() => markVersions(s));
  }
  stick(() => renderTail(s, t));
  revealIn(s);
}

// New entries append; entries no longer in the chain (Max moved it back, to
// another version, or a compaction replaced them) go, and the chain is drawn
// on from where the two share their prefix.
function appendChain(s, chain, t) {
  const ids = chain.map((e) => e.id);
  let keep = s.rendered.findIndex((r, i) => r.id !== ids[i]);
  if (keep === -1) keep = Math.min(s.rendered.length, ids.length);
  if (keep < s.rendered.length) {
    for (const r of s.rendered.splice(keep)) r.el.remove();
    s.group = null; s.prevText = null;
  }
  for (let i = s.rendered.length; i < chain.length; i++) {
    const e = chain[i];
    const into = el("div", "chunk");
    into.dataset.entry = e.id;
    if (s.exact) exactEntry(into, e);
    else entry(s, into, e, i, t, ids[i - 1]);
    into.dataset.role = e.role;
    s.rendered.push({ id: e.id, el: into });
    const tail = $(".tail", logEl);
    if (tail) tail.before(into); else logEl.append(into);
  }
}

function exactEntry(into, e) {
  const div = el("div", `exact-entry ${e.role}`);
  div.append(el("div", "who", e.role === "root" ? `root · ${e.model}` : e.role));
  if (e.role === "root") div.append(el("div", "io", e.system || ""), el("div", "io", e.tools || ""));
  else { const c = parse(e.content); div.append(el("div", "io", c === undefined ? e.content : JSON.stringify(c, null, 2))); }
  into.append(div);
}

export function blocksOf(e) {
  let c = parse(e.content);
  if (c === undefined) c = e.content;
  if (typeof c === "string") c = [{ type: "text", text: c }];
  return Array.isArray(c) ? c : [];
}

function entry(s, into, e, index, t, parent) {
  if (e.role === "root") { into.append(rootEntry(e)); return; }
  const blocks = blocksOf(e);
  // What the harness sent: each block as whoever wrote it, by the face's
  // account of the request that sent it. Max's words are his bubbles; the
  // harness's labels for them are not shown.
  if (e.role === "user") {
    const by = e.by || [];
    let run = null; // Max's bubble or the mandate's card a block continues
    // A compaction's carry keeps his words from what it replaced: his, but
    // editing or retrying them would replace the carry.
    const carry = by.some((w) => w?.by === "compacted");
    blocks.forEach((b, k) => {
      const who = by[k] || { by: "harness" };
      if (run && run.by !== who.by) run = null;
      if (b.type === "tool_result") { result(s, b); return; }
      closeGroup(s);
      if (b.type === "image") { const img = el("img"); img.src = `data:${b.source?.media_type};base64,${b.source?.data}`; img.style.maxWidth = "100%"; into.append(img); return; }
      if (b.type !== "text" || who.by === "label") return;
      const text = b.text || "";
      if (who.by === "max") {
        if (run) { $(".bubble", run.el).textContent += `\n${text}`; return; }
        run = { by: "max", el: said(text, clock(who.at)) };
        // His message: edit it, or run its turn again.
        if (!carry && !$(".moves", into)) {
          const bubble = $(".bubble", run.el);
          run.el.append(moves([
            ["✎", "Edit", "Edit this message: the box holds it, and sending replaces it and what followed", () => editMessage(t.id, parent, bubble.textContent)],
            ["↻", "Retry", "Run this turn again from this message", () => moveBack(e.id, "Retry")],
          ]));
        }
        into.append(run.el);
      } else if (who.by === "prompt") {
        into.append(prompted(who.prompt, text));
      } else if (who.by === "compacted") {
        if (!run) {
          run = { by: who.by, el: compacted(s, parent, who.from, t, text) };
          into.append(run.el);
        } else run.el.append(markdown(text));
      } else if (who.by === "brief" || who.by === "mandate") {
        if (!run) {
          run = { by: who.by, el: el("div", "entry card mandate") };
          run.el.title = who.by === "brief" ? "The brief its spawn shares" : `From ${byId[t.owner]?.name || "its owner"}`;
          into.append(run.el);
        }
        run.el.append(markdown(text));
        if (text.length > 900 && !$(".more-link", run.el)) {
          const card = run.el, m = el("button", "more-link", "Show all");
          card.classList.add("clip");
          m.onclick = () => { card.classList.toggle("clip"); m.textContent = card.classList.contains("clip") ? "Show all" : "Show less"; };
          card.after(m);
        }
      } else into.append(note(text));
    });
    return;
  }
  // The assistant's turn: its words read as prose; its tool uses as steps.
  for (const b of joined(blocks)) {
    if (b.type === "thinking") {
      if (!b.thinking) continue;
      const d = el("details", "thought"); d.append(el("summary", null, "Thinking"), el("div", "plain", b.thinking));
      (s.group ? s.group.el : into).append(d);
    } else if (b.type === "text") {
      closeGroup(s);
      const p = markdown(b.text || "");
      p.classList.add("entry");
      for (const a of p.querySelectorAll('a[title^="cite: "]')) { a.className = "cite"; a.title = a.title.slice(6); }
      const copy = el("button", "copy", "Copy"); copy.title = "Copy as markdown";
      copy.onclick = () => navigator.clipboard.writeText(b.text || "").then(() => { copy.textContent = "Copied"; setTimeout(() => { copy.textContent = "Copy"; }, 1200); });
      p.append(copy);
      into.append(p);
    } else if (b.type === "tool_use" || b.type === "server_tool_use") {
      step(s, into, b, t);
    } else if (b.type === "web_search_tool_result" || b.type === "web_fetch_tool_result") {
      served(s, b);
    }
  }
  // A response: sample it again, or go back to it if it ended a turn.
  const ended = !blocks.some((b) => b.type === "tool_use");
  into.append(moves([
    ["↻", "Retry", "Ask for this response again", () => moveBack(parent, "Retry")],
    ...(ended ? [["⤺", "Back to here", "Go back to this reply: what followed stays as another version", () => moveBack(e.id, "Back")]] : []),
  ]));
  const r = e.response;
  if (r && r.ended) {
    const meta = el("div", "meta", `${clock(r.ended)} · ${((r.ended - r.sent) / 1000).toFixed(1)}s · ${modelName(r.model)}${r.effort ? ` ${r.effort}` : ""} · ${kTokens(r.context)} context · ${kTokens(r.output)} out`);
    // A turn of only tools keeps its timing on its last step, so a folded group shows one.
    (s.group ? s.group.rows[s.group.rows.length - 1] : into).append(meta);
  }
}

// The root, marked up, as one folded block like a round of tool calls: the
// system prompt as it reads, a section to each of its blocks, then each tool
// by its name and the first line of what it says it does, opened to the rest
// and its parameters.
function rootEntry(e) {
  const system = parse(e.system), tools = parse(e.tools) || [];
  const d = fold(el("span", "what", `System prompt and ${tools.length} tools`), el("span", "n", modelName(e.model)));
  d.classList.add("root");
  for (const b of Array.isArray(system) ? system : [{ text: e.system || "" }]) {
    const section = markdown(b.text || "");
    section.classList.add("section");
    d.append(section);
  }
  for (const { name, description = "", input_schema, ...rest } of tools) {
    const [first, ...more] = description.split("\n");
    const detail = el("div", "detail");
    if (more.join("").trim()) detail.append(markdown(more.join("\n")));
    detail.append(params(input_schema || rest));
    d.append(foldRow(name, first, "", detail));
  }
  return d;
}
// A tool's parameters, each by its name and what it is for, a list of
// records by the fields of each.
function params(schema) {
  const d = el("div", "args");
  for (const [k, p] of Object.entries(schema?.properties || {})) {
    const v = el("div", "v");
    const inner = p.items?.properties ? p.items : p.properties ? p : null;
    if (p.description || !inner) v.append(markdown(p.description || p.type || ""));
    if (inner) v.append(params(inner));
    const f = el("div", "arg");
    f.append(el("span", "k", schema.required?.includes(k) ? k : `${k}?`), v);
    d.append(f);
  }
  return d;
}

// A block that folds to one line, its head, opened to rows that each open
// in turn: a round of tool calls, or the root.
function fold(...head) {
  const d = el("div", "fold folded");
  const h = el("div", "line head");
  h.append(...head);
  h.onclick = () => d.classList.toggle("folded");
  d.append(h);
  return d;
}
function foldRow(name, summary, glimpse, detail) {
  const row = el("div", "row"), line = el("div", "line");
  line.append(el("span", "tool", name));
  if (summary) line.append(el("span", "sum", summary));
  line.append(el("span", "gist", glimpse));
  line.onclick = () => row.classList.toggle("expanded");
  row.append(line, detail);
  return row;
}

// A compaction, where it happened: the summary the agent carried on with,
// and what it replaced (the work after `parent`, up to `from`), folded
// until opened.
function compacted(s, parent, from, t, framing) {
  const card = el("div", "entry card compacted");
  const replaced = el("details", "replaced");
  replaced.append(el("summary", "more-link", "What it replaced"));
  replaced.ontoggle = async () => {
    if (!replaced.open || replaced.dataset.done) return;
    replaced.dataset.done = "1";
    const chain = (await contextOf(from)) || [];
    // A fresh compaction's new root is not on the old chain: all of it
    // after its root was replaced.
    const at = Math.max(chain.findIndex((e) => e.id === parent), 0);
    const sub = { rows: new Map(), group: null, rendered: chain.slice(0, at + 1).map((e) => ({ id: e.id })), exact: false };
    const body = el("div", "chunk");
    for (let i = at + 1; i < chain.length; i++) entry(sub, body, chain[i], i, t, chain[i - 1].id);
    replaced.append(body);
  };
  card.append(el("div", "framing", framing.replace(/^\[harness\] /, "")), replaced);
  return card;
}

// Words in a bubble on the right: Max's, or what his user is to an agent.
// When and how they were sent is on hover.
function said(text, when, cls = "") {
  const d = el("div", `entry said ${cls}`);
  const words = parse(text);
  const body = Array.isArray(words) ? words.map((w) => w.type === "image" ? "[image]" : w.text || "").join("\n") : text;
  const bubble = el("div", "bubble", body);
  if (when) bubble.title = when;
  d.append(bubble);
  return d;
}
// A standard prompt, as the act it is: Max's Done; an abandon, his or the
// agent's owner's; his Stop, which parks it. The harness's instruction is a line under it, in full
// when clicked. Other standard prompts are the harness's notes.
const PROMPTS = { Done: "Done", Tidy: "Abandon", Park: "Stop" };
function prompted(prompt, text) {
  if (!PROMPTS[prompt]) return note(text);
  const d = said(PROMPTS[prompt], null, "prompt");
  const then = el("div", "then", text.replace(/^\[harness\] /, ""));
  then.onclick = () => then.classList.toggle("full");
  d.append(then);
  return d;
}
function note(text) {
  const n = el("div", "note"), tx = el("span", "t", text.replace(/^\[harness\] /, ""));
  tx.onclick = () => tx.classList.toggle("full");
  n.append(tx);
  return n;
}

// Consecutive tool calls with no words between them are one block, folded
// to what they did: each call's summary in turn, or the tools' names when
// the calls have none. A block of one call is just its row.
function step(s, into, b, t) {
  if (!s.group) {
    const g = fold();
    g.classList.add("steps");
    s.group = { el: g, head: $(".head", g), rows: [] };
    into.append(g);
  }
  const row = toolRow(s, b, t);
  s.group.el.append(row);
  s.group.rows.push(row);
  groupHead(s.group);
}
function groupHead(g) {
  const n = g.rows.length;
  g.el.classList.toggle("one", n === 1);
  if (n === 1) return;
  let what;
  if (g.rows.some((r) => r.dataset.summary)) what = g.rows.map((r) => r.dataset.summary || r.dataset.tool).join(" → ");
  else {
    const counts = {};
    for (const r of g.rows) counts[r.dataset.tool] = (counts[r.dataset.tool] || 0) + 1;
    what = Object.entries(counts).map(([k, v]) => v > 1 ? `${k} ×${v}` : k).join(", ");
  }
  g.head.replaceChildren(el("span", "what", what), el("span", "n", `${n} calls`));
}
function closeGroup(s) { s.group = null; }

const FILES = /\.(md|markdown|svg|html?|txt|json|rs|py|js|ts|toml|ya?ml|sh|css)$/i;
// A call's row: its tool, the agent's summary of it (its input's last
// field, `summary`; a compaction's `summary` is what it carries on with),
// and a glimpse of its real arguments; opened, its input and result.
function toolRow(s, b, t) {
  const input = b.input || {};
  const inner = b.name === "background" ? input.input || {} : input;
  const name = b.name === "background" ? `${input.tool} ↷` : b.name;
  const compact = (b.name === "background" ? input.tool : b.name)?.startsWith("compact");
  const said = compact ? "" : input.summary ?? inner.summary;
  const summary = typeof said === "string" ? said : "";
  const detail = el("div", "detail");
  detail.append(readable(compact ? input : unsummarised(input, b.name === "background")));
  const row = foldRow(name, summary, gist(inner), detail);
  row.dataset.tool = b.name;
  row.dataset.summary = summary;
  const line = $(".line", row);
  line.append(el("span", "st run", ""));
  const path = inner.path;
  if (path && FILES.test(path) && (b.name === "write" || input.tool === "write")) {
    const open = el("button", "open-file", "open"); open.onclick = (e) => { e.stopPropagation(); viewFile(path, inner.content); };
    line.insertBefore(open, line.lastChild);
  }
  // What a spawn or add_sibling started, beside it: each opens where it is
  // on the map.
  if (b.name === "spawn" || b.name === "add_sibling" || (b.name === "background" && input.tool === "spawn")) {
    const chips = el("div", "chips");
    row.insertBefore(chips, detail);
    row.chips = chips;
  }
  if (b.name.startsWith("compact")) {
    const q = el("div", "card");
    q.append(el("div", "label", input.fresh ? "Compacting into a new context" : "Compacting"), markdown(input.summary || ""));
    row.insertBefore(q, detail);
  }
  s.rows.set(b.id, { row, b, inner });
  fillChips(s, b.id);
  return row;
}
function unsummarised(input, wrapped) {
  const { summary, ...rest } = input;
  if (wrapped && rest.input && typeof rest.input === "object") { const { summary, ...inner } = rest.input; rest.input = inner; }
  return rest;
}
// What the provider's web search or fetch found, in the row of its use.
function served(s, b) {
  const r = s.rows.get(b.tool_use_id);
  if (!r) return;
  const c = b.content, failed = c && !Array.isArray(c) && c.error_code;
  const st = $(".st", r.row);
  st.className = `st${failed ? " bad" : ""}`;
  st.textContent = failed ? "failed" : "✓";
  const found = Array.isArray(c) ? c.filter((x) => x.url) : c?.url ? [{ url: c.url, title: c.content?.title }] : [];
  const out = el("div", "io found");
  if (failed) out.textContent = c.error_code;
  for (const x of found) {
    const a = el("a", null, x.title || x.url);
    a.href = x.url; a.target = "_blank"; a.rel = "noopener";
    const li = el("div"); li.append(a, el("span", "host", ` ${new URL(x.url).host}${x.page_age ? ` · ${x.page_age}` : ""}`)); out.append(li);
  }
  $(".detail", r.row).append(out);
}
// Consecutive text blocks read as one passage: a cited answer comes as
// many small blocks, each cited one followed by links to its sources.
function joined(blocks) {
  const out = [];
  for (const b of blocks) {
    if (b.type !== "text") { out.push(b); continue; }
    const urls = [...new Set((b.citations || []).map((c) => c.url).filter(Boolean))];
    const text = (b.text || "") + urls.map((u) => {
      const title = (b.citations.find((c) => c.url === u)?.title || u).replace(/["\\]/g, "");
      return ` [${new URL(u).host.replace(/^www\./, "")}](${u} "cite: ${title}")`;
    }).join("");
    const prev = out[out.length - 1];
    if (prev?.type === "text") prev.text += text;
    else out.push({ type: "text", text });
  }
  return out;
}
function fillChips(s, toolUse) {
  const r = s.rows.get(toolUse);
  if (!r?.row.chips) return;
  const members = view.tasks.filter((x) => x.started_by === toolUse).sort(order);
  const key = JSON.stringify(members.map((m) => [m.id, look(m).cls, glyph(m), shortName(m)]));
  if (r.row.chips.dataset.key === key) return;
  r.row.chips.dataset.key = key;
  r.row.chips.replaceChildren(...members.map(chip));
}
function chip(m) {
  const l = look(m);
  const a = el("a", `chip ${l.cls}`);
  a.href = `#${m.id}`;
  a.onclick = (e) => { if (e.button || e.ctrlKey || e.metaKey) return; e.preventDefault(); go(m.id); };
  a.append(el("span", "d", glyph(m)), shortName(m));
  a.title = l.text;
  return a;
}
// Moving the chat: a row of small buttons under an entry, refused with
// the reason while the task cannot move.
function moves(list) {
  const row = el("div", "moves");
  for (const [glyph, label, title, run] of list) {
    const b = el("button", "mv", `${glyph} ${label}`);
    b.type = "button"; b.title = title;
    b.onclick = (ev) => { ev.stopPropagation(); run(); };
    row.append(b);
  }
  return row;
}
// Each drawn entry's versions, drawn anew: the arrows between its siblings,
// and how many times the model gave it, when more than once.
function markVersions(s) {
  for (const r of s.rendered) {
    $(".versions", r.el)?.remove();
    const v = s.versions?.[r.id];
    if (v) r.el.prepend(versionArrows(v, r.el.dataset.role));
  }
}
function versionArrows(v, role) {
  const d = el("div", `versions ${role}`);
  const go = (to, word) => {
    const b = el("button", "mv", word);
    b.type = "button"; b.disabled = !to;
    b.title = word === "‹" ? "The version before" : "The version after";
    b.onclick = (ev) => { ev.stopPropagation(); moveBack(to, "Version"); };
    return b;
  };
  if (v.of) d.append(go(v.prev, "‹"), el("span", "n", `${v.n}/${v.of}`), go(v.next, "›"));
  if (v.tries) {
    const n = el("span", "tries", `said ${v.tries}×`);
    n.title = `Asked ${v.tries} times: the model gave these same words each time`;
    d.append(n);
  }
  return d;
}
// Why the task cannot be moved now, if it cannot.
export function unmovable(t) {
  if (!t) return "no task";
  if (isDone(t)) return "it has finished";
  if (["model", "calls", "starting"].includes(t.waiting?.kind)) return "it is working: stop it first";
  return null;
}
async function moveBack(to, what) {
  const t = byId[logState?.id];
  const blocked = unmovable(t);
  if (blocked) return say(`${what}: ${blocked}`, true);
  if (!to) return;
  const why = await act({ Task: [t.id, { Moved: { Back: { to } } }] });
  if (why) say(`${what}: ${why}`, true);
}

// A tool use's result, shown in its row: again, if a move back drew it anew.
function result(s, b) {
  const r = s.rows.get(b.tool_use_id);
  if (!r) return;
  for (const old of r.row.querySelectorAll(".res")) old.remove();
  r.row.classList.remove("live");
  const st = $(".st", r.row);
  const text = typeof b.content === "string" ? b.content : (b.content || []).map((x) => x.type === "text" ? x.text : `[${x.type}]`).join("\n");
  st.className = `st${b.is_error ? " bad" : ""}`;
  st.textContent = b.is_error ? "failed" : "✓";
  const detail = $(".detail", r.row);
  const fields = parse(text);
  const out = fields && typeof fields === "object" ? readable(fields) : el("div", "io", text);
  out.classList.add("res");
  if (b.is_error) out.classList.add("err");
  const path = r.inner.path;
  if (r.b.name === "read" && path && /\.(md|markdown|svg|html?)$/i.test(path) && !b.is_error) {
    const raw = text.split("\n").map((l) => l.replace(/^\s*\d+[\t│|:] ?/, "")).join("\n");
    const open = el("button", "open-file res", "open");
    open.onclick = (e) => { e.stopPropagation(); viewFile(path, raw); };
    $(".line", r.row).insertBefore(open, st);
  }
  if (r.b.name === "spawn") {
    const card = el("div", "card result");
    card.append(markdown(text));
    const d = el("details", "res"); d.append(el("summary", "more-link", "Their results"), card);
    r.row.insertBefore(d, detail);
  }
  for (const x of Array.isArray(b.content) ? b.content : []) if (x.type === "image") { const img = el("img", "res"); img.src = `data:${x.source?.media_type};base64,${x.source?.data}`; img.style.maxWidth = "100%"; detail.append(img); }
  detail.append(out);
}
// A tool's input, or a result that is data, as it reads: each field by its
// name, an empty one left out; a string as text (an agent's own words as
// markdown), code as a block; a list or a record as fields of its own.
const CODE = new Set(["command", "content", "code", "script", "patch", "old", "new", "old_string", "new_string"]);
const WORDS = new Set(["mandate", "brief", "note", "text", "message", "question", "prompt", "description"]);
function readable(value, key) {
  if (typeof value === "string") {
    if (CODE.has(key)) return el("div", "io", value);
    if (WORDS.has(key)) { const m = markdown(value); m.classList.add("v"); return m; }
    return el("div", value.includes("\n") ? "v long" : "v", value);
  }
  if (!value || typeof value !== "object") return el("div", "v code", JSON.stringify(value));
  const d = el("div", "args");
  for (const [k, v] of Object.entries(value)) {
    if (v === "" || v == null) continue;
    const f = el("div", "arg");
    f.append(el("span", "k", Array.isArray(value) ? `${+k + 1}` : k), readable(v, k));
    d.append(f);
  }
  return d;
}

// A file an agent wrote or read, shown as itself beside the log.
function viewFile(path, content) {
  const v = $(".viewer", panel);
  v.hidden = false;
  const head = el("header");
  const raw = el("button", "btn", "Raw"), close = el("button", "x"); close.title = "Close";
  head.append(el("span", "path", path), raw, close);
  const body = el("div", "content");
  const show = (asRaw) => {
    body.className = "content";
    if (asRaw) body.replaceChildren(el("div", "io", content));
    else if (/\.svg$/i.test(path)) { const img = el("img"); img.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(content)}`; img.style.maxWidth = "100%"; body.replaceChildren(img); }
    else if (/\.html?$/i.test(path)) { const f = el("iframe"); f.sandbox = ""; f.srcdoc = content; body.className = "content frame"; body.replaceChildren(f); }
    else if (/\.(md|markdown)$/i.test(path)) body.replaceChildren(markdown(content));
    else body.replaceChildren(el("div", "io", content));
  };
  let asRaw = false;
  raw.onclick = () => { asRaw = !asRaw; raw.classList.toggle("armed", asRaw); show(asRaw); };
  close.onclick = () => { v.hidden = true; };
  show(false);
  v.replaceChildren(head, body);
}

// What is not in the context yet: what is streaming now, what Max sent that
// the agent has not read, how it went wrong.
function renderTail(s, t) {
  let tail = $(".tail", logEl);
  if (!tail) { tail = el("div", "tail"); logEl.append(tail); }
  for (const [id] of s.rows) fillChips(s, id);
  logEl.classList.toggle("unmovable", !!unmovable(t));
  // Running calls of the round: their rows show them running.
  for (const c of t.waiting?.kind === "calls" ? t.waiting.calls : []) {
    if (c.children.length) continue;
    const r = [...s.rows.values()].findLast((r) => r.b.name === c.tool && JSON.stringify(r.b.input) === JSON.stringify(parse(c.input)));
    if (r && c.status === "running") followCall(c, r.row);
  }
  const parts = [];
  if (t.waiting?.kind === "model") parts.push(followModel(t.waiting.call));
  else stopModel();
  // Not read yet: dashed, until the request that reads it.
  for (const p of t.pending || []) {
    const when = `${clock(p.at)} · ${p.read === "TurnEnd" ? "read when its turn ends" : "not read yet"}`;
    parts.push(p.prompt ? said(PROMPTS[p.prompt] || p.prompt, when, "queued-words prompt") : said(p.content, when, "queued-words"));
  }
  if (!t.head && !isDone(t)) parts.push(t.owner ? mandateCard(t) : said(t.mandate || "", "not read yet", "queued-words"));
  const bad = trouble(t);
  if (bad) parts.push(el("div", "entry card ended", bad));
  const o = t.outcome;
  if (o?.kind === "failed") parts.push(el("div", "entry card ended", `Failed: ${o.reason}`));
  const key = JSON.stringify([t.waiting?.kind, t.waiting?.call, t.pending, bad, o, t.head]);
  if (tail.dataset.key !== key || parts.some((p) => p.classList?.contains("live-holder"))) {
    tail.dataset.key = key;
    tail.replaceChildren(...parts);
  }
}
function mandateCard(t) {
  const c = el("div", "entry card mandate");
  c.title = `From ${byId[t.owner]?.name || "its owner"}`;
  c.append(markdown(t.mandate || ""));
  return c;
}

// The log keeps to its bottom while Max is there, so live text scrolls into
// view; once he scrolls up, it stays where he is.
function stick(change) {
  const atBottom = scrollEl.scrollHeight - scrollEl.scrollTop - scrollEl.clientHeight < 80 || logState?.follow;
  change();
  if (atBottom) { scrollEl.scrollTop = scrollEl.scrollHeight; if (logState) logState.follow = false; }
  else $(".jump", panel).hidden = false;
}
scrollEl.addEventListener("scroll", () => { if (scrollEl.scrollHeight - scrollEl.scrollTop - scrollEl.clientHeight < 80) $(".jump", panel).hidden = true; });
$(".jump", panel).onclick = () => { scrollEl.scrollTo({ top: scrollEl.scrollHeight, behavior: "smooth" }); };

// A request streaming, or a command printing: neither is a fact. Each
// arrives on the link as live pieces, all of them again after a reconnect.
const pieces = new Map(), watchers = new Map();
export function liveArrived(l) {
  const [kind] = Object.keys(l), key = `${kind}:${l[kind].call}`;
  const text = (pieces.get(key) || "") + l[kind].chunk;
  pieces.set(key, text);
  watchers.get(key)?.(text);
}
export function liveReset() { pieces.clear(); for (const f of watchers.values()) f(""); }
function watch(key, f) { watchers.set(key, f); f(pieces.get(key) || ""); }
let modelStream = null;
const callStreams = new Set();
function followModel(call) {
  if (modelStream?.call === call) return modelStream.holder;
  stopModel();
  const holder = el("div", "entry live-holder");
  const prose = el("div", "prose live-text");
  holder.append(prose);
  let text = "", pendingDraw = null;
  modelStream = { call, holder };
  watch(`Model:${call}`, (t) => {
    text = t;
    if (pendingDraw) return;
    pendingDraw = setTimeout(() => {
      pendingDraw = null;
      stick(() => { if (window.marked && window.DOMPurify) prose.innerHTML = DOMPurify.sanitize(marked.parse(text)); else prose.textContent = text; });
    }, 80);
  });
  return holder;
}
function stopModel() { if (modelStream) { watchers.delete(`Model:${modelStream.call}`); modelStream = null; } }
function followCall(c, row) {
  if (callStreams.has(c.id)) return;
  // Shown while it runs, even in a folded block; its result replaces it.
  const out = el("div", "io res");
  $(".detail", row).append(out);
  row.classList.add("expanded", "live");
  callStreams.add(c.id);
  watch(`Call:${c.id}`, (raw) => stick(() => { out.textContent = screen(raw); }));
}
export function stopFollowing() { stopModel(); for (const id of callStreams) watchers.delete(`Call:${id}`); callStreams.clear(); }
// A command's output as a terminal would leave it: only the last screenful.
function screen(raw) {
  const lines = raw.replace(/\r\n/g, "\n").split("\n").map((l) => l.slice(l.lastIndexOf("\r") + 1));
  return (lines.length > 40 ? [`[${lines.length - 40} earlier lines]`, ...lines.slice(-40)] : lines).join("\n");
}

