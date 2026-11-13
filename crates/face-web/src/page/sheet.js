import { $, clock, diagram, el, panel, scrollEl } from "./base.js";
import { answer, answered } from "./compose.js";
import { load, oklch, paintScroll, wear } from "./theme.js";

// ─── A chat's page: what its agent presented to Max, drawn by the theme ───
// The brain parses every body (harness-body) and the row carries what it
// parsed, so this draws exactly what the brain checked: the agent gives
// structure, and the look is the theme's alone, through the language's CSS
// tokens on `.sheet` (theme.js `wear` sets them; sheet.css has the plain
// default).
//
// Facing Max (`page.current`), the page is the card's reading: it follows
// the chat, which scrolling up shows, and opens at its top. While the agent
// works, its progress is the card's reading and the page is one line at the
// top, which opens it.
const sheetEl = $(".sheet", panel), lineEl = $(".sheet-line", panel);
let opened = null, landed = null, unfolded = null;

// Draws `t`'s page, and says whether the card should open at its top
// (then `sheetSeen`): the first time this tab shows this version in full.
export function renderSheet(t) {
  const p = t.page;
  if (opened && opened !== t.id) opened = null;
  if (!p) { sheetEl.hidden = lineEl.hidden = true; sheetEl.drawn = null; return false; }
  const full = p.current || opened === t.id;
  lineEl.hidden = full;
  sheetEl.hidden = !full;
  if (!full) { line(p, t); return false; }
  const key = JSON.stringify([t.id, p, t.look, unfolded]);
  if (sheetEl.drawn !== key) { sheetEl.drawn = key; sheetEl.replaceChildren(...page(p, t)); dress(t); }
  return landed !== shown(t);
}
// Each present is landed on once: a new version, or one presented again.
const shown = (t) => `${t.id} ${t.page.version} ${t.page.held}`;
export const sheetSeen = (t) => { landed = shown(t); };
// Where the card lands on the page while it shows: its top, or with a quick
// answer, the chat's line above it, the page folded below it.
export function sheetTop() {
  if (sheetEl.hidden) return null;
  const top = scrollEl.scrollTop + sheetEl.getBoundingClientRect().top - scrollEl.getBoundingClientRect().top;
  return sheetEl.classList.contains("folded") ? Math.max(0, top + sheetEl.offsetHeight - scrollEl.clientHeight) : top;
}

// The page wears its task's look: the theme's tokens, and its ground painted down the whole page as
// a hanging scroll (theme.js `paintScroll`), once per task, width and layout. A task with none wears
// sheet.css's plain default.
const grounds = new Map();
async function dress(t) {
  if (!t.look) { sheetEl.removeAttribute("style"); lineEl.removeAttribute("style"); return; }
  const theme = await load(t.look.theme);
  await wear(sheetEl, theme);
  await wear(lineEl, theme);
  const lead = $(".lead", sheetEl);
  if (!lead?.isConnected) return;
  // After the browser has laid the page out and drawn it with its paper: the painting never holds it up.
  await new Promise((ok) => requestAnimationFrame(() => setTimeout(ok)));
  if (!lead.isConnected) return;
  const scroller = sheetEl.closest(".scroll"), box = sheetEl.getBoundingClientRect();
  const rel = (r) => ({ x: r.left - box.left, y: r.top - box.top, w: r.width, h: r.height });
  const h1 = rel($("h1", lead).getBoundingClientRect());
  const blocks = [...sheetEl.querySelectorAll(":scope > section :is(p, li, table, pre, figure, blockquote, summary, .ask, .head)")].map((e) => rel(e.getBoundingClientRect())).filter((b) => b.w && b.h);
  const colEl = $(":scope > section", sheetEl) || lead, cs = getComputedStyle(colEl), c = colEl.getBoundingClientRect();
  const col = [c.left - box.left + parseFloat(cs.paddingLeft), c.right - box.left - parseFloat(cs.paddingRight)];
  const L = { w: Math.round(box.width), h: Math.round(sheetEl.scrollHeight), screen: scroller?.clientHeight || innerHeight, quiet: h1, blocks, col, below: rel(lead.getBoundingClientRect()).y + rel(lead.getBoundingClientRect()).h - 10, dark: oklch(theme.palette.paper)[0] < 0.5 };
  const at = JSON.stringify([t.id, L.w, L.h, L.screen, blocks.length]);
  let ground = grounds.get(t.id);
  if (ground?.at !== at) {
    ground = el("canvas", "art");
    ground.at = at;
    ground.style.height = `${L.h}px`;
    ground.ms = paintScroll(ground, theme, t.look, L);
    grounds.set(t.id, ground);
  }
  sheetEl.prepend(ground);
}
const redraw = () => sheetEl.dispatchEvent(new CustomEvent("sheet", { bubbles: true }));

