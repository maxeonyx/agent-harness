// One request as the model received it: every message and every block, in order, the new ones in
// full. Blocks another request already sent are folded away. Click anything for its exact JSON.

import type { Request } from "./data"
import { h, json, int } from "./ui"

type Block = Record<string, any>

const blocks = (m: any): Block[] => (typeof m.content === "string" ? [{ type: "text", text: m.content }] : m.content)

/** How many of `req`'s blocks, counted from the start, `against` sent too, byte for byte. */
function sharedPrefix(req: Request, against: Request): number {
  const flat = (r: Request) => r.messages.flatMap((m: any, i: number) => blocks(m).map((b) => `${i}:${m.role}:${JSON.stringify(b)}`))
  const mine = flat(req), theirs = flat(against)
  let n = 0
  while (n < mine.length && mine[n] === theirs[n]) n++
  return n
}

function text(b: Block): string {
  switch (b.type) {
    case "text": return b.text
    case "tool_use": return `${b.name} ${JSON.stringify(b.input, null, 1)}`
    case "tool_result": return typeof b.content === "string" ? b.content : JSON.stringify(b.content)
    case "thinking": return b.thinking === "" ? "(empty, with a signature)" : b.thinking
    default: return JSON.stringify(b)
  }
}

const tag = (b: Block) => (b.type === "tool_use" ? `tool_use ${b.name}` : b.type === "tool_result" ? `tool_result ${b.tool_use_id.slice(-6)}` : b.type)
const oneLine = (s: string) => (s.length > 110 ? s.slice(0, 110).replace(/\s+/g, " ") + "…" : s.replace(/\s+/g, " "))

/** A row that shows `detail` under itself when clicked, and hides it again. */
function opens(row: HTMLElement, detail: () => HTMLElement): HTMLElement {
  const wrap = h("div", {}, row)
  let shown: HTMLElement | null = null
  row.classList.add("click")
  row.addEventListener("click", (e) => {
    e.stopPropagation()
    if (shown !== null) (shown.remove(), (shown = null))
    else wrap.append((shown = h("div", { class: "ctx-json" }, detail())))
  })
  return wrap
}

function block(b: Block, full: boolean): HTMLElement {
  const body = text(b)
  const row = h("div", { class: `ctx-block ${b.type}${full ? "" : " old"}` },
    h("span", { class: "ctx-tag" }, tag(b)),
    full ? h("pre", { class: "ctx-text" }, body) : h("span", { class: "ctx-line" }, oneLine(body)),
    b.cache_control !== undefined && h("span", { class: "bp" }, "◆ cache breakpoint"))
  return opens(row, () => json(b))
}

export type Options = {
  /** Fold the blocks this request shares with that one, and say so. */
  against?: Request
  /** What to call `against` when saying so. */
  as?: string
  /** Show only messages from this index on; the ones before are summarised in a line. */
  from?: number
}

export function context(req: Request, options: Options = {}): HTMLElement {
  const shared = options.against === undefined ? 0 : sharedPrefix(req, options.against)
  const as = options.as ?? (options.against && `${options.against.agent}'s request ${options.against.n + 1}`)
  const from = options.from ?? 0
  let seen = 0
  const rows: HTMLElement[] = []
  let folded: number[] = []
  const fold = () => {
    if (folded.length === 0) return
    const [a, b] = [folded[0], folded[folded.length - 1]]
    const msgs = req.messages.slice(a, b + 1)
    rows.push(opens(h("div", { class: "ctx-fold" }, h("span", { class: "ctx-n" }, a === b ? a : `${a}–${b}`), `${a === b ? "a message" : `${b - a + 1} messages`}, the same bytes as in ${as}`), () => h("div", {}, msgs.map((m: any, i: number) => message(m, a + i, 0)))))
    folded = []
  }
  const message = (m: any, i: number, oldBlocks: number) =>
    h("div", { class: "ctx-msg" },
      opens(h("div", { class: "ctx-head" }, h("span", { class: "ctx-n" }, i), h("span", { class: `role ${m.role}` }, m.role)), () => json(m)),
      blocks(m).map((b, j) => block(b, j >= oldBlocks)))

  if (from > 0) rows.push(h("div", { class: "ctx-fold" }, h("span", { class: "ctx-n" }, from === 1 ? 0 : `0–${from - 1}`), "earlier messages, not shown"))
  req.messages.forEach((m: any, i: number) => {
    const n = blocks(m).length
    const old = Math.max(0, Math.min(n, shared - seen))
    seen += n
    if (i < from) return
    if (old === n && options.against !== undefined) return void folded.push(i)
    fold()
    rows.push(message(m, i, old))
  })
  fold()

  const u = req.usage
  return h("div", { class: "ctx" },
    h("div", { class: "ctx-title" },
      h("b", { class: "mono" }, req.agent), `, request ${req.n + 1} of ${req.of}`,
      u !== null && h("span", { class: "tok" }, "  read ", h("span", { class: "c" }, int(u.cached)), " from cache · wrote ", h("span", { class: "w" }, int(u.written)), ` · ${int(u.uncached)} plain · `, h("span", { class: "o" }, `${int(u.out)} out`))),
    opens(h("div", { class: "ctx-fold" }, h("span", { class: "ctx-n" }, "sys"), `system prompt and tools (${req.tools.join(", ")}): the same in every request of the run`), () => json({ ...req.envelope, tools: req.tools })),
    rows,
    req.envelope.cache_control !== undefined && h("div", { class: "ctx-end" }, h("span", { class: "bp" }, "◆"), " end of the request: the body's own ", h("code", {}, "cache_control"), " caches through its last block"),
    req.reply !== null && opens(h("div", { class: "ctx-reply" }, h("span", { class: "ctx-tag" }, "reply"), req.reply.filter((b: Block) => b.type !== "thinking").map((b: Block) => h("span", { class: "ctx-line" }, `${tag(b)}: ${oneLine(text(b))}`))), () => json(req.reply)))
}
