// Every diagram on the dashboard. DIAGRAMS[name] = { title, src?, draw(el, data?) }: draw fills the empty element el.
// `src` is the generated JSON it is drawn from, handed to draw as `data`; gen/extract.py regenerates files.json and types.json.
// The hand-written specs describe main as of the review in index.html (#built); edit them when main changes.
const DIAGRAMS = {};

// The data flow: event sources, the one writer, the folds, the views.
DIAGRAMS.flow = {
  title: "One fold, many readers", src: "gen/flow.json",
  draw: (el, { nodes, edges }) => {
    const short = { v_face_view: "view::view (the page's JSON)", v_waiting: "State::waiting_on", v_queue: "State::queue", v_stale: "State::stale",
      v_lines: "describe(record)", v_sql: "SQL over the journal", src_live: "streamed tokens, command output", src_brain_loop: "brain loop, acting on todo", apply: "State::apply" };
    const kind = { source: "src", gate: "core", fold: "core", log: "store", view: "view", sink: "gap" };
    return diagram(el, {
    nodes: [
        ...nodes.map((n) => ({ id: n.id, label: short[n.id] ?? n.label, kind: n.id === "src_live" ? "part" : kind[n.kind],
          tip: [n.author && "author " + n.author, n.facts && "states " + n.facts.join(", "), n.does, n.consumer && "read by " + n.consumer, n.code].filter(Boolean).join(". ") })),
      ],
      direction: "RIGHT",
      // the loop's feedback edge (todo → brain loop) is the one to reverse, so sources stay first
      layout: { "elk.layered.cycleBreaking.strategy": "MODEL_ORDER", "elk.layered.considerModelOrder.strategy": "NODES_AND_EDGES" },
      edges: edges.map((e) => ({ from: e.from, to: e.to, tip: e.via, style: e.to === "refusal" ? "dashed" : e.from === "v_todo" ? "strong" : "" })),
    });
  },
};

// Every copy of the data at runtime, grouped by where it lives.
DIAGRAMS.replicas = {
  title: "Every copy of the data", src: "gen/replicas.json",
  draw: (el, { nodes, edges }) => {
    const where = (n) => ({ "harness-brain": "p_brain", file: "p_disk", browser: "p_browser", remote: "p_remote", "harness-limb-*": "p_limb" })[n.process];
    return diagram(el, {
    nodes: [
        { id: "p_brain", label: "harness-brain process" }, { id: "p_limb", label: "harness-limb-workspace process" },
        { id: "p_disk", label: "disk" }, { id: "p_browser", label: "browser tab" }, { id: "p_remote", label: "Anthropic" },
        ...nodes.map((n) => ({ id: n.id, label: n.label, parent: where(n), kind: n.durable === true ? "store" : n.id === "brain_state" ? "core" : n.id === "brain_caches" ? "gap" : undefined,
          sub: n.thread, tip: n.holds + (n.code ? " (" + n.code + ")" : "") })),
      ],
      edges: edges.map((e) => ({ from: e.from, to: e.to, tip: e.what + ", by " + e.mechanism })),
      direction: "DOWN",
    });
  },
};

