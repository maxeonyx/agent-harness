import { $, diagram, el, fail, panel, scrollEl } from "./base.js";
import { answer, answered } from "./compose.js";
import { beyond, hash, load, meanColour, mix, pagePresses, paintInto, quietOf, wear } from "./theme.js";

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
const sheetEl = $(".sheet", panel), lineEl = $(".sheet-line", panel), composeEl = $(".compose", panel);
let opened = null, landed = null, unfolded = null;

// Draws `t`'s page, and says whether the card should open at its top
// (then `sheetSeen`): the first time this tab shows this version in full.
export function renderSheet(t) {
  const p = t.page;
  if (opened && opened !== t.id) opened = null;
  if (!p) { sheetEl.hidden = lineEl.hidden = true; sheetEl.drawn = null; dressed = null; wearBox(null); return false; }
  const full = p.current || opened === t.id;
  lineEl.hidden = full;
  sheetEl.hidden = !full;
  if (!full) { dressed = null; wearBox(null); line(p, t); return false; }
  const key = JSON.stringify([t.id, p, t.look, unfolded]);
  if (sheetEl.drawn !== key) {
    sheetEl.drawn = key;
    try { sheetEl.replaceChildren(...seated(t), ...page(p, t)); dress(t); }
    catch (e) {
      sheetEl.replaceChildren(el("p", "", "This page could not be drawn."));
      fail(`drawing ${t.name}'s page`, e);
    }
  }
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

// The page wears its task's look: the theme's tokens, and its ground painted
// for the window the page is read in, behind the whole page. The painting
// holds still while the page scrolls over it (`.window`, sticky in the
// sheet), at full strength wherever there is no text; each block of text
// stands on a press of the painting that keeps its ink at 7:1 on every pixel
// (theme.js `pagePresses`, sheet.css). Painted once per task and window size, after
// the page has drawn, so the page never waits for it, and again when the
// window settles at another size. The reply box under the page wears the
// theme's paper, so a dark painting doesn't end on a cream band. A task with
// no look wears sheet.css's plain default.
//
// The last few paintings are kept, so going back to a task shows its
// painting at once; each is a window-sized canvas, a few MB.
const grounds = new Map(), KEEP = 4;
// How far the soft press goes toward the clamp's colour (0: the clamp alone).
const SOFT = 0.85;
// The task the page wears now, and the latest dress, which alone may seat
// what it painted.
let dressed = null, dressing = 0;
async function dress(t) {
  const mine = ++dressing;
  dressed = t;
  if (!t.look) { sheetEl.removeAttribute("style"); lineEl.removeAttribute("style"); wearBox(null); return; }
  const theme = await load(t.look.theme);
  await wear(sheetEl, theme);
  await wear(lineEl, theme);
  // On the painting, the page's text is set one step stronger than on
  // paper: its body in the ink, and what is quieter in the body's ink, so
  // the press under it can stay as near the painting as the contrast allows.
  const p = theme.palette, sp = sheetEl.style;
  sp.setProperty("--body", p.ink); sp.setProperty("--muted", p.body); sp.setProperty("--cover-body", p.cover_ink);
  if (!sheetEl.isConnected || sheetEl.hidden) return;
  wearBox(p);
  const W = scrollEl.clientWidth, V = scrollEl.clientHeight, dpr = Math.min(2, devicePixelRatio || 1);
  sp.setProperty("--win-w", `${W}px`);
  sp.setProperty("--win-h", `${V}px`);
  const at = JSON.stringify([t.id, t.look, W, V, dpr]);
  let g = grounds.get(at);
  grounds.delete(at);
  if (!g) {
    g = { at };
    g.ready = (async () => {
      // After the page has drawn: the paper, or the painting for the size
      // before, shows until this one lands.
      await new Promise((ok) => requestAnimationFrame(() => setTimeout(ok)));
      const s = getComputedStyle(sheetEl), pl = parseFloat(s.paddingLeft), pr = parseFloat(s.paddingRight);
      const column = { x: sheetEl.getBoundingClientRect().left - scrollEl.getBoundingClientRect().left + pl, w: sheetEl.clientWidth - pl - pr };
      const canvas = el("canvas"), t0 = performance.now();
      paintInto(canvas, theme, t.look, quietOf({ w: W, h: V }, column), { w: W, h: V, dpr }, column);
      // The press, at the painting's own hue where the text runs.
      const tint = meanColour(canvas, { x: column.x * dpr, y: 0, w: column.w * dpr, h: canvas.height });
      const { body, head } = pagePresses(p, tint);
      // Where down the window the press would flatten the painting, for the soft press.
      const rows = beyond(canvas, body.colour, body.blend);
      performance.measure(`ground ${theme.name} ${W}x${V}@${dpr}`, { start: t0 });
      Object.assign(g, { canvas, x: column.x, body, head, rows });
    })().catch((e) => { grounds.delete(at); fail(`painting ${t.name}'s ground`, e); });
  }
  grounds.set(at, g);
  for (const old of grounds.keys()) if (grounds.size > KEEP) grounds.delete(old);
  await g.ready;
  // A painting that failed leaves the page on its paper.
  if (mine !== dressing || sheetEl.hidden || !g.canvas) return;
  // Each block's press runs the window's full width, from its edge, fading
  // across gutters wide enough to fade in (a phone's are not: there it has no sides).
  sp.setProperty("--press-l", `${g.x}px`);
  sp.setProperty("--press-g", `${g.x >= 48 ? g.x : 0}px`);
  sp.setProperty("--press", g.body.colour); sp.setProperty("--press-blend", g.body.blend);
  sp.setProperty("--press-lead", g.head.colour); sp.setProperty("--press-lead-blend", g.head.blend);
  // Under the clamp, a screen (or multiply) toward the same colour lifts the
  // painting's darks (or dims its lights) most of the way, keeping their
  // texture, so what is left for the clamp to flatten is small.
  const soft = (c) => c.blend === "lighten" ? [mix("#000000", c.colour, SOFT), "screen"] : [mix("#ffffff", c.colour, SOFT), "multiply"];
  const [bs, bb] = soft(g.body), [hs, hb] = soft(g.head);
  sp.setProperty("--press-soft", bs); sp.setProperty("--press-soft-blend", bb);
  sp.setProperty("--press-lead-soft", hs); sp.setProperty("--press-lead-soft-blend", hb);
  // The soft press shows only where it has something to keep: a block's
  // place in the window as it scrolls (its view timeline, run by the
  // compositor) sets how strong it is, from how much of the painting at that
  // height the clamp would flatten. A property of the painting and the
  // window, never of the page.
  const name = `soft-${hash(g.at)}`;
  let style = $("style.soft", sheetEl);
  if (!style) { style = el("style", "soft"); sheetEl.append(style); }
  const n = g.rows.length, frames = [];
  for (let i = 0; i <= 20; i++) {
    const at = (1 - i / 20) * (n - 1), r = Math.round(at);
    const near = g.rows.slice(Math.max(0, r - 2), r + 3);
    frames.push(`${i * 5}% { opacity: ${Math.min(1, 1.6 * Math.max(...near)).toFixed(2)} }`);
  }
  style.textContent = `@keyframes ${name} { ${frames.join(" ")} }`;
  sp.setProperty("--soft-frames", name);
  seat(g);
}
// What dress seated for this task (its painting and its soft press's
// frames), kept through a redraw: dress seats it again only after awaiting
// the theme, so a page without it would show a frame of bare paper.
function seated(t) {
  return dressed?.id === t.id ? sheetEl.querySelectorAll(":scope > .window, :scope > style.soft") : [];
}
function seat(g) {
  let win = $(":scope > .window", sheetEl);
  if (!win) { win = el("div", "window"); sheetEl.prepend(win); }
  if (win.firstChild !== g.canvas) win.replaceChildren(g.canvas);
}
// The window settling at another size (a desktop window dragged, a phone
// turned) paints the page again for it, once the size has held a moment.
let settling;
new ResizeObserver(() => {
  clearTimeout(settling);
  settling = setTimeout(() => { if (dressed && !sheetEl.hidden) dress(dressed); }, 250);
}).observe(scrollEl);
// The reply box in the page's paper and inks while a page shows (`p`), the
// language's own otherwise.
function wearBox(p) {
  const s = composeEl.style, tokens = p && { paper: p.paper, white: p.paper, ink: p.ink, body: p.body, muted: p.muted, faint: p.muted, line: p.line };
  for (const k of ["paper", "white", "ink", "body", "muted", "faint", "line"]) {
    if (tokens) s.setProperty(`--${k}`, tokens[k]); else s.removeProperty(`--${k}`);
  }
}
const redraw = () => sheetEl.dispatchEvent(new CustomEvent("sheet", { bubbles: true }));

function line(p, t) {
  lineEl.replaceChildren(el("span", "title", p.lead));
  lineEl.onclick = () => { opened = t.id; landed = null; lineEl.dispatchEvent(new CustomEvent("sheet", { bubbles: true })); };
}

// The parts in the one order Max reads them, none of them named, and no
// line about the page itself: the lead, the end, the now, the plan with
// its asks (and asks no step holds), why. Presented again unchanged with a
// quick answer and nothing in the end moved since Max agreed with it, the
// answer is the chat's line above the page, which folds to its lead; a press
// opens the rest. A lead new to this tab stands taller, with the end still
// starting on the first screen; once seen, it is a short band.
function page(p, t) {
  const e = p.end;
  const moved = e != null && (e.goal_changed || e.shows_changed || e.uses.some((u) => u.changed));
  const held = p.quick && p.held > p.at && !moved;
  const lead = el("header", `lead${landed === shown(t) ? "" : " new"}`);
  lead.append(el("h1", null, p.lead));
  const out = [lead];
  const at = `${t.id} ${p.held}`;
  sheetEl.classList.toggle("folded", held && unfolded !== at);
  if (held && unfolded !== at) {
    lead.onclick = () => { unfolded = at; redraw(); };
    return out;
  }
  if (e != null) out.push(part("end", ends(e)));
  if (p.now != null) out.push(part("now", blocks(p.now.blocks)));
  if (p.plan.length) out.push(part("plan", [plan(p.plan, t.id)]));
  if (p.asks.length) out.push(part("asks", p.asks.map((a) => ask(a, t.id))));
  if (p.why) out.push(part("why", blocks(p.why.blocks)));
  return out;
}
function part(cls, children) {
  const s = el("section", cls);
  s.append(...children);
  return s;
}
// The end: what Max wants it for, the system as it will be, and what he
// does with it. A use reads aim first, unlike a step: what he does, then
// its mark and where it stands today, so the uses read as the end and not
// as the plan's first steps. What moved since he agreed with it carries
// the gutter bar.
const MARK = { done: "✓", failed: "✕", yours: "●", working: "◐", planned: "○", unknown: "?" };
const bar = (cls, changed) => `${cls}${changed ? " changed" : ""}`;
function ends(e) {
  const out = [el("p", bar("goal", e.goal_changed), e.goal)];
  if (e.shows) { const s = el("div", bar("shows", e.shows_changed)); s.append(...blocks(e.shows.blocks)); out.push(s); }
  if (e.uses.length) {
    const ul = el("ul", "uses");
    for (const u of e.uses) {
      const li = el("li", bar(`use ${u.state}`, u.changed));
      const stands = el("span", "stands");
      stands.append(el("span", "mark", MARK[u.state] ?? "○"));
      if (u.now != null) stands.append(inlines(u.now, "span", "now"));
      li.append(el("span", "does", u.does), stands);
      ul.append(li);
    }
    out.push(ul);
  }
  return out;
}
// The plan: each step one line, its mark, what it makes true and where it
// stands now, so it reads as a checklist. A step in play or ahead adds its
// body; a step behind him is its line alone.
function plan(steps, key) {
  const ol = el("ol", "steps");
  for (const s of steps) {
    const li = el("li", `step ${s.state}`);
    const head = el("div", "head");
    head.append(el("span", "mark", MARK[s.state] ?? "○"), el("span", "title", s.title ?? s.key));
    if (s.now != null) head.append(inlines(s.now, "span", "now"));
    li.append(head);
    if (s.failed != null) li.append(el("p", "failed", s.failed));
    if (s.body != null && s.state !== "done") li.append(...blocks(s.body.blocks));
    li.append(...s.asks.map((a) => ask(a, key)));
    ol.append(li);
  }
  return ol;
}

// An ask, standing on its own: the question; where it ends (a hollow ring)
// and where it is now (a filled dot), end first as a step reads; why it is
// his; what shows it; the answers; and what the agent does without one, in
// its own words. A press is held with what he types or dictates until he sends
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
  if (a.end?.length) { const e = el("p", "end"); e.append(el("span", "ring", "○"), a.end); box.append(e); }
  if (a.now?.length) { const n = inlines(a.now, "p", "now"); n.prepend(el("span", "dot", "●")); box.append(n); }
  if (a.why?.length) box.append(inlines(a.why, "p", "why"));
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
  if (a.otherwise != null) box.append(el("p", "otherwise", a.otherwise));
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
