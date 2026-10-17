// ─── Telling Max when he is not looking ───────────────────────────────────
// When something newly waits on him and this tab is not in front, the
// system says so, and a click brings the tab forward with it open. The
// browser asks his permission at his first touch of the page.
let notified = null;
addEventListener("pointerdown", () => { if (window.Notification?.permission === "default") Notification.requestPermission(); }, { once: true });
function notify(queue) {
  const before = notified;
  notified = new Set(queue.map((q) => q.task));
  if (!before || !document.hidden || window.Notification?.permission !== "granted") return;
  for (const q of queue) {
    const t = byId[q.task];
    if (before.has(q.task) || !t) continue;
    try {
      const n = new Notification(t.name, { body: "Your turn", tag: t.id });
      n.onclick = () => { focus(); n.close(); go(t.id); };
    } catch { /* a phone's browser notifies only from a service worker */ }
  }
}

// ─── Usage ─────────────────────────────────────────────────────────────────
const pct = (f) => f == null ? "?" : `${Math.round(f * 100)}%`;
const resets = (ms) => {
  if (ms == null) return "";
  const d = new Date(ms);
  return d.toDateString() === new Date().toDateString() ? `resets ${clock(ms)}` : `resets ${d.toLocaleDateString([], { weekday: "short" })} ${clock(ms)}`;
};
function renderUsage() {
  const u = view.usage, span = $("#usage");
  span.replaceChildren();
  if (u) for (const [label, w] of [["5h", u.five_hour], ["7d", u.seven_day]]) {
    const f = w?.used, r = w?.resets;
    const g = el("span", `gauge${f > .8 ? " high" : ""}`);
    const bar = el("i"); const fill = el("b"); fill.style.width = `${Math.round((f || 0) * 100)}%`; bar.append(fill);
    g.append(label, bar, el("span", "pc", pct(f)));
    g.title = `${label}: ${pct(f)} used · ${resets(r)} · as of ${clock(u.at)}`;
    span.append(g);
  }
  if (u && !u.on_plan) span.append(el("span", "alarm", "billed outside the plan: nothing more is sent"));
  if (view.limited) span.append(el("span", "alarm", `limited until ${clock(view.limited)}`));
  usageRight = span.getBoundingClientRect().right;
}
const fitButton = icon("fit", "", "Fit the map (F)"), newButton = icon("plus", "", "New chat (or double-click the map)");
newButton.id = "new";
fitButton.onclick = () => fit(true);
newButton.onclick = () => { newPoint = null; go(NEW); };
$("#bottom").append(fitButton, newButton);

