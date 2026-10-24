// Markdown from main, rendered with what the page knows around it: links
// to the page's own views, Rust paths opening their definitions, file
// paths opening on GitHub, glossary terms, S1-S6 as structure chips,
// mermaid diagrams, and marks on what changed since Max last looked.
import { marked } from "https://cdn.jsdelivr.net/npm/marked@12.0.2/lib/marked.esm.js";
import mermaid from "https://cdn.jsdelivr.net/npm/mermaid@11.4.1/dist/mermaid.esm.min.mjs";
import elkLayouts from "https://cdn.jsdelivr.net/npm/@mermaid-js/layout-elk@0.1.7/dist/mermaid-layout-elk.esm.min.mjs";

const dark = matchMedia("(prefers-color-scheme: dark)").matches;
mermaid.registerLayoutLoaders(elkLayouts);
mermaid.initialize({ startOnLoad: false, theme: dark ? "dark" : "neutral", securityLevel: "strict", layout: "elk", themeVariables: { fontSize: "14px" }, flowchart: { wrappingWidth: 280 } });

// What the page supplies: routes for docs, URLs for files, the glossary,
// resolved Rust items and what to do when one is clicked.
let site;
export function configure(s) {
  site = s;
}

export function slug(text) {
  return text.toLowerCase().trim().replace(/[^\p{L}\p{N}\s_-]/gu, "").replace(/\s/g, "-");
}

// A doc as its sections: each a heading and the blocks under it, each block
// knowing the source lines it came from (1-based, inclusive).
export function sections(src) {
  const tokens = marked.lexer(src);
  const out = [{ depth: 0, title: "", slug: "", start: 1, end: 1, blocks: [] }];
  const seen = new Map();
  let line = 1;
  for (const t of tokens) {
    const start = line;
    line += newlines(t.raw);
    const end = Math.max(start, t.raw.endsWith("\n") ? line - 1 : line);
    if (t.type === "heading") {
      let s = slug(t.text);
      const n = seen.get(s) ?? 0;
      seen.set(s, n + 1);
      if (n > 0) s = `${s}-${n}`;
      out.push({ depth: t.depth, title: t.text, slug: s, start, end, blocks: [] });
    } else if (t.type !== "space") {
      out.at(-1).blocks.push({ token: t, start, end });
      out.at(-1).end = end;
    }
  }
  const all = out.filter((s) => s.depth > 0 || s.blocks.length > 0);
  all.links = tokens.links;
  return all;
}

const newlines = (s) => (s.match(/\n/g) ?? []).length;
const overlaps = (ranges, a, b) => ranges !== undefined && ranges.some(([x, y]) => x <= b && a <= y);

// Render sections into an element. ctx: path (the repo path, for relative
// links), crate (whose Rust paths these are), changed (line ranges changed
// since Max last looked), shift (added to heading depths), lineOffset
// (source line of the doc's first line), edit (an Edit link per doc).
export function render(secs, ctx) {
  const root = document.createElement("div");
  root.className = "md";
  const offset = ctx.lineOffset ?? 0;
  const changed = ctx.changed;
  for (const sec of secs) {
    const el = document.createElement("section");
    if (sec.depth > 0) {
      el.id = sec.slug;
      const h = document.createElement(`h${Math.min(6, sec.depth + (ctx.shift ?? 0))}`);
      h.innerHTML = marked.parseInline(sec.title);
      const a = document.createElement("a");
      a.className = "anchor";
      a.href = `#/${ctx.route}#${sec.slug}`;
      a.textContent = "#";
      a.title = "Link to this section";
      h.append(a);
      el.append(h);
    }
    let revised = false;
    for (const b of sec.blocks) {
      const div = document.createElement("div");
      div.className = "blk";
      const toks = [b.token];
      toks.links = secs.links;
      div.innerHTML = marked.parser(toks);
      if (b.token.type === "list") {
        const lis = div.querySelectorAll(":scope > ul > li, :scope > ol > li");
        let l = b.start;
        b.token.items.forEach((item, i) => {
          const s = l;
          l += newlines(item.raw);
          const e = item.raw.endsWith("\n") ? l - 1 : l;
          if (overlaps(changed, s + offset, e + offset)) { lis[i]?.classList.add("rev"); revised = true; }
        });
      } else if (b.token.type === "table") {
        div.querySelectorAll("tbody > tr").forEach((tr, i) => {
          const l = b.start + 2 + i + offset;
          if (overlaps(changed, l, l)) { tr.classList.add("rev"); revised = true; }
        });
      } else if (overlaps(changed, b.start + offset, b.end + offset)) {
        div.classList.add("rev");
        revised = true;
      }
      el.append(div);
    }
    if (sec.depth > 0 && overlaps(changed, sec.start + offset, sec.start + offset)) { el.querySelector(":scope > h1, :scope > h2, :scope > h3, :scope > h4, :scope > h5, :scope > h6")?.classList.add("rev"); revised = true; }
    if (revised) el.classList.add("has-rev");
    root.append(el);
  }
  ctx.prepare?.(root);
  decorate(root, ctx);
  return root;
}

