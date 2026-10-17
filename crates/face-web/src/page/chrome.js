import { $, clock, el, icon } from "./base.js";
import { act, digest } from "./tab.js";
import { byId, view } from "./map.js";
import { fit, setNewPoint, setUsageRight } from "./camera.js";
import { go, NEW } from "./card.js";

// ─── Telling Max when he is not looking ───────────────────────────────────
// When something newly waits on him and this tab is not in front, the
// system says so, through the page's service worker (`sw.js`): a phone's
// browser shows a notification from nowhere else. A click brings the tab
// forward with that chat open. The browser asks his permission at his first
// touch of the page.
let notified = null;
const worker = navigator.serviceWorker?.register("/page/sw.js").catch((e) => console.warn(`no notifications: ${e}`));
navigator.serviceWorker?.addEventListener("message", (e) => { if (byId[e.data?.open]) go(e.data.open); });
navigator.serviceWorker?.startMessages();
addEventListener("pointerdown", () => { if (window.Notification?.permission === "default") Notification.requestPermission(); }, { once: true });
export async function notify(queue) {
  const before = notified;
  notified = new Set(queue.map((q) => q.task));
  if (!before || !document.hidden || window.Notification?.permission !== "granted") return;
  const reg = await worker;
  for (const q of queue) {
    const t = byId[q.task];
    if (before.has(q.task) || !t || !reg) continue;
    reg.showNotification(t.name, { body: q.why || "Your turn", tag: t.id, data: t.id }).catch((e) => console.warn(`not notified: ${e}`));
  }
}

// ─── Usage ─────────────────────────────────────────────────────────────────
// The gauges are one button: a click asks the provider for a reading, a call
// Max makes like any other, settled in band. Hovering or focusing them shows
// each window and how fresh the numbers are.
const pct = (f) => f == null ? "?" : `${Math.round(f * 100)}%`;
const resets = (ms) => {
  if (ms == null) return "";
  const d = new Date(ms);
  return d.toDateString() === new Date().toDateString() ? `resets ${clock(ms)}` : `resets ${d.toLocaleDateString([], { weekday: "short" })} ${clock(ms)}`;
};
const windowName = (n) => ({ five_hour: "5-hour", seven_day: "Weekly", seven_day_opus: "Weekly Opus", seven_day_sonnet: "Weekly Sonnet" })[n] ?? n.replaceAll("_", " ");
// This tab's click until the view shows the call, and why the brain refused it.
let refreshAsked = false, refreshRefused = null;
async function refreshUsage() {
  const u = view.usage;
  if (refreshAsked || u?.refreshing || !view.provider) return;
  refreshAsked = true; refreshRefused = null; renderUsage();
  const why = await act((at) => ({ Call: [`${at}.0`, { Made: {
    caller: { Max: { limb: view.provider, chat: null } }, tool: "usage", input: digest("{}"), expects: [],
  } }] }), ["{}"]);
  refreshRefused = why && `Could not ask for a reading: ${why}`;
  refreshAsked = false; renderUsage();
}
// Made once, so a focused button keeps its focus, and its card stays open,
// as the view moves on.
const gauges = el("button", "gauges"), usageCard = el("div", "usage-card"), usageAlarms = el("span", "alarms");
gauges.onclick = refreshUsage;
$("#usage").append(gauges, usageCard, usageAlarms);
export function renderUsage() {
  const u = view.usage, span = $("#usage"), w = u?.windows || {};
  const busy = refreshAsked || u?.refreshing, failed = refreshRefused || u?.failed;
  gauges.replaceChildren();
  for (const [label, name] of [["5h", "five_hour"], ["7d", "seven_day"]]) {
    const f = w[name]?.used;
    const g = el("span", `gauge${f > .8 ? " high" : ""}`);
    const bar = el("i"); const fill = el("b"); fill.style.width = `${Math.round((f || 0) * 100)}%`; bar.append(fill);
    g.append(label, bar, el("span", "pc", pct(f)));
    gauges.append(g);
  }
  usageCard.replaceChildren();
  for (const [name, x] of Object.entries(w)) {
    const row = el("div", "row");
    row.append(el("span", "", windowName(name)), el("b", "", pct(x.used)), el("span", "", resets(x.resets)));
    usageCard.append(row);
  }
  const asOf = el("div", "as-of");
  asOf.append(el("span", busy || failed ? "stale" : "", u?.at ? `as of ${clock(u.at)}` : "no reading yet"), busy ? " · refreshing…" : " · click to refresh");
  usageCard.append(asOf);
  if (failed && !busy) usageCard.append(el("div", "failed", failed));
  span.classList.toggle("busy", !!busy);
  usageAlarms.replaceChildren();
  if (u && !u.on_plan) usageAlarms.append(el("span", "alarm", "billed outside the plan: nothing more is sent"));
  if (view.limited) usageAlarms.append(el("span", "alarm", `limited until ${clock(view.limited)} · carries on by itself`));
  setUsageRight(span.getBoundingClientRect().right);
}
const fitButton = icon("fit", "", "Fit the map (F)"), newButton = icon("plus", "", "New chat (or double-click the map)");
newButton.id = "new";
fitButton.onclick = () => fit(true);
newButton.onclick = () => { setNewPoint(null); go(NEW); };
$("#bottom").append(fitButton, newButton);

