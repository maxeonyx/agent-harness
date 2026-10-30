// ─── Small things ──────────────────────────────────────────────────────────
export const $ = (s, root = document) => root.querySelector(s);
export const el = (tag, cls, text) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text != null) e.textContent = text;
  return e;
};
export const svgEl = (tag) => document.createElementNS("http://www.w3.org/2000/svg", tag);
export const clock = (ms) => new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false });
export const ago = (ms) => {
  const s = Math.max(0, Math.round((Date.now() - ms) / 1000));
  return s < 60 ? `${s}s` : s < 3600 ? `${Math.floor(s / 60)}m` : s < 86400 ? `${Math.floor(s / 3600)}h ${Math.floor(s / 60) % 60}m` : `${Math.floor(s / 86400)}d`;
};
export const kTokens = (n) => n == null ? "?" : n < 1000 ? `${n}` : n < 1e6 ? `${(n / 1000).toFixed(n < 10000 ? 1 : 0)}k` : `${+(n / 1e6).toFixed(1)}M`;
// A failure the page handles by showing something else, reported as well, to
// the brain's log (the first script in head.html): `doing` is what the page
// was trying to do.
export const fail = (doing, e) => reportError(Object.assign(new Error(String(e), { cause: e }), { doing }));
// The JSON at `url`; `fallback` after reporting why not.
export const getJson = (url, doing, fallback) => fetch(url)
  .then((r) => { if (!r.ok) throw new Error(`HTTP ${r.status}`); return r.json(); })
  .catch((e) => { fail(doing, e); return fallback; });
export const parse = (s) => { try { return JSON.parse(s); } catch { return undefined; } };
export const phone = () => matchMedia("(max-width: 760px)").matches;
export const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
export const lerp = (a, b, e) => a + (b - a) * e;
// The open card's fixed parts in the document, which every area writes into.
export const panel = $("#panel"), logEl = $(".log", panel), scrollEl = $(".scroll", panel);
// Icons, drawn in the colour of the text around them.
export const ICONS = {
  mic: '<path d="M12 3a3 3 0 0 0-3 3v6a3 3 0 0 0 6 0V6a3 3 0 0 0-3-3z"/><path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21"/>',
  send: '<path d="M12 19V5M5.5 11.5 12 5l6.5 6.5"/>',
  stop: '<rect x="6" y="6" width="12" height="12" rx="1.5"/>',
  pause: '<path d="M9 5.5v13M15 5.5v13"/>',
  play: '<path d="M8 5.5v13l10-6.5z"/>',
  cancel: '<circle cx="12" cy="12" r="8"/><path d="M6.5 17.5l11-11"/>',
  tick: '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
  attach: '<path d="M20 11.5l-7.8 7.8a5 5 0 0 1-7.1-7.1l8.5-8.5a3.3 3.3 0 0 1 4.7 4.7l-8.5 8.5a1.7 1.7 0 0 1-2.4-2.4l7.8-7.8"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  find: '<circle cx="11" cy="11" r="6"/><path d="M15.5 15.5 20 20"/>',
  fit: '<path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/>',
  toward: '<path d="M9 5l7 7-7 7"/>',
  down: '<path d="M12 5v14M5.5 12.5 12 19l6.5-6.5"/>',
  edit: '<path d="M4 20h4L19 9l-4-4L4 16z"/><path d="M13.5 6.5l4 4"/>',
  copy: '<rect x="9" y="9" width="11" height="11" rx="2"/><path d="M15 9V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v7a2 2 0 0 0 2 2h3"/>',
};
export function icon(name, cls, title) {
  const b = el("button", `icon ${cls || ""}`);
  b.type = "button";
  b.innerHTML = `<svg viewBox="0 0 24 24">${ICONS[name]}</svg>`;
  if (title) { b.title = title; b.setAttribute("aria-label", title); }
  return b;
}

// The gist of a call's input: its command, or the path or pattern it names.
export function gist(input) {
  const v = typeof input === "string" ? parse(input) : input;
  const s = v && (v.command ?? v.path ?? v.pattern ?? v.question ?? v.query ?? v.url ?? v.name ?? v.tool);
  if (typeof s !== "string") return "";
  const line = s.split("\n")[0];
  return line.length > 90 ? line.slice(0, 90) + "…" : line;
}

// Struck through only between `~~`: a single `~` is a home directory.
const STRUCK = /^~~(?=[^\s~])([\s\S]*?[^\s~])~~(?!~)/;
window.marked?.use({ tokenizer: { del(src) {
  const m = STRUCK.exec(src);
  return m ? { type: "del", raw: m[0], text: m[1], tokens: this.lexer.inlineTokens(m[1]) } : undefined;
} } });
// A path under the home directory, as Max writes it.
export const homePath = (p) => p.replace(/^\/(home|Users)\/[^/]+(?=\/|$)|^\/root(?=\/|$)/, "~");

export function markdown(text) {
  if (!window.marked || !window.DOMPurify) return el("div", "plain", text);
  const div = el("div", "prose");
  div.innerHTML = DOMPurify.sanitize(marked.parse(text, { gfm: true, breaks: false }));
  for (const a of div.querySelectorAll("a")) { a.target = "_blank"; a.rel = "noopener"; }
  // Code in the colours of its language, when it names one; a diagram drawn.
  for (const code of div.querySelectorAll("pre > code")) {
    const lang = [...code.classList].find((c) => c.startsWith("language-"))?.slice(9);
    if (lang === "mermaid") diagram(code.parentElement, code.textContent);
    else if (lang && window.hljs?.getLanguage(lang)) hljs.highlightElement(code);
  }
  // Every passage keeps its source: Raw shows the Markdown as written.
  const raw = el("button", "raw-toggle", "Raw");
  raw.type = "button"; raw.title = "The Markdown as written";
  raw.onclick = (e) => {
    e.stopPropagation();
    const on = div.classList.toggle("raw");
    raw.classList.toggle("armed", on);
    if (on && !$(":scope > .source", div)) raw.before(el("div", "io source", text));
  };
  div.append(raw);
  return div;
}
// A mermaid diagram where its source was. The library loads the first time
// one is shown; a source it cannot draw stays as written.
let mermaidLib = null, diagrams = 0;
async function diagram(pre, source) {
  try {
    mermaidLib ||= import("https://cdn.jsdelivr.net/npm/mermaid@11.4.1/dist/mermaid.esm.min.mjs")
      .then((m) => { m.default.initialize({ startOnLoad: false, theme: "neutral", securityLevel: "strict" }); return m.default; });
    const { svg } = await (await mermaidLib).render(`mermaid-${++diagrams}`, source);
    const fig = el("div", "diagram");
    fig.innerHTML = DOMPurify.sanitize(svg, { USE_PROFILES: { svg: true, svgFilters: true, html: true } });
    pre.replaceWith(fig);
  } catch (e) { fail("drawing a mermaid diagram, which stays as its source", e); }
}

