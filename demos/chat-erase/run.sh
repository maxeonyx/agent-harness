#!/usr/bin/env bash
# Each chat's name on the map has the lines behind it erased, in the
# name's shape and softly, so the ground under it shows through rather
# than a patch of the ground's colour painted over it. On the scripted
# test brain `chat-erase`, Root spawns seven agents, which
# work for ten minutes, so each is a named node. The layout keeps
# names clear of lines, so the demo slides the last of them sideways, as
# a glide does, until its name lies over another's line. It finds every
# point inside a name where the lines' geometry passes, hides the names
# (whatever they paint goes with them), and screenshots the map with the lines shown and
# with them hidden: at each of those points the two must match, since the
# lines are erased there. It fails on a page that halos the names with a
# ground-coloured text-shadow, and on one whose lines run under them.
# The brain stays up.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain chat-erase "${@:2}"; }
at=.drive.ignore/chat-erase
drive down --wipe
drive up
port=$(cat "$at/port")
mkdir -p "$at/r"

root=$(python3 - <<'PY'
import json
def tool(t, /, **input): return f"tool {t} {json.dumps(input)}"
check = "\n".join([tool("bash", command="sleep 600", timeout_ms=600000, summary="check"), "say Checked."])
print("\n".join([
    "Root",
    tool("spawn", summary="check seven ways", fork=[{"name": f"check-{i}", "mandate": check} for i in range(1, 8)]),
    "say All seven checked.",
]))
PY
)
r=$(drive chat chat "$PWD/$at/r" "$root")

# The page's modules, imported where the page has them.
mods='const base = performance.getEntriesByType("resource").map((e) => e.name).find((n) => n.endsWith("/draw.js")).replace(/draw\.js$/, "");'
# Points inside each name's box, in screen px, where a line's geometry
# (unmasked) passes; and the names hidden.
find="(async () => { $mods
  const { cam } = await import(base + \"camera.js\"), M = await import(base + \"map.js\"), D = await import(base + \"draw.js\");
  const paths = [...document.querySelectorAll(\"#edges path\")].filter((p) => !p.closest(\"mask\") && p.getAttribute(\"d\"));
  const pt = document.querySelector(\"#edges\").createSVGPoint(), pts = [];
  const on = (x, y) => { pt.x = x; pt.y = y; return paths.some((p) => (p.getAttribute(\"fill\") !== \"none\" && p.isPointInFill(pt)) || (p.getAttribute(\"stroke\") && p.isPointInStroke(pt))); };
  const [id, a] = [...D.nodeEls].find(([id]) => M.label(M.graph.nodes.get(id)) === \"check-7\");
  const s = M.shown.get(id), c = M.centre(s), w = a.querySelector(\".name\").offsetWidth;
  for (let x = c.x - 40; x > c.x - 2000; x--) if (on(x, c.y)) { s.x += x - (c.x + 26 + w / 2); break; }
  D.drawNodes(); D.drawEdges();
  for (const e of document.querySelectorAll(\".node:not(.archived) .name\")) {
    const b = e.getBoundingClientRect();
    for (let x = b.left + 1; x < b.right - 1; x += 2) for (let y = b.top + 3; y < b.bottom - 3; y += 2) {
      if (on((x - cam.x) / cam.k, (y - cam.y) / cam.k)) pts.push([Math.round(x), Math.round(y)]);
    }
  }
  const st = document.createElement(\"style\"); st.textContent = \".node .name { visibility: hidden !important }\"; document.head.append(st);
  return JSON.stringify(pts);
})()"
shadow='[...document.querySelectorAll(".node:not(.archived) .name")].map((e) => getComputedStyle(e).textShadow).find((s) => s !== "none") || "none"'
out=$(drive page \
  text check-7 sleep 3000 \
  eval "$shadow" \
  eval "$find" sleep 500 shot "$PWD/$at/lines.png" \
  eval 'document.querySelector("#edges").style.visibility = "hidden"' sleep 500 shot "$PWD/$at/bare.png")
echo "$out" > "$at/demo.out"

python3 - "$at" "$out" <<'PY'
import json, sys
from PIL import Image
at, out = sys.argv[1], sys.argv[2].splitlines()
values = [json.loads(l) for l in out if l.startswith('"')]
shadow, pts = values[0], json.loads(values[1])
lines, bare = Image.open(f"{at}/lines.png").convert("RGB"), Image.open(f"{at}/bare.png").convert("RGB")
shown = [p for p in pts if max(abs(a - b) for a, b in zip(lines.getpixel((p[0], p[1])), bare.getpixel((p[0], p[1])))) > 12]
print(f"== Names' text-shadow: {shadow}")
print(f"== Points inside a name where a line passes: {len(pts)}; where the line still shows: {len(shown)}")
if shadow != "none": sys.exit("FAIL: a name is haloed with a text-shadow")
if not pts: sys.exit("FAIL: no line crosses under a name, so nothing was shown")
if shown: sys.exit(f"FAIL: the lines show under names, e.g. at {shown[:5]}")
print("PASS: the lines are erased behind every name, and nothing paints over them")
PY
echo "Open https://mx-hv-mj.local:$port/#$r; end it with: node dev/drive.mjs down --brain chat-erase"
