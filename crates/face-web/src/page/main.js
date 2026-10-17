// ─── The view ──────────────────────────────────────────────────────────────
// The tab's rows, one per object, as the page draws them.
function assemble(v) {
  const out = { as_of: v.as_of, seq: Number(v.as_of.split(".")[1]), tasks: [], scopes: {}, queue: [], drafts: [], environments: [], workspaces: [] };
  for (const [key, row] of Object.entries(v.rows)) {
    const id = key.slice(key.indexOf(":") + 1);
    if (row.Task) out.tasks.push(row.Task);
    else if (row.Scope) out.scopes[id] = row.Scope;
    else if (row.Queue) out.queue = row.Queue;
    else if (row.Harness) Object.assign(out, row.Harness);
    else if (row.Draft) out.drafts.push({ id, ...row.Draft });
  }
  return out;
}
async function viewArrived(fresh) {
  if (fresh === null) { $("#link").className = "down"; $("#link").title = "reconnecting"; return; }
  $("#link").className = ""; $("#link").title = "live";
  if (fresh) liveReset();
  view = assemble(tab.view());
  settleLocal();
  await refresh();
  if (pendingOpen && byId[pendingOpen]) { const id = pendingOpen; pendingOpen = null; show(id); }
  else if (!openId && location.hash.length > 1 && (byId[location.hash.slice(1)] || location.hash === `#${NEW}`) && !moving.started) { moving.started = true; route(); }
}
boot().catch((e) => { $("#link").className = "down"; $("#link").title = `this tab could not start: ${e}`; console.error(e); });
// Waits grow between events: the hover's words, and the open card's.
setInterval(() => { if (view) { if (tipFor) fillTip(); if (openId) renderPanel(); } }, 1000);
