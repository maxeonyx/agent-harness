import { render } from "./draw.js";
import { liveArrived } from "./log.js";
import { restoreLocal } from "./compose.js";
import { viewArrived } from "./main.js";

// ─── This tab: a replica on the substrate ────────────────────────────────────
// The tab is the wasm build of harness-face-tab: its own records, kept in
// IndexedDB until the brain's standing covers them, and its replica of the
// rows the brain derives for it. This script is glue: the DOM, the link and
// IndexedDB. Whatever the tab says is parsed into core's facts by the wasm,
// stored, and only then shipped; what became of it is read off the tab's
// standing in a later view, so a refusal shows here, where it was made.
export let can = { act: false, dictation: false, sample: false };
fetch("/wiring").then((r) => r.json()).then((c) => { can = c; render(); });

const enc = new TextEncoder(), dec = new TextDecoder();
let wasm = null;
// One call into the tab: `input` (a string or bytes) in its I/O buffer,
// the export's JSON output back.
function tabCall(name, input, ...args) {
  if (input !== undefined) {
    const bytes = typeof input === "string" ? enc.encode(input) : input;
    const at = wasm.io(bytes.length);
    new Uint8Array(wasm.memory.buffer, at, bytes.length).set(bytes);
  }
  const ret = wasm[name](...args);
  const out = new Uint8Array(wasm.memory.buffer, wasm.io_ptr(), wasm.io_len());
  return { ret, out: out.length ? JSON.parse(dec.decode(out)) : null };
}
// What names a value in the facts: its digest, as the core computes it.
export const digest = (text) => tabCall("digest", text).out;

// IndexedDB: every replica's records this browser holds, by time.
const db = new Promise((ok, fail) => {
  const open = indexedDB.open("harness-tabs", 2);
  open.onupgradeneeded = (e) => {
    if (!e.oldVersion) return open.result.createObjectStore("records", { keyPath: "at" }).createIndex("replica", "replica");
    // Version 1 called a record's replica its place.
    const store = open.transaction.objectStore("records");
    store.deleteIndex("place");
    store.createIndex("replica", "replica");
    store.openCursor().onsuccess = ({ target: { result: c } }) => {
      if (!c) return;
      const { place, ...rest } = c.value;
      c.update({ ...rest, replica: place });
      c.continue();
    };
  };
  open.onsuccess = () => ok(open.result);
  open.onerror = () => fail(open.error);
});
const done = (tx) => new Promise((ok, fail) => { tx.oncomplete = ok; tx.onerror = tx.onabort = () => fail(tx.error); });
async function loadReplica(replica) {
  const store = (await db).transaction("records").objectStore("records");
  const req = store.index("replica").getAll(replica);
  return new Promise((ok, fail) => { req.onsuccess = () => ok(req.result.map((r) => r.record)); req.onerror = () => fail(req.error); });
}
async function replicasHeld() {
  const req = (await db).transaction("records").objectStore("records").index("replica").getAllKeys();
  const keys = await new Promise((ok, fail) => { req.onsuccess = () => ok(req.result); req.onerror = () => fail(req.error); });
  return [...new Set(keys.map((at) => at.split(".")[0]))];
}
async function forget(replica) {
  const tx = (await db).transaction("records", "readwrite");
  const store = tx.objectStore("records");
  for (const at of await new Promise((ok) => { const r = store.index("replica").getAllKeys(replica); r.onsuccess = () => ok(r.result); })) store.delete(at);
  await done(tx);
}