function line(p, t) {
  lineEl.replaceChildren(el("span", "title", p.lead.title), ...(p.lead.hook ? [el("span", "hook", p.lead.hook)] : []));
  lineEl.onclick = () => { opened = t.id; landed = null; lineEl.dispatchEvent(new CustomEvent("sheet", { bubbles: true })); };
}

// The parts in the one order Max reads them, none of them named: each
// claim frames its part in its own words. Presented again unchanged with a
// quick answer, the answer is the chat's line above the page, which folds to
// its lead, saying when it was checked; a press opens the rest. A lead new
// to this tab fills the first screen; once seen, it is a short band.
function page(p, t) {
  const held = p.quick && p.held > p.at;
  const lead = el("header", `lead${landed === shown(t) ? "" : " new"}`);
  lead.append(el("h1", null, p.lead.title));
  if (p.lead.hook) lead.append(el("p", "hook", p.lead.hook));
  if (held) lead.append(el("p", "checked", `Still holds, checked ${clock(p.held)}`));
  const out = [lead];
  const at = `${t.id} ${p.held}`;
  sheetEl.classList.toggle("folded", held && unfolded !== at);
  if (held && unfolded !== at) {
    lead.onclick = () => { unfolded = at; redraw(); };
    return out;
  }
  out.push(end(p.end), claim("now", p.now));
  if (p.asks.length) out.push(part("asks", p.asks.map((a) => ask(a, t.id))));
  if (p.plan.length) out.push(part("plan", [plan(p.plan, t.id)]));
  if (p.why) out.push(part("why", blocks(p.why.blocks)));
  return out;
}
function part(cls, children) {
  const s = el("section", cls);
  s.append(...children);
  return s;
}
const claim = (cls, c) => part(cls, [el("p", "says", c.says), ...(c.shows ? blocks(c.shows.blocks) : [])]);
// Where this ends: folded to its claim once Max has agreed with it, and
// marked in the gutter when it changed since.
function end(e) {
  if (!e.agreed) {
    const s = claim("end", e);
    s.classList.toggle("changed", e.changed);
    return s;
  }
  const d = el("details");
  d.append(el("summary", "says", e.says), ...(e.shows ? blocks(e.shows.blocks) : []));
  return part("end agreed", [d]);
}

const MARK = { done: "✓", failed: "✕", yours: "●", working: "◐", planned: "○" };
function plan(steps, key) {
  const ol = el("ol", "steps");
  for (const s of steps) {
    const li = el("li", `step ${s.state}`);
    const head = el("div", "head");
    head.append(el("span", "mark", MARK[s.state] || "○"), el("span", "title", s.title || s.key));
    li.append(head);
    if (s.failed) li.append(el("p", "failed", s.failed));
    if (s.body) li.append(...blocks(s.body.blocks));
    li.append(...s.asks.map((a) => ask(a, key)));
    ol.append(li);
  }
  return ol;
}

