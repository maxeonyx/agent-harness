"use strict";
// Core: DOM/SVG builders, formatting, the tree model, the drawer, the code viewer, routing.

const NS = "http://www.w3.org/2000/svg";
const REPO = "https://github.com/maxeonyx/agent-harness";
const BRANCH = "forked-subagents";
const EXP = "experiments/forked-subagents/";
// ?still renders every animation at its end state: for screenshots and thumbnails.
const STILL = new URLSearchParams(location.search).has("still");

function h(tag, attrs, ...kids) {
  const el = document.createElement(tag);
  return dress(el, attrs, kids);
}
function s(tag, attrs, ...kids) {
  const el = document.createElementNS(NS, tag);
  return dress(el, attrs, kids);
}
function dress(el, attrs, kids) {
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v == null || v === false) continue;
    if (k.startsWith("on")) el.addEventListener(k.slice(2), v);
    else if (k === "style" && typeof v === "object") Object.assign(el.style, v);
    else if (k === "html") el.innerHTML = v;
    else el.setAttribute(k, v === true ? "" : v);
  }
  for (const kid of kids.flat(Infinity)) {
    if (kid == null || kid === false) continue;
    el.append(kid instanceof Node ? kid : document.createTextNode(String(kid)));
  }
  return el;
}

const fmt = {
  tok: n => n >= 10000 ? (n / 1000).toFixed(1) + "k" : n >= 1000 ? (n / 1000).toFixed(1) + "k" : String(n),
  int: n => Math.round(n).toLocaleString("en-NZ"),
  usd: x => "$" + (x < 0.01 ? x.toFixed(4) : x < 1 ? x.toFixed(3) : x.toFixed(2)),
  pct: x => Math.round(x * 100) + "%",
  sec: ms => (ms / 1000).toFixed(1) + "s",
};

const MODEL = { "anthropic/claude-sonnet-5": "sonnet", "openai/gpt-5.6-luna": "luna" };
const short = model => MODEL[model] || model;

// --- tip -------------------------------------------------------------------
const tip = document.getElementById("tip");
function tipOn(el, text) {
  el.addEventListener("mousemove", e => {
    tip.textContent = typeof text === "function" ? text() : text;
    tip.style.opacity = 1;
    const x = Math.min(e.clientX + 14, innerWidth - tip.offsetWidth - 10);
    const y = e.clientY + 18 + tip.offsetHeight > innerHeight ? e.clientY - tip.offsetHeight - 12 : e.clientY + 18;
    tip.style.left = x + "px";
    tip.style.top = y + "px";
  });
  el.addEventListener("mouseleave", () => (tip.style.opacity = 0));
  return el;
}

// --- lineage: which part of the fixture a name belongs to --------------------
const BRANCHES = { maunga: ["kowhai", "rimu", "totara"], awa: ["tui", "kea", "weka"] };
function lineage(name) {
  const n = name.toLowerCase();
  for (const [region, branches] of Object.entries(BRANCHES)) {
    if (branches.some(b => n.includes(b))) return region;
  }
  if (n.includes("maunga")) return "maunga";
  if (n.includes("awa")) return "awa";
  return n === "root" ? "root" : "other";
}
const HUE = { maunga: "var(--maunga)", awa: "var(--awa)", root: "var(--root)", other: "var(--mute)" };

// --- the tree of one trial ---------------------------------------------------
function treeOf(trial) {
  const nodes = new Map();
  for (const agent of trial.agents) {
    const parts = agent.path.split(" › ");
    nodes.set(agent.path, { agent, path: agent.path, name: parts[parts.length - 1], depth: parts.length - 1, parent: parts.slice(0, -1).join(" › ") || null, kids: [] });
  }
  for (const node of nodes.values()) if (node.parent && nodes.has(node.parent)) nodes.get(node.parent).kids.push(node);
  const root = nodes.get("root");
  const over = new Set(trial.overreached || []);
  const unscored = new Set(trial.unscoreable || []);
  for (const node of nodes.values()) {
    node.over = over.has(node.path);
    node.unscored = unscored.has(node.path);
  }
  return { root, nodes };
}

