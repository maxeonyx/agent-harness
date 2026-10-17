"use strict";
// ─── Small things ──────────────────────────────────────────────────────────
const $ = (s, root = document) => root.querySelector(s);
const el = (tag, cls, text) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text != null) e.textContent = text;
  return e;
};
const svgEl = (tag) => document.createElementNS("http://www.w3.org/2000/svg", tag);
const clock = (ms) => new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false });
const ago = (ms) => {
  const s = Math.max(0, Math.round((Date.now() - ms) / 1000));
  return s < 60 ? `${s}s` : s < 3600 ? `${Math.floor(s / 60)}m` : s < 86400 ? `${Math.floor(s / 3600)}h ${Math.floor(s / 60) % 60}m` : `${Math.floor(s / 86400)}d`;
};
const kTokens = (n) => n == null ? "?" : n < 1000 ? `${n}` : n < 1e6 ? `${(n / 1000).toFixed(n < 10000 ? 1 : 0)}k` : `${+(n / 1e6).toFixed(1)}M`;
const parse = (s) => { try { return JSON.parse(s); } catch { return undefined; } };
const phone = () => matchMedia("(max-width: 760px)").matches;
const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
const lerp = (a, b, e) => a + (b - a) * e;
// Icons, drawn in the colour of the text around them.
const ICONS = {
  mic: '<path d="M12 3a3 3 0 0 0-3 3v6a3 3 0 0 0 6 0V6a3 3 0 0 0-3-3z"/><path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21"/>',
  send: '<path d="M12 19V5M5.5 11.5 12 5l6.5 6.5"/>',
  stop: '<rect x="6" y="6" width="12" height="12" rx="1.5"/>',
  pause: '<path d="M9 5.5v13M15 5.5v13"/>',
  play: '<path d="M8 5.5v13l10-6.5z"/>',
  cross: '<path d="M6.5 6.5l11 11M17.5 6.5l-11 11"/>',
  tick: '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
  queue: '<path d="M4 6h11M4 11h11M4 16h5M17 13v7M14 17l3 3 3-3"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  fit: '<path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/>',
  toward: '<path d="M9 5l7 7-7 7"/>',
  pause: '<rect x="6.5" y="5" width="3.6" height="14" rx="1.8" fill="currentColor" stroke="none"/><rect x="13.9" y="5" width="3.6" height="14" rx="1.8" fill="currentColor" stroke="none"/>',
};
function icon(name, cls, title) {
  const b = el("button", `icon ${cls || ""}`);
  b.type = "button";
  b.innerHTML = `<svg viewBox="0 0 24 24">${ICONS[name]}</svg>`;
  if (title) { b.title = title; b.setAttribute("aria-label", title); }
  return b;
}

// The gist of a call's input: its command, or the path or pattern it names.
function gist(input) {
  const v = typeof input === "string" ? parse(input) : input;
  const s = v && (v.command ?? v.path ?? v.pattern ?? v.question ?? v.query ?? v.url ?? v.name ?? v.tool);
  if (typeof s !== "string") return "";
  const line = s.split("\n")[0];
  return line.length > 90 ? line.slice(0, 90) + "…" : line;
}

function markdown(text) {
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
  } catch { /* stays as its source */ }
}

