#!/usr/bin/env bash
# The map, on the scripted test brain `map`. Root runs a quick `bash`, then
# spawns seven agents at once: `build` backgrounds a server `srv` and passes
# it up to Root; the other six each run one `bash`. Root backgrounds
# `preview` and hands it to Max. Open the link it prints and watch the map:
# the seven starts taper out of Root's dot, its own line runs straight down
# the middle between them, and their seven results thin back into Root's
# next dot, spread evenly either side of that line, none from the left or
# right; `srv` changes hands at a ring and its line bends into Root;
# `preview` is Max's orange card with a gear, on an orange line from the
# ring where it was handed over; each finished `bash` is a small green
# gear. The project stays open while Max holds `preview`.
# The brain stays up.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain map "${@:2}"; }
at=.drive.ignore/map
drive down --wipe
drive up
port=$(cat "$at/port")
mkdir -p "$at/r"

root=$(python3 - <<'PY'
import json
def tool(t, /, **input): return f"tool {t} {json.dumps(input)}"
build = "\n".join([
    tool("background", name="srv", tool="bash", summary="serve",
         input={"command": "sleep 600", "summary": "serve the build"}),
    tool("pass", call="srv", why="Root keeps it running", summary="pass it up"),
    "say Built, and the server is yours.",
])
check = "\n".join([
    tool("bash", command="sleep 2", summary="check the build"),
    "say Checked.",
])
print("\n".join([
    "Root",
    tool("bash", command="true", summary="look around"),
    tool("spawn", summary="build and check six ways",
         tasks=[{"name": "build", "mandate": build}] + [{"name": f"check{i}", "mandate": check} for i in range(1, 7)]),
    tool("background", name="preview", tool="bash", summary="preview",
         input={"command": "sleep 600", "summary": "serve a preview"}),
    tool("pass", call="preview", why="Max looks at it", summary="hand it to Max"),
    "say Max holds the preview.",
]))
PY
)
r=$(drive chat chat "$PWD/$at/r" "$root")
drive settle "$r" >/dev/null

echo "== Running work and who holds it"
curl -sk "https://localhost:$port/view" | jq -r '
  [.rows[] | .Task? | select(.)] as $tasks |
  ($tasks | map({key: .id, value: .name}) | from_entries) as $names |
  $tasks[] | .work[]? | select(.ended == null) |
  "  \(.name): held by \($names[.holder] // .holder)"'
echo
echo "Open https://mx-hv-mj.local:$port/#$r; end it with: node dev/drive.mjs down --brain map"
