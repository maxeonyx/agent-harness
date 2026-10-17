#!/usr/bin/env node
// A throwaway test brain, and its page driven the way Max drives it.
//
//   node dev/drive.mjs up PORT [MODEL[:EFFORT]] [ROLE=MODEL[:EFFORT]]...
//   node dev/drive.mjs page PORT [--size WxH[@DPR] | --size phone] [--timeout S] STEP...
//   node dev/drive.mjs down PORT [--wipe]
//
// `up` builds the harness, ends whatever this clone left on PORT (the
// brain and its limbs), and starts a brain there on its own database and
// a copy of roles/ in .drive.ignore/PORT/. Every role runs on Sonnet at
// low effort unless told otherwise: `haiku:low` sets every role, and
// `engineer=opus:high` one. Its database outlives `up`, so the next `up`
// relaunches a rebuilt brain on the same chats; `down --wipe` forgets them.
//
// `page` opens the face in a fresh headless Chromium at 1920x1080 (or a
// phone: 390x844, touch, twice the pixels) and runs the steps in order:
//
//   chat ROLE DIR WORDS  start a chat as Max does: +, its role, its
//                        workspace directory (- for none), the words,
//                        send twice; prints its id
//   open ID              open a chat by its link
//   click CSS / hover CSS / type CSS TEXT / key KEY (Enter, Escape, Tab, a letter)
//   wait CSS             until something it matches is visible
//   gone CSS             until nothing it matches is visible
//   text WORDS           until the page shows the words
//   reply                until the open chat's turn is Max's again
//   shot FILE            a PNG, for `read`
//   eval JS              prints what the expression comes to
//   sleep MS
//
// It fails, with a screenshot in .drive.ignore/PORT/failed.png, the
// page's errors and its console, at the first step that cannot be done,
// the first wait that times out (60 s unless --timeout), and the first
// exception the page throws.
import { execFileSync, spawn } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
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

