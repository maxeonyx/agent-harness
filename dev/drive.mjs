#!/usr/bin/env node
// Throwaway test brains by name, their chats, and their pages driven the
// way Max drives them. Every command takes `--brain NAME` (default `test`):
//
//   node dev/drive.mjs up [MODEL]
//   node dev/drive.mjs chat ROLE DIR|- WORDS
//   node dev/drive.mjs say CHAT WORDS
//   node dev/drive.mjs settle CHAT [--timeout S]
//   node dev/drive.mjs log CHAT [--full]
//   node dev/drive.mjs page [--size WxH[@DPR] | --size phone] [--timeout S] [--log FILE] STEP...
//   node dev/drive.mjs down [--wipe]
//
// A brain is everything in .drive.ignore/NAME/: its port, pid, database,
// log (brain.log), its limbs' state (state/), and the last page run's log
// (page.log). Everything it starts, and every Chromium `page` starts for
// it, carries one mark (demos/end.sh), named for the brain and this clone.
//
// `up` builds the harness, ends the brain if it still runs (SIGTERM, then
// SIGKILL after 10 s), and starts it again on its database and this clone's roles/, every role on MODEL: the
// scripted model, which costs nothing, unless told (`sonnet/low`, or
// `sonnet` to keep each role's effort: the brain's `--model`). It keeps
// the brain's port if that is free, or takes a free one in 4340-4499
// (DRIVE_PORTS=LOW-HIGH for others), and succeeds once this launch's log
// says it serves (`face: …`), or fails with the log's tail. The database
// outlives `up`, and so do its limbs, as any brain's do: the next `up`
// relaunches a rebuilt brain on the same chats, linked to the same limbs,
// so `kill -9 $(cat .drive.ignore/NAME/pid)` then `up` is a crash and a
// relaunch; `down` ends everything of the brain's, and `down --wipe` forgets
// it. The face serves the page from crates/face-web/src/page as it is on disk
// (`--page`), so an edit to the page shows on reload, with no rebuild.
//
// `chat` starts a chat as Max does at a tab, with no browser (the `act`
// example): in ROLE, in a workspace over DIR or in no place, with WORDS as
// its mandate, and prints its id. `say` sends a chat WORDS as Max's next
// words, the same way. On the scripted model the words are the
// script (crates/provider/src/scripted.rs). `settle` waits until the chat
// waits on Max or is done, and fails at once on trouble, a failure, or a
// limb that cannot start. `log` prints its transcript: what each said, each
// tool call with its result cut short (all of it with --full), its state
// and its children's.
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
// It fails, with a screenshot in .drive.ignore/NAME/failed.png, the
// page's errors and its console, at the first step that cannot be done,
// the first wait that times out (60 s unless --timeout), and the first
// exception the page throws. Every run keeps its whole log, each console
// line and exception timed against each step, in .drive.ignore/NAME/page.log
// (or --log FILE). Its Chromium ends with it, killed or not.
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import https from "node:https";
import net from "node:net";
import path from "node:path";

const repo = path.resolve(import.meta.dirname, "..");