// Processes, threads, and the transport on each seam.
DIAGRAMS.topology = {
  title: "Processes and seams",
  draw: (el) => diagram(el, {
    direction: "DOWN",
    nodes: [
      { id: "browser", label: "browser" }, { id: "page", label: "page.html", parent: "browser", tip: "One static page, include_str! into the face. Holds the rendered view, not a replica." },
      { id: "brainp", label: "harness-brain process" },
      { id: "t_loop", label: "brain loop", sub: "main thread", parent: "brainp", kind: "core", tip: "brain.run() inside thread::scope (brain/src/main.rs:161). The only writer: mpsc inbox in, 250 ms tick." },
      { id: "t_follow", label: "face follower", sub: "thread", parent: "brainp", tip: "face.follow() (face-web/src/lib.rs:165): receives each Commit, the brain's State as of it plus its new records, and renders the view. It folds nothing itself." },
      { id: "t_http", label: "HTTPS connections", sub: "thread each", parent: "brainp", tip: "std TcpListener + rustls, hand-written HTTP/1.1. /events and /live are SSE; /dictation is a WebSocket." },
      { id: "t_req", label: "model requests", sub: "thread each", parent: "brainp", tip: "thread::spawn per request (brain.rs:780); ureq blocking HTTPS; abort flag checked per SSE event." },
      { id: "t_lread", label: "limb readers", sub: "thread per limb", parent: "brainp", tip: "Reads FromLimb JSON lines from the limb's stdout into the inbox (brain/src/process.rs:44)." },
      { id: "feed", label: "Feed", sub: "shared memory", parent: "brainp", kind: "part", tip: "Arc<Mutex<HashMap>> + Condvar of live pieces. Not a wire: a face in another process could not read it." },
      { id: "disk", label: "disk" }, { id: "sqlite", label: "journal.sqlite", sub: "WAL", parent: "disk", kind: "store" },
      { id: "limbp", label: "harness-limb-workspace process" },
      { id: "l_main", label: "stdin loop", parent: "limbp", tip: "First line is the Limb definition (limb-workspace/src/process.rs:34), then one ToLimb per line." },
      { id: "l_call", label: "calls", sub: "thread each", parent: "limbp", tip: "One thread per running call; record file written Started before effects." },
      { id: "l_rec", label: "call records", sub: "files", parent: "disk", kind: "store" },
      { id: "bash", label: "bash -c", sub: "own process group", parent: "limbp", tip: "SIGTERM to the group, 1 s grace, then SIGKILL (shell.rs:219)." },
      { id: "anth", label: "Anthropic" },
      { id: "a_msg", label: "/v1/messages", sub: "HTTPS + SSE", parent: "anth" }, { id: "a_voice", label: "voice_stream", sub: "WSS", parent: "anth" },
    ],
    edges: [
      { from: "page", to: "t_http", label: "HTTPS", tip: "GET /events (SSE), /live (SSE), /view, /context/<entry>; POST /act, /digest; /dictation WebSocket" },
      { from: "t_http", to: "t_loop", label: "Act trait → mpsc", tip: "In-process sync RPC: POST /act blocks its thread on the brain's reply channel, no timeout." },
      { from: "t_loop", to: "sqlite", label: "commit", tip: "Journal::commit: fold, then one SQLite transaction, synchronous=FULL." },
      { from: "t_loop", to: "t_follow", label: "Commit → mpsc", tip: "After each commit (brain.rs:1329): the persistent State as of it, a cheap clone, and the records it added." },
      { from: "sqlite", to: "t_follow", label: "values: own connection", tip: "Values (brain/src/main.rs:73): the face's own SQLite connection behind a Mutex, read by id to render the view and serve /context. Values never change, so nothing wakes it." },
      { from: "t_follow", to: "t_http", label: "whole view per fold", tip: "Shown: Mutex + Condvar; every /events stream sends id: seq and the whole view." },
      { from: "t_loop", to: "t_req", label: "spawn", tip: "After committing ModelCall Sent, with its base entries and bytes." },
      { from: "t_req", to: "a_msg", label: "ureq", tip: "No timeouts (provider/src/client.rs:53)." },
      { from: "t_req", to: "t_loop", label: "inbox", style: "dashed", tip: "What::FirstByte, What::Finished" },
      { from: "t_req", to: "feed", tip: "Live::Model: the visible text as it streams" },
      { from: "feed", to: "t_http", tip: "/live SSE, resumable by piece count" },
      { from: "t_loop", to: "l_main", label: "stdin JSON lines", tip: "ToLimb: Run | Stop | Watch" },
      { from: "l_main", to: "t_lread", label: "stdout JSON lines", style: "dashed", tip: "FromLimb: Hello | Facts | Live. The limb's first facts on a connection are its Described." },
      { from: "t_lread", to: "t_loop", label: "inbox", style: "dashed" },
      { from: "l_main", to: "l_call" }, { from: "l_call", to: "bash" }, { from: "l_call", to: "l_rec", tip: "atomic replace + fsync" },
      { from: "t_http", to: "a_voice", label: "dictation relay", tip: "tungstenite over rustls; one connection thread serves both sockets" },
    ],
  }),
};

