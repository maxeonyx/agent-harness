import { $, clock, el, fail, icon } from "./base.js";
import { act, digest } from "./tab.js";
import { byId, view } from "./map.js";
import { fit, setNewPoint, setUsageRight } from "./camera.js";
import { go, NEW } from "./card.js";
import { startFind } from "./newchat.js";

// ─── Telling Max when he is not looking ───────────────────────────────────
// When something newly waits on him and this tab is not in front, the
// system says so, through the page's service worker (`sw.js`): a phone's
// browser shows a notification from nowhere else. A click brings the tab
// forward with that chat open. The browser asks his permission at his first
// touch of the page.
let notified = null;
const worker = navigator.serviceWorker?.register("/page/sw.js").catch((e) => fail("registering the service worker that shows notifications", e));
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
    reg.showNotification(t.name, { body: q.why || "Your turn", tag: t.id, data: t.id }).catch((e) => fail(`notifying that ${t.name} waits on Max`, e));
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
  return d.toDateString() === new Date().toDateString() ? `Resets ${clock(ms)}` : `Resets ${d.toLocaleDateString([], { weekday: "short" })} ${clock(ms)}`;
};
const windowName = (n) => ({ five_hour: "5-hour", seven_day: "Weekly", seven_day_opus: "Weekly Opus", seven_day_sonnet: "Weekly Sonnet" })[n] ?? n.replaceAll("_", " ");
// This tab's click until the view shows the call, and why the brain refused it.
let refreshAsked = false, refreshRefused = null;
async function refreshUsage() {
  const u = view.usage;
  if (refreshAsked || u?.refreshing || !view.brain) return;
  refreshAsked = true; refreshRefused = null; renderUsage();
  const why = await act((at) => ({ Call: [`${at}.0`, { Made: {
    caller: { Max: { limb: view.brain, chat: null } }, tool: "usage", input: digest("{}"), expects: [],
  } }] }), ["{}"]);
  refreshRefused = why && `Could not ask for a reading: ${why}`;
  refreshAsked = false; renderUsage();
}
// Made once, so a focused button keeps its focus, and its card stays open,
// as the view moves on.
const gauges = el("button", "gauges"), usageCard = el("div", "pop"), usageAlarms = el("span", "alarms");
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
  const table = el("table");
  for (const [name, x] of Object.entries(w)) {
    const tr = el("tr");
    tr.append(el("td", "", windowName(name)), el("td", "n", pct(x.used)), el("td", "", resets(x.resets)));
    table.append(tr);
  }
  const foot = el("div", "foot");
  foot.append(el("span", busy || failed ? "stale" : "", u?.at ? `As of ${clock(u.at)}` : "No reading yet"), busy ? "Refreshing…" : "Click to refresh");
  usageCard.replaceChildren(table, foot);
  if (failed && !busy) usageCard.append(el("div", "failed", failed));
  span.classList.toggle("busy", !!busy);
  usageAlarms.replaceChildren();
  if (view.limited) usageAlarms.append(el("span", "alarm", `Limited until ${clock(view.limited)}, then carries on by itself`));
  if (view.signedOut) usageAlarms.append(signIn);
  setUsageRight(span.getBoundingClientRect().right);
}
// Signing the harness in to claude.ai, when it is not: every task waits
// until it is. One click on Sign in opens `/login` in a new tab, which
// begins a sign-in and goes on to claude.ai, and at the same time shows a
// box here for the code claude.ai shows; pasting it finishes the sign-in.
const signIn = el("span", "alarm sign-in", "Not signed in to claude.ai, so work waits ");
const signInLink = el("a", "button", "Sign in");
signInLink.href = "/login";
signInLink.target = "_blank";
signInLink.rel = "noopener";
signIn.append(signInLink);
const codeBox = el("dialog", "sign-in-box");
const codeForm = el("form");
const codeInput = el("input");
codeInput.placeholder = "Paste the code";
codeInput.autocomplete = "off";
const codeSaid = el("div", "said");
const codeCancel = el("button", "", "Cancel");
codeCancel.type = "button";
codeCancel.onclick = () => codeBox.close();
const codeActions = el("div", "actions");
codeActions.append(codeCancel, el("button", "", "Sign in"));
codeForm.append(el("b", "", "Sign in to claude.ai"), el("p", "", "Approve the sign-in in the tab that opened, then paste the code it shows here."), codeInput, codeSaid, codeActions);
codeBox.append(codeForm);
document.body.append(codeBox);
signInLink.onclick = () => {
  codeInput.value = "";
  codeSaid.textContent = "";
  codeBox.showModal();
};
async function finishSignIn() {
  const code = codeInput.value.trim();
  if (!code) return;
  codeSaid.textContent = "Signing in…";
  const r = await fetch(`/login?code=${encodeURIComponent(code)}`).catch((e) => e);
  const said = r instanceof Response ? await r.text() : String(r);
  if (r.ok) return codeBox.close();
  codeSaid.textContent = said;
}
codeForm.onsubmit = (e) => { e.preventDefault(); finishSignIn(); };
codeInput.addEventListener("input", (e) => { if (e.inputType === "insertFromPaste") finishSignIn(); });
const findButton = icon("find", "", "Find a chat (Ctrl+K or /)");
const fitButton = icon("fit", "", "Fit the map (F)"), newButton = icon("plus", "", "New chat (or double-click the map)");
newButton.id = "new";
fitButton.onclick = () => fit(true);
findButton.onclick = startFind;
newButton.onclick = () => { setNewPoint(null); go(NEW); };
$("#bottom").append(findButton, fitButton, newButton);

