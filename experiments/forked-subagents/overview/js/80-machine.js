// Machine: the one loop every agent runs, drawn as the code runs it — a task call re-enters the same loop one level down.
// Every step opens the lines that do it. Below, the modules, sized by lines.

view("machine", "machine", el => {
  const A = "src/agent.rs";
  const at = needle => lineOf(A, needle);
  const N = {
    loop: { x: 40, y: 20, w: 300, t: "send the whole context", d: "one request per turn", code: [A, fnLine(A, "request"), 24] },
    wire: { x: 40, y: 125, w: 300, t: "the provider answers", d: "cap · retries · 429s waited out · cancel", code: ["src/wire.rs", fnLine("src/wire.rs", "send_once"), 40] },
    calls: { x: 40, y: 230, w: 300, t: "did it call any tools?", d: "", shape: "decision", code: [A, at("if calls.is_empty()"), 8] },
    report: { x: 40, y: 360, w: 300, t: "no: its last message is its report", d: "the agent ends; its parent gets this", code: [A, at("if calls.is_empty()"), 3], end: true },
    append: { x: 420, y: 20, w: 300, t: "append every result", d: "then go round again", code: [A, at("for result in results {"), 5] },
    limb: { x: 420, y: 125, w: 300, t: "run list_dir / read_file", d: "the read-only limb, one directory", code: ["src/limb.rs", fnLine("src/limb.rs", "read_file"), 12] },
    sort: { x: 420, y: 230, w: 300, t: "sort the calls", d: "locals · refusals · at most one task", code: [A, at("Sort the turn's calls once"), 30] },
    result: { x: 800, y: 20, w: 330, t: "every child done → one tool result", d: "reports in declared order", code: ["src/framing.rs", fnLine("src/framing.rs", "scope_result"), 10] },
    scope: { x: 800, y: 230, w: 330, t: "task → open a scope", d: "the caller is suspended inside this call", code: [A, fnLine(A, "scope"), 30], hot: true },
    parse: { x: 800, y: 335, w: 330, t: "check the children", d: "names unique · after names exist · no cycles", code: [A, fnLine(A, "parse_children"), 70] },
    wait: { x: 800, y: 440, w: 330, t: "each child waits for its after-siblings", d: "their reports join its assignment", code: [A, at("receiver.wait_for"), 12] },
    cut: { x: 800, y: 545, w: 330, t: "build what the child is shown", d: "the cut: full · own · before · fresh", code: [A, fnLine(A, "child_context"), 38], hot: true },
    child: { x: 800, y: 650, w: 330, t: "run the child", d: "every child at once", code: [A, at("let end = run_agent(run.clone(), child_path"), 6], hot: true },
  };
  const H = 64;
  const W = 1190, HH = 760;
  const svg = s("svg", { width: W, height: HH, viewBox: `0 0 ${W} ${HH}` });
  svg.append(s("defs", {},
    s("marker", { id: "fa", viewBox: "0 0 10 10", refX: 9, refY: 5, markerWidth: 7, markerHeight: 7, orient: "auto" }, s("path", { d: "M0 0 L10 5 L0 10 z", fill: "#5a637b" })),
    s("marker", { id: "fi", viewBox: "0 0 10 10", refX: 9, refY: 5, markerWidth: 7, markerHeight: 7, orient: "auto" }, s("path", { d: "M0 0 L10 5 L0 10 z", fill: "var(--ice)" }))));
  const c = k => ({ x: N[k].x + N[k].w / 2, y: N[k].y + H / 2 });
  const arrow = (d, opts = {}) => svg.append(s("path", { d, fill: "none", stroke: opts.ice ? "var(--ice)" : opts.gold ? "var(--gold)" : "#4a5268", "stroke-width": opts.ice ? 1.8 : 1.4, "stroke-dasharray": opts.dash, "marker-end": opts.ice ? "url(#fi)" : "url(#fa)" }));
  const label = (x, y, t, colour, anchor) => svg.append(s("text", { x, y, class: "tape-label", fill: colour || "var(--mute)", "text-anchor": anchor || "middle" }, t));
  const bottom = k => N[k].y + H, right = k => N[k].x + N[k].w;
  arrow(`M${c("loop").x} ${bottom("loop")} V ${N.wire.y}`);
  arrow(`M${c("wire").x} ${bottom("wire")} V ${N.calls.y}`);
  arrow(`M${c("calls").x} ${bottom("calls")} V ${N.report.y}`); label(c("calls").x + 16, bottom("calls") + 36, "no");
  arrow(`M${right("calls")} ${c("calls").y} H ${N.sort.x}`); label((right("calls") + N.sort.x) / 2, c("calls").y - 8, "yes");
  arrow(`M${c("sort").x} ${N.sort.y} V ${bottom("limb")}`);
  arrow(`M${c("limb").x} ${N.limb.y} V ${bottom("append")}`);
  arrow(`M${N.append.x} ${c("append").y} H ${right("loop")}`); label((N.append.x + right("loop")) / 2, c("append").y - 8, "round again");
  arrow(`M${right("sort")} ${c("sort").y} H ${N.scope.x}`); label((right("sort") + N.scope.x) / 2, c("sort").y - 8, "task");
  arrow(`M${c("scope").x} ${bottom("scope")} V ${N.parse.y}`);
  arrow(`M${c("parse").x} ${bottom("parse")} V ${N.wait.y}`);
  arrow(`M${c("wait").x} ${bottom("wait")} V ${N.cut.y}`);
  arrow(`M${c("cut").x} ${bottom("cut")} V ${N.child.y}`);
  arrow(`M${right("child")} ${c("child").y} H ${W - 22} V ${c("result").y} H ${right("result")}`, { gold: true }); svg.append(s("text", { x: W - 10, y: 420, class: "tape-label", fill: "var(--gold)", transform: `rotate(90 ${W - 10} 420)`, "text-anchor": "middle" }, "all reported"));
  arrow(`M${N.result.x} ${c("result").y} H ${right("append")}`, { gold: true });
  // the recursion: every child runs the very same loop
  arrow(`M${N.child.x} ${c("child").y} H 18 V ${c("loop").y} H ${N.loop.x}`, { ice: true, dash: "6 4" });
  label(420, c("child").y - 8, "each child enters the same loop — one level down", "var(--ice)", "start");
  for (const [k, n] of Object.entries(N)) {
    const g = s("g", { class: "mod", style: { cursor: "pointer" } });
    const fill = n.hot ? "#132029" : n.end ? "#1d1a12" : "#121622";
    const stroke = n.hot ? "#3b6b78" : n.end ? "#6b5a2a" : "#2a3142";
    if (n.shape === "decision") g.append(s("path", { class: "box", d: `M${n.x + 20} ${n.y} H ${n.x + n.w - 20} L ${n.x + n.w} ${n.y + H / 2} L ${n.x + n.w - 20} ${n.y + H} H ${n.x + 20} L ${n.x} ${n.y + H / 2} Z`, fill, stroke }));
    else g.append(s("rect", { class: "box", x: n.x, y: n.y, width: n.w, height: H, rx: 12, fill, stroke }));
    g.append(s("text", { x: n.x + (n.shape ? n.w / 2 : 14), y: n.y + (n.d ? 26 : 37), "text-anchor": n.shape ? "middle" : "start", fill: n.end ? "var(--gold)" : "var(--bright)", "font-size": 13.5, "font-weight": 500 }, n.t));
    if (n.d) g.append(s("text", { x: n.x + 14, y: n.y + 46, class: "tape-label", fill: "#8f96aa" }, n.d));
    g.append(s("text", { x: n.x + n.w - 10, y: n.y + 14, "text-anchor": "end", "font-size": 9.5, "font-family": "var(--mono)", fill: "#4a5268" }, n.code[0].split("/").pop() + ":" + n.code[1]));
    g.addEventListener("click", () => openSource(n.code[0], n.code[1], n.code[1] + n.code[2]));
    tipOn(g, `${n.code[0]}:${n.code[1]}\nclick: the lines that do this`);
    svg.append(g);
  }
  const total = DATA.modules.reduce((a, m) => a + m.lines, 0);
  const tiles = h("div", { style: { display: "flex", gap: "8px", flexWrap: "wrap", marginTop: "10px" } }, [...DATA.modules].sort((a, b) => b.lines - a.lines).map(m => {
    const test = m.path.startsWith("tests") || m.path.includes("fake_provider");
    return tipOn(h("div", { onclick: () => openSource(m.path), style: { cursor: "pointer", flex: `${m.lines} 1 ${Math.max(120, (m.lines / total) * 1500)}px`, background: "#121622", borderRadius: "10px", padding: "10px 12px", boxShadow: `0 0 0 1px ${test ? "#2c4a37" : m.name === "agent" || m.name === "framing" ? "#355a66" : "var(--line)"} inset` } },
      h("div", { style: { display: "flex", justifyContent: "space-between", gap: "8px" } }, h("b", { class: "mono", style: { fontSize: "12.5px", color: "var(--bright)" } }, m.path.split("/").pop()), h("span", { class: "mono", style: { fontSize: "11px", color: "var(--mute)" } }, m.lines)),
      h("div", { style: { fontSize: "11.5px", color: "#9aa1b3", marginTop: "4px", lineHeight: 1.35 } }, m.name === "main" ? "the CLI: run, chat, bench, rescore" : m.doc)), (m.uses.length ? "uses " + m.uses.join(", ") + "\n" : "") + "click: the source");
  }));
  el.append(h("h1", {}, "One loop, ", h("em", {}, "re-entered")),
    h("p", { class: "lede" }, "Every agent — root, region, leaf — runs this loop. A task call opens a scope, and each child runs the same loop one level down; the parent's loop is paused inside that one step until all of them report. Click any step for the lines that do it."),
    h("div", { class: "machine", style: { overflowX: "auto" } }, svg),
    h("h2", {}, `the modules — ${fmt.int(total)} lines of Rust, sized by lines`), tiles);
});
