// Part 3: the code as nested boxes. An arrow from A to B means: delete B, and A no longer compiles —
// measured by deleting every item in turn and asking the compiler.

import { graph, type Node, type Edge } from "underview/graph"
import { deps, source } from "./data" with { type: "macro" }
import { h, code, part, cameTo } from "./ui"

export const DEPS = deps()
export const SOURCES: Record<string, string> = {
  "src/main.rs": source("src/main.rs"), "src/session.rs": source("src/session.rs"), "src/agent.rs": source("src/agent.rs"),
  "src/framing.rs": source("src/framing.rs"), "src/wire.rs": source("src/wire.rs"), "src/limb.rs": source("src/limb.rs"),
  "src/record.rs": source("src/record.rs"), "src/face.rs": source("src/face.rs"), "src/bench.rs": source("src/bench.rs"),
  "src/rescore.rs": source("src/rescore.rs"), "src/bin/fake_provider.rs": source("src/bin/fake_provider.rs"), "tests/scenario.rs": source("tests/scenario.rs"),
}

type Item = (typeof DEPS.items)[number]
const FILES = Object.keys(SOURCES)
const FOLDERS: [string, string[]][] = [["src/", FILES.filter((f) => /^src\/[^/]+$/.test(f))], ["src/bin/", ["src/bin/fake_provider.rs"]], ["tests/", ["tests/scenario.rs"]]]
const itemsOf = (file: string) => DEPS.items.filter((i: Item) => i.file === file)
const fileOf = (id: string) => id.split("::")[0]
const dependents = new Map<string, number>()
for (const e of DEPS.edges) dependents.set(e.to, (dependents.get(e.to) ?? 0) + 1)
const lines = (file: string) => SOURCES[file].split("\n").length

export function excerptOf(item: Item) {
  const all = SOURCES[item.file].split("\n")
  return { path: "experiments/forked-subagents/" + item.file, from: item.start, lines: all.slice(item.start - 1, item.end) }
}

const NAME = (item: Item) => (item.kind === "fn" || item.kind === "method" || item.kind === "test" ? item.name + "()" : item.name)

/** The folders and files, and the work outside them. Click a file to draw its insides below. */
function fileMap(onOpen: (file: string) => void): HTMLElement {
  const host = h("div", { class: "graph" })
  const fileBox = (file: string): Node => {
    const el = h("div", { class: "node file click" }, file.split("/").pop(), h("span", { class: "lines" }, lines(file)))
    el.addEventListener("click", () => {
      host.querySelectorAll(".node.file.hot").forEach((n) => n.classList.remove("hot"))
      el.classList.add("hot")
      onOpen(file)
    })
    return { id: file, el }
  }
  const folder = (name: string, files: string[]): Node => ({ id: "dir:" + name, el: h("div", { class: "node group" }, h("div", { class: "gh" }, name)), children: files.map(fileBox) })
  const root: Node = {
    id: "dir:root",
    el: h("div", { class: "node group" }, h("div", { class: "gh" }, "agent-harness/experiments/forked-subagents/")),
    children: [...FOLDERS.map(([n, f]) => folder(n, f)), { id: "probe", el: h("div", { class: "node file" }, "probe/probe.py", h("span", { class: "lines" }, "Python, no deps")) }],
  }
  const external: Node[] = [
    { id: "ext:skeleton", el: h("div", { class: "node ext" }, h("div", { class: "t" }, "experiments/walking-skeleton/"), h("div", { class: "s" }, "before: patterns borrowed, no code shared")) },
    { id: "ext:core", el: h("div", { class: "node ext" }, h("div", { class: "t" }, "src/ — the harness core"), h("div", { class: "s" }, "after: to be rebuilt fresh from this evidence")) },
    { id: "ext:probe", el: h("div", { class: "node ext" }, h("div", { class: "t" }, "provider-cache-probe, PR #10"), h("div", { class: "s" }, "after: first-party cache facts")) },
  ]
  const across = new Map<string, [string, string]>()
  for (const e of DEPS.edges) if (fileOf(e.from) !== fileOf(e.to)) across.set(fileOf(e.from) + ">" + fileOf(e.to), [fileOf(e.from), fileOf(e.to)])
  const edges: Edge[] = [
    ...[...across.values()].map(([from, to]) => ({ from, to })),
    { from: "tests/scenario.rs", to: "src/main.rs", class: "dashed" },
    { from: "tests/scenario.rs", to: "src/bin/fake_provider.rs", class: "dashed" },
    { from: "dir:root", to: "ext:skeleton", class: "dashed" },
    { from: "ext:core", to: "dir:root", class: "dashed" },
    { from: "ext:core", to: "ext:probe", class: "dashed" },
  ]
  queueMicrotask(() => graph(host, [root, ...external], edges, { direction: "DOWN", gap: 16, elk: { "elk.layered.considerModelOrder.strategy": "NODES_AND_EDGES" } }))
  return host
}

