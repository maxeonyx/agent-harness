// The dashboard page: main's docs and code, read from /repo.json, and what
// is in flight, from /live.json. Every view is a hash route (#/vision#s1-…),
// so any piece of it can be linked.
import { chip, configure, inline, render, sections, slug } from "./md.js";
import { codeLayout, crateGraph } from "./layout.js";

const get = async (url) => {
  const r = await fetch(url);
  if (!r.ok) throw new Error(`${url}: ${r.status} ${await r.text()}`);
  return r.json();
};

function h(tag, props, ...kids) {
  const [name, ...classes] = tag.split(".");
  const el = document.createElement(name);
  if (classes.length > 0) el.className = classes.join(" ");
  for (const [k, v] of Object.entries(props ?? {})) {
    if (v === undefined || v === null || v === false) continue;
    if (k === "class") el.classList.add(...v.split(" ").filter((c) => c !== ""));
    else if (k.startsWith("on")) el.addEventListener(k.slice(2), v);
    else el.setAttribute(k, v === true ? "" : v);
  }
  el.append(...kids.flat(Infinity).filter((k) => k !== null && k !== undefined && k !== false).map((k) => (typeof k === "number" ? String(k) : k)));
  return el;
}

const nzTime = new Intl.DateTimeFormat("en-NZ", { timeZone: "Pacific/Auckland", weekday: "short", day: "numeric", month: "short", hour: "numeric", minute: "2-digit" });
const when = (t) => nzTime.format(new Date(t));
function ago(t) {
  const s = (Date.now() - t) / 1000;
  if (s < 90) return "just now";
  if (s < 90 * 60) return `${Math.round(s / 60)} min ago`;
  if (s < 36 * 3600) return `${Math.round(s / 3600)} h ago`;
  return `${Math.round(s / 86400)} days ago`;
}
const plural = (n, one, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

// --- what the docs say, read once per commit ---------------------------------

let repo;
let m;
let live = null;
let seen;
let changes = null;

function model() {
  const doc = (path) => {
    const src = repo.docs[path];
    if (src === undefined) throw new Error(`main has no ${path}, which the page renders`);
    return { path, src, secs: sections(src), lines: src.split("\n") };
  };
  const body = (d, sec) => d.lines.slice(sec.start - 1, sec.end).join("\n");
  const bullet = (text, label) => text.match(new RegExp(`^- \\*\\*${label}\\*\\* ?(.*)$`, "m"))?.[1] ?? null;
  const bolds = (text) => [...text.matchAll(/^(?:- |\d+\. )\*\*(.+?)\*\*/gm)].map((x) => x[1].replace(/[.:]$/, ""));
  const find = (d, title) => {
    const s = d.secs.find((x) => x.title === title);
    if (s === undefined) throw new Error(`${d.path} has no section "${title}", which the page shows`);
    return s;
  };
  const under = (d, parent) => {
    const i = d.secs.indexOf(parent);
    const out = [];
    for (const s of d.secs.slice(i + 1)) {
      if (s.depth <= parent.depth) break;
      out.push(s);
    }
    return out;
  };

  const vision = doc("docs/vision.md");
  const plan = doc("docs/plan.md");
  const todo = doc("docs/todo.md");
  const glossary = doc("docs/glossary.md");

  const arch = find(vision, "Architecture");
  const everyStructure = [...body(vision, arch).matchAll(/`(crates\/[\w-]+)`/g)].map((x) => x[1]);
  const structures = vision.secs.filter((s) => /^S[1-6]\. /.test(s.title)).map((s) => {
    const text = body(vision, s);
    const [short, sub] = s.title.replace(/^S\d\. /, "").split(/: /);
    return {
      n: Number(s.title[1]), title: s.title, short, sub, slug: s.slug, sec: s,
      want: bullet(text, "The want\\."), tell: bullet(text, "You can tell by:"),
      dirs: [...(bullet(text, "In code\\.") ?? "").matchAll(/`(crates\/[\w-]+)`/g)].map((x) => x[1]),
    };
  });
  const stories = under(vision, find(vision, "User stories")).map((s) => ({ title: s.title, slug: s.slug, stories: bolds(body(vision, s)) }));
  const episodes = [];
  let group = null;
  for (const line of body(vision, find(vision, "How to tell a design is it")).split("\n")) {
    const g = line.match(/^\*\*(.+?) \(([S\d, ]+)\)\.\*\*$/);
    if (g !== null) { group = { title: g[1], structures: [...g[2].matchAll(/S(\d)/g)].map((x) => Number(x[1])), items: [] }; episodes.push(group); continue; }
    const e = line.match(/^(\d+)\. (.*)$/);
    if (e !== null && group !== null) group.items.push({ n: Number(e[1]), text: e[2] });
  }

  const flow = plan.src.match(/```mermaid\n([\s\S]*?)```/)?.[1] ?? "";
  const labels = Object.fromEntries([...flow.matchAll(/\b([A-F])\["([^"]+)"\]/g)].map((x) => [x[1], x[2].split("<br/>")]));
  const milestones = [...plan.src.matchAll(/^\| \*\*([A-F])\. (.+?)\*\* \| (.+?) \| (.+?) \|$/gm)].map(([, letter, name, can, status]) => ({
    letter, name, can, status,
    state: /^done/i.test(status) ? "done" : /^building/i.test(status) ? "building" : "later",
    track: labels[letter]?.[1] ?? "",
    structures: [...(labels[letter]?.[1] ?? "").matchAll(/S(\d)/g)].map((x) => Number(x[1])),
  }));
  if (milestones.length === 0) throw new Error("docs/plan.md's milestone table didn't parse");
  const questions = bolds(body(plan, find(plan, "Questions only Max can answer")));

  const todoGroups = todo.secs.filter((s) => s.depth === 2).map((s) => ({
    title: s.title, slug: s.slug, sec: s,
    n: Number(s.title.match(/\(S(\d)\)/)?.[1] ?? 0) || null,
    items: bolds(body(todo, s)),
  }));

  const terms = [];
  for (const line of glossary.lines) {
    if (!line.startsWith("| ") || /^\| (Term|---)/.test(line) || line.startsWith("|---")) continue;
    const [term, meaning, code = ""] = line.slice(2, -2).split(" | ");
    const names = term.split(/, /);
    terms.push({ term, names, slug: slug(names[0]), meaning, code: code.trim() });
  }
  for (const t of terms) t.state = /^Not built/.test(t.code) ? "not built" : /^Partly/.test(t.code) ? "partly" : /^Differs/.test(t.code) ? "differs" : null;

  const byDir = Object.fromEntries(repo.crates.map((c) => [c.dir, c.name]));
  const crates = repo.crates.map((c) => {
    // Rustdoc's [`path`] links, with no target written, read as the path alone.
    const secs = sections(c.docs.replace(/\[(`[^`\n]+`)\](?![(\[])/g, (all, code, at, src) => (src[at + all.length] === ":" && (at === 0 || src[at - 1] === "\n") ? all : code)));
    const gaps = [];
    for (const s of secs) {
      if (/not built|provisional|not covered/i.test(s.title)) { gaps.push({ title: s.title, blocks: s.blocks }); continue; }
      for (const b of s.blocks) {
        const label = b.token.type === "paragraph" ? b.token.text.match(/^\*\*(Not built yet|Not covered|Provisional)\.\*\*/)?.[1] : undefined;
        if (label !== undefined) gaps.push({ title: label, blocks: [b], labelled: true });
      }
    }
    const count = (g) => g.blocks.reduce((n, b) => n + (b.token.type === "list" ? b.token.items.length : 0), 0) || g.blocks.length;
    return {
      ...c, short: c.name.replace(/^harness-/, ""), secs, gaps, gapCount: gaps.reduce((n, g) => n + count(g), 0),
      structures: everyStructure.includes(c.dir) ? [1, 2, 3, 4, 5, 6] : structures.filter((s) => s.dirs.includes(c.dir)).map((s) => s.n),
      files: repo.files.filter((f) => f.path.startsWith(`${c.dir}/`)).length,
    };
  });
  for (const s of structures) {
    for (const d of s.dirs) if (!(d in byDir)) throw new Error(`docs/vision.md's ${s.title} names ${d}, which main has no crate at`);
  }

  const variants = new Map();
  for (const t of terms) {
    for (const n of t.names) {
      if (n.includes("`")) continue;
      variants.set(n.toLowerCase().replace(/^the /, ""), t);
    }
  }
  const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const termPattern = new RegExp(`\\b(?:${[...variants.keys()].sort((a, b) => b.length - a.length).map(escape).join("|")})(?:e?s)?\\b`, "gi");
  const termOf = (text) => {
    const t = text.toLowerCase();
    return variants.get(t) ?? variants.get(t.replace(/es$/, "")) ?? variants.get(t.replace(/s$/, ""));
  };

  return { vision, plan, todo, glossary, structures, stories, episodes, milestones, questions, todoGroups, terms, crates, termPattern, termOf };
}

// --- links between everything ------------------------------------------------

const docRoutes = { "docs/vision.md": "vision", "docs/plan.md": "plan", "docs/todo.md": "todo", "docs/glossary.md": "glossary", "demos/README.md": "demos" };
const routeOf = (path) => docRoutes[path] ?? `doc/${path}`;
const github = (path, lines) => `https://github.com/${repo.github}/blob/${repo.commit}/${path}${lines === undefined ? "" : `#L${lines[0]}-L${lines[1]}`}`;
const editUrl = (path) => `https://github.com/${repo.github}/edit/main/${path}`;
let dirs;

function link(path, anchor, lines) {
  path = path.replace(/\/$/, "");
  const crate = m.crates.find((c) => c.dir === path || (c.src === path && lines === undefined && anchor === undefined));
  if (crate !== undefined) return `#/crate/${crate.name}`;
  if (path in repo.docs && path.endsWith(".md")) return `#/${routeOf(path)}${anchor === undefined ? "" : `#${anchor}`}`;
  if (path === "" || dirs.has(path)) return `#/code/${path}`;
  return github(path, lines);
}

function findPath(text, from) {
  text = text.replace(/^\.\//, "").replace(/\/$/, "");
  const files = repo.files.map((f) => f.path);
  if (files.includes(text) || dirs.has(text)) return text;
  if (!/[/.]/.test(text)) return null;
  const near = (from ?? "").split("/").slice(0, -1).join("/");
  if (near !== "" && files.includes(`${near}/${text}`)) return `${near}/${text}`;
  const hits = [...files, ...dirs].filter((p) => p.endsWith(`/${text}`));
  return hits.length === 1 ? hits[0] : null;
}

// --- since Max last looked -----------------------------------------------------

// The baseline lives in this browser: the commit shown when he last came back
// after an hour away. A first visit marks the past week.
function baseline() {
  let s = null;
  try { s = JSON.parse(localStorage.getItem("dashboard.seen") ?? "null"); } catch { s = null; }
  const now = Date.now();
  if (s === null) s = { base: repo.weekAgo, baseAt: now - 7 * 864e5, current: repo.commit, last: now };
  else if (now - s.last > 3600e3) s = { base: s.current, baseAt: s.last, current: repo.commit, last: now };
  else s = { ...s, current: repo.commit, last: now };
  save(s);
  return s;
}
function save(s) {
  try { localStorage.setItem("dashboard.seen", JSON.stringify(s)); } catch { /* private window: marks last this visit only */ }
}
const changedIn = (path) => changes?.files[path];
const isNew = (t) => seen !== undefined && t > seen.baseAt;

// --- the shell ------------------------------------------------------------------

const nav = [
  ["Overview", ""],
  ["Eventual", [["Vision", "vision", "docs/vision.md"], ["To do", "todo", "docs/todo.md"], ["Glossary", "glossary", "docs/glossary.md"]]],
  ["Current", [["Built", "built"], ["Code", "code"], ["Types", "types"], ["Demos", "demos", "demos/README.md"], ["Gaps", "gaps"], ["Terms", "terms"]]],
  ["Plan", [["Plan", "plan", "docs/plan.md"], ["Process", "process"], ["In flight", "inflight"]]],
];

function header() {
  const el = document.querySelector("header");
  const brains = live?.brains ?? [];
  const waiting = brains.flatMap((b) => b.waiting ?? []);
  const behind = brains.filter((b) => (b.behind ?? 0) > 0).length;
  const open = live?.prs.open ?? [];
  const needs = 1 + m.questions.length;
  const changed = changes === null ? null : changes.unknown ? "baseline not in this clone" : `${plural(changes.commits.length, "commit")} since ${when(seen.baseAt)}`;
  el.replaceChildren(
    h("div.top", {},
      h("a.brand", { href: "#/" }, "agent-harness"),
      h("a.main", { href: `https://github.com/${repo.github}/commit/${repo.commit}`, target: "_blank", title: repo.subject }, `${repo.rev} ${repo.commit.slice(0, 7)} · ${ago(repo.time)}`),
      h("span.spacer"),
      h("a.stat.needs", { href: "#/#needs", title: "What only Max can do: the process discussion and his open questions" }, `Needs you ${needs}`, waiting.length > 0 ? ` · ${plural(waiting.length, "chat")} waiting` : ""),
      live === null ? h("span.stat.muted", {}, "Reading what's in flight…") : [
        h("a.stat", { href: "#/inflight#brains", title: brains.map((b) => `${b.name} :${b.port} ${b.commit?.slice(0, 7) ?? "?"}${b.behind ? `, ${b.behind} behind main` : ""}`).join("\n") }, `${plural(brains.length, "brain")}${behind > 0 ? ` · ${behind} behind main` : ""}`),
        h("a.stat", { href: "#/inflight#prs" }, live.errors.some((e) => e.source === "GitHub") ? "PRs unknown" : plural(open.length, "open PR")),
        live.errors.length > 0 ? h("a.stat.bad", { href: "#/inflight", title: live.errors.map((e) => `${e.source}: ${e.message}`).join("\n") }, `${plural(live.errors.length, "source")} failed`) : null,
      ],
      changed === null ? null : h("span.stat.rev-key", { title: "Marked in the page with a bar like this one. The baseline is kept in this browser." },
        h("span.bar"), changed,
        h("button", { onclick: () => { seen = { ...seen, base: repo.commit, baseAt: Date.now() }; save(seen); changes = { since: repo.commit, files: {}, commits: [] }; header(); show(current, true); } }, "Mark seen")),
    ),
    h("nav", {}, nav.map(([label, items]) => typeof items === "string"
      ? h("a", { href: `#/${items}`, class: current === items ? "on" : "" }, label)
      : h("span.group", {}, h("span.label", {}, label), items.map(([l, r, path]) => h("a", { href: `#/${r}`, class: `${current.split("/")[0] === r ? "on" : ""} ${path !== undefined && changedIn(path) !== undefined ? "dot" : ""}` }, l))))),
  );
}

let current = null;
function route() {
  const raw = decodeURIComponent(location.hash.replace(/^#\/?/, ""));
  const i = raw.indexOf("#");
  const path = i < 0 ? raw : raw.slice(0, i);
  const anchor = i < 0 ? null : raw.slice(i + 1);
  const moved = path !== current;
  if (moved) { document.querySelector("aside.drawer")?.remove(); show(path); }
  if (anchor !== null) requestAnimationFrame(() => target(anchor));
  else if (moved) scrollTo(0, 0);
}

function target(id) {
  const el = document.getElementById(id);
  if (el === null) return;
  el.scrollIntoView({ block: "start" });
  el.classList.remove("target");
  void el.offsetWidth;
  el.classList.add("target");
}

const views = {
  "": overview, vision: () => docView("docs/vision.md"), todo: () => docView("docs/todo.md"), glossary: glossaryView,
  plan: () => docView("docs/plan.md"), demos: () => docView("demos/README.md"), doc: (p) => docView(p),
  built, crate: crateView, code: codeView, types, gaps, terms: termsView, process, inflight, s: structureView,
};

function show(path, keepScroll = false) {
  current = path;
  const [view, ...rest] = path.split("/");
  const main = document.querySelector("main");
  const y = scrollY;
  header();
  try {
    if (!(view in views)) throw new Error(`There's no view called "${view}".`);
    main.replaceChildren(views[view](rest.join("/")));
  } catch (e) {
    console.error(e);
    main.replaceChildren(h("div.error", {}, h("b", {}, "This view failed: "), e.message));
  }
  document.title = `${document.querySelector("main h1")?.textContent ?? "Overview"} · agent-harness`;
  if (keepScroll) scrollTo(0, y);
}

// --- small shared pieces ---------------------------------------------------------

const chips = (ns) => (ns.length === 6 ? h("span.chip.all", { title: "Every structure" }, "S1–S6") : ns.map((n) => chip(n)));
const md = (secs, ctx) => render(secs, { route: routeOf(ctx.path ?? ""), ...ctx, changed: ctx.changed ?? (ctx.path === undefined ? undefined : changedIn(ctx.path)) });
const docSecs = (d, pred) => {
  const out = d.secs.filter(pred);
  out.links = d.secs.links;
  return out;
};
const first = (s) => s?.match(/^.*?[.!?](?=\s|$)/)?.[0] ?? s ?? "";
const livePart = (fill) => {
  const el = h("div.live");
  el.fill = fill;
  fill(el);
  return el;
};
const card = (title, sub, ...kids) => h("div.card", {}, h("h3", {}, title, sub === null ? null : h("small", {}, sub)), kids);
const builtBy = (s) => m.crates.filter((c) => c.structures.length === 1 && c.structures[0] === s.n);
const todoOf = (s) => m.todoGroups.find((g) => g.n === s.n);
const changedCrate = (c) => Object.keys(changes?.files ?? {}).some((p) => p.startsWith(`${c.dir}/`));
// A crate's gaps as sections to render, each linkable as #<crate>-<title> on the crate's own page.
const gapSecs = (c, depth) => Object.assign(c.gaps.map((g) => ({ depth: g.labelled ? 0 : depth, title: g.title, slug: `${c.short}-${slug(g.title)}`, start: g.blocks[0].start, blocks: g.blocks })), { links: c.secs.links });
const prTitle = (p) => inline(p.title, { terms: false });
const crateLink = (c) => h("a.crate", { href: `#/crate/${c.name}`, class: c.structures.length === 1 ? `s${c.structures[0]}` : "" }, c.short);
const msChip = (ms) => h("a.ms", { href: "#/plan#milestones", class: ms.state, title: `${ms.letter}. ${ms.name}: ${ms.status}` }, ms.letter, " ", ms.state);
const touched = (paths) => {
  const s = new Set();
  const other = new Set();
  for (const p of paths) {
    const c = m.crates.find((x) => p.startsWith(`${x.dir}/`));
    if (c !== undefined && c.structures.length < 6) c.structures.forEach((n) => s.add(n));
    else other.add(c !== undefined ? c.short : p.split("/")[0].replace(/\.md$/, ""));
  }
  return [...[...s].sort().map((n) => chip(n)), ...[...other].sort().map((o) => h("span.tag", {}, o))];
};
const faceUrl = (b, id) => `https://${location.hostname}:${b.port}/#${id}`;

// --- overview ---------------------------------------------------------------------

function overview() {
  return h("div.overview", {},
    h("h1.visually-hidden", {}, "Overview"),
    h("section.band", {},
      h("h2", {}, "Eventual", h("small", {}, "what the harness is when it's finished · ", h("a", { href: "#/vision" }, "vision.md"))),
      card("Six structures", "everything Max named is an instance of one", h("div.tiles", {}, m.structures.map(tile))),
      card("What Max does with it", "user stories, by the part of his day", h("div.stories", {}, m.stories.map((g) =>
        h("a.story", { href: `#/vision#${g.slug}`, class: revisedSec(m.vision, g.slug) ? "rev" : "" }, h("b", {}, g.title, h("small", {}, ` ${g.stories.length}`)), h("span", {}, g.stories.join(" · ")))))),
      card("Architecture", "domains, brains, limbs and tabs", md(docSecs(m.vision, (s) => s.title === "Architecture").map((s) => ({ ...s, depth: 0, blocks: s.blocks.filter((b) => b.token.type === "code") })), { path: "docs/vision.md" })),
    ),
    h("section.band", {},
      h("h2", {}, "Current", h("small", {}, "what main has built · ", h("a", { href: "#/built" }, "crates"), " · ", h("a", { href: "#/code" }, "code"))),
      card("Crates", "arrows: depends on, the shortest way · colour: the structure it builds", graphOf(m.crates), h("p.note", {}, "The number on a crate counts what its own docs list as not built, provisional or not covered. ", h("a", { href: "#/code" }, "Open the files →"))),
      card("Gaps", null, gapSummary()),
    ),
    h("section.band", {},
      h("h2", {}, "Plan", h("small", {}, "the way between · ", h("a", { href: "#/plan" }, "plan.md"))),
      h("div.card.needs", { id: "needs" }, h("h3", {}, "Needs you"), livePart(needsYou)),
      card("Milestones", "named by what Max can do when each lands", milestoneTrack()),
      card("What is next", null, md(docSecs(m.plan, (s) => s.title === "What is next").map((s) => ({ ...s, depth: 0 })), { path: "docs/plan.md" })),
      card("In flight", "from GitHub and the running brains, now", livePart(flightSummary)),
    ),
  );
}

function revisedSec(d, s) {
  const sec = d.secs.find((x) => x.slug === s);
  const ranges = changedIn(d.path);
  if (sec === undefined || ranges === undefined) return false;
  const end = d.secs[d.secs.indexOf(sec) + 1]?.start ?? d.lines.length;
  return ranges.some(([a, b]) => a < end && b >= sec.start);
}

function tile(s) {
  const todo = todoOf(s);
  const crates = builtBy(s);
  return h("a.tile", { href: `#/s/${s.n}`, class: `s${s.n} ${revisedSec(m.vision, s.slug) ? "rev" : ""}` },
    h("div.t", {}, h("span.chip", { class: `s${s.n}` }, `S${s.n}`), h("b", {}, s.short)),
    h("div.sub", {}, s.sub),
    h("p.want", {}, first(s.want)),
    h("div.foot", {},
      m.milestones.filter((ms) => ms.structures.includes(s.n)).map((ms) => h("span.ms", { class: ms.state }, ms.letter, " ", ms.state)),
      h("span", {}, crates.map((c) => c.short).join(", ")),
      todo === undefined ? null : h("span", {}, `${todo.items.length} to do`)));
}

function graphOf(crates) {
  const el = h("div.graph");
  crateGraph(el, {
    crates: crates.map((c) => ({ id: c.name, label: c.short, deps: c.deps, structures: c.structures, count: c.gapCount, changed: changedCrate(c) })),
    open: (id) => { location.hash = `#/crate/${id}`; },
  }).catch((e) => { el.replaceChildren(h("div.error", {}, `The crate graph didn't lay out: ${e.message}`)); });
  return el;
}

function gapSummary() {
  const wrong = m.todoGroups.find((g) => g.slug === "wrong-steps-to-undo");
  const differ = m.terms.filter((t) => t.state !== null);
  const max = Math.max(...m.todoGroups.map((g) => g.items.length));
  return [
    h("h4", {}, h("a", { href: "#/gaps#wrong-steps-to-undo" }, "Wrong steps to undo"), h("small", {}, " where main went a way the vision doesn't")),
    h("ul.compact", {}, (wrong?.items ?? []).map((t) => h("li", {}, inline(t)))),
    h("h4", {}, h("a", { href: "#/todo" }, "Open work, by structure")),
    h("div.bars", {}, m.todoGroups.filter((g) => g !== wrong).map((g) => h("a.barrow", { href: `#/todo#${g.slug}`, class: revisedSec(m.todo, g.slug) ? "rev" : "" },
      h("span.l", {}, g.n === null ? g.title : [chip(g.n), " ", g.title.replace(/ \(S\d\)$/, "")]),
      h("span.b", {}, h("span", { class: g.n === null ? "" : `s${g.n}`, style: `width:${(100 * g.items.length) / max}%` })),
      h("span.n", {}, g.items.length)))),
    h("h4", {}, h("a", { href: "#/gaps#crates" }, "Not built, by the crates' own docs")),
    h("div.crates", {}, m.crates.filter((c) => c.gapCount > 0).map((c) => h("a.crate", { href: `#/crate/${c.name}#gaps`, class: c.structures.length === 1 ? `s${c.structures[0]}` : "" }, c.short, h("span.count", {}, c.gapCount)))),
    h("h4", {}, h("a", { href: "#/terms" }, `${plural(differ.length, "word")} the code doesn't use the vision's way yet`)),
    h("div.crates", {}, differ.map((t) => h("a.tag", { href: `#/glossary#term-${t.slug}`, title: t.code }, t.names[0], h("small", {}, ` ${t.state}`)))),
  ];
}

function milestoneTrack() {
  return h("div.track", {}, m.milestones.map((ms) => h("a.mrow", { href: "#/plan#milestones", class: ms.state },
    h("span.letter", {}, ms.letter),
    h("div", {}, h("b", {}, ms.name), " ", h("span.state", {}, ms.state), " ", chips(ms.structures), ms.letter === "A" ? h("span.tag", {}, "feature track") : null,
      h("div.can", {}, inline(ms.can, { path: m.plan.path }))))));
}

function needsYou(el) {
  el.replaceChildren(h("ol.needs", {},
    h("li", {}, h("a", { href: "#/process" }, h("b", {}, "Discuss the new process")), " — next in his order, once the cleanup is done (", h("a", { href: "#/plan#what-is-next" }, "plan"), ")"),
    m.questions.map((q) => h("li", {}, h("a", { href: "#/plan#questions-only-max-can-answer" }, h("b", {}, q)), " — an open question, with a recommendation"))),
  live === null ? h("p.muted", {}, "Reading the brains for chats waiting on him…")
    : h("ul.compact.chats", {}, live.brains.filter((b) => (b.waiting ?? []).length > 0).map((b) => h("li", {},
      h("a", { href: `https://${location.hostname}:${b.port}/`, target: "_blank", title: b.waiting.map((w) => w.name ?? w.id).join("\n") }, `${plural(b.waiting.length, "chat")} waiting on him`), ` in the ${b.name} brain: `,
      h("span.muted", {}, [...new Set(b.waiting.map((w) => w.name ?? w.id))].join(" · "))))));
}

function flightSummary(el) {
  if (live === null) return el.replaceChildren(h("p.muted", {}, "Reading GitHub, the brains and the clones…"));
  const merged = live.prs.merged.filter((p) => isNew(Date.parse(p.mergedAt)));
  el.replaceChildren(
    h("h4", {}, h("a", { href: "#/inflight#prs" }, plural(live.prs.open.length, "open pull request"))),
    h("ul.compact", {}, live.prs.open.map((p) => h("li", {}, h("a", { href: p.url, target: "_blank" }, `#${p.number}`), " ", prTitle(p), p.isDraft ? h("span.tag", {}, "draft") : null, " ", touched(p.files)))),
    h("h4", {}, h("a", { href: "#/inflight#brains" }, plural(live.brains.length, "brain"))),
    h("ul.compact", {}, live.brains.map(brainLine)),
    h("h4", {}, h("a", { href: "#/inflight#merged" }, `${plural(merged.length, "PR")} merged since you last looked`)),
    h("ul.compact", {}, merged.slice(0, 6).map((p) => h("li.rev", {}, h("a", { href: p.url, target: "_blank" }, `#${p.number}`), " ", prTitle(p)))),
    errorsOf(),
  );
}

function brainLine(b) {
  if (b.error !== undefined) return h("li", {}, h("b", {}, b.name), ` :${b.port} — `, h("span.bad", {}, b.error));
  const working = b.working.length;
  return h("li", {}, h("b", {}, b.name), ` :${b.port} on `, h("a", { href: `https://github.com/${repo.github}/commit/${b.commit}`, target: "_blank" }, b.commit?.slice(0, 7) ?? "an unknown build"), b.dirty ? " +dirty" : "",
    b.behind === null ? "" : b.behind > 0 ? h("span.warn", {}, ` · ${b.behind} commits behind main`) : " · up to date",
    ` · ${plural(working, "task")} working`, b.waiting.length > 0 ? h("span.warn", {}, ` · ${b.waiting.length} waiting on Max`) : null);
}

const errorsOf = () => (live.errors.length === 0 ? "" : h("div.error", {}, live.errors.map((e) => h("div", {}, h("b", {}, e.source), ": ", e.message))));

// --- docs -------------------------------------------------------------------------

function docView(path) {
  const src = repo.docs[path];
  if (src === undefined) throw new Error(`main has no ${path}.`);
  const secs = path === m.vision.path ? m.vision.secs : sections(src);
  const doc = md(secs, { path });
  if (path === m.vision.path) structureStrips(doc);
  return h("div.docpage", {}, toc(secs, path), h("article", {}, docHead(path), doc));
}

function docHead(path) {
  return h("div.dochead", {}, h("code", {}, path), h("span.spacer"),
    h("a", { href: github(path), target: "_blank" }, "On GitHub"),
    h("a", { href: editUrl(path), target: "_blank", title: "Opens GitHub's editor on main: saving it makes a commit or a branch for a PR" }, "Edit"));
}

function toc(secs, path) {
  const ranges = changedIn(path);
  return h("nav.toc", {}, secs.filter((s) => s.depth > 0 && s.depth <= 3).map((s, i, all) => {
    const end = all[i + 1]?.start ?? Infinity;
    const rev = ranges?.some(([a, b]) => a < end && b >= s.start);
    return h("a", { href: `#/${routeOf(path)}#${s.slug}`, class: `d${s.depth} ${rev ? "rev" : ""}` }, inline(s.title, { terms: false }));
  }));
}

// Each structure's section in the vision says where it is now: its crates,
// its milestones, its open work.
function structureStrips(doc) {
  for (const s of m.structures) {
    const sec = doc.querySelector(`section#${CSS.escape(s.slug)}`);
    if (sec === null) continue;
    sec.classList.add("structure", `s${s.n}`);
    sec.append(strip(s));
  }
}

function strip(s) {
  const todo = todoOf(s);
  const crates = builtBy(s);
  return h("div.strip", {},
    h("span", {}, "Built in ", crates.length === 0 ? "no crate yet" : crates.map(crateLink)),
    h("span", {}, "Milestones ", m.milestones.filter((ms) => ms.structures.includes(s.n)).map(msChip)),
    todo === undefined ? null : h("a", { href: `#/todo#${todo.slug}` }, `${todo.items.length} open in to do`),
    h("a", { href: `#/s/${s.n}` }, `Everything about S${s.n} →`));
}

function glossaryView() {
  const doc = render(m.glossary.secs, {
    route: "glossary", path: m.glossary.path, changed: changedIn(m.glossary.path),
    prepare: (root) => {
      for (const tr of root.querySelectorAll("tbody tr")) {
        const t = m.terms.find((x) => x.term === tr.cells[0].textContent.trim() || x.term.replace(/`/g, "") === tr.cells[0].textContent.trim());
        tr.cells[0].classList.add("term-cell");
        if (t === undefined) continue;
        tr.id = `term-${t.slug}`;
        if (t.state !== null) tr.cells[tr.cells.length - 1].append(" ", h("span.tag.warn", {}, t.state));
      }
    },
  });
  return h("div.docpage", {}, toc(m.glossary.secs, m.glossary.path), h("article", {}, docHead(m.glossary.path), doc));
}

// --- current ------------------------------------------------------------------------

function built() {
  const diagrams = m.crates.flatMap((c) => c.secs.filter((s) => s.blocks.some((b) => b.token.type === "code" && b.token.lang === "mermaid")).map((s) => ({ c, s })));
  return h("div.page", {},
    h("h1", {}, "Built"),
    h("p.lead", {}, "What main has built, from each crate's own docs on ", h("a", { href: `https://github.com/${repo.github}/commit/${repo.commit}`, target: "_blank" }, repo.commit.slice(0, 7)), ". Colour is the structure a crate builds; core and brain serve all six."),
    diagrams.map(({ c, s }) => card(s.title === "" ? c.short : inline(s.title, { terms: false }), h("a", { href: `#/crate/${c.name}` }, `from ${c.short}'s docs`),
      render(Object.assign([{ ...s, depth: 0, blocks: s.blocks.filter((b) => b.token.type === "code" && b.token.lang === "mermaid") }], { links: c.secs.links }), { route: `crate/${c.name}`, crate: c.name, path: c.src }))),
    card("Crates", "arrows: depends on, the shortest way", graphOf(m.crates)),
    h("div.cards", {}, m.crates.map((c) => h("a.cratecard", { href: `#/crate/${c.name}`, class: c.structures.length === 1 ? `s${c.structures[0]}` : "" },
      h("div.t", {}, h("b", {}, c.short), " ", chips(c.structures)),
      h("p", {}, c.description),
      h("div.foot", {}, plural(c.files, "file"), c.gapCount > 0 ? h("span.warn", {}, ` · ${c.gapCount} not built or provisional`) : null,
        changedCrate(c) ? h("span.revtag", {}, "changed") : null)))),
  );
}

function crateView(name) {
  const c = m.crates.find((x) => x.name === name);
  if (c === undefined) throw new Error(`main has no crate ${name}.`);
  const users = m.crates.filter((x) => x.deps.includes(c.name));
  const ctx = { route: `crate/${c.name}`, crate: c.name, path: c.src, changed: changedIn(c.src), lineOffset: c.docsStart - 1, shift: 1 };
  return h("div.page", {},
    h("h1", {}, c.name, " ", chips(c.structures)),
    h("p.lead", {}, c.description),
    h("p.meta", {}, "Depends on ", c.deps.length === 0 ? "no other crate" : m.crates.filter((x) => c.deps.includes(x.name)).map(crateLink), " · used by ", users.length === 0 ? "nothing" : users.map(crateLink),
      " · ", h("a", { href: `#/code/${c.dir}` }, `${plural(c.files, "file")} in the code layout`), " · ", h("a", { href: github(c.src), target: "_blank" }, c.src)),
    c.gaps.length === 0 ? null : h("div.card.gaps", { id: "gaps" }, h("h3", {}, "What its docs say isn't built, or is provisional"),
      render(gapSecs(c, 4), { ...ctx, shift: 0 })),
    h("article", {}, render(c.secs, ctx)),
  );
}

function codeView(path) {
  const panel = h("aside.filepanel", {}, h("p.muted", {}, "Hover a file to see what it names and what names it; click one for the list. Click a box to open or close it."));
  const box = h("div.layout");
  const files = repo.files.map((f) => f.path);
  const changed = new Set(Object.keys(changes?.files ?? {}));
  const colour = (p) => {
    const c = m.crates.find((x) => p === x.dir || p.startsWith(`${x.dir}/`));
    return c !== undefined && c.structures.length === 1 ? c.structures[0] : null;
  };
  const layout = codeLayout(box, {
    files, refs: repo.refs, changed, colour, focus: path === "" ? null : path,
    pick: (p) => panel.replaceChildren(filePanel(p)),
  });
  return h("div.page.wide", {},
    h("h1", {}, "Code"),
    h("p.lead", {}, "The whole repo on main as nested boxes: one per directory (a chain of single directories is one box), a node per file. A line joins two files when one names the other: its path, a sibling's name, or a Rust module path; ↔ when each names the other. Lines into a closed box are summed onto it."),
    h("div.toolbar", {},
      h("button", { onclick: () => layout.expand("all") }, "Open every box"),
      h("button", { onclick: () => layout.expand("crates") }, "Crates only"),
      h("button", { onclick: () => layout.zoom(1.25) }, "Zoom in"),
      h("button", { onclick: () => layout.zoom(0.8) }, "Zoom out"),
      changed.size > 0 ? h("span.rev-key", {}, h("span.bar"), "changed since you last looked") : null),
    h("div.codegrid", {}, box, panel));
}

function filePanel(p) {
  const out = repo.refs.filter(([a]) => a === p).map(([, b]) => b).sort();
  const into = repo.refs.filter(([, b]) => b === p).map(([a]) => a).sort();
  const crate = m.crates.find((c) => p.startsWith(`${c.dir}/`));
  const list = (ps) => (ps.length === 0 ? h("p.muted", {}, "none") : h("ul.compact", {}, ps.map((x) => h("li", {}, h("a", { href: `#/code/${x}` }, x)))));
  return h("div", {},
    h("h3", {}, h("code", {}, p)),
    h("p", {}, h("a", { href: github(p), target: "_blank" }, "Open on GitHub"), crate === undefined ? null : [" · ", crateLink(crate)], p in repo.docs ? [" · ", h("a", { href: link(p) }, "Rendered")] : null),
    h("h4", {}, `Names ${out.length}`), list(out),
    h("h4", {}, `Named by ${into.length}`), list(into));
}

function types() {
  const core = m.crates.find((c) => c.name === "harness-core");
  const i = core.secs.findIndex((s) => s.slug === "identities");
  if (i < 0) throw new Error("harness-core's docs have no Identities section");
  const secs = Object.assign([{ ...core.secs[i], depth: 0 }], { links: core.secs.links });
  return h("div.page", {},
    h("h1", {}, "Types"),
    h("p.lead", {}, "The core crate's identities: what each thing in the harness is named by, the facts it has, and where what is true now is read from. Each type opens its definition on main."),
    render(secs, { route: "types", crate: core.name, path: core.src, changed: changedIn(core.src), lineOffset: core.docsStart - 1 }),
    h("p", {}, h("a", { href: `#/crate/${core.name}` }, "The rest of the core crate's docs →")));
}

function gaps() {
  const wrong = docSecs(m.todo, (s) => s.slug === "wrong-steps-to-undo");
  return h("div.page", {},
    h("h1", {}, "Gaps"),
    h("p.lead", {}, "Where main differs from the vision: the wrong steps, then what each crate's own docs say isn't built. The open work toward the vision is in ", h("a", { href: "#/todo" }, "To do"), "."),
    md(wrong, { path: m.todo.path, route: "gaps" }),
    h("section", { id: "crates" }, h("h2", {}, "Not built, by the crates' own docs"),
      m.crates.filter((c) => c.gaps.length > 0).map((c) => h("div.card", {}, h("h3", {}, crateLink(c), " ", chips(c.structures)),
        render(gapSecs(c, 4),
          { route: `crate/${c.name}`, crate: c.name, path: c.src, changed: changedIn(c.src), lineOffset: c.docsStart - 1 })))),
    h("section", {}, h("h2", {}, "Open work, by structure"), gapSummary()[3]));
}

function termsView() {
  const differ = m.terms.filter((t) => t.state !== null);
  return h("div.page", {},
    h("h1", {}, "Terms"),
    h("p.lead", {}, "The glossary's words where the code says something else, or has nothing yet. The difference is the vision's naming waiting to reach the code, unless the row says otherwise."),
    h("table.terms", {}, h("thead", {}, h("tr", {}, h("th", {}, "Term"), h("th", {}, "In the vision"), h("th", {}, "In code"))),
      h("tbody", {}, differ.map((t) => h("tr", {},
        h("td.term-cell", {}, h("a", { href: `#/glossary#term-${t.slug}` }, t.names.join(", ")), h("div", {}, h("span.tag.warn", {}, t.state))),
        h("td", {}, inline(t.meaning, { path: m.glossary.path, route: "glossary" })),
        h("td", {}, inline(t.code, { path: m.glossary.path, route: "glossary", terms: false })))))));
}

// --- plan ---------------------------------------------------------------------------

function process() {
  const agents = repo.docs["AGENTS.md"];
  const asecs = sections(agents);
  const pick = (secs, titles) => Object.assign(secs.filter((s) => titles.includes(s.title)), { links: secs.links });
  return h("div.page", {},
    h("h1", {}, "Process"),
    md(pick(m.plan.secs, ["How the work runs"]), { path: m.plan.path }),
    h("h2.from", {}, "Today, from ", h("a", { href: "#/doc/AGENTS.md" }, "AGENTS.md")),
    render(pick(asecs, ["What is settled", "How the work runs"]), { route: "doc/AGENTS.md", path: "AGENTS.md", changed: changedIn("AGENTS.md") }));
}

function inflight() {
  return h("div.page", {}, h("h1", {}, "In flight"), livePart((el) => {
    if (live === null) return el.replaceChildren(h("p.muted", {}, "Reading GitHub, the brains and the clones…"));
    const prFor = (branch) => live.prs.open.find((p) => p.headRefName === branch);
    el.replaceChildren(
      h("p.lead", {}, `Read ${ago(live.at)} (${when(live.at)}) from GitHub, every running brain's /view and the clones beside the dashboard's; never stored. Agents running in Claude Code aren't shown.`),
      errorsOf(),
      h("h2", { id: "prs" }, "Open pull requests"),
      table(["PR", "Title", "Branch", "Touches", "Updated"], live.prs.open.map((p) => [
        h("a", { href: p.url, target: "_blank" }, `#${p.number}`), [prTitle(p), p.isDraft ? h("span.tag", {}, "draft") : null], h("code", {}, p.headRefName), touched(p.files), ago(Date.parse(p.updatedAt))])),
      h("h2", { id: "brains" }, "Brains"),
      h("div.cards", {}, live.brains.map(brainCard)),
      h("h2", { id: "merged" }, "Merged recently"),
      table(["PR", "Title", "Touches", "Merged"], live.prs.merged.map((p) => Object.assign([
        h("a", { href: p.url, target: "_blank" }, `#${p.number}`), prTitle(p), touched(p.files), when(Date.parse(p.mergedAt))], { rev: isNew(Date.parse(p.mergedAt)) }))),
      h("h2", { id: "clones" }, "Clones with work on them"),
      table(["Clone", "Branch", "Ahead, behind", "Uncommitted", "Last commit", "PR"], live.clones.map((c) => {
        const pr = prFor(c.branch);
        return [h("code", {}, c.dir), h("code", {}, c.branch ?? "detached"), `${c.ahead ?? "?"} ahead, ${c.behind ?? "?"} behind`, c.dirty === 0 ? "" : plural(c.dirty, "file"), ago(c.last), pr === undefined ? "" : h("a", { href: pr.url, target: "_blank" }, `#${pr.number}`)];
      })));
  }));
}

function brainCard(b) {
  if (b.error !== undefined) return h("div.card", {}, h("h3", {}, b.name, h("small", {}, ` :${b.port}`)), h("p.bad", {}, b.error));
  return h("div.card", {},
    h("h3", {}, h("a", { href: `https://${location.hostname}:${b.port}/`, target: "_blank" }, b.name), h("small", {}, ` :${b.port} · pid ${b.pid}`)),
    h("p", {}, "Build ", h("a", { href: `https://github.com/${repo.github}/commit/${b.commit}`, target: "_blank" }, b.commit?.slice(0, 7) ?? "unknown"), b.dirty ? " +dirty" : "",
      b.behind === null ? "" : b.behind > 0 ? h("span.warn", {}, `, ${b.behind} commits behind main`) : ", up to date with main", ` · ${plural(b.tasks, "task")} in all`),
    h("h4", {}, `Waiting on Max ${b.waiting.length}`),
    h("ul.compact", {}, b.waiting.map((w) => h("li", {}, h("a", { href: faceUrl(b, w.id), target: "_blank" }, w.name ?? w.id), h("small", {}, ` ${w.environment ?? ""}`)))),
    h("h4", {}, `Working ${b.working.length}`),
    h("ul.compact", {}, b.working.map((w) => h("li", {}, h("a", { href: faceUrl(b, w.id), target: "_blank" }, w.name ?? w.id), h("small", {}, ` ${w.environment ?? ""} · ${w.kind ?? "running"}`)))));
}

function table(head, rows) {
  if (rows.length === 0) return h("p.muted", {}, "None.");
  return h("div.scroll", {}, h("table", {}, h("thead", {}, h("tr", {}, head.map((x) => h("th", {}, x)))), h("tbody", {}, rows.map((r) => h("tr", { class: r.rev ? "rev" : "" }, r.map((c) => h("td", {}, c)))))));
}

// --- one structure, from every doc ------------------------------------------------

function structureView(arg) {
  const s = m.structures.find((x) => x.n === Number(arg));
  if (s === undefined) throw new Error(`There's no structure S${arg}.`);
  const todo = todoOf(s);
  const crates = builtBy(s);
  const episodes = m.episodes.filter((e) => e.structures.includes(s.n));
  const touching = () => live.prs.open.filter((p) => p.files.some((f) => crates.some((c) => f.startsWith(`${c.dir}/`))));
  return h("div.page", {},
    h("div.shead", { class: `s${s.n}` }, h("h1", {}, h("span.chip", { class: `s${s.n}` }, `S${s.n}`), " ", s.short, h("small", {}, `: ${s.sub}`)), h("p", {}, inline(s.want ?? ""))),
    h("div.split", {},
      h("div", {},
        card("In the vision", h("a", { href: `#/vision#${s.slug}` }, "vision.md"), md(docSecs(m.vision, (x) => x === s.sec).map((x) => ({ ...x, depth: 0 })), { path: m.vision.path })),
        todo === undefined ? null : card("Open work", h("a", { href: `#/todo#${todo.slug}` }, "todo.md"), md(docSecs(m.todo, (x) => x === todo.sec).map((x) => ({ ...x, depth: 0 })), { path: m.todo.path })),
      ),
      h("div", {},
        card("Milestones", null, h("div.track", {}, m.milestones.filter((ms) => ms.structures.includes(s.n)).map((ms) => h("a.mrow", { href: "#/plan#milestones", class: ms.state }, h("span.letter", {}, ms.letter), h("div", {}, h("b", {}, ms.name), " ", h("span.state", {}, ms.state), h("div.can", {}, inline(ms.status, { path: m.plan.path })))))),
          h("p.note", {}, "Core and brain hold and run every structure's types.")),
        card("Built in", null, crates.length === 0 ? h("p.muted", {}, "No crate of its own yet.") : crates.map((c) => h("div.cratebox", {},
          h("h4", {}, crateLink(c), " ", h("small", {}, c.description)),
          c.gaps.length === 0 ? null : render(gapSecs(c, 5), { route: `crate/${c.name}`, crate: c.name, path: c.src })))),
        card("Episodes that show it's right", h("a", { href: "#/vision#how-to-tell-a-design-is-it" }, "vision.md"), episodes.length === 0 ? h("p.muted", {}, `No episode in the vision names S${s.n}.`) : episodes.map((e) => [h("h4", {}, e.title, " ", chips(e.structures)), h("ol.compact", {}, e.items.map((x) => h("li", { value: x.n }, inline(x.text))))])),
        card("In flight", null, livePart((el) => {
          if (live === null) return el.replaceChildren(h("p.muted", {}, "Reading…"));
          const prs = touching();
          el.replaceChildren(prs.length === 0 ? h("p.muted", {}, "No open PR touches its crates.") : h("ul.compact", {}, prs.map((p) => h("li", {}, h("a", { href: p.url, target: "_blank" }, `#${p.number}`), " ", prTitle(p)))));
        })),
      )));
}

// --- the drawer for a Rust item -------------------------------------------------------

function openItem(key) {
  const it = repo.items[key];
  const name = key.split(" ")[1];
  const code = h("code.language-rust", {}, it.code ?? `// ${it.path}: the module's file`);
  globalThis.hljs?.highlightElement(code);
  const drawer = h("aside.drawer", {},
    h("div.dh", {}, h("code", {}, name), h("span.spacer"), h("a", { href: github(it.path, [it.line, it.line]), target: "_blank" }, `${it.path}:${it.line}`), h("button", { onclick: () => drawer.remove(), title: "Close (Esc)" }, "×")),
    h("pre", {}, code));
  document.querySelector("aside.drawer")?.remove();
  document.body.append(drawer);
}
addEventListener("keydown", (e) => { if (e.key === "Escape") document.querySelector("aside.drawer")?.remove(); });

// --- start --------------------------------------------------------------------------

async function load() {
  repo = await get("/repo.json");
  dirs = new Set(repo.files.flatMap((f) => f.path.split("/").slice(0, -1).map((_, i, a) => a.slice(0, i + 1).join("/"))));
  configure({
    items: repo.items, link, findPath, openItem,
    get termPattern() { return m.termPattern; },
    termOf: (x) => m.termOf(x),
    term: (s) => m.terms.find((t) => t.slug === s),
    structureTitle: (n) => m.structures.find((s) => s.n === n)?.title ?? `S${n}`,
  });
  m = model();
  seen = baseline();
  changes = seen.base === null ? null : await get(`/changes.json?since=${seen.base}`);
}

async function refreshLive() {
  try {
    live = await get("/live.json");
  } catch (e) {
    live = { at: Date.now(), main: repo.commit, prs: { open: [], merged: [] }, brains: [], clones: [], errors: [{ source: "dashboard server", message: e.message }] };
  }
  if (live.main !== repo.commit) {
    await load();
    show(current, true);
    return;
  }
  header();
  for (const el of document.querySelectorAll(".live")) el.fill(el);
}

try {
  await load();
  addEventListener("hashchange", route);
  route();
  await refreshLive();
  setInterval(refreshLive, 60_000);
  setInterval(() => { seen = { ...seen, last: Date.now() }; save(seen); }, 60_000);
} catch (e) {
  console.error(e);
  document.querySelector("main").replaceChildren(h("div.error", {}, h("b", {}, "The dashboard couldn't load: "), e.message));
}
