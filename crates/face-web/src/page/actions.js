// ─── What Max can do to a task ─────────────────────────────────────────────
// One list per task, in the order it shows: the card's controls, the hover
// preview and the right-click menu all draw from it. Each is the facts it
// writes, and whether it asks once more first.
//
// The stop slot holds one control at a time: Pause while its loop would run
// on its own (one press, at once: the brake); once it is paused, Stop, the
// graceful end of its turn unless it already waits on Max, with Resume
// beside it. Abandon stops it and what it started; while that tidies up,
// End now ends it at once. Done, in a chat, has it write its result.
function actionsOf(t) {
  if (!can.act || !t || isDone(t)) return [];
  const out = [], holder = heldBy(t), stopping = !!t.stop;
  if (!stopping && holder) out.push(
    ...t.waiting?.kind === "max" ? [] : [{ icon: "stop", label: "Stop", title: "It ends its turn and waits for you", facts: () => stopFacts(t) }],
    { icon: "play", label: "Resume", title: holder === t ? "It and everything under it carry on" : `Paused with ${holder.name}: it and everything under it carry on`, facts: () => ({ Task: [holder.id, { Held: false }] }) },
  );
  else if (!stopping && runsOnItsOwn(t)) out.push({ icon: "pause", label: "Pause", title: "It and everything under it finish what is running, then wait", facts: () => ({ Task: [t.id, { Held: true }] }) });
  if (!stopping) out.push({ icon: "cross", label: "Abandon", confirm: true, title: "It and everything it started stop, tidy up and report", facts: () => ({ Task: [t.id, { Drain: { Stop: { by: "Max", within: 120000 } } }] }) });
  else if (!t.stop.hard) out.push({ icon: "cross", label: "End now", confirm: true, title: "End it and everything it started now, with no tidy-up", facts: () => ({ Task: [t.id, { Drain: { Stop: { by: "Max", within: 0 } } }] }) });
  if (t.mode === "user-facing" && !stopping) out.push({ icon: "tick", label: "Done", confirm: true, title: "It writes its result and finishes", facts: (at) => ({ Delivery: { id: `${at}.0`, to: t.id, content: { Prompt: "Done" } } }) });
  return out;
}
// Its loop would go on without Max: it is not waiting on him, paused or
// finished.
const runsOnItsOwn = (t) => !["max", "held", "done"].includes(t.waiting?.kind);
// What Stop writes: for now, the interrupt, which ends its turn at the next
// safe point and leaves it waiting on Max.
const stopFacts = (t) => ({ Task: [t.id, { Drain: { ToMax: { turn: t.turn } } }] });
// What an action list looks like, to know when to draw it again.
const actionsKey = (list) => list.map((a) => `${a.label}:${a.title}`).join("|");

// An action's button: its icon, with its label beside it in a menu's rows.
// One that asks first arms for 4 s, widened to show its word, and runs on
// a second press. `report` hears a refusal.
function actionButton(a, rows, report) {
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
