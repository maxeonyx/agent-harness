import { $, fail } from "./base.js";
import { boot, tab } from "./tab.js";
import { byId, takeView, view } from "./map.js";
import "./camera.js";
import { refresh } from "./draw.js";
import { fillTip, tipFor } from "./hover.js";
import "./chrome.js";
import "./actions.js";
import { moving, NEW, openId, openPending, renderPanel, route } from "./card.js";
import { liveReset } from "./log.js";
import { settleLocal } from "./compose.js";
import "./newchat.js";
import "./hints.js";

// ─── The page, started ─────────────────────────────────────────────────────
export async function viewArrived(fresh) {
  if (fresh) liveReset();
  takeView(tab.view());
  settleLocal();
  await refresh();
  if (!openPending() && !openId && location.hash.length > 1 && (byId[location.hash.slice(1)] || location.hash === `#${NEW}`) && !moving.started) { moving.started = true; route(); }
}
boot().catch((e) => { $("#link").className = "down"; $("#link").title = `this tab could not start: ${e}`; fail("starting this tab", e); });
// Waits grow between events: the hover's words, and the open card's.
setInterval(() => { if (view) { if (tipFor) fillTip(); if (openId) renderPanel(); } }, 1000);
