import { el } from "./base.js";

// ─── Hints ─────────────────────────────────────────────────────────────────
// What a control does, shown when a mouse rests on it, in the page's own
// style rather than the browser's. An element's `title` is its hint: while
// the pointer is over it the title is set aside, so the browser shows none
// of its own (and again whenever the page changes it meanwhile), and it
// comes back when the pointer leaves.
const hint = el("div");
hint.id = "hint";
hint.hidden = true;
document.body.append(hint);
let over = null, timer = 0;
const retitled = new MutationObserver(() => {
  if (!over?.hasAttribute("title")) return;
  over.dataset.hint = over.title;
  over.removeAttribute("title");
  if (!hint.hidden) show();
});

document.addEventListener("pointerover", (e) => {
  if (e.pointerType !== "mouse") return;
  const target = e.target.closest?.("[title]");
  if (!target || target === over) return;
  leave();
  over = target;
  target.dataset.hint = target.title;
  target.removeAttribute("title");
  retitled.observe(target, { attributeFilter: ["title"] });
  timer = setTimeout(show, hint.hidden ? 450 : 0);
});
document.addEventListener("pointerout", (e) => { if (over && !over.contains(e.relatedTarget)) leave(); });
document.addEventListener("pointerdown", leave, true);
addEventListener("blur", leave);

function show() {
  if (!over?.isConnected || !over.dataset.hint) return;
  hint.textContent = over.dataset.hint;
  hint.hidden = false;
  const r = over.getBoundingClientRect(), h = hint.getBoundingClientRect();
  const x = Math.max(6, Math.min(innerWidth - h.width - 6, r.left + r.width / 2 - h.width / 2));
  const y = r.top - h.height - 6 >= 6 ? r.top - h.height - 6 : r.bottom + 6;
  hint.style.left = `${x}px`;
  hint.style.top = `${y}px`;
}
// Gone, and the title back where it was.
function leave() {
  clearTimeout(timer);
  hint.hidden = true;
  if (!over) return;
  retitled.disconnect();
  if (over.dataset.hint) over.title = over.dataset.hint;
  delete over.dataset.hint;
  over = null;
}
