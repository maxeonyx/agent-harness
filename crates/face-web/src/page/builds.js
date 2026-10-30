import { $, el } from "./base.js";
import { view } from "./map.js";

// ─── Which build runs ──────────────────────────────────────────────────────
// A quiet line beside the usage: the commit the brain runs, and while a
// relaunch is in play, how far it has got: under way, took over and on
// probation (a response, then a round trip through a limb), proven, or
// gone back and why. Limbs still on the build before say so until they
// move. Hovering it shows all of that as a table.
const badge = el("span", "build"), commitEl = el("span"), nowEl = el("span", "now"), card = el("div", "pop");
badge.tabIndex = 0;
badge.append(commitEl, nowEl, card);
$("#corner").append(badge);
const tick = (done) => done ? "✓" : "Not yet";
export function renderBuilds() {
  const r = view.relaunch, v = view.probation || {};
  const old = view.superseded || [];
  const rows = [["Build", view.source || "Unknown"]];
  let now = "";
  if (r?.stage === "under way") now = "Relaunching…";
  else if (r?.stage === "went back") { now = "Went back"; rows.push(["Went back", r.why]); }
  else if (r?.stage === "took over" && v.Proven) now = "Proven ✓";
  else if (r?.stage === "took over" && v.Refused) { now = "Refused"; rows.push(["Refused", v.Refused.message]); }
  else if (r?.stage === "took over") { now = "On probation"; rows.push(["Response", tick(v.Proving?.responded)], ["Round trip", tick(v.Proving?.round_trip)]); }
  if (old.length) rows.push(["On the old build", old.join(", ")]);
  commitEl.textContent = (view.source || "").replace(/^(\w{7})\w*/, "$1");
  nowEl.textContent = old.length && !now ? `${old.length} limb${old.length > 1 ? "s" : ""} on the old build` : now;
  badge.classList.toggle("back", r?.stage === "went back" || !!(r?.stage === "took over" && v.Refused));
  const key = JSON.stringify(rows);
  if (card.dataset.key === key) return;
  card.dataset.key = key;
  const table = el("table");
  for (const [k, x] of rows) { const tr = el("tr"); tr.append(el("td", null, k), el("td", null, x)); table.append(tr); }
  card.replaceChildren(table);
}
