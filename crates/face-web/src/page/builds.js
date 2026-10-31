import { $, el } from "./base.js";
import { view } from "./map.js";

// ─── Which build runs ──────────────────────────────────────────────────────
// A quiet line beside the usage: the commit the brain runs, and while a
// relaunch is in play, how it ended: opened, refused at rehearsal, failed
// its gate, or went back, and why. Limbs still on the build before say so
// until they move. Hovering it shows all of that as a table.
const badge = el("span", "build"), commitEl = el("span"), nowEl = el("span", "now"), card = el("div", "pop");
badge.tabIndex = 0;
badge.append(commitEl, nowEl, card);
$("#corner").append(badge);
export function renderBuilds() {
  const r = view.relaunch;
  const old = view.superseded || [];
  const rows = [["Build", view.source || "Unknown"]];
  let now = "";
  if (r?.stage === "under way") now = "Relaunching…";
  else if (r) { now = r.stage[0].toUpperCase() + r.stage.slice(1); rows.push([now, r.why || ""]); }
  if (old.length) rows.push(["On the old build", old.join(", ")]);
  commitEl.textContent = (view.source || "").replace(/^(\w{7})\w*/, "$1");
  nowEl.textContent = old.length && !now ? `${old.length} limb${old.length > 1 ? "s" : ""} on the old build` : now;
  badge.classList.toggle("back", !!r && r.stage !== "under way" && r.stage !== "opened");
  const key = JSON.stringify(rows);
  if (card.dataset.key === key) return;
  card.dataset.key = key;
  const table = el("table");
  for (const [k, x] of rows) { const tr = el("tr"); tr.append(el("td", null, k), el("td", null, x)); table.append(tr); }
  card.replaceChildren(table);
}