// An ask: the question, what answering it needs, and what happens without
// an answer. A press is held with what he types or dictates until he sends
// it, which carries both; pressing again takes it back. "Wrong question"
// is always last.
function ask(a, key) {
  const box = el("div", "ask");
  box.append(el("p", "question", a.question));
  // Settled, it stays where it was, quiet, saying how it settled.
  if (a.settled != null) {
    box.classList.add("settled");
    box.append(el("p", "how", a.settled));
    return box;
  }
  const said = [...(a.now?.length ? [inlines(a.now, "p", "now")] : []), ...(a.why?.length ? [inlines(a.why, "p", "why")] : [])];
  box.append(...said);
  if (a.shows) box.append(...blocks(a.shows.blocks));
  const want = a.want;
  const choices = el("div", "choices");
  const press = (label, verdict, recommended, means) => pill(key, a, label, verdict, recommended, means);
  if (want === "Go") choices.append(press("Go", "Yes"));
  else if (want.Choose) for (const o of want.Choose.options) choices.append(press(o.label, { Chose: o.key }, o.key === want.Choose.recommended, o.means));
  else if (want.Do) {
    if (want.Do.link) { const l = el("a", "do", want.Do.link.label); l.href = want.Do.link.url; l.target = "_blank"; l.rel = "noopener"; choices.append(l); }
    if (want.Do.paste) box.append(code(null, want.Do.paste));
    choices.append(press("Done", "Yes"));
  }
  choices.append(press("Wrong question", "Reject"));
  box.append(choices);
  if (a.otherwise) box.append(el("p", "otherwise", `Otherwise: ${a.otherwise}`));
  return box;
}
// One answer's pill: armed while its press is held. One press per ask, so
// arming one disarms the others.
function pill(key, a, label, verdict, recommended, means) {
  const b = el("button", `btn press${recommended ? " recommended" : ""}`, label);
  b.type = "button";
  if (means) b.title = means;
  const mine = JSON.stringify(verdict);
  b.dataset.verdict = mine;
  b.classList.toggle("armed", JSON.stringify(answered(key, a.call)) === mine);
  b.onclick = () => {
    answer(key, a.call, verdict);
    const now = JSON.stringify(answered(key, a.call));
    for (const o of b.parentElement.querySelectorAll(".press")) o.classList.toggle("armed", o.dataset.verdict === now);
  };
  return b;
}

// ─── The body language, as harness-body parses it ──────────────────────────
function blocks(list) { return list.map(block); }
function block(b) {
  if (b.Para) return inlines(b.Para, "p");
  if (b.List) {
    const l = el(b.List.ordered ? "ol" : "ul");
    for (const it of b.List.items) {
      const li = el("li", it.holds == null ? null : it.holds ? "holds" : "fails");
      li.append(...blocks(it.blocks));
      l.append(li);
    }
    return l;
  }
  if (b.Quote) { const q = el("blockquote"); q.append(...blocks(b.Quote)); return q; }
  if (b.Code) return code(b.Code.lang, b.Code.text);
  if (b.Said) return el("blockquote", "said", b.Said.words);
  if (b.Table) return table(b.Table);
  if (b.Fold) {
    const d = el("details");
    d.append(el("summary", null, b.Fold.summary), ...blocks(b.Fold.body));
    return d;
  }
  if (b.Figure) return figure(b.Figure);
  return el("p", null, JSON.stringify(b));
}
function code(lang, text) {
  const pre = el("pre");
  pre.append(el("code", lang ? `language-${lang}` : null, text));
  return pre;
}
function figure(f) {
  const fig = el("figure");
  if (f.Mermaid) { const pre = code("mermaid", f.Mermaid.source); fig.append(pre); diagram(pre, f.Mermaid.source); }
  else if (f.Svg) fig.innerHTML = DOMPurify.sanitize(f.Svg.source, { USE_PROFILES: { svg: true, svgFilters: true } });
  else if (f.Images) fig.append(...f.Images.map((i) => el("span", "image", i.alt)));
  return fig;
}
function table(t) {
  const tb = el("table");
  const row = (cells, tag) => {
    const tr = el("tr");
    cells.forEach((c, i) => { const td = inlines(c, tag); if (t.numeric[i]) td.className = "numeric"; tr.append(td); });
    return tr;
  };
  const head = el("thead"), body = el("tbody");
  head.append(row(t.head, "th"));
  body.append(...t.rows.map((r) => row(r, "td")));
  tb.append(head, body);
  return tb;
}
function inlines(list, tag, cls) {
  const e = el(tag, cls);
  for (const i of list) {
    if (i === "Break") e.append(el("br"));
    else if (i.Text != null) e.append(i.Text);
    else if (i.Code != null) e.append(el("code", null, i.Code));
    else if (i.Link) { const a = el("a", null, i.Link.words); a.href = i.Link.url; a.target = "_blank"; a.rel = "noopener"; e.append(a); }
    else if (i.Live) e.append(el("span", "live", i.Live.words || JSON.stringify(i.Live.of)));
  }
  return e;
}