class Failed extends Error {}
const fail = (why) => { throw new Failed(why); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// A test brain: its directory, its mark, and the port it last served on.
function brainOf(name) {
  if (!/^[\w-]+$/.test(name)) fail(`--brain: a name is letters, digits, - and _: ${name}`);
  const dir = path.join(repo, ".drive.ignore", name);
  const clone = createHash("sha256").update(repo).digest("hex").slice(0, 8);
  const file = (f) => path.join(dir, f);
  const read = (f) => fs.existsSync(file(f)) ? fs.readFileSync(file(f), "utf8").trim() : undefined;
  return {
    name, dir, file, read,
    mark: `HARNESS_OWNER_drive_${name}_${clone}`,
    recorded: () => read("port") === undefined ? undefined : Number(read("port")),
    get port() {
      return this.recorded() ?? fail(`no brain ${name}: node dev/drive.mjs up --brain ${name}`);
    },
  };
}

// End the brain's own process, if it still runs, and leave its limbs.
async function stop(brain) {
  const pid = Number(brain.read("pid"));
  const ours = () => {
    try { return fs.readFileSync(`/proc/${pid}/environ`, "utf8").split("\0").includes(`${brain.mark}=1`); } catch { return false; }
  };
  if (!pid || !ours()) return;
  process.kill(pid, "SIGTERM");
  for (let i = 0; i < 100 && ours(); i++) await sleep(100);
  if (ours()) process.kill(pid, "SIGKILL");
  while (ours()) await sleep(100);
}

// End every process carrying the brain's mark: the brain, its limbs, any
// Chromium a page run left.
function end(brain) {
  execFileSync(path.join(repo, "demos/end.sh"), [brain.mark], { stdio: "inherit" });
}

const tail = (text, n) => text.trimEnd().split("\n").slice(-n).join("\n");
const listening = (port) => new Promise((r) => {
  const s = net.connect(port, "127.0.0.1", () => { s.destroy(); r(true); });
  s.on("error", () => r(false));
});
// Whether a brain could bind `port` now, as it does: every address.
const free = (port) => new Promise((r) => {
  const s = net.createServer().once("error", () => r(false));
  s.listen({ port, host: "0.0.0.0", exclusive: true }, () => s.close(() => r(true)));
});

// The ports a brain may take: its own first, then the range from a place
// its name picks, so two brains coming up at once rarely try the same one.
function* candidates(brain) {
  const [low, high] = (process.env.DRIVE_PORTS ?? "4340-4499").split("-").map(Number);
  if (brain.recorded() !== undefined) yield brain.recorded();
  const start = parseInt(createHash("sha256").update(brain.name).digest("hex").slice(0, 8), 16) % (high - low + 1);
  for (let i = 0; i <= high - low; i++) yield low + ((start + i) % (high - low + 1));
}

async function up(brain, args) {
  if (args.length > 1) fail("usage: up [MODEL]");
  const model = args[0] ?? "scripted";
  execFileSync("cargo", ["build", "-q", "--release", "--bins", "--example", "act"], { cwd: repo, stdio: "inherit" });
  await stop(brain);
  fs.mkdirSync(brain.file("state"), { recursive: true });
  const log = brain.file("brain.log");
  const tried = new Set();
  for (const port of candidates(brain)) {
    if (tried.has(port) || !(await free(port))) continue;
    tried.add(port);
    const launched = await launch(brain, model, port, log);
    if (launched === "taken") continue;
    fs.writeFileSync(brain.file("port"), `${port}\n`);
    fs.writeFileSync(brain.file("pid"), `${launched}\n`);
    console.log(`brain ${brain.name} on https://mx-hv-mj.local:${port}/ (pid ${launched}; every role on ${model})\nlog ${log}\nend it with: node dev/drive.mjs down --brain ${brain.name}`);
    return;
  }
  fail(`no free port in ${process.env.DRIVE_PORTS ?? "4340-4499"}`);
}

// Start the brain on `port`: its pid once this launch's log says it
// serves, or "taken" if another process bound the port first.
async function launch(brain, model, port, log) {
  const from = fs.existsSync(log) ? fs.statSync(log).size : 0;
  const out = fs.openSync(log, "a");
  const child = spawn(path.join(repo, "target/release/harness-brain"),
    ["--roles", path.join(repo, "roles"), "--db", brain.file("brain.sqlite"), "--port", String(port), "--model", model,
      "--page", path.join(repo, "crates/face-web/src/page")],
    { detached: true, stdio: ["ignore", out, out], env: { ...process.env, [brain.mark]: "1", XDG_STATE_HOME: brain.file("state") } });
  child.unref();
  fs.closeSync(out);
  let exited = null;
  child.on("exit", (code, signal) => { exited = signal ?? code; });
  const said = () => {
    const fd = fs.openSync(log, "r");
    const size = fs.fstatSync(fd).size;
    const buf = Buffer.alloc(size - from);
    fs.readSync(fd, buf, 0, buf.length, from);
    fs.closeSync(fd);
    return buf.toString("utf8");
  };
  const deadline = Date.now() + 60_000;
  for (;;) {
    const text = said();
    if (/^face: /m.test(text)) return child.pid;
    if (exited !== null) {
      if (text.includes("Address already in use")) return "taken";
      fail(`the brain exited (${exited}) before it served:\n${tail(text, 20)}`);
    }
    if (Date.now() > deadline) fail(`the brain has not served in 60 s:\n${tail(text, 20)}`);
    await sleep(100);
  }
}

function down(brain, wipe) {
  end(brain);
  if (wipe) fs.rmSync(brain.dir, { recursive: true, force: true });
}

// ─── A chat, from the brain ───────────────────────────────────────────────

// What Max does at a tab, with no browser: one act of `act`, a tab
// replica. Its last line says how the brain took it.
function act(brain, ...args) {
  const r = spawnSync(path.join(repo, "target/release/examples/act"), [String(brain.port), ...args], { encoding: "utf8" });
  const last = (r.stdout ?? "").trim().split("\n").pop();
  const why = last.startsWith("refused:") ? last : (r.stderr ?? "").trim().split("\n").pop() || last;
  if (r.status !== 0) fail(why || `act ${args[0]}: ${r.error?.message ?? `exit ${r.status}`}`);
  return last;
}

// Max starts a chat; prints its id, alone, for a script to keep.
function chat(brain, [role, dir, words, ...more]) {
  if (words === undefined || more.length > 0) fail("usage: chat ROLE DIR|- WORDS");
  const id = act(brain, "chat", role, dir === "-" ? "-" : path.resolve(dir), words).replace(/^chat /, "");
  console.log(id);
}

// Max's next words to a chat; returns once the chat has moved on from
// where it was, so a `settle` after it waits for its answer.
async function say(brain, [id, words, ...more]) {
  if (words === undefined || more.length > 0) fail("usage: say CHAT WORDS");
  const was = (await task(brain, id)).task.waiting;
  act(brain, "say", id, words);
  const deadline = Date.now() + timeout;
  for (;;) {
    const now = (await task(brain, id)).task.waiting;
    if (now?.kind !== was?.kind || now?.since !== was?.since) return;
    if (Date.now() > deadline) fail(`${id} took the words but has not moved on in ${timeout / 1000} s`);
    await sleep(100);
  }
}

// What the face serves: the view (every task's state) and a context's chain.
function get(brain, route) {
  const port = brain.port;
  return new Promise((resolve, reject) => {
    https.get({ host: "127.0.0.1", port, path: route, servername: "mx-hv-mj.local", rejectUnauthorized: false }, (res) => {
      let body = "";
      res.setEncoding("utf8");
      res.on("data", (d) => { body += d; });
      res.on("end", () => res.statusCode === 200 ? resolve(JSON.parse(body)) : reject(new Failed(`${route}: ${res.statusCode} ${body}`)));
    }).on("error", (e) => reject(new Failed(`brain ${brain.name} does not answer on ${port} (${e.message}): node dev/drive.mjs up --brain ${brain.name}`)));
  });
}

async function task(brain, id) {
  const { rows } = await get(brain, "/view");
  return { task: rows[`task:${id}`]?.Task ?? fail(`no chat ${id} on brain ${brain.name}`), rows };
}

const clock = (ms) => new Date(ms).toLocaleTimeString("en-NZ", { timeZone: "Pacific/Auckland", hour12: false });

// What a task waits on, in a line: every kind the face's view names.
function state(t) {
  const w = t.waiting ?? {};
  const since = w.since === undefined ? "" : ` since ${clock(w.since)}`;
  const calls = (cs) => cs.map((c) => c.tool ?? "a call").join(", ");
  const why = {
    max: () => "Max's turn",
    done: () => t.outcome?.kind === "failed" ? `failed: ${t.outcome.reason}` : (t.outcome?.kind ?? "done"),
    model: () => w.streaming ? "the model is writing" : "the model is thinking",
    calls: () => {
      const kids = w.calls.flatMap((c) => c.children ?? []);
      return kids.length > 0 ? `waiting on ${kids.length} agent(s)` : `running ${calls(w.calls)}`;
    },
    starting: () => `starting ${w.limbs.join(", ")}${w.failed.map((f) => `; ${f.limb} failed: ${f.why}`).join("")}`,
    harness: () => "the harness is working",
    held: () => "paused",
    parked: () => "parked",
    limited: () => `limited until ${clock(w.until)}`,
    listening: () => `listening to ${calls(w.calls)} in the background`,
  }[w.kind]?.() ?? `waiting on ${w.kind ?? "nothing it names"}`;
  return `${why}${since}${t.trouble ? ` (trouble: ${JSON.stringify(t.trouble)})` : ""}`;
}

// Until the chat waits on Max or is done. Trouble, a failure, or a limb
// that cannot start fails it at once.
async function settle(brain, id) {
  const deadline = Date.now() + timeout;
  for (;;) {
    const { task: t } = await task(brain, id);
    const failed = t.trouble || t.outcome?.kind === "failed" || t.waiting?.failed?.length > 0;
    if (failed) fail(`${id}: ${state(t)}`);
    if (t.waiting?.kind === "max" || t.waiting?.kind === "done") return t;
    if (Date.now() > deadline) fail(`waited ${timeout / 1000} s for ${id} to settle: ${state(t)}`);
    await sleep(200);
  }
}

// A chat's transcript: who said what, each tool call with its result, cut
// short unless `full`; then its state, and its children's.
async function log(brain, id, full) {
  const { task: t, rows } = await task(brain, id);
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
  const chain = t.head ? await get(brain, `/context/${t.head}`) : [];
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
  // Its Chromium stays in this process's group, so whatever kills the
  // group kills it too, and carries the brain's mark, so `down` finds it.
  static async open(brain, size, log) {
    const profile = fs.mkdtempSync(path.join(brain.dir, "chrome-"));
    const proc = spawn(chromium(), [
      "--headless=new", "--remote-debugging-port=0", `--user-data-dir=${profile}`,
      "--ignore-certificate-errors", "--no-first-run", "--no-default-browser-check", "--hide-scrollbars",
      `--window-size=${size.width},${size.height}`, "about:blank",
    ], { stdio: "ignore", env: { ...process.env, [brain.mark]: "1" } });
    const b = new Browser(proc, profile, size, log);
    b.brain = brain;
    for (const signal of ["SIGTERM", "SIGINT", "SIGHUP"]) {
      process.on(signal, () => { b.close(); process.exit(128 + { SIGHUP: 1, SIGINT: 2, SIGTERM: 15 }[signal]); });
    }
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

  // Chromium and every process it started, each of which names its
  // profile; the profile goes once they have, as one still writing would
  // put it back.
  close() {
    const pause = new Int32Array(new SharedArrayBuffer(4));
    for (let i = 0; i < 100; i++) {
      const left = naming(this.profile);
      if (left.length === 0) break;
      for (const pid of left) {
        try { process.kill(pid, "SIGKILL"); } catch { /* already gone */ }
      }
      Atomics.wait(pause, 0, 0, 20);
    }
    fs.rmSync(this.profile, { recursive: true, force: true });
  }
}

// The pids of the processes whose command line names `text`.
function naming(text) {
  return fs.readdirSync("/proc").filter((d) => /^\d+$/.test(d)).filter((d) => {
    try { return fs.readFileSync(`/proc/${d}/cmdline`, "utf8").includes(text); } catch { return false; }
  }).map(Number);
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
  reply: async (b) => { await settle(b.brain, (await b.eval("location.hash")).slice(1)); b.check(); },
  shot: async (b, file) => { await b.shot(file); console.log(`shot ${path.resolve(file)}`); },
  eval: async (b, js) => console.log(JSON.stringify(await b.eval(js)) ?? "undefined"),
  sleep: (b, ms) => sleep(Number(ms)).then(() => b.check()),
};

async function page(brain, args) {
  let size = { width: 1920, height: 1080, dpr: 1, phone: false };
  let log = brain.file("page.log");
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
  const port = brain.port;
  if (!(await listening(port))) fail(`brain ${brain.name} does not listen on ${port}: node dev/drive.mjs up --brain ${brain.name}`);
  const b = await Browser.open(brain, size, log);
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
    const shot = brain.file("failed.png");
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

// `--brain NAME` and `--timeout S` go anywhere; the rest is the command's.
const args = process.argv.slice(2);
const option = (flag) => {
  const at = args.indexOf(flag);
  return at < 0 ? undefined : args.splice(at, 2)[1] ?? fail(`${flag} needs a value`);
};
try {
  const brain = brainOf(option("--brain") ?? "test");
  const seconds = option("--timeout");
  if (seconds !== undefined) timeout = Number(seconds) * 1000;
  const [command, ...rest] = args;
  if (command === "up") await up(brain, rest);
  else if (command === "chat") chat(brain, rest);
  else if (command === "say") await say(brain, rest);
  else if (command === "settle") console.log(state(await settle(brain, rest[0] ?? fail("usage: settle CHAT"))));
  else if (command === "log") await log(brain, rest[0] ?? fail("usage: log CHAT [--full]"), rest[1] === "--full");
  else if (command === "down") down(brain, rest[0] === "--wipe");
  else if (command === "page") await page(brain, rest);
  else fail(`usage: node dev/drive.mjs up|chat|say|settle|log|page|down [--brain NAME] ... (see the top of dev/drive.mjs)`);
} catch (e) {
  if (!(e instanceof Failed)) throw e;
  console.error(e.message);
  process.exitCode = 1;
}
// Exit once what was printed has been written: a pipe takes it in
// asynchronously, and exiting at once would cut a long log short.
process.stdout.write("", () => process.exit());