// The cancellation cascade on main at the review, with what Max asked for that wasn't there.
DIAGRAMS.cancel = {
  title: "Stopping a task",
  draw: (el) => diagram(el, {
    direction: "DOWN",
    nodes: [
      { id: "max", label: "Max: Stop", kind: "src" },
      { id: "drain", label: "Drain(Stop) fact", sub: "phase 1: draining", kind: "core", tip: "TaskEvent::Drain sets the live task's drain and nothing else; no request is sent while it is set." },
      { id: "fold", label: "State::todo", sub: "the fold decides", kind: "core" },
      { id: "abandon", label: "Todo::Abandon", tip: "For a task Thinking while draining." },
      { id: "flag", label: "abort flag", sub: "checked per SSE event", kind: "part", tip: "brain.rs:795. Not read while connecting or in a silent gap in the stream; no timeouts." },
      { id: "ended", label: "Ended Abandoned{partial}" },
      { id: "interrupt", label: "Todo::Interrupt", tip: "For each outstanding call every reader of which is draining." },
      { id: "stopping", label: "Stopping fact + ToLimb::Stop" },
      { id: "signal", label: "SIGTERM, 1 s, SIGKILL", sub: "process group" },
      { id: "settled", label: "Settled Interrupted" },
      { id: "stopm", label: "Todo::Stop(member)", tip: "For each live member of a scope whose owner is ending." },
      { id: "child", label: "child: Drain(Stop(Owner))", sub: "same cascade, recursively" },
      { id: "childdone", label: "child Done, Cancelled" },
      { id: "drained", label: "drained()", sub: "phase 2: confirmed", kind: "core", tip: "task.rs:1224: Life::Done with Outcome::Cancelled { by } only once nothing in the round is outstanding, and no background work runs." },
      { id: "done", label: "Done, Cancelled", kind: "view" },
      { id: "tidy", label: "\"please tidy up\" turn", sub: "missing", kind: "gap", tip: "Max: send the agent a message to tidy up, and it asks the same of its children. Today a stop kills the request and the tools." },
      { id: "handoff", label: "pass ownership to parent", sub: "missing", kind: "gap", tip: "Max: to finish while something sticks around, pass ownership to the parent. Today a task with background work cannot finish (Listening)." },
      { id: "shutdown", label: "graceful shutdown", sub: "missing", kind: "gap", tip: "No signal handling: the brain runs until killed; in-flight requests become Lost, commands Unknown." },
    ],
    edges: [
      { from: "max", to: "drain" }, { from: "drain", to: "fold" },
      { from: "fold", to: "abandon" }, { from: "abandon", to: "flag" }, { from: "flag", to: "ended" }, { from: "ended", to: "drained" },
      { from: "fold", to: "interrupt" }, { from: "interrupt", to: "stopping" }, { from: "stopping", to: "signal" }, { from: "signal", to: "settled" }, { from: "settled", to: "drained" },
      { from: "fold", to: "stopm" }, { from: "stopm", to: "child" }, { from: "child", to: "childdone" }, { from: "childdone", to: "drained" },
      { from: "drained", to: "done" },
      { from: "drain", to: "tidy", style: "dashed" }, { from: "drained", to: "handoff", style: "dashed" }, { from: "max", to: "shutdown", style: "dashed", tip: "the same discipline one level up: the brain itself" },
    ],
  }),
};