function drawTree(trial, w, hgt, opts = {}) {
  const { root } = treeOf(trial);
  let leafIndex = 0;
  const depthMax = Math.max(...trial.agents.map(a => a.path.split(" › ").length - 1), 1);
  const place = node => {
    if (!node.kids.length) node.x = leafIndex++;
    else { node.kids.forEach(place); node.x = node.kids.reduce((a, k) => a + k.x, 0) / node.kids.length; }
  };
  place(root);
  const pad = opts.pad ?? 10;
  const r = opts.r ?? 4;
  const X = x => pad + (leafIndex <= 1 ? (w - 2 * pad) / 2 : (x / (leafIndex - 1)) * (w - 2 * pad));
  const Y = d => pad + (d / Math.max(depthMax, 2)) * (hgt - 2 * pad);
  const svg = s("svg", { width: w, height: hgt, viewBox: `0 0 ${w} ${hgt}` });
  const walk = (node, f) => { f(node); node.kids.forEach(k => walk(k, f)); };
  walk(root, node => node.kids.forEach(k => svg.append(s("path", {
    d: `M${X(node.x)} ${Y(node.depth)} C ${X(node.x)} ${(Y(node.depth) + Y(k.depth)) / 2}, ${X(k.x)} ${(Y(node.depth) + Y(k.depth)) / 2}, ${X(k.x)} ${Y(k.depth)}`,
    fill: "none", stroke: k.over ? "#ff4f7b88" : "#3a4256", "stroke-width": opts.edge ?? 1.2,
  }))));
  walk(root, node => {
    const colour = HUE[lineage(node.name)];
    const state = node.agent.state;
    const g = s("g", { class: opts.onNode ? "tnode" : null, style: opts.onNode ? { cursor: "pointer" } : null });
    if (node.over) g.append(s("circle", { cx: X(node.x), cy: Y(node.depth), r: r + 3.5, fill: "none", stroke: "var(--bad)", "stroke-width": 1.6 }));
    g.append(s("circle", {
      cx: X(node.x), cy: Y(node.depth), r,
      fill: node.unscored ? "none" : node.over ? "var(--bad)" : colour,
      stroke: node.unscored ? colour : state === "faulted" ? "var(--ember)" : state === "suspended" ? "#0a0c11" : "none",
      "stroke-width": node.unscored ? 1.4 : 1.5,
      "stroke-dasharray": state === "suspended" && !node.unscored ? "2 1.5" : null,
    }));
    if (opts.labels) {
      g.append(s("text", { x: X(node.x), y: Y(node.depth) + (node.depth === 0 ? -r - 7 : r + 14), "text-anchor": "middle", fill: node.over ? "#ff9db5" : "#aab0c0", "font-size": opts.labels, "font-family": "var(--mono)" }, node.name));
    }
    if (opts.onNode) { g.addEventListener("click", e => { e.stopPropagation(); opts.onNode(node); }); tipOn(g, () => nodeTip(node)); }
    svg.append(g);
  });
  return svg;
}
function nodeTip(node) {
  const a = node.agent;
  const reads = a.tools.filter(t => t.name === "read_file").map(t => { try { return JSON.parse(t.args).path; } catch { return "?"; } });
  return [
    node.path,
    `${a.state}${node.over ? " · OVER-REACHED" : ""}${node.unscored ? " · unscoreable" : ""}`,
    `${a.requests} requests · ${fmt.tok(a.cached)} cached · ${fmt.tok(a.uncached)} new · ${fmt.tok(a.out)} out · ${fmt.usd(a.cost)}`,
    reads.length ? "read: " + reads.join(", ") : "",
    a.tools.some(t => t.name === "task") ? "called task" : "",
  ].filter(Boolean).join("\n");
}