// Inline markdown (a table cell, a quoted line), decorated the same way.
export function inline(src, ctx = {}) {
  const span = document.createElement("span");
  span.innerHTML = marked.parseInline(src);
  decorate(span, ctx);
  return span;
}

export function decorate(root, ctx) {
  links(root, ctx);
  codes(root, ctx);
  if (ctx.terms !== false) terms(root);
  structures(root);
  diagrams(root);
  for (const c of root.querySelectorAll("pre code[class*=language-]")) globalThis.hljs?.highlightElement(c);
}

function links(root, ctx) {
  for (const a of root.querySelectorAll("a[href]")) {
    const href = a.getAttribute("href");
    if (a.classList.contains("anchor")) continue;
    if (/^https?:/.test(href)) { a.target = "_blank"; a.rel = "noopener"; continue; }
    if (href.startsWith("#")) { a.href = `#/${ctx.route}${href}`; continue; }
    if (/^[\w:]+$/.test(href) && ctx.crate !== undefined) {
      a.removeAttribute("href");
      item(a, `${ctx.crate} ${href}`);
      continue;
    }
    const [rel, anchor] = href.split("#");
    const path = join(ctx.path ?? "", rel);
    a.href = site.link(path, anchor);
    if (a.href.startsWith("http")) { a.target = "_blank"; a.rel = "noopener"; }
  }
}

function join(from, rel) {
  const parts = from.split("/").slice(0, -1);
  for (const seg of rel.split("/")) {
    if (seg === "..") parts.pop();
    else if (seg !== "." && seg !== "") parts.push(seg);
  }
  return parts.join("/");
}

// A code span is a Rust item the server resolved, a file (with lines) or a
// directory in the repo, or just code.
function codes(root, ctx) {
  for (const c of root.querySelectorAll("code")) {
    if (c.closest("pre, a") !== null) continue;
    const text = c.textContent;
    const key = `${ctx.crate ?? "docs"} ${text}`;
    if (key in site.items) { item(c, key); continue; }
    const m = text.match(/^([\w./-]+?)(?::(\d+)(?:-(\d+))?(?:,\d+)*)?$/);
    if (m === null) continue;
    const target = site.findPath(m[1], ctx.path);
    if (target === null) continue;
    const a = document.createElement("a");
    a.href = site.link(target, undefined, m[2] === undefined ? undefined : [Number(m[2]), Number(m[3] ?? m[2])]);
    if (a.href.startsWith("http")) { a.target = "_blank"; a.rel = "noopener"; }
    c.replaceWith(a);
    a.append(c);
  }
}

function item(el, key) {
  el.classList.add("item");
  el.tabIndex = 0;
  el.title = "Show its definition on main";
  el.addEventListener("click", (e) => { e.preventDefault(); site.openItem(key); });
  el.addEventListener("keydown", (e) => { if (e.key === "Enter") site.openItem(key); });
}

