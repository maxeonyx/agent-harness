// The areas on the map, drawn off the page's thread. regions.js sends the
// map's areas (each its theme, its nodes' and names' boxes) and its lines
// here; this works out the drawing (territories.js) and keeps it, and sends
// back a small picture of the whole map's washes. Then, each time the camera
// comes to rest, regions.js asks for the part of the map in view, with a
// margin round it, and gets it back drawn at the screen's own pixels, as an
// ImageBitmap, so the map is sharp at every zoom and never waits on a chart.
import { load } from "./theme.js";
import { render, territories } from "./territories.js";

// The whole map's washes, at most this many pixels per map unit and this many pixels in all: what
// shows where the camera has moved past the last picture of the view.
const WHOLE = 0.25, WHOLE_PX = 4e6;
let drawing = null;
// The names are lettered in Literata, loaded here as the page loads it; one that fails to load leaves
// the fallback serif.
const lettering = new FontFace("Literata", `url(${new URL("./font-literata.woff2", import.meta.url)})`, { weight: "400 700" }).load()
  .then((f) => self.fonts.add(f), () => {});

self.onmessage = async ({ data }) => {
  const { type, key } = data;
  try {
    if (type === "draw") {
      const t0 = performance.now();
      const themes = await Promise.all(data.areas.map((a) => load(a.theme)));
      await lettering;
      const d = territories(data.areas.map((a, i) => ({ ...a, theme: themes[i] })), data.lines, data.paper);
      drawing = { key, d };
      const b = d.bounds, s = b && Math.min(WHOLE, Math.sqrt(WHOLE_PX / (b.w * b.h)));
      const wash = b && render(d, b, Math.max(1, Math.round(b.w * s)), Math.max(1, Math.round(b.h * s)), { washOnly: true }).transferToImageBitmap();
      self.postMessage({ type, key, rect: b, wash, ms: performance.now() - t0 }, wash ? [wash] : []);
    } else if (type === "view") {
      if (!drawing?.d.bounds) return self.postMessage({ type, key: drawing?.key, empty: true });
      const t0 = performance.now(), bitmap = render(drawing.d, data.rect, data.W, data.H, { dpr: data.dpr }).transferToImageBitmap();
      self.postMessage({ type, key: drawing.key, rect: data.rect, bitmap, ms: performance.now() - t0 }, [bitmap]);
    }
  } catch (e) {
    self.postMessage({ type, key, error: `${type === "draw" ? "drawing the map's areas" : "drawing the map in view"}: ${e.message}` });
  }
};