// --- drawer ----------------------------------------------------------------
const drawer = document.getElementById("drawer");
const drawerTitle = document.getElementById("drawer-title");
const drawerBody = document.getElementById("drawer-body");
function openDrawer(title, ...body) {
  drawerTitle.textContent = title;
  drawerBody.replaceChildren(...body.flat());
  drawerBody.scrollTop = 0;
  drawer.classList.add("open");
  drawer.setAttribute("aria-hidden", "false");
}
function closeDrawer() { drawer.classList.remove("open"); drawer.setAttribute("aria-hidden", "true"); }
document.getElementById("drawer-close").onclick = closeDrawer;
addEventListener("keydown", e => { if (e.key === "Escape") closeDrawer(); });

// --- source ----------------------------------------------------------------
const RUST_KW = new Set("as async await break const continue crate else enum false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while dyn".split(" "));
function highlight(text, lang) {
  // A small tokenizer: enough to make Rust, Python and TOML readable. Returns one HTML string per line.
  const out = [];
  let i = 0;
  const esc = t => t.replace(/&/g, "&amp;").replace(/</g, "&lt;");
  const push = (cls, t) => out.push(cls ? `<span class="tk-${cls}">${esc(t)}</span>` : esc(t));
  const lineComment = lang === "rs" ? "//" : "#";
  while (i < text.length) {
    const rest = text.slice(i);
    let m;
    if (rest.startsWith(lineComment)) { m = rest.match(/^[^\n]*/)[0]; push("c", m); i += m.length; continue; }
    if (lang === "rs" && (m = rest.match(/^r#*"/))) {
      const hashes = m[0].length - 2; const end = rest.indexOf('"' + "#".repeat(hashes), m[0].length);
      const t = rest.slice(0, end < 0 ? rest.length : end + 1 + hashes); push("s", t); i += t.length; continue;
    }
    if (rest[0] === '"' || (lang === "py" && rest[0] === "'")) {
      const q = rest[0]; let j = 1;
      if (lang === "py" && rest.startsWith(q.repeat(3))) { const end = rest.indexOf(q.repeat(3), 3); j = end < 0 ? rest.length : end + 3; }
      else { while (j < rest.length && rest[j] !== q) { if (rest[j] === "\\") j++; j++; } j++; }
      push("s", rest.slice(0, j)); i += j; continue;
    }
    if (lang === "rs" && (m = rest.match(/^#!?\[[^\]\n]*\]/))) { push("a", m[0]); i += m[0].length; continue; }
    if (lang === "rs" && (m = rest.match(/^'[a-z_]+\b(?!')/))) { push("t", m[0]); i += m[0].length; continue; }
    if ((m = rest.match(/^\d[\d_]*(\.\d+)?/))) { push("n", m[0]); i += m[0].length; continue; }
    if ((m = rest.match(/^[A-Za-z_]\w*/))) {
      const w = m[0];
      if (lang === "rs" && rest[w.length] === "!") push("m", w + "!"), i += 1;
      else if (lang === "rs" && RUST_KW.has(w)) push("k", w);
      else if (lang === "py" && /^(def|class|import|from|return|for|in|if|else|elif|with|as|not|and|or|None|True|False|lambda|try|except|raise|while|yield)$/.test(w)) push("k", w);
      else if (/^[A-Z]/.test(w)) push("t", w);
      else push("", w);
      i += w.length; continue;
    }
    push("", rest[0]); i += 1;
  }
  // re-split into lines, carrying open spans across line breaks
  const html = out.join("");
  const lines = [];
  let open = null, cur = "";
  for (const part of html.split(/(<span class="tk-\w+">|<\/span>|\n)/)) {
    if (!part) continue;
    if (part === "\n") { lines.push(cur + (open ? "</span>" : "")); cur = open || ""; continue; }
    if (part.startsWith("<span")) open = part;
    else if (part === "</span>") open = null;
    cur += part;
  }
  lines.push(cur);
  return lines;
}

function githubLink(path, line) {
  const repoPath = path.startsWith("docs/") ? path : EXP + path;
  return `${REPO}/blob/${BRANCH}/${repoPath}${line ? "#L" + line : ""}`;
}

function openSource(path, focus, focusEnd) {
  const text = DATA.sources[path];
  if (text == null) return;
  if (path.endsWith(".md")) return openDoc(path, focus);
  const lang = path.endsWith(".rs") ? "rs" : path.endsWith(".py") ? "py" : path.endsWith(".toml") ? "toml" : "";
  const lines = highlight(text, lang);
  const lo = focus || 0, hi = focusEnd || (focus ? focus + 12 : 0);
  const code = h("div", { class: "code" }, lines.map((l, n) => h("div", { class: "ln" + (n + 1 >= lo && n + 1 <= hi && lo ? " hot" : ""), id: "L" + (n + 1) }, h("span", {}, n + 1), h("span", { html: l || " " }))));
  const mod = DATA.modules.find(m => m.path === path);
  const outline = mod ? h("div", { class: "outline" }, mod.fns.map(f => h("span", { class: "chip click" + (f.pub ? " pub" : ""), onclick: () => jump(code, f.line, f.line + 20) }, f.name))) : null;
  openDrawer(path,
    h("div", { class: "filebar" }, `${lines.length} lines`, mod && mod.doc ? h("span", {}, "· " + mod.doc) : null, h("a", { class: "link", href: githubLink(path, focus), target: "_blank", style: { marginLeft: "auto" } }, "on GitHub ↗")),
    outline, code);
  if (focus) requestAnimationFrame(() => jump(code, lo, hi));
}
function jump(code, lo, hi) {
  code.querySelectorAll(".hot").forEach(e => e.classList.remove("hot"));
  for (let n = lo; n <= hi; n++) code.querySelector("#L" + n)?.classList.add("hot");
  const target = code.querySelector("#L" + lo);
  if (target) code.scrollTop = target.offsetTop - 80;
}
function lineOf(path, needle) {
  const text = DATA.sources[path] || "";
  const at = text.indexOf(needle);
  return at < 0 ? 1 : text.slice(0, at).split("\n").length;
}
function fnLine(path, name) {
  const mod = DATA.modules.find(m => m.path === path);
  return mod?.fns.find(f => f.name === name)?.line || lineOf(path, "fn " + name);
}
function srcLink(label, path, line, end) {
  return h("span", { class: "link mono", onclick: e => { e.stopPropagation(); openSource(path, line, end); } }, label);
}

// --- markdown, only as much as these docs use --------------------------------
function md(text) {
  const esc = t => t.replace(/&/g, "&amp;").replace(/</g, "&lt;");
  const inline = t => esc(t).replace(/`([^`]+)`/g, "<code>$1</code>").replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>").replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a class="link" href="$2" target="_blank">$1</a>');
  const out = [];
  let inCode = false, list = false;
  for (const line of text.split("\n")) {
    if (line.startsWith("```")) { out.push(inCode ? "</pre>" : '<pre class="text">'); inCode = !inCode; continue; }
    if (inCode) { out.push(esc(line)); continue; }
    if (list && !/^\s*[-*\d]/.test(line)) { out.push("</ul>"); list = false; }
    let m;
    if ((m = line.match(/^(#{1,4}) (.*)/))) out.push(`<h${m[1].length + 1} style="font-family:var(--sans);color:var(--bright);margin:18px 0 6px">${inline(m[2])}</h${m[1].length + 1}>`);
    else if (line.startsWith(">")) out.push(`<blockquote style="margin:4px 0;padding:4px 12px;border-left:2px solid var(--user);color:#d9ccff">${inline(line.replace(/^>\s?/, ""))}</blockquote>`);
    else if ((m = line.match(/^\s*(?:[-*]|\d+\.) (.*)/))) { if (!list) { out.push('<ul style="margin:4px 0 4px 18px;padding:0">'); list = true; } out.push(`<li>${inline(m[1])}</li>`); }
    else if (line.startsWith("|")) out.push(`<div class="mono" style="font-size:11.5px;white-space:pre;color:#c9cedb">${esc(line)}</div>`);
    else if (line.trim()) out.push(`<p style="margin:6px 0;max-width:90ch">${inline(line)}</p>`);
  }
  return out.join("\n");
}
function openDoc(path, focus) {
  openDrawer(path, h("div", { class: "filebar" }, h("a", { class: "link", href: githubLink(path), target: "_blank" }, "on GitHub ↗")), h("div", { html: md(DATA.sources[path]) }));
  if (typeof focus === "string") requestAnimationFrame(() => {
    const all = [...drawerBody.querySelectorAll("h2, h3, h4, h5, li, p")];
    const hit = all.find(e => /^H/.test(e.tagName) && e.textContent.trim() === focus) || all.find(e => e.textContent.includes(focus));
    if (hit) { hit.scrollIntoView({ block: "start" }); hit.style.background = "#1d2a33"; }
  });
}

// --- views and routing -------------------------------------------------------
const VIEWS = [];
function view(id, label, build) { VIEWS.push({ id, label, build, built: false }); }

// A hash is a view, optionally followed by a drill-down: #watch/request/clean/5, #trials/trial/<id>, #machine/source/<path>/<line>.
function route(hash) {
  const [id, kind, ...rest] = hash.split("/");
  show(id);
  if (kind === "request") openRequest(rest[0], +rest[1]);
  if (kind === "trial") openTrial(trialById(rest[0]), rest[1] && decodeURIComponent(rest[1]));
  if (kind === "source") { const line = +rest[rest.length - 1]; openSource(isNaN(line) ? rest.join("/") : rest.slice(0, -1).join("/"), isNaN(line) ? 0 : line); }
}

function show(id) {
  const target = VIEWS.find(v => v.id === id) || VIEWS[0];
  for (const v of VIEWS) {
    const el = document.getElementById("v-" + v.id);
    if (v === target && !v.built) { v.build(el); v.built = true; }
    el.classList.toggle("on", v === target);
    document.querySelector(`#views a[data-v="${v.id}"]`).classList.toggle("on", v === target);
  }
  target.onShow?.();
  if (location.hash.slice(1).split("/")[0] !== target.id) history.replaceState(null, "", "#" + target.id);
}

function boot() {
  const nav = document.getElementById("views");
  const stage = document.getElementById("stage");
  VIEWS.forEach((v, n) => {
    nav.append(h("a", { href: "#" + v.id, "data-v": v.id, onclick: e => { e.preventDefault(); show(v.id); } }, h("kbd", {}, n + 1), v.label));
    stage.append(h("section", { class: "view", id: "v-" + v.id }));
  });
  addEventListener("keydown", e => {
    if (e.target.closest?.("input, textarea") || e.metaKey || e.ctrlKey || e.altKey) return;
    const n = parseInt(e.key, 10);
    if (n >= 1 && n <= VIEWS.length) show(VIEWS[n - 1].id);
    const at = VIEWS.findIndex(v => document.getElementById("v-" + v.id).classList.contains("on"));
    if (e.key === "ArrowRight" && !drawer.classList.contains("open")) show(VIEWS[Math.min(at + 1, VIEWS.length - 1)].id);
    if (e.key === "ArrowLeft" && !drawer.classList.contains("open")) show(VIEWS[Math.max(at - 1, 0)].id);
  });
  addEventListener("hashchange", () => route(location.hash.slice(1)));
  const beacon = document.getElementById("beacon");
  beacon.querySelector("b").textContent = WAITING.length;
  beacon.onclick = () => { show("path"); setTimeout(() => document.getElementById("waits")?.scrollIntoView({ behavior: "smooth", block: "center" }), 80); };
  route(location.hash.slice(1) || VIEWS[0].id);
}
