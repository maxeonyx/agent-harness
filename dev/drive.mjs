#!/usr/bin/env node
// A throwaway test brain, its chats, and its page driven the way Max drives it.
//
//   node dev/drive.mjs up PORT [MODEL]
//   node dev/drive.mjs chat PORT ROLE DIR|- WORDS
//   node dev/drive.mjs settle PORT CHAT [--timeout S]
//   node dev/drive.mjs log PORT CHAT [--full]
//   node dev/drive.mjs say PORT CHAT WORDS
//   node dev/drive.mjs page PORT [--size WxH[@DPR] | --size phone] [--timeout S] [--log FILE] STEP...
//   node dev/drive.mjs down PORT [--wipe]
//
// `up` builds the harness, ends whatever this clone left on PORT (the
// brain and its limbs), and starts a brain there on its own database in
// .drive.ignore/PORT/ and this clone's roles/, every role on MODEL: the
// scripted model, which costs nothing, unless told (`sonnet/low`, or
// `sonnet` to keep each role's effort: the brain's `--model`). Its database outlives `up`, so the next `up`
// relaunches a rebuilt brain on the same chats; `down --wipe` forgets them.
// Its face serves the page from crates/face-web/src/page as it is on disk
// (`--page`), so an edit to the page shows on reload, with no rebuild.
//
// `chat` starts a chat as Max does at a tab, with no browser (the `act`
// example): in ROLE, in a workspace over DIR or in no place, with WORDS as
// its mandate, and prints its id. `say` sends a chat WORDS as Max's next
// words, the same way. On the scripted model the words are the
// script (crates/provider/src/scripted.rs). `settle` waits until the chat
// waits on Max or is done (a limb that cannot start fails it at once), and
// `log` prints its transcript: what each said, each tool call with its
// result cut short (all of it with --full), its state and its children's.
//
// `page` opens the face in a fresh headless Chromium at 1920x1080 (or a
// phone: 390x844, touch, twice the pixels) and runs the steps in order:
//
//   chat ROLE DIR WORDS  start a chat by the clicks Max makes: +, its
//                        role, its workspace directory (- for none), the
//                        words, send twice; prints its id
//   open ID              open a chat by its link
//   click CSS / hover CSS / type CSS TEXT / key KEY (Enter, Escape, Tab, a letter)
//                        (on a phone a click is a tap, and there is no hover)
//   wait CSS             until something it matches is visible
//   gone CSS             until nothing it matches is visible
//   box CSS              prints where it is and its size, in CSS pixels
//   text WORDS           until the page shows the words
//   reply                until the open chat settles, as `settle`
//   shot FILE            a PNG, for `read`
//   eval JS              prints what the expression comes to
//   sleep MS
//
// CSS means the first visible match; `CSS @N` the Nth (from 1), and
// `CSS @last` the last: `click ".moves .mv @last"`.
//
// It fails, with a screenshot in .drive.ignore/PORT/failed.png, the
// page's errors and its console, at the first step that cannot be done,
// the first wait that times out (60 s unless --timeout), and the first
// exception the page throws. Every run keeps its whole log, each console
// line and exception timed against each step, in .drive.ignore/PORT/page.log
// (or --log FILE).
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import https from "node:https";
import net from "node:net";
import path from "node:path";

const repo = path.resolve(import.meta.dirname, "..");
const placeOf = (port) => path.join(repo, ".drive.ignore", String(port));
// Everything a brain on PORT starts carries this mark (see demos/end.sh),
// named for this clone so another clone's brain on the port is not ours.
const markOf = (port) =>
  `HARNESS_OWNER_drive_${port}_${createHash("sha256").update(repo).digest("hex").slice(0, 8)}`;