// The three lifecycles: a task, a model request, a tool call.
DIAGRAMS.life = {
  title: "Three lifecycles",
  draw: (el) => diagram(el, {
    nodes: [
      { id: "task", label: "Task: Life (task.rs:112)" },
      { id: "Starting", parent: "task", tip: "No context yet: some limb it can reach has not connected or described itself. Max can stop it; it cannot be interrupted." },
      { id: "live", label: "Live: Phase (task.rs:162)", parent: "task" },
      { id: "Ready", parent: "live", kind: "core", tip: "The task's move: the brain sends its next request unless held or limited." },
      { id: "Thinking", parent: "live", tip: "A request is in flight." },
      { id: "Acting", parent: "live", tip: "A round's calls are outstanding: tool uses, a spawn, an ask of Max." },
      { id: "AwaitingMax", parent: "live", tip: "Max's move: a user-facing turn ended, or he interrupted. Every task Max starts is user-facing; his Done makes it autonomous, so its next turn's end finishes it." },
      { id: "Listening", parent: "live", tip: "Waiting for something that wakes it: background work, a change to what it embodies." },
      { id: "Done", parent: "task", sub: "Result | Failed | Cancelled", kind: "view" },
      { id: "mc", label: "Model request (ModelCallEvent)" },
      { id: "Sent", parent: "mc" }, { id: "FirstByte", parent: "mc" }, { id: "Ended", parent: "mc", sub: "Responded | Failed | Limited | Abandoned | Lost", kind: "view" },
      { id: "call", label: "Tool call (CallEvent)" },
      { id: "Made", parent: "call" }, { id: "Dispatched", parent: "call", tip: "Resent after every limb reconnection while unsettled." }, { id: "Stopping", parent: "call" },
      { id: "Settled", parent: "call", sub: "Completed | Failed | Refused | Interrupted | Unknown", kind: "view" },
    ],
    edges: [
      { from: "Starting", to: "Ready", label: "Root" }, { from: "Ready", to: "Thinking", label: "Send" },
      { from: "Thinking", to: "Acting", label: "tool uses" }, { from: "Acting", to: "Ready", label: "round settled" },
      { from: "Thinking", to: "AwaitingMax", label: "user-facing turn ends" }, { from: "AwaitingMax", to: "Ready", label: "Max's words" },
      { from: "Thinking", to: "Listening", label: "background running" }, { from: "Listening", to: "Ready", label: "woken" },
      { from: "Thinking", to: "Done", label: "autonomous turn ends" }, { from: "Acting", to: "Done", label: "drained", style: "dashed" },
      { from: "Starting", to: "Done", label: "stopped", style: "dashed" },
      { from: "Sent", to: "FirstByte" }, { from: "FirstByte", to: "Ended" }, { from: "Sent", to: "Ended", style: "dashed", label: "Lost / Abandoned" },
      { from: "Made", to: "Dispatched" }, { from: "Dispatched", to: "Settled" }, { from: "Dispatched", to: "Stopping", style: "dashed" }, { from: "Stopping", to: "Settled" },
    ],
  }),
};