/** One file's items, nested in their types, with the arrows between them. What crosses the file's border is listed on each item. */
function inside(file: string): HTMLElement {
  const host = h("div", { class: "graph" })
  const mine = itemsOf(file)
  const ids = new Set(mine.map((i: Item) => i.id))
  let focus: string | null = null
  const outside = (item: Item) => {
    const needs = new Set(DEPS.edges.filter((e: any) => e.from === item.id && !ids.has(e.to)).map((e: any) => fileOf(e.to).split("/").pop()))
    const needed = new Set(DEPS.edges.filter((e: any) => e.to === item.id && !ids.has(e.from)).map((e: any) => fileOf(e.from).split("/").pop()))
    return [needed.size ? h("span", { class: "x up", title: "files that break without it: " + [...needed].join(", ") }, "↑" + [...needed].join(" ")) : null,
      needs.size ? h("span", { class: "x down", title: "files it breaks without: " + [...needs].join(", ") }, "↓" + [...needs].join(" ")) : null]
  }

  const box = (item: Item): Node => {
    const type = item.kind === "struct" || item.kind === "enum"
    const used = dependents.get(item.id) ?? 0
    const el = h("div", { class: `node item click${type ? " type" : ""}${item.kind === "test" ? " test" : ""}${used ? "" : " dead"}` }, h("span", {}, NAME(item)), outside(item))
    let shown: HTMLElement | null = null
    el.addEventListener("click", (e) => {
      e.stopPropagation()
      focus = shown ? null : item.id
      if (shown) (shown.remove(), (shown = null))
      else el.append((shown = h("div", { class: "detail" }, relations(item), code(excerptOf(item)))))
      el.classList.toggle("open", !!shown)
      highlight()
    })
    return { id: item.id, el }
  }
  // A type that has methods becomes a box holding them; the type itself is the box's heading.
  const owners = new Set(mine.filter((i: Item) => i.kind === "method").map((i: Item) => i.owner))
  const heads = new Map<string, string>()
  const nodes: Node[] = []
  for (const owner of owners) {
    const head = mine.find((i: Item) => i.name === owner && i.kind !== "method")
    const id = head ? head.id : `${file}::impl ${owner}`
    const heading = head ? box(head).el : h("div", { class: "gh" }, `impl ${owner}`)
    heading.classList.add("heading")
    nodes.push({ id, el: h("div", { class: "node group" }, heading), children: mine.filter((i: Item) => i.kind === "method" && i.owner === owner).map(box) })
    if (head) heads.set(head.id, id)
  }
  for (const i of mine) if (i.kind !== "method" && !owners.has(i.name)) nodes.push(box(i))
  const pairs = new Map<string, [string, string]>()
  for (const e of DEPS.edges) if (ids.has(e.from) && ids.has(e.to) && e.from !== e.to) pairs.set(e.from + ">" + e.to, [e.from, e.to])
  // A method needing its own type is what nesting already says.
  const within = [...pairs.values()].filter(([a, b]) => !(DEPS.items.find((i: Item) => i.id === a)?.owner && heads.get(b) === b && a.startsWith(b + "::")))
  const edges = reduce(within).map(([from, to]) => ({ from, to }))

  const highlight = () => {
    for (const g of host.querySelectorAll<SVGGElement>(".edges .edge")) {
      const on = focus && (g.dataset.from === focus || g.dataset.to === focus)
      g.classList.toggle("hot", !!on)
      g.classList.toggle("dim", !!focus && !on)
    }
  }
  queueMicrotask(() => graph(host, nodes, edges, { direction: "DOWN", gap: 14 }).ready.then(highlight))
  return h("div", {},
    h("h3", {}, `Inside ${file}`),
    h("p", { class: "s" }, `${mine.length} items. `, h("span", { class: "x up" }, "↑ file"), " — that file breaks without this item; ", h("span", { class: "x down" }, "↓ file"), " — this item breaks without that file. Click an item for every item on both sides, and its lines."),
    host)
}

export function codeMap(): HTMLElement {
  const below = h("div")
  const open = (file: string) => below.replaceChildren(inside(file))
  queueMicrotask(() => open("src/agent.rs"))
  return h("div", {}, fileMap(open), below)
}