class Failed extends Error {}
const fail = (why) => { throw new Failed(why); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function end(port) {
  execFileSync(path.join(repo, "demos/end.sh"), [markOf(port)], { stdio: "inherit" });
}

async function up(port, args) {
  if (args.length > 1) fail("usage: up PORT [MODEL]");
  const model = args[0] ?? "scripted";
  execFileSync("cargo", ["build", "-q", "--release", "--bins", "--example", "act"], { cwd: repo, stdio: "inherit" });
  end(port);
  const place = placeOf(port);
  fs.mkdirSync(place, { recursive: true });
  const log = path.join(place, "brain.log");
  const out = fs.openSync(log, "a");
  const brain = spawn(path.join(repo, "target/release/harness-brain"),
    ["--roles", path.join(repo, "roles"), "--db", path.join(place, "brain.sqlite"), "--port", String(port), "--model", model,
      "--page", path.join(repo, "crates/face-web/src/page")],
    { detached: true, stdio: ["ignore", out, out], env: { ...process.env, [markOf(port)]: "1" } });
  brain.unref();
  let exited = null;
  brain.on("exit", (code) => { exited = code; });
  for (let i = 0; i < 300 && !(await listening(port)); i++) {
    if (exited !== null) fail(`the brain exited (${exited}):\n${tail(log, 20)}`);
    await sleep(100);
  }
  if (!(await listening(port))) fail(`the brain is not listening on ${port} after 30 s:\n${tail(log, 20)}`);
  console.log(`brain on https://mx-hv-mj.local:${port}/ (pid ${brain.pid}; every role on ${model})\nlog ${log}\nend it with: node dev/drive.mjs down ${port}`);
}

const tail = (file, n) => fs.readFileSync(file, "utf8").trimEnd().split("\n").slice(-n).join("\n");
const listening = (port) => new Promise((r) => {
  const s = net.connect(port, "127.0.0.1", () => { s.destroy(); r(true); });
  s.on("error", () => r(false));
});

function down(port, wipe) {
  end(port);
  if (wipe) fs.rmSync(placeOf(port), { recursive: true, force: true });
}

// ─── A chat, from the brain ───────────────────────────────────────────────

// What Max does at a tab, with no browser: one act of `act`, a tab
// replica. Its last line says how the brain took it.
function act(port, ...args) {
  const r = spawnSync(path.join(repo, "target/release/examples/act"), [String(port), ...args], { encoding: "utf8" });
  const last = (r.stdout ?? "").trim().split("\n").pop();
  const why = last.startsWith("refused:") ? last : (r.stderr ?? "").trim().split("\n").pop() || last;
  if (r.status !== 0) fail(why || `act ${args[0]}: ${r.error?.message ?? `exit ${r.status}`}`);
  return last;
}

// Max starts a chat.
function chat(port, [role, dir, words, ...more]) {
  if (words === undefined || more.length > 0) fail("usage: chat PORT ROLE DIR|- WORDS");
  const id = act(port, "chat", role, dir === "-" ? "-" : path.resolve(dir), words).replace(/^chat /, "");
  console.log(`chat ${id}`);
  return id;
}

// Max's next words to a chat; returns once the chat has moved on from
// where it was, so a `settle` after it waits for its answer.
async function say(port, [id, words, ...more]) {
  if (words === undefined || more.length > 0) fail("usage: say PORT CHAT WORDS");
  const was = (await task(port, id)).task.waiting;
  act(port, "say", id, words);
  const deadline = Date.now() + timeout;
  for (;;) {
    const now = (await task(port, id)).task.waiting;
    if (now?.kind !== was?.kind || now?.since !== was?.since) return;
    if (Date.now() > deadline) fail(`${id} took the words but has not moved on in ${timeout / 1000} s`);
    await sleep(100);
  }
}

// What the face serves: the view (every task's state) and a context's chain.
function get(port, route) {
  return new Promise((resolve, reject) => {
    https.get({ host: "127.0.0.1", port, path: route, servername: "mx-hv-mj.local", rejectUnauthorized: false }, (res) => {
      let body = "";
      res.setEncoding("utf8");
      res.on("data", (d) => { body += d; });
      res.on("end", () => res.statusCode === 200 ? resolve(JSON.parse(body)) : reject(new Failed(`${route}: ${res.statusCode} ${body}`)));
    }).on("error", (e) => reject(new Failed(`nothing answers on ${port} (${e.message}): node dev/drive.mjs up ${port}`)));
  });
}

async function task(port, id) {
  const { rows } = await get(port, "/view");
  return { task: rows[`task:${id}`]?.Task ?? fail(`no chat ${id} on ${port}`), rows };
}

const clock = (ms) => new Date(ms).toLocaleTimeString("en-NZ", { timeZone: "Pacific/Auckland", hour12: false });

// What a task waits on, in a line.
function state(t) {
  const w = t.waiting ?? {};
  const since = w.since === undefined ? "" : ` since ${clock(w.since)}`;
  const why = {
    max: () => "Max's turn",
    done: () => t.outcome?.kind === "failed" ? `failed: ${t.outcome.reason}` : (t.outcome?.kind ?? "done"),
    model: () => w.streaming ? "the model is writing" : "the model is thinking",
    calls: () => `running ${w.calls.map((c) => c.tool ?? "a call").join(", ")}`,
    starting: () => `starting ${w.limbs.join(", ")}${w.failed.map((f) => `; ${f.limb} failed: ${f.why}`).join("")}`,
    limited: () => `limited until ${clock(w.until)}`,
  }[w.kind]?.() ?? w.kind ?? "unknown";
  return `${why}${since}${t.trouble ? ` (trouble: ${JSON.stringify(t.trouble)})` : ""}`;
}

const settled = (t) => t.waiting?.kind === "max" || t.waiting?.kind === "done";

// Until the chat waits on Max or is done; a limb that cannot start fails it.
async function settle(port, id) {
  const deadline = Date.now() + timeout;
  for (;;) {
    const { task: t } = await task(port, id);
    if (settled(t)) return t;
    if (t.waiting?.failed?.length > 0) fail(`${id}: ${state(t)}`);
    if (Date.now() > deadline) fail(`waited ${timeout / 1000} s for ${id} to settle: ${state(t)}`);
    await sleep(200);
  }
}

// A chat's transcript: who said what, each tool call with its result, cut
// short unless `full`; then its state, and its children's.
async function log(port, id, full) {
  const { task: t, rows } = await task(port, id);
  const cut = (text, lines, chars) => {
    if (full) return text;
    const all = text.split("\n");
    let kept = all.slice(0, lines).join("\n");
    if (kept.length > chars) kept = kept.slice(0, chars);
    return kept.length < text.length ? `${kept} … (${all.length} lines, ${text.length} chars)` : kept;
  };
  const indent = (text) => text.replaceAll("\n", "\n    ");
  const place = Object.values(t.places ?? {}).join(", ");
  console.log(`${t.id} "${t.name}": ${t.environment} on ${t.tuning?.model ?? "?"}${place ? ` in ${place}` : ""}`);
  const chain = t.head ? await get(port, `/context/${t.head}`) : [];
  for (const entry of chain.slice(1)) {
    const blocks = JSON.parse(entry.content);
    blocks.forEach((b, i) => {
      const by = entry.by?.[i]?.by ?? entry.by?.at(-1)?.by;
      const who = entry.role === "assistant" ? "agent" : by === "max" ? "max" : "harness";
      if (b.type === "text") console.log(`${who}: ${indent(cut(b.text, 6, 400))}`);
      else if (b.type === "thinking") console.log(`${who}: (thinking, ${b.thinking.length} chars)`);
      else if (b.type === "tool_use" || b.type === "server_tool_use") console.log(`  → ${b.name} ${cut(JSON.stringify(b.input), 1, 200)}`);
      else if (b.type === "tool_result") {
        const text = typeof b.content === "string" ? b.content : (b.content ?? []).map((c) => c.type === "text" ? c.text : `[${c.type}]`).join("\n");
        console.log(`  ← ${b.is_error ? "error " : ""}${indent(cut(text, 3, 300))}`);
      } else console.log(`${who}: [${b.type}]`);
    });
  }
  console.log(`state: ${state(t)}`);
  for (const [key, row] of Object.entries(rows)) {
    const c = row.Task;
    if (key.startsWith("task:") && c.owner === t.id) console.log(`child ${c.id} "${c.name}": ${state(c)}`);
  }
}

// ─── The page ──────────────────────────────────────────────────────────────

function chromium() {
  if (process.env.CHROME !== undefined) return process.env.CHROME;
  const cache = path.join(process.env.HOME, ".cache/ms-playwright");
  const builds = (fs.existsSync(cache) ? fs.readdirSync(cache) : []).filter((d) => /^chromium-\d+$/.test(d))
    .sort((a, b) => Number(b.split("-")[1]) - Number(a.split("-")[1]));
  if (builds.length === 0) fail(`no Chromium in ${cache}; set CHROME`);
  return path.join(cache, builds[0], "chrome-linux64/chrome");
}

// One Chromium, its one tab, and what the page said while we drove it.
class Browser {
  static async open(place, size, log) {
    const profile = fs.mkdtempSync(path.join(place, "chrome-"));
    const proc = spawn(chromium(), [
      "--headless=new", "--remote-debugging-port=0", `--user-data-dir=${profile}`,
      "--ignore-certificate-errors", "--no-first-run", "--no-default-browser-check", "--hide-scrollbars",
      `--window-size=${size.width},${size.height}`, "about:blank",
    ], { detached: true, stdio: "ignore" });
    const b = new Browser(proc, profile, size, log);
    const portFile = path.join(profile, "DevToolsActivePort");
    for (let i = 0; i < 100 && !fs.existsSync(portFile); i++) await sleep(100);
    if (!fs.existsSync(portFile)) { b.close(); fail("Chromium did not start"); }
    const devtools = fs.readFileSync(portFile, "utf8").split("\n")[0];
    const tab = (await (await fetch(`http://127.0.0.1:${devtools}/json/list`)).json()).find((t) => t.type === "page");
    b.ws = new WebSocket(tab.webSocketDebuggerUrl);
    await new Promise((r, j) => { b.ws.onopen = r; b.ws.onerror = () => j(new Failed("no DevTools connection")); });
    b.ws.onmessage = (e) => b.receive(JSON.parse(e.data));
    await b.send("Runtime.enable");
    await b.send("Log.enable");
    await b.send("Page.enable");
    await b.send("Emulation.setDeviceMetricsOverride", { width: size.width, height: size.height, deviceScaleFactor: size.dpr, mobile: size.phone });
    if (size.phone) await b.send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
    return b;
  }

  constructor(proc, profile, size, log) {
    Object.assign(this, { proc, profile, size, log, next: 0, waiting: new Map(), errors: [], console: [], started: Date.now() });
    fs.writeFileSync(log, "");
  }

  // A line of the run's log: what the page said, and each step, timed.
  note(line) {
    fs.appendFileSync(this.log, `${((Date.now() - this.started) / 1000).toFixed(3).padStart(8)}  ${line}\n`);
  }

  send(method, params = {}) {
    const id = ++this.next;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => this.waiting.set(id, { method, resolve, reject }));
  }

  receive(m) {
    if (m.id !== undefined) {
      const w = this.waiting.get(m.id);
      this.waiting.delete(m.id);
      if (m.error !== undefined) w.reject(new Failed(`${w.method}: ${m.error.message}`));
      else w.resolve(m.result);
    } else if (m.method === "Runtime.exceptionThrown") {
      this.errors.push(described(m.params.exceptionDetails));
      this.note(`threw: ${this.errors.at(-1)}`);
    } else if (m.method === "Runtime.consoleAPICalled") {
      this.said(`${m.params.type}: ${m.params.args.map((a) => a.value ?? a.description ?? a.type).join(" ")}`);
    } else if (m.method === "Log.entryAdded") {
      const e = m.params.entry;
      this.said(`${e.level} (${e.source}): ${e.text}${e.url ? " " + e.url : ""}`);
    } else if (m.method === "Page.javascriptDialogOpening") {
      this.said(`dialog ${m.params.type}: ${m.params.message}`);
      this.send("Page.handleJavaScriptDialog", { accept: true });
    }
  }

  said(line) {
    this.console.push(line);
    this.note(line);
  }

  // The value of a JS expression in the page; its exception is a failure.
  async eval(expression) {
    const r = await this.send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true, userGesture: true });
    if (r.exceptionDetails !== undefined) fail(described(r.exceptionDetails));
    this.check();
    return "value" in r.result ? r.result.value : r.result.description;
  }

  check() {
    if (this.errors.length > 0) fail("the page threw");
  }

  async until(what, test) {
    const deadline = Date.now() + timeout;
    while (!(await this.eval(test))) {
      if (Date.now() > deadline) fail(`waited ${timeout / 1000} s for ${what}`);
      await sleep(100);
    }
  }

  // Where to put the pointer on what CSS matches: its middle, in view, not
  // under anything else, enabled, and still, as Max waits for a card to land.
  async point(css) {
    const deadline = Date.now() + 2000;
    let last = null;
    for (;;) {
      const p = await this.where(css);
      if (typeof p !== "string" && last !== null && p.x === last.x && p.y === last.y) return p;
      if (Date.now() > deadline) fail(`${css}: ${typeof p === "string" ? p : "it keeps moving"}`);
      last = typeof p === "string" ? null : p;
      await sleep(50);
    }
  }

  async where(css) {
    return this.eval(`(() => {
      const e = ${pick(css)};
      if (e === undefined) return "nothing visible matches";
      if (e.disabled) return "it is disabled" + (e.title ? ": " + e.title : "");
      // Scrolled to only if it is out of view, as Max would: scrolling
      // what the page keeps unscrolled moves it back under the pointer.
      const out = (r) => r.left < 0 || r.top < 0 || r.right > innerWidth || r.bottom > innerHeight;
      if (out(e.getBoundingClientRect())) e.scrollIntoView({ block: "nearest", inline: "nearest" });
      const r = e.getBoundingClientRect(), x = r.x + r.width / 2, y = r.y + r.height / 2;
      const top = document.elementFromPoint(x, y);
      if (top !== e && !e.contains(top)) return "it is under " + (top === null ? "nothing" : top.tagName.toLowerCase() + (top.id ? "#" + top.id : "") + [...top.classList].map((c) => "." + c).join(""));
      return { x, y };
    })()`);
  }

  async click(css) {
    const { x, y } = await this.point(css);
    if (this.size.phone) {
      await this.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x, y, radiusX: 1, radiusY: 1 }] });
      await this.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
    } else {
      await this.send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
      await this.send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", clickCount: 1 });
      await this.send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", clickCount: 1 });
    }
    await sleep(50);
    this.check();
  }

  async hover(css) {
    if (this.size.phone) fail("a phone has no hover: tap it, or use another --size");
    const { x, y } = await this.point(css);
    await this.send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
    await sleep(50);
    this.check();
  }

  // A select is chosen from, as its list would; anything else is typed into.
  async type(css, text) {
    const chose = await this.eval(`(() => {
      const e = ${pick(css)};
      if (!(e instanceof HTMLSelectElement)) return null;
      const names = [...e.options].map((o) => o.value);
      if (!names.includes(${JSON.stringify(text)})) return "no option " + ${JSON.stringify(text)} + " among " + names.join(", ");
      e.value = ${JSON.stringify(text)};
      e.dispatchEvent(new Event("change", { bubbles: true }));
      return "chosen";
    })()`);
    if (chose === "chosen") return;
    if (chose !== null) fail(`${css}: ${chose}`);
    await this.click(css);
    await this.send("Input.insertText", { text });
    this.check();
  }

  async key(name) {
    const codes = { Enter: 13, Escape: 27, Tab: 9, Backspace: 8, ArrowUp: 38, ArrowDown: 40, ArrowLeft: 37, ArrowRight: 39 };
    const code = codes[name] ?? (name.length === 1 ? name.toUpperCase().charCodeAt(0) : fail(`no key ${name}`));
    const text = name === "Enter" ? "\r" : name.length === 1 ? name : undefined;
    await this.send("Input.dispatchKeyEvent", { type: text === undefined ? "rawKeyDown" : "keyDown", key: name, windowsVirtualKeyCode: code, text });
    await this.send("Input.dispatchKeyEvent", { type: "keyUp", key: name, windowsVirtualKeyCode: code });
    await sleep(50);
    this.check();
  }

  async shot(file) {
    const { data } = await this.send("Page.captureScreenshot", { format: "png" });
    fs.writeFileSync(file, Buffer.from(data, "base64"));
  }

  close() {
    try { process.kill(-this.proc.pid, "SIGKILL"); } catch { /* already gone */ }
    fs.rmSync(this.profile, { recursive: true, force: true });
  }
}