// The first time each glossary term appears in a paragraph, list item or
// cell, it links to its row; hovering shows the definition.
function terms(root) {
  const re = site.termPattern;
  const used = new Map();
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const nodes = [];
  for (let n = walker.nextNode(); n !== null; n = walker.nextNode()) nodes.push(n);
  for (const n of nodes) {
    const p = n.parentElement;
    if (p.closest("a, code, pre, h1, h2, h3, h4, h5, h6, .mermaid, .term-cell, .no-terms, svg") !== null) continue;
    const block = p.closest("p, li, td, th, dd, blockquote") ?? p;
    if (!used.has(block)) used.set(block, new Set());
    const seen = used.get(block);
    const text = n.textContent;
    const parts = [];
    let at = 0;
    for (const m of text.matchAll(re)) {
      const term = site.termOf(m[0]);
      if (term === undefined || seen.has(term.slug)) continue;
      seen.add(term.slug);
      parts.push(text.slice(at, m.index));
      const a = document.createElement("a");
      a.className = "term";
      a.href = `#/glossary#term-${term.slug}`;
      a.dataset.term = term.slug;
      a.textContent = m[0];
      parts.push(a);
      at = m.index + m[0].length;
    }
    if (parts.length === 0) continue;
    parts.push(text.slice(at));
    n.replaceWith(...parts);
  }
}

// S1 to S6 in prose become chips in the structure's colour, linking to it.
function structures(root) {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const nodes = [];
  for (let n = walker.nextNode(); n !== null; n = walker.nextNode()) if (/\bS[1-6]\b/.test(n.textContent)) nodes.push(n);
  for (const n of nodes) {
    if (n.parentElement.closest("a, code, pre, .mermaid, svg") !== null) continue;
    const parts = n.textContent.split(/\b(S[1-6])\b/);
    n.replaceWith(...parts.map((s, i) => (i % 2 === 1 ? chip(Number(s[1])) : s)));
  }
}

export function chip(n) {
  const a = document.createElement("a");
  a.className = `chip s${n}`;
  a.href = `#/s/${n}`;
  a.textContent = `S${n}`;
  a.title = site.structureTitle(n);
  return a;
}

function diagrams(root) {
  const nodes = [];
  for (const c of root.querySelectorAll("pre code.language-mermaid")) {
    const div = document.createElement("div");
    div.className = "mermaid";
    div.textContent = c.textContent;
    c.parentElement.replaceWith(div);
    nodes.push(div);
  }
  if (nodes.length > 0) renderDiagrams(nodes);
}

// Mermaid renders one diagram at a time, so calls queue behind each other.
let queue = Promise.resolve();
function renderDiagrams(nodes) {
  queue = queue.then(async () => {
    for (const n of nodes) {
      if (!n.isConnected) continue;
      try {
        await mermaid.run({ nodes: [n] });
      } catch (e) {
        n.classList.add("error");
        n.textContent = `This diagram didn't render: ${e.message}\n\n${n.textContent}`;
      }
    }
  });
}

// One hover card for glossary terms, anywhere on the page.
const card = document.createElement("div");
card.className = "peek";
card.hidden = true;
document.addEventListener("mouseover", (e) => {
  const a = e.target.closest?.(".term");
  if (a === null || a === undefined) return;
  const t = site.term(a.dataset.term);
  card.replaceChildren();
  const h = document.createElement("b");
  h.textContent = t.names.join(", ");
  const ctx = { terms: false, path: "docs/glossary.md", route: "glossary" };
  card.append(h, " ", inline(t.meaning, ctx));
  if (t.code !== "") {
    const c = document.createElement("div");
    c.className = "in-code";
    c.append("In code: ", inline(t.code, ctx));
    card.append(c);
  }
  document.body.append(card);
  card.hidden = false;
  const r = a.getBoundingClientRect();
  const w = Math.min(420, innerWidth - 16);
  card.style.width = `${w}px`;
  card.style.left = `${Math.max(8, Math.min(r.left, innerWidth - w - 8))}px`;
  const below = r.bottom + 6;
  card.style.top = `${below + card.offsetHeight > innerHeight ? Math.max(8, r.top - card.offsetHeight - 6) : below}px`;
});
document.addEventListener("mouseout", (e) => {
  if (e.target.closest?.(".term") !== null) card.hidden = true;
});
addEventListener("scroll", () => { card.hidden = true; }, true);