async function up(port, picks) {
  const tunings = { "": { model: "sonnet", effort: "low" } };
  for (const pick of picks) {
    const [, role = "", model, effort] = pick.match(/^(?:([\w-]+)=)?([\w.-]+)(?::(\w+))?$/) ?? fail(`not MODEL[:EFFORT] or ROLE=MODEL[:EFFORT]: ${pick}`);
    if (role !== "" && !fs.existsSync(path.join(repo, "roles", `${role}.md`))) fail(`no role ${role} in roles/`);
    tunings[role] = { model, effort: effort ?? "low" };
  }
  execFileSync("cargo", ["build", "-q", "--release", "--bins"], { cwd: repo, stdio: "inherit" });
  end(port);
  const place = placeOf(port);
  const roles = path.join(place, "roles");
  fs.rmSync(roles, { recursive: true, force: true });
  fs.cpSync(path.join(repo, "roles"), roles, { recursive: true });
  for (const file of fs.readdirSync(roles).filter((f) => f.endsWith(".md"))) {
    const role = path.basename(file, ".md");
    const { model, effort } = tunings[role] ?? tunings[""];
    const text = fs.readFileSync(path.join(roles, file), "utf8")
      .replace(/^model: .*$/m, `model: ${model}`)
      .replace(/^effort: .*$/m, `effort: ${effort}`);
    fs.writeFileSync(path.join(roles, file), text);
  }

  const log = path.join(place, "brain.log");
  const out = fs.openSync(log, "a");
  const brain = spawn(path.join(repo, "target/release/harness-brain"),
    ["--roles", roles, "--db", path.join(place, "brain.sqlite"), "--port", String(port)],
    { detached: true, stdio: ["ignore", out, out], env: { ...process.env, [markOf(port)]: "1" } });
  brain.unref();
  let exited = null;
  brain.on("exit", (code) => { exited = code; });
  for (let i = 0; i < 300 && !(await listening(port)); i++) {
    if (exited !== null) fail(`the brain exited (${exited}):\n${tail(log, 20)}`);
    await sleep(100);
  }
  if (!(await listening(port))) fail(`the brain is not listening on ${port} after 30 s:\n${tail(log, 20)}`);
  const tuned = Object.entries(tunings).map(([r, t]) => `${r || "every role"} ${t.model}:${t.effort}`).join(", ");
  console.log(`brain on https://mx-hv-mj.local:${port}/ (pid ${brain.pid}; ${tuned})\nlog ${log}\nend it with: node dev/drive.mjs down ${port}`);
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
  static async open(place, size) {
    const profile = fs.mkdtempSync(path.join(place, "chrome-"));
    const proc = spawn(chromium(), [
      "--headless=new", "--remote-debugging-port=0", `--user-data-dir=${profile}`,
      "--ignore-certificate-errors", "--no-first-run", "--no-default-browser-check", "--hide-scrollbars",
      `--window-size=${size.width},${size.height}`, "about:blank",
    ], { detached: true, stdio: "ignore" });
    const b = new Browser(proc, profile, size);
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

  constructor(proc, profile, size) {
    Object.assign(this, { proc, profile, size, next: 0, waiting: new Map(), errors: [], console: [] });
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
    } else if (m.method === "Runtime.consoleAPICalled") {
      this.console.push(`${m.params.type}: ${m.params.args.map((a) => a.value ?? a.description ?? a.type).join(" ")}`);
    } else if (m.method === "Log.entryAdded") {
      const e = m.params.entry;
      this.console.push(`${e.level} (${e.source}): ${e.text}${e.url ? " " + e.url : ""}`);
    } else if (m.method === "Page.javascriptDialogOpening") {
      this.console.push(`dialog ${m.params.type}: ${m.params.message}`);
      this.send("Page.handleJavaScriptDialog", { accept: true });
    }
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
  // under anything else, and still, as Max waits for a card to land.
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
      const e = [...document.querySelectorAll(${JSON.stringify(css)})].find((e) => e.getClientRects().length > 0);
      if (e === undefined) return "nothing visible matches";
      e.scrollIntoView({ block: "center", inline: "center" });
      const r = e.getBoundingClientRect(), x = r.x + r.width / 2, y = r.y + r.height / 2;
      const top = document.elementFromPoint(x, y);
      if (top !== e && !e.contains(top)) return "it is under " + (top === null ? "nothing" : top.tagName.toLowerCase() + (top.id ? "#" + top.id : "") + [...top.classList].map((c) => "." + c).join(""));
      return { x, y };
    })()`);
  }

  async click(css) {
    const { x, y } = await this.point(css);
    if (this.size.phone) {
      await this.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x, y }] });
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
    const { x, y } = await this.point(css);
    await this.send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
    await sleep(50);
    this.check();
  }

  // A select is chosen from, as its list would; anything else is typed into.
  async type(css, text) {
    const chose = await this.eval(`(() => {
      const e = document.querySelector(${JSON.stringify(css)});
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
const visible = (css) => `[...document.querySelectorAll(${JSON.stringify(css)})].some((e) => e.getClientRects().length > 0)`;

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
  text: (b, words) => b.until(JSON.stringify(words), `document.body.innerText.includes(${JSON.stringify(words)})`),
  reply: (b) => b.until("Max's turn in the open chat", visible("#panel .mini.needs")),
  shot: async (b, file) => { await b.shot(file); console.log(`shot ${path.resolve(file)}`); },
  eval: async (b, js) => console.log(JSON.stringify(await b.eval(js)) ?? "undefined"),
  sleep: (b, ms) => sleep(Number(ms)).then(() => b.check()),
};

async function page(port, args) {
  let size = { width: 1920, height: 1080, dpr: 1, phone: false };
  while (args[0]?.startsWith("--")) {
    const [flag, value] = args.splice(0, 2);
    if (value === undefined) fail(`${flag} needs a value`);
    if (flag === "--timeout") timeout = Number(value) * 1000;
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
  const b = await Browser.open(place, size);
  let at = "loading the page";
  try {
    await b.send("Page.navigate", { url: `https://mx-hv-mj.local:${port}/` });
    await b.until("the page to link to the brain", `document.querySelector("#link")?.className === ""`);
    for (const [name, step, words] of plan) {
      at = [name, ...words.map((w) => JSON.stringify(w))].join(" ");
      await step(b, ...words);
    }
  } catch (e) {
    if (!(e instanceof Failed)) throw e;
    const shot = path.join(place, "failed.png");
    await b.shot(shot).catch(() => {});
    console.error(`failed at ${at}: ${e.message}`);
    if (b.errors.length > 0) console.error(`page errors:\n  ${b.errors.join("\n  ")}`);
    if (b.console.length > 0) console.error(`console:\n  ${b.console.slice(-30).join("\n  ")}`);
    console.error(`screenshot ${shot}`);
    process.exitCode = 1;
  } finally {
    b.close();
  }
  if (process.exitCode !== 1 && b.console.length > 0) console.log(`console:\n  ${b.console.join("\n  ")}`);
}

const [command, port, ...rest] = process.argv.slice(2);
try {
  if (!/^\d+$/.test(port ?? "")) fail("usage: node dev/drive.mjs up|page|down PORT ... (see the top of dev/drive.mjs)");
  if (command === "up") await up(Number(port), rest);
  else if (command === "down") down(Number(port), rest[0] === "--wipe");
  else if (command === "page") await page(Number(port), rest);
  else fail(`unknown command ${command}; up, page or down`);
} catch (e) {
  if (!(e instanceof Failed)) throw e;
  console.error(e.message);
  process.exitCode = 1;
}
process.exit();