export function codePart(): HTMLElement {
  const attributed = DEPS.edges.length
  return part(3, "code", "The code",
    "Every file of the experiment as a box inside its folder, with the work before and after it outside. Open a file to see its types — each holding its methods — and its functions; open one of those for its lines.",
    h("p", {}, "An arrow from A to B means ", h("b", {}, "if B were deleted, A would stop working"), `. That was measured, not read: each of the ${DEPS.items.length} items in turn was blanked out of a copy of the crate and `, h("code", {}, "cargo check --all-targets"), ` asked what broke; every error became an arrow from the item it landed in to the item deleted — ${attributed} arrows in all. An arrow a longer path already implies is left out of the picture; open an item to see every one of its own. The tests drive the `, h("code", {}, "forks"), " binary and the fake provider as processes rather than importing them, so those arrows are dashed."),
    h("div", { class: "legend" },
      h("span", {}, h("i", { class: "sw" }), "file, or fn"), h("span", {}, h("i", { class: "sw", style: { background: "#eef5f9", borderColor: "#b9d6e4" } }), "type"),
      h("span", {}, h("i", { class: "sw", style: { background: "#eef7f1", borderColor: "#b8dcc6" } }), "test"),
      h("span", {}, h("i", { class: "sw", style: { borderStyle: "dotted" } }), "nothing breaks if it's deleted"),
      h("span", {}, h("i", { class: "sw", style: { borderStyle: "dashed" } }), "outside the experiment"),
      h("span", {}, h("i", { class: "ln", style: { color: "#9aa0ad" } }), "breaks if deleted"), h("span", {}, h("i", { class: "ln dashed", style: { color: "#a4a8b3" } }), "drives, or informs")),
    codeMap(),
    cameTo(...summary()))
}

// What the file-level picture says, computed from the same edges so it cannot drift from them.
function summary() {
  const across = new Map<string, Set<string>>()
  for (const e of DEPS.edges) {
    const a = fileOf(e.from), b = fileOf(e.to)
    if (a !== b) across.set(a, (across.get(a) ?? new Set()).add(b))
  }
  const name = (f: string) => h("code", {}, f.split("/").pop()!)
  const list = (fs: string[]) => fs.flatMap((f, i) => [i ? (i === fs.length - 1 ? " and " : ", ") : "", name(f)])
  const needAgent = FILES.filter((f) => across.get(f)?.has("src/agent.rs"))
  const agentNeeds = [...(across.get("src/agent.rs") ?? [])]
  const standalone = FILES.filter((f) => !across.has(f))
  return [name("src/agent.rs"), " is the centre: ", ...list(needAgent), " break without it, and it breaks without ", ...list(agentNeeds), ". ", ...list(standalone), " depend on no other file — the fake provider and the scenarios among them, because they talk to the binary over HTTP and stdio, not through its code."]
}

/** Drops an arrow when a longer path already says it: if A needs C and C needs B, deleting B breaks A anyway. */
function reduce(pairs: [string, string][]): [string, string][] {
  const out = new Map<string, string[]>()
  for (const [a, b] of pairs) out.set(a, [...(out.get(a) ?? []), b])
  const reachesWithout = (from: string, to: string) => {
    const seen = new Set([from])
    const queue = (out.get(from) ?? []).filter((n) => n !== to)
    while (queue.length) {
      const n = queue.shift()!
      if (n === to) return true
      if (seen.has(n)) continue
      seen.add(n)
      queue.push(...(out.get(n) ?? []))
    }
    return false
  }
  return pairs.filter(([a, b]) => !reachesWithout(a, b))
}

/** Every item this one breaks without, and every item that breaks without it — unreduced, straight from the compiler. */
function relations(item: Item): HTMLElement {
  const name = (id: string) => id.split("::").slice(1).join("::") + (fileOf(id) === item.file ? "" : ` (${fileOf(id).split("/").pop()})`)
  const needs = DEPS.edges.filter((e: any) => e.from === item.id).map((e: any) => name(e.to))
  const needed = DEPS.edges.filter((e: any) => e.to === item.id).map((e: any) => name(e.from))
  return h("div", { class: "s", style: { margin: "4px 0 6px" } },
    h("div", {}, h("b", {}, "breaks without: "), needs.length ? needs.join(", ") : "nothing else"),
    h("div", {}, h("b", {}, "delete it and these break: "), needed.length ? needed.join(", ") : "nothing"),
    item.doc && h("div", { style: { marginTop: "4px" } }, item.doc))
}
