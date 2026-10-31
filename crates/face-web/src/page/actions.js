import { el, icon } from "./base.js";
import { act } from "./tab.js";
import { heldBy, isDone } from "./map.js";

// ─── What Max can do to a task ─────────────────────────────────────────────
// One list per task, in the order it shows: the card's controls, the hover
// preview and the right-click menu all draw from it. Each is the facts it
// writes, and whether it asks once more first.
//
// The stop slot holds one control at a time: Pause while its loop would run
// on its own (one press, at once: nothing new starts); once it is paused,
// Stop, which parks it: each agent under it in the middle of its own work
// takes one turn to bring it to a safe point, then waits. Resume comes
// before it, so a second press where Pause was is Stop. Abandon stops it
// and what it started; while that tidies up, End now ends it at once. Done,
// in a chat, has it write its result.
export function actionsOf(t) {
  if (!t || isDone(t)) return [];
  const out = [], holder = heldBy(t), stopping = !!t.stop;
  if (!stopping && holder) out.push(
    { icon: "play", label: "Resume", title: holder === t ? "It and everything under it carry on" : `Paused with ${holder.name}: it and everything under it carry on`, facts: () => ({ Task: [holder.id, { Pause: "Off" }] }) },
    ...t.pause?.how !== "now" ? [] : [{ icon: "stop", label: "Stop", title: "Pause for now: each agent in the middle of its work brings it to a safe point, says where it stands, and waits", facts: () => stopFacts(t) }]
  );
  else if (!stopping && runsOnItsOwn(t)) out.push({ icon: "pause", label: "Pause", title: "Nothing new starts in it or under it: what is running finishes, then waits", facts: () => ({ Task: [t.id, { Pause: "Now" }] }) });
  if (!stopping) out.push({ icon: "cancel", label: "Abandon", confirm: true, title: "The work is no longer needed: it and everything it started stop, tidy up and report", facts: () => ({ Task: [t.id, { Drain: { Abandon: { by: "Max", within: 120000 } } }] }) });
  else if (!t.stop.hard) out.push({ icon: "cancel", label: "End now", confirm: true, title: "End it and everything it started now, with no tidy-up", facts: () => ({ Task: [t.id, { Drain: { Abandon: { by: "Max", within: 0 } } }] }) });
  if (t.mode === "user-facing" && !stopping) out.push({ icon: "tick", label: "Done", confirm: true, title: "It writes its result and finishes", facts: (at) => ({ Delivery: { id: `${at}.0`, to: t.id, content: { Prompt: "Done" } } }) });
  return out;
}
// What Max can do to background work: stop it. What an agent passed to him
// is his to end; an agent's own he can stop too, and the agent is told.
export function callActionsOf(call) {
  if (call.status === "settled" || call.abandoned) return [];
  const yours = call.owner === "max";
  return [{ icon: "stop", label: "Stop", confirm: !yours, title: yours ? "Stop it: it was passed to you, so it is yours to end" : "Stop it: the agent that owns it gets its outcome", facts: () => ({ Call: [call.id, "Abandon"] }) }];
}
// Its loop would go on without Max: it is not waiting on him, paused or
// finished.
const runsOnItsOwn = (t) => !["max", "held", "parked", "done"].includes(t.waiting?.kind);
// What Stop writes: park it and everything under it.
const stopFacts = (t) => ({ Task: [t.id, { Pause: "Park" }] });
// What an action list looks like, to know when to draw it again.
export const actionsKey = (list) => list.map((a) => `${a.label}:${a.title}`).join("|");

// An action's button: its icon, with its label beside it in a menu's rows.
// One that asks first arms for 4 s, widened to show its word, and runs on
// a second press. `report` hears a refusal.
export function actionButton(a, rows, report) {
  const b = icon(a.icon, `act${rows ? " row" : ""}`, a.title);
  b.setAttribute("aria-label", a.label);
  const word = el("span", "word", rows ? a.label : "");
  b.append(word);
  let timer = 0;
  const disarm = () => { clearTimeout(timer); b.classList.remove("armed"); word.textContent = rows ? a.label : ""; };
  b.onclick = async (e) => {
    e.preventDefault(); e.stopPropagation();
    if (a.confirm && !b.classList.contains("armed")) {
      b.classList.add("armed"); word.textContent = `${a.label}?`;
      timer = setTimeout(disarm, 4000);
      return;
    }
    disarm();
    const why = await act(a.facts);
    if (why) report(`${a.label}: ${why}`);
  };
  return b;
}