// One real tool-using turn, journal dev-process-run on main at 256eb78, 2026-10-17 11:53:55 NZT.
DIAGRAMS.turn = {
  title: "A real turn",
  draw: (el) => sequence(el, {
    actors: [{ id: "limb", label: "workspace limb" }, { id: "brain", label: "brain loop" }, { id: "journal", label: "journal" }, { id: "req", label: "request thread" }, { id: "anth", label: "Anthropic" }, { id: "face", label: "face → page" }],
    steps: [
      { from: "limb", to: "brain", text: "Hello{instance}, Facts{Described}", tip: "the limb's description is its first fact on the connection: instructions, and the tools bash, read, write, edit, grep" },
      { from: "brain", to: "journal", text: "Reach Connected", seq: 13 },
      { from: "brain", to: "journal", text: "Described (the limb's)", seq: 14, tip: "authored by the limb, committed from its Facts batch. The user limb connects and describes itself the same way (#15, #16)." },
      { from: "brain", to: "journal", text: "Task Moved Root", seq: 17, tip: "first root on claude-opus-5-5: system + tools values, a binding of 10 tools; observes the role file and each limb's instructions. Caused by Todo::Root." },
      { from: "brain", to: "limb", text: "Watch{known}", tip: "the instructions versions the root observes" },
      { from: "brain", to: "journal", text: "ModelCall Sent  +252 ms", seq: 18, tip: "base = root, appends the mandate note; effort medium, thinking adaptive; 2 cache breakpoints. Caused by Todo::Send." },
      { from: "brain", to: "req", text: "spawn" },
      { from: "req", to: "anth", text: "POST /v1/messages, stream" },
      { from: "anth", to: "req", text: "message_start", style: "dashed" },
      { from: "req", to: "brain", text: "What::FirstByte", style: "dashed" },
      { from: "brain", to: "journal", text: "FirstByte  +1.3 s", seq: 19 },
      { group: "streaming", steps: [
        { from: "anth", to: "req", text: "thinking, tool_use bash, message_stop", style: "dashed" },
        { from: "req", to: "face", text: "Live::Model visible text → /live", style: "dashed", tip: "never a fact" },
      ] },
      { from: "req", to: "brain", text: "What::Finished", style: "dashed" },
      { from: "brain", to: "journal", text: "Call Made (bash)", seq: 20, tip: "cat README.md; ls -R; cat check.sh; git log --oneline | head. Same batch as #21." },
      { from: "brain", to: "journal", text: "ModelCall Ended Responded  +2.5 s", seq: 21, tip: "stop ToolUse; usage: input 4, cache write 8061, output 105; quota 5h 23 %, 7d 45 %" },
      { from: "brain", to: "journal", text: "Call Dispatched", seq: 22, tip: "Todo::Dispatch" },
      { from: "brain", to: "limb", text: "Run{id, call, input}" },
      { from: "limb", to: "limb", text: "record Started; bash in its own group" },
      { from: "limb", to: "face", text: "Live::Call chunks (via inbox → Feed)", style: "dashed" },
      { from: "limb", to: "brain", text: "Facts{Call Settled Completed}", tip: "written to the limb's record as Settled before it is sent" },
      { from: "brain", to: "journal", text: "Call Settled  +14 ms after dispatch", seq: 23, tip: "authored by the limb" },
      { from: "brain", to: "journal", text: "ModelCall Sent (tool_result)", seq: 24, tip: "base = the response; appends the tool_result; 3 breakpoints. Todo::Send: the round's only call settled." },
      { from: "journal", to: "face", text: "after each commit: Commit{state, records} → view → /events", style: "dashed", tip: "the brain hands its face the State as of the commit; every page gets id: seq, the whole view, and the new fact lines" },
    ],
  }),
};

// The dictation relay: nothing enters the journal until Max sends the box.
DIAGRAMS.dictation = {
  title: "Dictation",
  draw: (el) => sequence(el, {
    colW: 210,
    actors: [{ id: "page", label: "page (AudioWorklet)" }, { id: "face", label: "face connection thread" }, { id: "login", label: "Login" }, { id: "anth", label: "Anthropic voice_stream" }],
    steps: [
      { from: "page", to: "page", text: "Dictate: mic, 16-bit PCM, 100 ms buffers" },
      { from: "page", to: "face", text: "GET /dictation?rate=48000 (WebSocket)" },
      { from: "face", to: "login", text: "token()", tip: "re-reads ~/.claude/.credentials.json; access token only" },
      { from: "face", to: "anth", text: "WSS connect, Bearer, KeepAlive" },
      { from: "face", to: "page", text: "{listening: true}", style: "dashed" },
      { group: "every 10 ms, until stop", steps: [
        { from: "page", to: "face", text: "PCM frame" }, { from: "face", to: "anth", text: "same frame" },
        { from: "anth", to: "face", text: "TranscriptText: whole session so far", style: "dashed" },
        { from: "face", to: "face", text: "word-level diff against what's shown" },
        { from: "face", to: "page", text: "{edits: [{at, cut, put}]}", style: "dashed", tip: "applied from the remembered caret, keeping Max's own edits; draft saved to localStorage" },
      ] },
      { from: "page", to: "face", text: "{type: stop}" }, { from: "face", to: "anth", text: "CloseStream" },
      { from: "anth", to: "face", text: "final text, TranscriptEndpoint", style: "dashed" },
      { from: "face", to: "page", text: "last edits, {final: true}, close", style: "dashed" },
      { from: "page", to: "face", text: "Send: POST /digest, POST /act Delivery{Words}", tip: "the first moment dictation touches the journal" },
    ],
  }),
};

