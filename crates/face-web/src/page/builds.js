import { $, el } from "./base.js";
import { view } from "./map.js";

// ─── Which build runs ──────────────────────────────────────────────────────
// A quiet line beside the usage: the commit the brain runs, and while a
// relaunch is in play, how far it has got: under way, took over and on
// probation (a response, then a round trip through a limb), proven, or
// gone back and why. Limbs still on the build before say so until they
// move.
const badge = el("span", "build");
$("#usage").after(badge);
const tick = (done, what) => `${what} ${done ? "✓" : "·"}`;
export function renderBuilds() {
  const commit = (view.source || "").replace(/^(\w{7})\w*/, "$1");
  const r = view.relaunch, v = view.probation || {};
  const parts = [commit];
  let why = "";
  if (r?.stage === "under way") parts.push("relaunching…");
  else if (r?.stage === "went back") { parts.push(`went back: ${r.why}`); why = r.why; }
  else if (r?.stage === "took over" && v.Proven) parts.push("proven ✓");
  else if (r?.stage === "took over" && v.Refused) parts.push(`refused: ${v.Refused.message}`);
  else if (r?.stage === "took over") parts.push(`probation: ${tick(v.Proving?.responded, "response")} ${tick(v.Proving?.round_trip, "round trip")}`);
  const old = view.superseded || [];
  if (old.length) parts.push(`${old.length} limb${old.length > 1 ? "s" : ""} on the old build`);
  badge.textContent = parts.join(" · ");
  badge.title = [`Running ${view.source || "an unknown build"}`, why, old.length ? `On the old build until idle: ${old.join(", ")}` : ""].filter(Boolean).join("\n");
  badge.classList.toggle("back", r?.stage === "went back" || !!(r?.stage === "took over" && v.Refused));
}
