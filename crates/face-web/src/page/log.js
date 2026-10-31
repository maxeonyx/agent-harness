import { $, clock, el, getJson, gist, icon, ICONS, kTokens, logEl, markdown, panel, parse, scrollEl } from "./base.js";
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
// and each request that ends, since any request can add one.
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
    else {
      const carry = carryBefore(chain, i);
      if (carry) into.append(compaction(carry, t));
      entry(s, into, e, i, t, ids[i - 1]);
    }
    into.dataset.role = e.role;
    s.rendered.push({ id: e.id, el: into });
    const tail = $(".tail", logEl);
    if (tail) tail.before(into); else logEl.append(into);
  }
}

function exactEntry(into, e) {
  const div = el("div", `exact-entry ${e.role}`);
  const who = e.role[0].toUpperCase() + e.role.slice(1);
  div.append(el("div", "who", e.role === "root" ? `${who}: ${e.model}` : who));
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

// `only` draws part of a message: its tool results ("results"), or all
// but them ("words").
function entry(s, into, e, index, t, parent, only) {
  if (e.role === "root") { into.append(rootEntry(e)); return; }
  const blocks = blocksOf(e);
  // What the harness sent: each block as whoever wrote it, by the face's
  // account of the request that sent it. Max's words are his bubbles; the
  // harness's labels for them are not shown.
  if (e.role === "user") {
    const by = e.by || [];
    let run = null; // Max's bubble or the mandate's card a block continues
    // A compaction's carry keeps his words from what it replaced: the
    // harness handing them back, so a note of his words, not a new turn.
    const carry = by.some((w) => w?.by === "compacted");
    blocks.forEach((b, k) => {
      const who = by[k] || { by: "harness" };
      if (run && run.by !== who.by) run = null;
      if (b.type === "tool_result") { if (only !== "words") result(s, b); return; }
      if (only === "results") return;
      closeGroup(s);
      if (b.type === "image") { const img = el("img"); img.src = `data:${b.source?.media_type};base64,${b.source?.data}`; img.style.maxWidth = "100%"; into.append(img); return; }
      if (b.type !== "text" || who.by === "label") return;
      const text = b.text || "";
      if (who.by === "max" && carry) {
        if (run) { run.text += `\n${text}`; run.el.lastChild.replaceWith(quiet(run.text, true)); return; }
        run = { by: "max", text, el: el("div", "note kept") };
        const w = el("span", "who", "Max wrote");
        w.title = `Carried from the work it replaced; he wrote it at ${clock(who.at)}`;
        run.el.append(w, quiet(text, true));
        into.append(run.el);
      } else if (who.by === "max") {
        if (run) { $(".bubble", run.el).textContent += `\n${text}`; return; }
        run = { by: "max", el: said(text, clock(who.at)) };
        // His message: edit it.
        if (!carry && !s.past && !$(".moves", into)) {
          const bubble = $(".bubble", run.el);
          run.el.append(moves([
            ["edit", "Edit", "Edit this message: the box holds it, and sending replaces it and what followed", () => editMessage(t.id, parent, bubble.textContent)],
          ]));
        }
        into.append(run.el);
      } else if (who.by === "prompt") {
        into.append(prompted(who.prompt, text));
      } else if (who.by === "compacted") {
        // The harness's framing is its note; the summary is the agent's own.
        const carried = text.match(/^What you carried:\n\n([\s\S]*)$/);
        if (!carried) { into.append(note(text)); return; }
        const card = el("div", "entry card carried");
        card.append(el("div", "label", "What it carried"), markdown(carried[1]));
        into.append(card);
      } else if (who.by === "brief" || who.by === "mandate") {
        if (!run) {
          run = { by: who.by, el: mandateCard(who.by === "brief" ? "The brief its spawn shares" : `From ${byId[t.owner]?.name || "its owner"}`) };
          into.append(run.el);
        }
        $(".body", run.el).append(markdown(text));
      } else into.append(note(text));
    });
    return;
  }
  // The assistant's turn: its words read as prose; its tool uses as steps.
  closeGroup(s);
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
      const shown = p.cloneNode(true);
      $(".raw-toggle", shown).remove();
      const html = shown.innerHTML;
      const copy = icon("copy", "copy", "Copy");
      copy.onclick = () => copied(copy, b.text || "", html);
      p.append(copy);
      into.append(p);
    } else if (b.type === "tool_use" || b.type === "server_tool_use") {
      step(s, into, b, t);
    } else if (b.type === "web_search_tool_result" || b.type === "web_fetch_tool_result") {
      served(s, b);
    }
  }
  const r = e.response;
  if (r && r.ended) {
    const meta = el("div", "meta");
    for (const [text, title] of [
      [clock(r.ended), "When it ended"], [`${((r.ended - r.sent) / 1000).toFixed(1)}s`, "How long it took"],
      [`${modelName(r.model)}${r.effort ? ` ${r.effort}` : ""}`, "The model, and its effort"],
      [`${kTokens(r.context)} context`, "The context it was sent"], [`${kTokens(r.output)} out`, "What it wrote"],
    ]) { const s = el("span", null, text); s.title = title; meta.append(s); }
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
  // The summary says what the call is for; its arguments are one click further.
  line.append(summary ? el("span", "sum", summary) : el("span", "gist", glimpse));
  line.onclick = () => row.classList.toggle("expanded");
  row.append(line, detail);
  return row;
}