// The six crates, from gen/files.json: an arrow means files in one name things in the other.
const crateOf = (f) => f.split("/")[1];
DIAGRAMS.crates = {
  title: "Crates", src: "gen/files.json",
  draw: (el, { nodes, edges }) => {
    const crates = new Map();
    for (const f of nodes) {
      const c = crates.get(crateOf(f.id)) ?? { files: 0, lines: 0 };
      c.files++; c.lines += f.lines; crates.set(crateOf(f.id), c);
    }
    const between = new Map();
    for (const e of edges) {
      const k = crateOf(e.from) + ">" + crateOf(e.to);
      if (crateOf(e.from) !== crateOf(e.to)) between.set(k, (between.get(k) ?? 0) + 1);
    }
    return diagram(el, {
      nodes: [...crates].map(([id, c]) => ({ id, sub: `${c.files} files, ${c.lines} lines`, kind: id === "core" ? "core" : undefined })),
      edges: [...between].map(([k, n]) => { const [from, to] = k.split(">"); return { from, to, label: String(n) }; }),
      direction: "DOWN",
    });
  },
};

// Inside crates/core, file by file, transitively reduced; files that all name each other share a box.
DIAGRAMS.corefiles = {
  title: "core, file by file", src: "gen/files.json",
  draw: (el, { nodes, edges }) => {
    const inCore = (f) => crateOf(f) === "core" && !f.endsWith("lib.rs");
    const ce = edges.filter((e) => inCore(e.from) && inCore(e.to));
    const cn = nodes.filter((f) => inCore(f.id));
    const { comp, dag } = condense(cn.map((f) => f.id), ce);
    const cyc = new Set(cn.filter((f) => cn.some((g) => g.id !== f.id && comp.get(g.id) === comp.get(f.id))).map((f) => f.id));
    return diagram(el, {
      nodes: [...(cyc.size > 0 ? [{ id: "cycle", label: "names each other" }] : []),
        ...cn.map((f) => ({ id: f.id, label: f.id.split("/").at(-1), sub: f.lines + " lines", tip: f.doc, parent: cyc.has(f.id) ? "cycle" : undefined }))],
      edges: ce.filter((e) => comp.get(e.from) === comp.get(e.to) || dag(e)),
      direction: "DOWN",
    });
  },
};

// Strongly connected components, and a test for whether an edge between two of them survives transitive reduction.
function condense(ids, es) {
  const out = new Map(ids.map((i) => [i, []]));
  for (const e of es) out.get(e.from).push(e.to);
  const comp = new Map(); let idx = 0; const index = new Map(), low = new Map(), st = [], on = new Set();
  const visit = (v) => {
    index.set(v, idx); low.set(v, idx++); st.push(v); on.add(v);
    for (const w of out.get(v)) {
      if (!index.has(w)) { visit(w); low.set(v, Math.min(low.get(v), low.get(w))); }
      else if (on.has(w)) low.set(v, Math.min(low.get(v), index.get(w)));
    }
    if (low.get(v) === index.get(v)) { let w; do { w = st.pop(); on.delete(w); comp.set(w, v); } while (w !== v); }
  };
  ids.forEach((i) => index.has(i) || visit(i));
  const cout = new Map();
  for (const e of es) { const a = comp.get(e.from), b = comp.get(e.to); if (a !== b) (cout.get(a) ?? cout.set(a, new Set()).get(a)).add(b); }
  const reaches = (a, c, skip) => { const seen = new Set(), stk = [...(cout.get(a) ?? [])].filter((x) => x !== skip);
    while (stk.length > 0) { const x = stk.pop(); if (x === c) return true; if (seen.has(x)) continue; seen.add(x); stk.push(...(cout.get(x) ?? [])); } return false; };
  return { comp, dag: (e) => !reaches(comp.get(e.from), comp.get(e.to), comp.get(e.to)) };
}