// A replica: a tab's handle in the wasm and its link to the brain.
class Replica {
  constructor(id, handle, onView) { this.id = id; this.h = handle; this.onView = onView; this.ws = null; this.waiting = []; this.flushing = Promise.resolve(); }
  static async open(id, onView) {
    const { ret, out } = tabCall("open", JSON.stringify({ replica: id, records: await loadReplica(id) }));
    if (ret === 0xffffffff) throw new Error(out.error);
    return new Replica(id, ret, onView);
  }
  // Make the tab's writes durable, in order.
  persist() {
    const writes = tabCall("writes", undefined, this.h).out;
    if (!writes.length) return this.flushing;
    this.flushing = this.flushing.then(async () => {
      const tx = (await db).transaction("records", "readwrite");
      const store = tx.objectStore("records");
      for (const w of writes) {
        if (w.Put) store.put({ at: w.Put.at, replica: this.id, record: w.Put });
        else store.delete(w.Drop);
      }
      await done(tx);
    });
    return this.flushing;
  }
  connect() {
    const ws = this.ws = new WebSocket(`wss://${location.host}/link?replica=${this.id}`);
    ws.onopen = () => this.shipUnsent();
    ws.onmessage = (m) => {
      const { out } = tabCall("down", m.data, this.h);
      if (out.error) return console.error(out.error);
      if (out.live) return liveArrived(out.live.live);
      const fresh = m.data.startsWith('{"View"');
      this.persist();
      this.waiting = this.waiting.filter((w) => {
        const s = this.status(w.n);
        if (s === "pending") return true;
        w.ok(s === "included" ? null : s.refused);
        return false;
      });
      this.onView?.(fresh);
    };
    ws.onclose = () => { if (this.ws === ws) { this.onView?.(null); setTimeout(() => this.connect(), 1000); } };
  }
  close() { const ws = this.ws; this.ws = null; ws?.close(); tabCall("close", undefined, this.h); }
  shipUnsent() {
    const { out } = tabCall("unsent", undefined, this.h);
    if (out.restored) return restored(this);
    for (const up of out.ups) this.ws?.send(JSON.stringify(up));
  }
  status(n) { return tabCall("status", undefined, this.h, n).out; }
  view() { return tabCall("view", undefined, this.h).out; }
  next() { return tabCall("next", undefined, this.h).out; }
  records() { return tabCall("records", undefined, this.h).out; }
  settled() { return wasm.settled(this.h) === 1; }
  // Say a batch: stored, then shipped. Resolves with null once the brain
  // includes it, or with why it refused it; until then it is pending.
  async say(batch) {
    const { out } = tabCall("say", JSON.stringify(batch), this.h, Date.now());
    if (out.error) return out.error;
    await this.persist();
    if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(JSON.stringify(out.up));
    return new Promise((ok) => this.waiting.push({ n: out.n, ok }));
  }
}

// This tab's replica: kept in sessionStorage across reloads, and held by a
// Web Lock, so a duplicated tab (which copies sessionStorage) becomes a
// replica of its own. A replica is per tab per brain: IndexedDB is per origin.
const mintPlace = () => [...crypto.getRandomValues(new Uint8Array(8))].map((b) => b.toString(16).padStart(2, "0")).join("");
function hold(id) {
  return new Promise((ok) => navigator.locks.request(`replica:${id}`, { ifAvailable: true }, (lock) => {
    ok(!!lock);
    return lock ? new Promise(() => {}) : null;
  }));
}
export let tab = null;
export async function boot() {
  wasm = (await WebAssembly.instantiateStreaming(fetch("/tab.wasm"))).instance.exports;
  let id = sessionStorage.getItem("replica");
  if (!id || !(await hold(id))) { do id = mintPlace(); while (!(await hold(id))); }
  sessionStorage.setItem("replica", id);
  tab = await Replica.open(id, viewArrived);
  restoreLocal();
  tab.connect();
  adoptOrphans();
}
// A closed tab's replica, whose lock is free: ship what it had not, then
// forget it, so nothing typed in a tab that was closed offline is lost.
async function adoptOrphans() {
  for (const id of await replicasHeld()) {
    if (id === tab.id || !(await hold(id))) continue;
    const orphan = await Replica.open(id, () => {
      if (!orphan.settled()) return;
      orphan.close();
      forget(id);
    });
    orphan.connect();
  }
}
// The brain holds more of this replica than this browser does: its storage
// was restored or lost. It goes on as a new replica.
async function restored(replica) {
  if (replica !== tab) { replica.close(); return forget(replica.id); }
  tab.close();
  await forget(tab.id);
  let id; do id = mintPlace(); while (!(await hold(id)));
  sessionStorage.setItem("replica", id);
  tab = await Replica.open(id, viewArrived);
  tab.connect();
}

// Max's facts, in one record at this tab. `facts` may be a function of the
// record's time, `replica.n`, which names what the record creates
// (`${at}.0`). Values are the texts the facts name. Resolves with null once
// included, or the brain's reason for refusing it.
export async function act(facts, values = []) {
  if (!tab) return "this tab has not started";
  const at = tab.next();
  const made = typeof facts === "function" ? facts(at) : facts;
  return tab.say({ values, facts: Array.isArray(made) ? made : [made] });
}