// A compaction, as it happened, where its new context starts: what it
// replaced, drawn once Max scrolls up to it; the harness's words the agent
// compacted on, and its compaction call; then a line across. The new
// context follows: a fresh one's root, then the carry.
const isCarry = (e) => e?.by?.some((w) => w?.by === "compacted");
// The carry whose compaction is drawn before entry `i`, and the checkpoint
// it went back to (none for a fresh context): a fresh context's before its
// root, one onto a checkpoint before itself.
function carryBefore(chain, i) {
  if (chain[i].role === "root") return isCarry(chain[i + 1]) ? { carry: chain[i + 1] } : null;
  return i > 1 && isCarry(chain[i]) ? { carry: chain[i], onto: chain[i - 1].id } : null;
}
function compaction({ carry, onto }, t) {
  const from = carry.by.find((w) => w?.by === "compacted").from;
  const box = el("div", "compaction");
  const earlier = el("div", "earlier", "Earlier context"), last = el("div"), line = el("div", "divider");
  line.append(el("span", null, onto ? "Compacted back to a checkpoint" : "Compacted into a new context"));
  box.append(earlier, last, line);
  contextOf(from).then((old) => {
    if (!old) return;
    const start = onto ? old.findIndex((e) => e.id === onto) + 1 : 0;
    const end = old.length - 1;
    const call = old[end].role === "assistant" && blocksOf(old[end]).some((b) => b.type === "tool_use" && b.name.startsWith("compact"));
    // The message the call answered: its words show with the call, its
    // results with the calls above that they answer.
    const said = call && end - 1 >= start && old[end - 1].role === "user" ? end - 1 : -1;
    const only = (i) => i === said ? "results" : undefined;
    if (call) {
      line.lastChild.textContent += ` at ${clock(old[end].response?.ended)}`;
      keep(line, () => {
        const s = { rows: new Map(), group: null, past: true };
        if (said >= 0) drawPast(s, last, old, said, said + 1, t, () => "words");
        drawPast(s, last, old, end, end + 1, t);
      });
    } else line.lastChild.textContent = "Full: the harness started the context again";
    const to = call ? end : end + 1;
    if (start >= to) { earlier.remove(); return; }
    onSight(earlier, () => {
      const body = el("div", "replaced");
      drawPast({ rows: new Map(), group: null, past: true }, body, old, start, to, t, only);
      return body;
    });
  });
  return box;
}
// Entries `from` to `to` of a context that is no longer the task's.
function drawPast(s, into, chain, from, to, t, only = () => undefined) {
  for (let i = from; i < to; i++) {
    const chunk = el("div", "chunk");
    const carry = carryBefore(chain, i);
    if (carry) chunk.append(compaction(carry, t));
    entry(s, chunk, chain[i], i, t, chain[i - 1]?.id, only(i));
    into.append(chunk);
  }
}
// `placeholder` replaced by what `draw` makes, once Max scrolls near it.
function onSight(placeholder, draw) {
  const seen = new IntersectionObserver((sighted) => {
    if (!sighted.some((x) => x.isIntersecting)) return;
    seen.disconnect();
    keep(placeholder.nextElementSibling, () => placeholder.replaceWith(draw()));
  }, { root: scrollEl, rootMargin: "200px 0px 0px 0px" });
  seen.observe(placeholder);
}
// A change above `anchor` that leaves what Max sees where it was.
function keep(anchor, change) {
  const y = anchor.getBoundingClientRect().top;
  change();
  scrollEl.scrollTop += anchor.getBoundingClientRect().top - y;
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
  d.append(quiet(text));
  return d;
}
function note(text) {
  const n = el("div", "note");
  n.append(quiet(text));
  return n;
}
// What its CSS cuts short (`cutShort` says when) opens to all of it at a
// tap on itself, and closes again at another; what fits has no affordance.
const cuts = new ResizeObserver((seen) => {
  for (const { target } of seen) if (!target.classList.contains("full")) target.classList.toggle("cut", target.cutShort());
});
function openable(node, cutShort) {
  node.cutShort = cutShort;
  node.addEventListener("click", (e) => {
    if (node.classList.contains("cut") && !e.target.closest("a, button") && !getSelection().toString()) node.classList.toggle("full");
  });
  cuts.observe(node);
}
// The harness's words, one quiet line of Markdown. Max's words it hands
// back are plain, as his bubbles are.
function quiet(text, plain) {
  const q = plain ? el("div", "prose") : markdown(text.replace(/^\[harness\] /, ""));
  if (plain) q.append(el("div", "plain", text));
  q.classList.add("quiet");
  openable(q, () => {
    const parts = [...q.children].filter((c) => !c.matches(".raw-toggle"));
    return parts.length > 1 || parts.some((c) => c.scrollWidth > c.clientWidth || c.scrollHeight > c.clientHeight + 1 || (c.matches(".plain") && c.textContent.includes("\n")));
  });
  return q;
}
// What Max copies: the Markdown as plain text, and as rich text where it
// is pasted somewhere that takes it.
async function copied(button, text, html) {
  const item = window.ClipboardItem && new ClipboardItem({
    "text/plain": new Blob([text], { type: "text/plain" }), "text/html": new Blob([html], { type: "text/html" }),
  });
  await (item ? navigator.clipboard.write([item]) : navigator.clipboard.writeText(text));
  $("svg", button).innerHTML = ICONS.tick;
  setTimeout(() => { $("svg", button).innerHTML = ICONS.copy; }, 1200);
}

