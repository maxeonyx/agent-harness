// The areas on the map, drawn off the page's thread: regions.js sends the
// map's areas (each its theme, its nodes' and names' boxes) and its lines
// here, and gets back each area's territory (territories.js) as SVG in the
// map's units and a small picture of its wash, as an ImageBitmap, so the map
// never waits on a theme's chart.
import { load } from "./theme.js";
import { territories } from "./territories.js";

self.onmessage = async ({ data: { key, areas, lines, paper } }) => {
  try {
    const themes = await Promise.all(areas.map((a) => load(a.theme)));
    const out = territories(areas.map((a, i) => ({ ...a, theme: themes[i] })), lines, paper).areas
      .map(({ id, box, svg, tint }) => ({ id, box, svg, tint: tint.transferToImageBitmap() }));
    self.postMessage({ key, areas: out }, out.map((a) => a.tint));
  } catch (e) {
    self.postMessage({ key, error: `drawing the map's areas: ${e.message}` });
  }
};
