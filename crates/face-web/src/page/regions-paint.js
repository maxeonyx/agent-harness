// The areas' grounds, painted off the page's thread: regions.js sends each
// painting's spec here and gets its picture back as an ImageBitmap, so the
// map never waits on a theme's paint. A theme makes its working canvases
// with document.createElement, which a worker has not: here each one is an
// OffscreenCanvas, which draws, reads back and feeds a shader the same way.
import { load, paintRegion } from "./theme.js";

globalThis.document = { createElement: () => new OffscreenCanvas(1, 1) };

const faces = new Set();

// A ground that letters (Harbour's soundings) paints in its display face,
// loaded here as the page's wear loads it there.
async function face(theme) {
  const d = theme.display;
  if (faces.has(d.family)) return;
  faces.add(d.family);
  const loading = d.files.map((file) => new FontFace(d.family, `url(${new URL(`./${file}`, import.meta.url)})`, { weight: d.weights }).load());
  // A face that fails to load leaves the fallback.
  for (const f of await Promise.allSettled(loading)) if (f.status === "fulfilled") self.fonts.add(f.value);
}

self.onmessage = async ({ data: { id, key, theme, who, spec } }) => {
  try {
    const ground = await load(theme);
    await face(ground);
    const bitmap = paintRegion(ground, who, spec).transferToImageBitmap();
    self.postMessage({ id, key, bitmap }, [bitmap]);
  } catch (e) {
    self.postMessage({ id, key, error: `painting the ${theme} ground: ${e.message}` });
  }
};