function described(d) {
  const where = d.url ? ` (${d.url.split("/").pop()}:${d.lineNumber + 1}:${d.columnNumber + 1})` : "";
  return (d.exception?.description ?? d.exception?.value ?? d.text) + where;
}

let timeout = 60_000;
// The element `CSS`, `CSS @N` or `CSS @last` names among the visible
// matches, as a JS expression: undefined if there is none.
function pick(target) {
  const [, css, which] = target.match(/^(.*?)(?:\s+@(\d+|last))?$/s);
  const all = `[...document.querySelectorAll(${JSON.stringify(css)})].filter((e) => e.getClientRects().length > 0)`;
  return which === undefined ? `${all}[0]` : which === "last" ? `${all}.at(-1)` : `${all}[${Number(which) - 1}]`;
}
const visible = (css) => `(${pick(css)} !== undefined)`;

// Each step: how many words it takes, and what it does with them.
const steps = {
  async chat(b, role, dir, words) {
    await b.click("#new");
    await b.until("the new chat's card", visible(".compose textarea"));
    if (await b.eval(`document.querySelector("#n-role") !== null`)) await b.type("#n-role", role);
    else if (role !== "-") fail(`there is no choice of role; use - for ${role}`);
    const places = await b.eval(`[...document.querySelectorAll(".compose .place")].map((p) => p.dataset.place)`);
    if (places.some((p) => p !== "workspace")) fail(`${role} names places this cannot fill: ${places.join(", ")}`);
    if (places.length > 0 && dir === "-") fail(`${role} needs a workspace directory`);
    if (places.length === 0 && dir !== "-") fail(`${role} has no workspace, so no directory: use -`);
    if (places.length > 0) await b.type(".compose .place", path.resolve(dir));
    await b.type(".compose textarea", words);
    await b.click(".compose .go.send");
    await b.click(".compose .go.send");
    await b.until("the chat to start", `location.hash.length > 1 && location.hash !== "#new"`);
    console.log(`chat ${(await b.eval("location.hash")).slice(1)}`);
  },
  open: async (b, id) => { await b.eval(`location.hash = ${JSON.stringify(id)}`); await sleep(300); b.check(); },
  click: (b, css) => b.click(css),
  hover: (b, css) => b.hover(css),
  type: (b, css, text) => b.type(css, text),
  key: (b, name) => b.key(name),
  wait: (b, css) => b.until(css, visible(css)),
  gone: (b, css) => b.until(`${css} to go`, `!${visible(css)}`),
  box: async (b, css) => {
    const r = await b.eval(`(() => { const e = ${pick(css)}; if (e === undefined) return null; const r = e.getBoundingClientRect(); return [r.x, r.y, r.width, r.height].map((n) => Math.round(n * 10) / 10); })()`);
    if (r === null) fail(`${css}: nothing visible matches`);
    console.log(`box ${css}: x ${r[0]} y ${r[1]} width ${r[2]} height ${r[3]}`);
  },
  text: (b, words) => b.until(JSON.stringify(words), `document.body.innerText.includes(${JSON.stringify(words)})`),
  reply: async (b) => { await settle(b.port, (await b.eval("location.hash")).slice(1)); b.check(); },
  shot: async (b, file) => { await b.shot(file); console.log(`shot ${path.resolve(file)}`); },
  eval: async (b, js) => console.log(JSON.stringify(await b.eval(js)) ?? "undefined"),
  sleep: (b, ms) => sleep(Number(ms)).then(() => b.check()),
};

