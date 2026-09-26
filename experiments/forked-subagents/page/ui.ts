// The few DOM helpers every section uses.

type Kid = Node | string | number | null | undefined | false | Kid[]
type Props = Record<string, unknown>

export function h(tag: string, props: Props = {}, ...kids: Kid[]): HTMLElement {
  const el = document.createElement(tag)
  for (const [k, v] of Object.entries(props)) {
    if (v == null || v === false) continue
    if (k.startsWith("on")) el.addEventListener(k.slice(2), v as EventListener)
    else if (k === "class") el.className = String(v)
    else if (k === "style") Object.assign(el.style, v)
    else el.setAttribute(k, v === true ? "" : String(v))
  }
  el.append(...flat(kids))
  return el
}

function flat(kids: Kid[]): (Node | string)[] {
  return kids.flatMap((k) => (Array.isArray(k) ? flat(k) : k == null || k === false ? [] : [k instanceof Node ? k : String(k)]))
}

export const usd = (x: number) => "$" + (x < 0.01 ? x.toFixed(4) : x.toFixed(3))
export const pct = (x: number) => Math.round(x * 100) + "%"
export const int = (n: number) => n.toLocaleString("en-NZ")

/** JSON, pretty-printed, with keys, strings and numbers told apart. */
export function json(value: unknown): HTMLElement {
  const text = JSON.stringify(value, null, 2)
  const pre = h("pre", { class: "json" })
  const token = /("(?:\\.|[^"\\])*")(\s*:)?|(-?\d+(?:\.\d+)?)/g
  let at = 0
  for (const m of text.matchAll(token)) {
    pre.append(text.slice(at, m.index))
    if (m[1]) pre.append(h("span", { class: m[2] ? "k" : "s" }, m[1]), m[2] ?? "")
    else pre.append(h("span", { class: "n" }, m[3]))
    at = m.index! + m[0].length
  }
  pre.append(text.slice(at))
  return pre
}

/** A span of real lines, numbered as they are in the file. */
export function code(ex: { path: string; from: number; lines: string[] }, caption?: string): HTMLElement {
  const rel = ex.path.slice(ex.path.indexOf("experiments/forked-subagents/") + "experiments/forked-subagents/".length)
  return h("div", { class: "code" },
    h("div", { class: "cap" }, caption ?? `${rel}:${ex.from}–${ex.from + ex.lines.length - 1}`),
    ex.lines.map((line, i) => h("div", { class: "l" }, h("span", {}, ex.from + i), h("span", {}, line || " "))))
}

/** Something to click that shows more in place, and hides it again. */
export function toggle(label: Kid, build: () => HTMLElement, host?: HTMLElement): HTMLElement {
  let shown: HTMLElement | null = null
  const link = h("span", { class: "linkish" }, label)
  link.addEventListener("click", (e) => {
    e.stopPropagation()
    if (shown) (shown.remove(), (shown = null))
    else (host ?? link.parentElement!).append((shown = build()))
  })
  return link
}

export function part(n: number, id: string, title: string, covers: string, ...body: Kid[]): HTMLElement {
  return h("section", { class: "part", id },
    h("header", {}, h("span", { class: "n" }, n), h("h2", {}, title), h("p", { class: "covers" }, covers)),
    ...flat(body))
}

export function cameTo(...text: Kid[]): HTMLElement {
  return h("div", { class: "came-to" }, h("b", {}, "What this came to"), h("div", {}, ...flat(text)))
}
