// ─── The link's liveness ─────────────────────────────────────────────────────
// Whether this tab's link to the brain is alive, and when data crosses it.
// The brain ships a keepalive when its side has been quiet for 4 s (face-web's
// KEEPALIVE), so a link that has brought nothing for longer than two of them
// is taken for dead. Subscribe with `liveness.addEventListener`:
//   "pulse": data was sent or received, at most once per PULSE_MS;
//   "alive": `alive` changed.
export const liveness = new EventTarget();
export let alive = false;

const KEEPALIVE_MS = 4000, PULSE_MS = 300;
let heard = 0, pulsed = 0;

function setAlive(now) {
  if (now === alive) return;
  alive = now;
  const link = document.getElementById("link");
  link.classList.toggle("down", !alive);
  link.title = alive ? "live" : "reconnecting";
  liveness.dispatchEvent(new Event("alive"));
}
function pulse() {
  const now = Date.now();
  if (now - pulsed < PULSE_MS) return;
  pulsed = now;
  liveness.dispatchEvent(new Event("pulse"));
}

// The tab's link calls these.
export function received() { heard = Date.now(); setAlive(true); pulse(); }
export function sent() { pulse(); }
export function dropped() { setAlive(false); }

setInterval(() => { if (alive && Date.now() - heard > 2 * KEEPALIVE_MS + 1000) setAlive(false); }, 1000);
