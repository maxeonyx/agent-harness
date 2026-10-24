#!/usr/bin/env node
// The project's dashboard: the vision, what main has built, and the plan
// between, rendered from main itself, with what is in flight beside them.
//
//   node dev/dashboard/serve.mjs [--port 4330] [--rev origin/main] [--tls DIR]
//
// It reads the repo at REV through git (fetching origin at most once a
// minute), never the working tree, so it shows main whatever branch its
// clone is on. The docs are sent as they are and rendered by the page;
// the code layout, the crates and the Rust items crate docs link to are
// derived from the same tree. What is in flight is read when asked and
// never stored: pull requests from `gh`, every running brain's /view, and
// the clones beside this one that are on a branch or hold unpushed work.
// The workspace is the directory this clone sits in.
//
// TLS uses DIR/wildcard-local.{crt,key} (default ~/.config/dev-certs/shared-local).
// Max's instance is the user unit harness-dashboard:
//   systemctl --user cat harness-dashboard   # its WorkingDirectory is the clone it runs from
import { execFile, spawn } from "node:child_process";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { createServer, get } from "node:https";
import { homedir } from "node:os";
import { basename, dirname, extname, join, posix } from "node:path";
import { promisify } from "node:util";

const run = promisify(execFile);
const here = dirname(new URL(import.meta.url).pathname);
const args = {};
for (let i = 2; i < process.argv.length; i += 2) {
  const flag = process.argv[i].match(/^--(port|rev|tls)$/)?.[1];
  if (flag === undefined || process.argv[i + 1] === undefined) throw new Error(`usage: serve.mjs [--port N] [--rev REV] [--tls DIR], not ${process.argv[i]}`);
  args[flag] = process.argv[i + 1];
}
const port = Number(args.port ?? 4330);
const rev = args.rev ?? "origin/main";
const tls = args.tls ?? join(homedir(), ".config/dev-certs/shared-local");
const root = (await run("git", ["rev-parse", "--show-toplevel"], { cwd: here })).stdout.trim();
const workspace = dirname(root);
const github = (await git("remote", "get-url", "origin")).trim().match(/github\.com[:/](.+?)(\.git)?$/)?.[1];
if (github === undefined) throw new Error(`origin of ${root} is not a GitHub remote; the page links every file to GitHub`);

async function git(...a) {
  return (await run("git", a, { cwd: root, maxBuffer: 1 << 28 })).stdout;
}

// --- main, as a tree of files ------------------------------------------------

let fetched = 0;
async function head() {
  if (rev.startsWith("origin/") && Date.now() - fetched > 60_000) {
    fetched = Date.now();
    await git("fetch", "-q", "origin").catch((e) => console.error(`git fetch: ${e.message}`));
  }
  return (await git("rev-parse", rev)).trim();
}

async function blobs(commit) {
  const entries = (await git("ls-tree", "-r", "-z", commit)).split("\0").filter(Boolean).map((l) => {
    const [meta, path] = l.split("\t");
    return { sha: meta.split(" ")[2], path };
  });
  const out = await new Promise((resolve, reject) => {
    const p = spawn("git", ["cat-file", "--batch"], { cwd: root });
    const chunks = [];
    p.stdout.on("data", (c) => chunks.push(c));
    p.on("error", reject);
    p.on("close", (code) => (code === 0 ? resolve(Buffer.concat(chunks)) : reject(new Error(`git cat-file exited ${code}`))));
    p.stdin.end(entries.map((e) => e.sha).join("\n") + "\n");
  });
  let at = 0;
  return entries.map((e) => {
    const nl = out.indexOf(10, at);
    const size = Number(out.toString("latin1", at, nl).split(" ")[2]);
    const bytes = out.subarray(nl + 1, nl + 1 + size);
    at = nl + 1 + size + 1;
    const binary = bytes.includes(0) || size > 1 << 20;
    return { path: e.path, size, text: binary ? null : bytes.toString("utf8") };
  });
}

