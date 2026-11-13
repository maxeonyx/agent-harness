#!/usr/bin/env bash
# A chat's images come by reference, each fetched only when it is about to
# be seen. A scripted chat reads twelve 1.9 MB PNGs (noise, so they do not
# compress), one tool call to a turn. The demo prints how big the chat's
# `/context` is next to its images' bytes, then opens the chat in a
# headless Chromium and prints how many of the log's images it has fetched:
# at the bottom, only those within a screen of view (or of a folded tool
# call holding one); after scrolling to the top, the ones near there too.
# Each image's room is kept before it comes: it has its width and height
# from the start. Last, `/image` serves an entry's image, and refuses an
# image the entry does not have and an id that is not an entry's. The
# brain stays up: open the chat's link on a phone or the desktop and
# scroll. On the scripted model: free, about a minute.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain chat-images "${@:2}"; }
drive down --wipe
drive up
dir=$PWD/.drive.ignore/chat-images/wd
mkdir -p "$dir"
python3 - "$dir" <<'PY'
import json, os, struct, sys, zlib
dir = sys.argv[1]
def png(w, h):
    rows = b"".join(b"\0" + os.urandom(w * 3) for _ in range(h))
    chunk = lambda t, d: struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d))
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(rows, 0)) + chunk(b"IEND", b"")
for i in range(12):
    with open(f"{dir}/art{i}.png", "wb") as f:
        f.write(png(800, 800))
    read = json.dumps({"path": f"art{i}.png", "summary": f"look at art{i}.png"})
    with open(f"{dir}/turn{i}.ignore.txt", "w") as f:
        f.write(f"tool read {read}\nlong 40")
PY
chat=$(drive chat chat "$dir" "$(cat "$dir/turn0.ignore.txt")")
drive settle "$chat" --timeout 120 >/dev/null
for i in $(seq 1 11); do
  drive say "$chat" "$(cat "$dir/turn$i.ignore.txt")" >/dev/null
  drive settle "$chat" --timeout 120 >/dev/null
done
port=$(cat .drive.ignore/chat-images/port)
echo "== /context/$chat: $(curl -sk "https://localhost:$port/context/$chat" | wc -c) bytes, for $(cat "$dir"/art*.png | wc -c) bytes of images"
count='(() => { const i = [...document.querySelectorAll(".log img")]; return `${i.filter((x) => x.getAttribute("src")).length} of ${i.length} images fetched; every one ${i.every((x) => x.getAttribute("width") === "800" && x.getAttribute("height") === "800") ? "is" : "is not"} 800x800 from the start`; })()'
drive page open "$chat" text "Line 40 of 40" sleep 1500 eval "$count" \
  eval 'document.querySelector(".scroll").scrollTop = 0' sleep 1500 eval "$count" 2>&1 | grep '^"'
entry=$(curl -sk "https://localhost:$port/context/$chat" | python3 -c 'import json,sys; print(next(e["id"] for e in json.load(sys.stdin) if "/image/" in (e.get("content") or "")))')
for at in "$entry/0" "$entry/1" "nonsense/0"; do
  echo "== /image/${at:0:8}…${at: -2}: $(curl -sk -o /dev/null -w '%{http_code} %{content_type}, %{size_download} bytes' "https://localhost:$port/image/$at")"
done
echo "== the chat: the link above, then #$chat"
echo "end it with: node dev/drive.mjs down --brain chat-images"
