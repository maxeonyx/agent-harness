import { el, reduced } from "./base.js";

// ─── Why a pan did or did not coast, on the screen ─────────────────────────
// For a phone with no devtools: the page opened once with `?glide` in its
// address, on this device, shows after each pan let go a line saying how it
// ended and why it coasted or not, until it is opened with `?glide=off`. The
// choice is kept in the origin's storage, so an installed app, whose address
// cannot be typed, shows it too once the browser has opened it.
const flag = new URLSearchParams(location.search).get("glide");
if (flag !== null) {
  if (flag === "off") localStorage.removeItem("glide");
  else localStorage.setItem("glide", "1");
}
const on = localStorage.getItem("glide") === "1";
let box = null, cutBy = null;

function show(line) {
  if (box === null) {
    box = el("pre", "glidelog");
    box.style.cssText = "position: fixed; left: 4px; right: 4px; top: max(4px, env(safe-area-inset-top)); z-index: 100; margin: 0; padding: 4px 6px; max-height: 40vh; overflow: hidden; font: 11px/1.35 monospace; white-space: pre-wrap; background: rgba(255, 255, 255, .88); color: #222; border-radius: 6px; pointer-events: none";
    document.body.append(box);
  }
  box.textContent = `${line}\n${box.textContent}`.split("\n").slice(0, 12).join("\n");
}

const ms = (t) => `${Math.round(t)}ms`;

// A pan let go: `end` is its last event, `trail` its samples from the last
// 100 ms, and `c` what `coast` made of them: why not (`no`), or how far and
// how long it goes and a promise of how it went.
export function glideNote(end, trail, c) {
  if (!on) return;
  const a = trail[0], b = trail.at(-1);
  const facts = [
    `${end.pointerType} ${end.type}`,
    `reduced ${reduced() ? "yes" : "no"}`,
    `${trail.length} samples over ${a && b ? ms(b.t - a.t) : "-"}`,
    `last→lift ${b ? ms(end.timeStamp - b.t) : "-"}`,
    `handled ${ms(performance.now() - end.timeStamp)} late`,
  ];
  if (c.v !== undefined) facts.push(`${c.v.toFixed(2)} px/ms`);
  if (c.no !== undefined) { show(`${facts.join(", ")}: no coast (${c.no})`); return; }
  facts.push(`coast ${Math.round(c.D)}px in ${ms(c.T)}`);
  cutBy = null;
  c.ran.then(({ frames, took }) => {
    const how = cutBy === null ? "ran out" : `cut short by ${cutBy}`;
    show(`${facts.join(", ")}: ${frames} frames over ${ms(took)}, ${how}`);
  });
}

// A coast stopped before it ran out: names what stopped it, from the stack.
export function glideCut() {
  if (!on) return;
  cutBy = (new Error().stack ?? "").split("\n").slice(2, 5).map((l) => l.trim().replace(/^at /, "").replace(/\(.*\/([^/]+)\)$/, "($1)")).join(" ← ");
}