// One response's tool calls, with no words between them, are one block, folded
// to what they did: each call's summary in turn, or the tools' names when
// the calls have none. A block of one call is just its row.
function step(s, into, b, t) {
  if (!s.group) {
    const g = fold();
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
  const what = el("span", "what");
  what.append(...g.rows.flatMap((r, i) => [...(i ? [" → "] : []), el("span", "tool", $(".tool", r).textContent)]));
  g.head.replaceChildren(what, el("span", "n", `${n} calls`));
}
function closeGroup(s) { s.group = null; }

const FILES = /\.(md|markdown|svg|html?|txt|json|rs|py|js|ts|toml|ya?ml|sh|css)$/i;
// A call's row: its tool, the agent's summary of it (its input's last
// field, `summary`), and a glimpse of its real arguments; opened, its input
// and result. A compaction carries on with its `carry`; one recorded before
// `carry` existed has its text in `summary` instead.
function toolRow(s, b, t) {
  let input = b.input || {};
  if (b.name.startsWith("compact") && input.carry === undefined) {
    const { summary: carry, ...rest } = input;
    input = { ...rest, carry };
  }
  const inner = b.name === "background" ? input.input || {} : input;
  const name = b.name === "background" ? `${input.tool} ↷` : b.name;
  const said = input.summary ?? inner.summary;
  const summary = typeof said === "string" ? said : "";
  const detail = el("div", "detail");
  detail.append(readable(unsummarised(input, b.name === "background")));
  const row = foldRow(name, summary, gist(inner), detail);
  const line = $(".line", row);
  // A compaction call has no result: the context it would go to is gone.
  const compact = b.name.startsWith("compact");
  line.append(el("span", compact ? "st" : "st run", compact ? "✓" : ""));
  const path = inner.path;
  if (path && FILES.test(path) && (b.name === "write" || input.tool === "write")) {
    const open = icon("open", "open-file", "Open"); open.onclick = (e) => { e.stopPropagation(); viewFile(path, inner.content); };
    line.insertBefore(open, line.lastChild);
  }
  // What a spawn or add_sibling started, under its parameters.
  if (b.name === "spawn" || b.name === "add_sibling" || (b.name === "background" && input.tool === "spawn")) {
    row.members = el("div", "members");
    row.append(row.members);
  }
  s.rows.set(b.id, { row, b, inner, results: new Map(), open: new Set() });
  fillMembers(s, b.id);
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
  st.textContent = failed ? "Failed" : "✓";
  const found = Array.isArray(c) ? c.filter((x) => x.url) : c?.url ? [{ url: c.url, title: c.content?.title }] : [];
  const out = el("div", "io found");
  if (failed) out.textContent = c.error_code;
  for (const x of found) {
    const a = el("a", null, x.title || x.url);
    a.href = x.url; a.target = "_blank"; a.rel = "noopener";
    const li = el("div"); li.append(a, el("span", "host", ` ${new URL(x.url).host}${x.page_age ? `, ${x.page_age}` : ""}`)); out.append(li);
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
// The agents a call started: a pill to each while it works, and once it
// is done a line, opened to its part of the call's result. Each name goes
// to the agent. A part no agent here answers to (one the map has not
// shown yet) is a line by its name alone.
function fillMembers(s, toolUse) {
  const r = s.rows.get(toolUse);
  if (!r?.row.members) return;
  const members = view.tasks.filter((x) => x.started_by === toolUse).sort(order);
  const partOf = (m) => r.results.has(m.name) ? m.name : shortName(m);
  const key = JSON.stringify([members.map((m) => [m.id, look(m).cls, glyph(m), shortName(m)]), [...r.results.keys()], [...r.open]]);
  if (r.row.members.dataset.key === key) return;
  r.row.members.dataset.key = key;
  const working = members.filter((m) => !isDone(m) && !r.results.has(partOf(m)));
  const named = new Set(members.map(partOf));
  const lines = [
    ...members.filter((m) => !working.includes(m)).map((m) => memberLine(r, partOf(m), m)),
    ...[...r.results.keys()].filter((n) => !named.has(n)).map((n) => memberLine(r, n, null)),
  ];
  const pills = el("div", "chips");
  pills.append(...working.map(chip));
  r.row.members.replaceChildren(...lines, ...(working.length > 0 ? [pills] : []));
}
function memberLine(r, name, m) {
  const said = r.results.get(name);
  const d = el("div", `member${r.open.has(name) ? " open" : ""}${said === undefined ? "" : " has"}`);
  const line = el("div", "line");
  line.append(m ? chip(m) : el("span", "tool", name), el("span", "gist", said === undefined ? "" : firstLine(said)));
  if (said !== undefined) line.onclick = (e) => { if (e.target.closest("a")) return; if (!r.open.delete(name)) r.open.add(name); d.classList.toggle("open"); };
  d.append(line);
  if (said !== undefined) d.append(markdown(said));
  return d;
}
const firstLine = (text) => text.split("\n").map((l) => l.replace(/^[#>*\-\s]+/, "").replace(/[*_`]/g, "")).find((l) => l.trim()) || "";
function chip(m) {
  const l = look(m);
  const a = el("a", `chip ${l.cls}`);
  a.href = `#${m.id}`;
  a.onclick = (e) => { if (e.button || e.ctrlKey || e.metaKey) return; e.preventDefault(); go(m.id); };
  a.append(el("span", "d", glyph(m)), shortName(m));
  a.title = l.text;
  return a;
}
// A scope's result, each agent's part under its name (harness-program's
// `meta::result`): `## name` alone in a text block, for a name the call
// gave, then what it said.
function parts(content, names) {
  if (!Array.isArray(content)) return null;
  const out = new Map();
  let name = null;
  for (const b of content.map((x) => x.type === "text" ? x.text : `[${x.type}]`)) {
    const head = /^## ([^\n]+)$/.exec(b);
    if (head && names.has(head[1])) { name = head[1]; out.set(name, ""); continue; }
    if (name === null) return null;
    out.set(name, [out.get(name), b].filter((x) => x).join("\n\n"));
  }
  return out;
}
// Moving the chat: a row of small buttons under an entry, refused with
// the reason while the task cannot move.
function moves(list) {
  const row = el("div", "moves");
  for (const [glyph, label, title, run] of list) {
    const b = el("button", "mv");
    b.innerHTML = `<svg viewBox="0 0 24 24">${ICONS[glyph]}</svg>`;
    b.append(label);
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
    b.onclick = (ev) => { ev.stopPropagation(); moveBack(to); };
    return b;
  };
  if (v.of) d.append(go(v.prev, "‹"), el("span", "n", `${v.n}/${v.of}`), go(v.next, "›"));
  if (v.tries) {
    const n = el("span", "tries", `Said ${v.tries} times`);
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
async function moveBack(to) {
  const t = byId[logState?.id];
  const blocked = unmovable(t);
  if (blocked) return say(`Version: ${blocked}`, true);
  if (!to) return;
  const why = await act({ Task: [t.id, { Moved: { Back: { to } } }] });
  if (why) say(`Version: ${why}`, true);
}

// A tool use's result, shown in its row: again, if a move back drew it anew.
function result(s, b) {
  const r = s.rows.get(b.tool_use_id);
  if (!r) return;
  for (const old of r.row.querySelectorAll(".res")) old.remove();
  // A call watched live folds when it settles, so its output does not stay in the chat.
  if (r.row.classList.contains("live")) r.row.classList.remove("live", "expanded");
  const st = $(".st", r.row);
  const text = typeof b.content === "string" ? b.content : (b.content || []).map((x) => x.type === "text" ? x.text : `[${x.type}]`).join("\n");
  st.className = `st${b.is_error ? " bad" : ""}`;
  st.textContent = b.is_error ? "Failed" : "✓";
  const detail = $(".detail", r.row);
  const fields = parse(text);
  const out = fields && typeof fields === "object" ? readable(fields) : el("div", "io", text);
  out.classList.add("res");
  if (b.is_error) out.classList.add("err");
  const path = r.inner.path;
  if (r.b.name === "read" && path && /\.(md|markdown|svg|html?)$/i.test(path) && !b.is_error) {
    const raw = text.split("\n").map((l) => l.replace(/^\s*\d+[\t│|:] ?/, "")).join("\n");
    const open = icon("open", "open-file res", "Open");
    open.onclick = (e) => { e.stopPropagation(); viewFile(path, raw); };
    $(".line", r.row).insertBefore(open, st);
  }
  // A spawn's result is its agents' parts, each in its agent's line.
  const byAgent = r.b.name === "spawn" && !b.is_error ? parts(b.content, new Set((r.inner.tasks || []).map((x) => x.name))) : null;
  if (byAgent) { r.results = byAgent; fillMembers(s, b.tool_use_id); return; }
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
  // Markdown has its own Raw.
  raw.hidden = /\.(md|markdown)$/i.test(path);
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
  for (const [id] of s.rows) fillMembers(s, id);
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
    const when = `${clock(p.at)}, not read yet`;
    parts.push(p.prompt ? said(PROMPTS[p.prompt] || p.prompt, when, "queued-words prompt") : said(p.content, when, "queued-words"));
  }
  if (!t.head && !isDone(t)) parts.push(t.owner ? unread(t) : said(t.mandate || "", "Not read yet", "queued-words"));
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
function unread(t) {
  const c = mandateCard(`From ${byId[t.owner]?.name || "its owner"}`);
  $(".body", c).append(markdown(t.mandate || ""));
  return c;
}
// A card of an agent's brief or mandate, cut short when long.
function mandateCard(title) {
  const c = el("div", "entry card mandate"), body = el("div", "body");
  c.title = title;
  c.append(body);
  openable(c, () => body.scrollHeight > body.clientHeight + 1);
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
  return (lines.length > 12 ? [`[${lines.length - 12} earlier lines]`, ...lines.slice(-12)] : lines).join("\n");
}