const snapshots = new Map();
function snapshot(commit) {
  if (!snapshots.has(commit)) {
    for (const old of snapshots.keys()) snapshots.delete(old);
    snapshots.set(commit, build(commit).catch((e) => { snapshots.delete(commit); throw e; }));
  }
  return snapshots.get(commit);
}

async function build(commit) {
  const [files, log, weekAgo] = await Promise.all([
    blobs(commit),
    git("log", "-1", "--format=%ct%x09%s", commit),
    git("rev-list", "-1", "--before=7.days.ago", commit),
  ]);
  const [time, subject] = log.trim().split("\t");
  const text = Object.fromEntries(files.filter((f) => f.text !== null).map((f) => [f.path, f.text]));
  const crates = cratesOf(text);
  const docs = Object.fromEntries(Object.entries(text).filter(([p]) => p.endsWith(".md") && !p.startsWith("crates/")));
  return {
    rev: rev.replace(/^origin\//, ""), commit, time: Number(time) * 1000, subject, github, weekAgo: weekAgo.trim() || null,
    docs,
    crates: crates.map(({ modules, ...c }) => c),
    items: items(crates, text),
    files: files.map((f) => ({ path: f.path, size: f.size })),
    refs: references(files, crates, text),
  };
}

// --- crates ------------------------------------------------------------------

function cratesOf(text) {
  const crates = Object.keys(text).filter((p) => /^crates\/[^/]+\/Cargo\.toml$/.test(p)).map((toml) => {
    const dir = dirname(toml);
    const t = text[toml];
    const field = (k) => t.match(new RegExp(`^${k}\\s*=\\s*"(.*)"`, "m"))?.[1];
    const deps = (t.match(/^\[dependencies\]\n([\s\S]*?)(?=^\[|$(?![\s\S]))/m)?.[1] ?? "").match(/^harness-[\w-]+(?=\s*=)/gm) ?? [];
    const src = [`${dir}/src/lib.rs`, `${dir}/src/main.rs`].find((p) => p in text);
    if (src === undefined) throw new Error(`${dir} has neither src/lib.rs nor src/main.rs`);
    const modules = Object.fromEntries(Object.keys(text).filter((p) => p.startsWith(`${dir}/src/`) && p.endsWith(".rs") && !p.startsWith(`${dir}/src/bin/`)).map((p) => {
      const rel = p.slice(`${dir}/src/`.length).replace(/(^|\/)(mod|lib|main)\.rs$|\.rs$/, "");
      return [rel.replaceAll("/", "::"), p];
    }));
    const lines = text[src].split("\n");
    return {
      name: field("name"), dir, description: field("description"), src, deps,
      docs: lines.filter((l) => l.startsWith("//!")).map((l) => l.replace(/^\/\/! ?/, "")).join("\n"),
      docsStart: lines.findIndex((l) => l.startsWith("//!")) + 1,
      modules,
    };
  });
  return crates.sort((a, b) => a.name.localeCompare(b.name));
}

const ident = (crate) => crate.name.replaceAll("-", "_");

// Every path a crate's docs link to ([`state::State`], [text](task::Move::Back)),
// keyed "<crate> <path>", and every Rust path a doc outside the crates names
// (`task::Task`), keyed "docs <path>" and looked up in the core crate first:
// each resolved to the Rust it names on main: the file, the line, and the item's text.
function items(crates, text) {
  const byIdent = Object.fromEntries(crates.map((c) => [ident(c), c]));
  const out = {};
  for (const c of crates) {
    for (const m of c.docs.matchAll(/\[`([\w:]+)`\](?!\()|\]\(([\w:]+)\)/g)) {
      const path = m[1] ?? m[2];
      const key = `${c.name} ${path}`;
      if (!(key in out)) out[key] = resolve(c, path.split("::"), byIdent, text);
    }
  }
  const coreFirst = [...crates].sort((a, b) => (b.name === "harness-core") - (a.name === "harness-core"));
  for (const [p, t] of Object.entries(text)) {
    if (!p.endsWith(".md") || p.startsWith("crates/")) continue;
    for (const m of t.matchAll(/`([a-z_]+(?:::\w+)+)`/g)) {
      const key = `docs ${m[1]}`;
      if (key in out) continue;
      out[key] = null;
      for (const c of coreFirst) {
        const r = resolve(c, m[1].split("::"), byIdent, text);
        if (r !== null && r.code !== null) { out[key] = r; break; }
      }
    }
  }
  return Object.fromEntries(Object.entries(out).filter(([, v]) => v !== null));
}

function resolve(crate, segs, byIdent, text) {
  if (segs[0] === "crate" || segs[0] === "self") segs = segs.slice(1);
  if (segs[0] in byIdent) { crate = byIdent[segs[0]]; segs = segs.slice(1); }
  else if (!(segs[0] in crate.modules) && `harness_${segs[0]}` in byIdent) { crate = byIdent[`harness_${segs[0]}`]; segs = segs.slice(1); }
  let mod = "";
  for (let i = segs.length; i > 0; i--) {
    if (segs.slice(0, i).join("::") in crate.modules) { mod = segs.slice(0, i).join("::"); segs = segs.slice(i); break; }
  }
  const file = crate.modules[mod];
  if (segs.length === 0) return { path: file, line: 1, code: null };
  const def = (name) => new RegExp(`^[ \\t]*(?:pub(?:\\([^)]*\\))?[ \\t]+)?(?:(?:async|unsafe|const)[ \\t]+)*(?:struct|enum|trait|type|fn|const|static|mod|union)[ \\t]+${name}\\b`, "m");
  const at = (path, re) => {
    const m = text[path].match(re);
    return m === null ? null : text[path].slice(0, m.index).split("\n").length;
  };
  let path = file;
  let line = at(file, def(segs[0]));
  if (line === null) {
    path = Object.values(crate.modules).find((p) => at(p, def(segs[0])) !== null);
    if (path === undefined) {
      // A name another crate defines and this module re-exports resolves there.
      const via = text[file].match(new RegExp(`^[ \\t]*pub use (\\w+)::(?:\\{[^}]*\\b${segs[0]}\\b[^}]*\\}|(?:\\w+::)*${segs[0]}\\b)`, "m"));
      if (via !== null && via[1] in byIdent && byIdent[via[1]] !== crate) return resolve(byIdent[via[1]], segs, byIdent, text);
      // A name a macro defines, given alone on a line of its invocation (`id!(… DeliveryId)`).
      const alone = new RegExp(`^[ \\t]*${segs[0]}[ \\t]*,?[ \\t]*$`, "m");
      path = Object.values(crate.modules).find((p) => at(p, alone) !== null);
      if (path === undefined) return null;
      return { path, line: at(path, alone), code: extract(text[path], at(path, alone)) };
    }
    line = at(path, def(segs[0]));
  }
  if (segs.length > 1) {
    const member = at(path, new RegExp(`^[ \\t]*(?:pub(?:\\([^)]*\\))?[ \\t]+)?(?:(?:async|unsafe|const)[ \\t]+)*fn[ \\t]+${segs[1]}\\b|^[ \\t]+(?:pub[ \\t]+)?${segs[1]}\\b[ \\t]*[({:,]`, "m"));
    if (member !== null) line = member;
  }
  return { path, line, code: extract(text[path], line) };
}

// The item starting at `line` (1-based), with the doc comments and attributes
// above it, up to the brace or semicolon that ends it.
function extract(src, line) {
  const lines = src.split("\n");
  let start = line - 1;
  while (start > 0 && /^\s*(\/\/\/|#\[)/.test(lines[start - 1])) start--;
  let depth = 0;
  let i = lines.slice(0, line - 1).join("\n").length + (line > 1 ? 1 : 0);
  for (; i < src.length; i++) {
    const ch = src[i];
    if (ch === "/" && src[i + 1] === "/") { i = src.indexOf("\n", i); if (i < 0) break; continue; }
    if (ch === "/" && src[i + 1] === "*") { i = src.indexOf("*/", i + 2) + 1; continue; }
    if (ch === '"') { for (i++; i < src.length && src[i] !== '"'; i++) if (src[i] === "\\") i++; continue; }
    if (ch === "r" && /^r#*"/.test(src.slice(i, i + 4)) && !/\w/.test(src[i - 1])) {
      const hashes = src.slice(i + 1).match(/^#*/)[0];
      i = src.indexOf(`"${hashes}`, i + hashes.length + 2) + hashes.length;
      continue;
    }
    if (ch === "'" && /^'(\\.|[^\\'])'/.test(src.slice(i, i + 4))) { i = src.indexOf("'", i + 2); continue; }
    if (ch === "{") depth++;
    if (ch === "}" && --depth === 0) break;
    if (ch === ";" && depth === 0) break;
  }
  const end = src.slice(0, i).split("\n").length;
  const body = lines.slice(start, end);
  return body.length > 160 ? [...body.slice(0, 160), "    // …"].join("\n") : body.join("\n");
}

// --- which file names which --------------------------------------------------

// A file names another when its text holds the other's path (as far as it is
// unique in the repo, or a sibling's bare name), or, in Rust, a module path
// into the other: `state::` within a crate, or through `harness_core` into one
// it depends on (naming only the crate counts as naming its root file).
function references(files, crates, text) {
  const paths = files.map((f) => f.path);
  const suffixes = new Map();
  for (const p of paths) {
    const segs = p.split("/");
    for (let i = segs.length - 1; i >= 0; i--) {
      const s = segs.slice(i).join("/");
      suffixes.set(s, suffixes.has(s) ? null : p);
    }
  }
  const named = new Map();
  for (const p of paths) {
    const segs = p.split("/");
    for (let i = segs.length - 1; i >= 0; i--) {
      const s = segs.slice(i).join("/");
      if (suffixes.get(s) === p) { if (s.includes(".")) named.set(s, p); break; }
    }
  }
  const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const anyName = new RegExp(`(?<![\\w.-])(${[...named.keys()].sort((a, b) => b.length - a.length).map(escape).join("|")})(?![\\w-])`, "g");
  const crateOf = (p) => crates.find((c) => p.startsWith(`${c.dir}/`));
  const byName = Object.fromEntries(crates.map((c) => [c.name, c]));
  const edges = new Set();
  const add = (a, b) => { if (a !== b) edges.add(`${a}\t${b}`); };
  for (const p of paths) {
    const t = text[p];
    if (t === undefined) continue;
    for (const m of t.matchAll(anyName)) add(p, named.get(m[1]));
    const dir = dirname(p);
    for (const m of t.matchAll(/(?<![\w./-])(?:\.\/)?([\w.-]+\.\w+)(?![\w-])/g)) {
      const sibling = dir === "." ? m[1] : `${dir}/${m[1]}`;
      if (sibling !== p && text[sibling] !== undefined) add(p, sibling);
    }
    if (!p.endsWith(".rs")) continue;
    const own = crateOf(p);
    if (own === undefined) continue;
    const mods = new Set([...t.matchAll(/\b([a-z_][a-z0-9_]*)::/g)].map((m) => m[1]));
    for (const m of mods) if (m in own.modules) add(p, own.modules[m]);
    for (const dep of own.deps.map((d) => byName[d]).filter((d) => d !== undefined)) {
      if (!new RegExp(`\\b${ident(dep)}\\b`).test(t)) continue;
      const into = [...t.matchAll(new RegExp(`\\b${ident(dep)}::(?:\\{[^;]*?\\b)?([a-z_][a-z0-9_]*)::`, "g"))].map((m) => m[1]);
      const hits = [...mods].filter((m) => m in dep.modules && !(m in own.modules)).concat(into.filter((m) => m in dep.modules));
      if (hits.length === 0) add(p, dep.src);
      for (const m of hits) add(p, dep.modules[m]);
    }
  }
  return [...edges].map((e) => e.split("\t"));
}

// --- what changed since a commit ------------------------------------------------

async function changes(since, commit) {
  if (!/^[0-9a-f]{7,40}$/.test(since)) throw new Error(`not a commit: ${since}`);
  const known = await git("cat-file", "-t", since).then((t) => t.trim() === "commit", () => false);
  if (!known) return { since, unknown: true, files: {}, commits: [] };
  const [diff, log] = await Promise.all([
    git("diff", "-U0", "--no-color", "--no-ext-diff", since, commit),
    git("log", "--format=%H%x09%ct%x09%s", `${since}..${commit}`),
  ]);
  const files = {};
  let file = null;
  for (const l of diff.split("\n")) {
    if (l.startsWith("+++ ")) { file = l === "+++ /dev/null" ? null : l.slice(6); if (file !== null) files[file] = []; continue; }
    const h = l.match(/^@@ -\S+ \+(\d+)(?:,(\d+))? @@/);
    if (h !== null && file !== null) {
      const start = Number(h[1]);
      const n = h[2] === undefined ? 1 : Number(h[2]);
      files[file].push([Math.max(start, 1), start + Math.max(n, 1) - 1]);
    }
  }
  const commits = log.trim().split("\n").filter(Boolean).map((l) => {
    const [sha, t, subject] = l.split("\t");
    return { sha, time: Number(t) * 1000, subject };
  });
  return { since, files, commits };
}

// --- what is in flight ----------------------------------------------------------

async function prs() {
  const list = async (state, fields, limit) =>
    JSON.parse((await run("gh", ["pr", "list", "-R", github, "--state", state, "--limit", String(limit), "--json", fields], { timeout: 20_000 })).stdout);
  const [open, merged] = await Promise.all([
    list("open", "number,title,url,isDraft,headRefName,createdAt,updatedAt,files", 50),
    list("merged", "number,title,url,mergedAt,headRefName,files", 25),
  ]);
  const slim = (p) => ({ ...p, files: p.files.map((f) => f.path) });
  return { open: open.map(slim), merged: merged.map(slim) };
}

function view(port) {
  return new Promise((resolve, reject) => {
    const req = get({ host: "localhost", port, path: "/view", rejectUnauthorized: false, timeout: 3000 }, (res) => {
      const chunks = [];
      res.on("data", (c) => chunks.push(c));
      res.on("end", () => {
        if (res.statusCode !== 200) return reject(new Error(`/view on ${port} answered ${res.statusCode}`));
        try { resolve(JSON.parse(Buffer.concat(chunks).toString())); } catch (e) { reject(e); }
      });
    });
    req.on("timeout", () => req.destroy(new Error(`/view on ${port} timed out`)));
    // Connecting to localhost can fail on both ::1 and 127.0.0.1: an AggregateError with no message of its own.
    req.on("error", (e) => reject(e.message === "" ? new Error(`/view on ${port}: ${e.errors?.[0]?.message ?? e.code ?? "failed"}`) : e));
  });
}

async function brains(main) {
  const found = [];
  for (const pid of readdirSync("/proc").filter((d) => /^\d+$/.test(d))) {
    let argv;
    try { argv = readFileSync(`/proc/${pid}/cmdline`, "utf8").split("\0"); } catch { continue; }
    if (basename(argv[0]) !== "harness-brain") continue;
    const flag = (f) => argv[argv.indexOf(f) + 1];
    if (!argv.includes("--port")) continue;
    found.push({ pid: Number(pid), port: Number(flag("--port")), name: argv.includes("--db") ? basename(dirname(flag("--db"))) : `port ${flag("--port")}` });
  }
  return Promise.all(found.sort((a, b) => a.port - b.port).map(async (b) => {
    try {
      const v = await view(b.port);
      const h = v.rows.harness.Harness;
      // Harness.source is the build's commit, with "+dirty" when it had uncommitted changes.
      const source = h.source ?? "";
      const commit = source.replace(/\+dirty$/, "") || null;
      const behind = commit === null ? null : await git("rev-list", "--count", `${commit}..${main}`).then((n) => Number(n), () => null);
      const tasks = Object.entries(v.rows).filter(([k]) => k.startsWith("task:")).map(([, r]) => r.Task);
      const waiting = new Set((v.rows.queue?.Queue ?? []).map((q) => q.task));
      return {
        ...b, commit, dirty: source.endsWith("+dirty"), behind,
        waiting: tasks.filter((t) => waiting.has(t.id)).map((t) => ({ id: t.id, name: t.name, environment: t.environment })),
        working: tasks.filter((t) => !["done", "max"].includes(t.waiting?.kind)).map((t) => ({ id: t.id, name: t.name, environment: t.environment, kind: t.waiting?.kind ?? null })),
        tasks: tasks.length,
      };
    } catch (e) {
      return { ...b, error: e.message };
    }
  }));
}

async function clones() {
  const dirs = readdirSync(workspace).map((d) => join(workspace, d)).filter((d) => {
    try { return statSync(join(d, ".git")).isDirectory(); } catch { return false; }
  });
  const out = await Promise.all(dirs.map(async (dir) => {
    const g = (...a) => run("git", a, { cwd: dir }).then((r) => r.stdout);
    const url = (await g("remote", "get-url", "origin").catch(() => "")).trim();
    if (!url.includes(github)) return null;
    const status = await g("status", "--porcelain=v2", "--branch");
    const branch = status.match(/^# branch\.head (.+)$/m)?.[1];
    const ab = status.match(/^# branch\.ab \+(\d+) -(\d+)$/m);
    const dirty = status.split("\n").filter((l) => /^[12u?] /.test(l)).length;
    const ahead = ab === null ? null : Number(ab[1]);
    const last = Number((await g("log", "-1", "--format=%ct")).trim()) * 1000;
    if (branch === "main" && dirty === 0 && (ahead ?? 0) === 0) return null;
    return { dir: basename(dir), branch, ahead, behind: ab === null ? null : Number(ab[2]), dirty, last };
  }));
  return out.filter((c) => c !== null);
}

// One gather serves every request for 20 s, including those that arrive while it runs.
let live = null;
function liveState() {
  if (live === null || Date.now() - live.at >= 20_000) {
    const mine = { at: Date.now(), value: gather() };
    mine.value.catch(() => { if (live === mine) live = null; });
    live = mine;
  }
  return live.value;
}

async function gather() {
  const main = await head();
  const errors = [];
  const settle = (source, p, empty) => p.catch((e) => { errors.push({ source, message: e.message }); return empty; });
  const [pr, br, cl] = await Promise.all([
    settle("GitHub", prs(), { open: [], merged: [] }),
    settle("brains", brains(main), []),
    settle("clones", clones(), []),
  ]);
  return { at: Date.now(), main, prs: pr, brains: br, clones: cl, errors };
}

// --- serving -----------------------------------------------------------------

const types = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css" };
const page = new Set(readdirSync(here).filter((f) => extname(f) in types));

async function respond(req, res) {
  const url = new URL(req.url, "https://localhost");
  const json = (v) => {
    res.writeHead(200, { "content-type": "application/json", "cache-control": "no-store" });
    res.end(JSON.stringify(v));
  };
  if (url.pathname === "/repo.json") return json(await snapshot(await head()));
  if (url.pathname === "/live.json") return json(await liveState());
  if (url.pathname === "/changes.json") return json(await changes(url.searchParams.get("since") ?? "", await head()));
  const file = url.pathname === "/" ? "index.html" : posix.basename(url.pathname);
  if (!page.has(file) || url.pathname !== (file === "index.html" ? "/" : `/${file}`)) {
    res.writeHead(404, { "content-type": "text/plain" });
    return res.end(`no ${url.pathname}`);
  }
  res.writeHead(200, { "content-type": `${types[extname(file)]}; charset=utf-8`, "cache-control": "no-cache" });
  res.end(readFileSync(join(here, file)));
}

createServer({ cert: readFileSync(join(tls, "wildcard-local.crt")), key: readFileSync(join(tls, "wildcard-local.key")) }, (req, res) => {
  respond(req, res).catch((e) => {
    console.error(`${req.url}: ${e.stack}`);
    res.writeHead(500, { "content-type": "text/plain" });
    res.end(e.message);
  });
}).listen(port, () => console.log(`dashboard: https://localhost:${port}/ rendering ${rev} of ${root}`));