// The plan: each of Max's asks, and the structures it is built from.
const S6 = (id, label, sub, tip) => ({ id, label, sub, kind: "core", tip });
const Ask = (id, label, tip) => ({ id, label, kind: "view", tip });
DIAGRAMS.structures = {
  title: "Your asks, and the structures they are built from",
  draw: (el) => diagram(el, {
    direction: "RIGHT",
    nodes: [
      S6("s1", "S1 Substrate", "facts at a time and a place", "Records are a fact at a place at a time. Views, validity and replication filters are derived. Durability is declared per kind of fact. Coordination is per object."),
      S6("s2", "S2 Tasks", "one tree, every level", "Anything asynchronous is a task with one owner. Two-phase cancel with shrinking deadlines, then hard kill. Durable tasks outlive processes; process tasks die with them."),
      S6("s3", "S3 Contexts", "what an agent was told, as values", "Immutable, content-addressed; forks share prefix and cache; changes arrive as dated notices. Mostly built on main."),
      S6("s4", "S4 Environments", "limbs and nothing else", "An agent can do only what its limbs give it. You are a limb. Roles dissolve into limbs. Definitions are versioned facts."),
      S6("s5", "S5 Places and versions", "a version is a place", "One binary runs any mix of brain, limb, face. Each process is one build; upgrade = start a new place and route new sessions to it."),
      S6("s6", "S6 You", "a participant, attention, domains", "Your acts are facts at your place. One actionable tree per domain. The scheduler is a standing program in each domain."),
      Ask("durability", "durability classes", "S1: declared per kind of fact"),
      Ask("invalid", "invalid in band; by construction", "S1: the validity view is the parse at every boundary"),
      Ask("sent", "sent status; drafts on phone + PC", "S1: a draft is a fact at the tab; sent once the brain's view includes the tab's time"),
      Ask("dataflow", "one data flow, across the network", "S1 + S5"),
      Ask("cancel", "two-phase cancel, Ctrl-C twice, process groups", "S2"),
      Ask("aside", "asides, asks, done is up to you", "S2 (a user-facing child) + S4 (you are a limb)"),
      Ask("versions", "sessions as programs at versions", "S5 (+ S2: drain, then relaunch)"),
      Ask("extension", "extension and sandboxing", "S4 (a sandbox is a limb) + S5 (a module is a finer place)"),
      Ask("clients", "multiple clients, the phone", "S1 + S5 + S6"),
      Ask("domains", "domains, scheduler, action lock", "S6 on S1"),
      Ask("handover", "handover, compaction, notices", "S3 + S2"),
    ],
    edges: [
      ...[["durability", "s1"], ["invalid", "s1"], ["sent", "s1"], ["dataflow", "s1"], ["dataflow", "s5"], ["cancel", "s2"], ["aside", "s2"], ["aside", "s4"],
        ["versions", "s5"], ["extension", "s4"], ["extension", "s5"], ["clients", "s1"], ["clients", "s5"], ["clients", "s6"], ["domains", "s6"], ["handover", "s3"], ["handover", "s2"]]
        .map(([from, to]) => ({ from, to, tip: "an instance of" })),
    ],
  }),
};