async function page(port, args) {
  let size = { width: 1920, height: 1080, dpr: 1, phone: false };
  let log = path.join(placeOf(port), "page.log");
  while (args[0]?.startsWith("--")) {
    const [flag, value] = args.splice(0, 2);
    if (value === undefined) fail(`${flag} needs a value`);
    if (flag === "--log") log = path.resolve(value);
    else if (flag === "--size" && value === "phone") size = { width: 390, height: 844, dpr: 2, phone: true };
    else if (flag === "--size") {
      const [, w, h, dpr = "1"] = value.match(/^(\d+)x(\d+)(?:@([\d.]+))?$/) ?? fail(`--size: not WxH[@DPR] or phone: ${value}`);
      size = { width: Number(w), height: Number(h), dpr: Number(dpr), phone: false };
    } else fail(`unknown option ${flag}`);
  }
  const plan = [];
  while (args.length > 0) {
    const name = args.shift();
    const step = steps[name] ?? fail(`unknown step ${name}; the steps are ${Object.keys(steps).join(", ")}`);
    const words = args.splice(0, step.length - 1);
    if (words.length < step.length - 1) fail(`${name} needs ${step.length - 1} argument(s)`);
    plan.push([name, step, words]);
  }
  const place = placeOf(port);
  fs.mkdirSync(place, { recursive: true });
  if (!(await listening(port))) fail(`nothing listens on ${port}: node dev/drive.mjs up ${port}`);
  const b = await Browser.open(place, size, log);
  b.port = port;
  let at = "loading the page";
  try {
    await b.send("Page.navigate", { url: `https://mx-hv-mj.local:${port}/` });
    await b.until("the page to link to the brain", `document.querySelector("#link")?.className === ""`);
    for (const [name, step, words] of plan) {
      at = [name, ...words.map((w) => JSON.stringify(w))].join(" ");
      b.note(`» ${at}`);
      await step(b, ...words);
    }
  } catch (e) {
    if (!(e instanceof Failed)) throw e;
    const shot = path.join(place, "failed.png");
    await b.shot(shot).catch(() => {});
    b.note(`failed: ${e.message}`);
    console.error(`failed at ${at}: ${e.message}`);
    if (b.errors.length > 0) console.error(`page errors:\n  ${b.errors.join("\n  ")}`);
    if (b.console.length > 0) console.error(`console, its last 30:\n  ${b.console.slice(-30).join("\n  ")}`);
    console.error(`screenshot ${shot}\nthe run's log ${log}`);
    process.exitCode = 1;
  } finally {
    b.close();
  }
  if (process.exitCode !== 1 && b.console.length > 0) {
    console.log(`console, its last 10:\n  ${b.console.slice(-10).join("\n  ")}\nthe run's log ${log}`);
  }
}

const [command, port, ...rest] = process.argv.slice(2);
const at = rest.indexOf("--timeout");
if (at >= 0) timeout = Number(rest.splice(at, 2)[1]) * 1000;
try {
  if (!/^\d+$/.test(port ?? "")) fail("usage: node dev/drive.mjs up|chat|say|settle|log|page|down PORT ... (see the top of dev/drive.mjs)");
  const n = Number(port);
  if (command === "up") await up(n, rest);
  else if (command === "chat") chat(n, rest);
  else if (command === "say") await say(n, rest);
  else if (command === "settle") console.log(state(await settle(n, rest[0] ?? fail("usage: settle PORT CHAT"))));
  else if (command === "log") await log(n, rest[0] ?? fail("usage: log PORT CHAT [--full]"), rest[1] === "--full");
  else if (command === "down") down(n, rest[0] === "--wipe");
  else if (command === "page") await page(n, rest);
  else fail(`unknown command ${command}; up, chat, say, settle, log, page or down`);
} catch (e) {
  if (!(e instanceof Failed)) throw e;
  console.error(e.message);
  process.exitCode = 1;
}
// Exit once what was printed has been written: a pipe takes it in
// asynchronously, and exiting at once would cut a long log short.
process.stdout.write("", () => process.exit());
