#!/usr/bin/env bash
# An agent reads 21 images, the last 3600x900 px, so its next request holds
# more than 20 images and one over the API's 3000 px limit for such a
# request. Sent as stored, the API refuses it, which the demo shows first
# with the same images in one raw request (`harness-api`, a 400). Then a
# chat on the test brain `image-limit` reads them through the harness,
# whose provider shrinks the big one, and says what text it shows:
# "PELICAN 42". Sonnet at low effort unless MODEL names another. Needs
# python3 with Pillow. Leaves the brain serving:
# `node dev/drive.mjs down --brain image-limit` ends it.
set -euo pipefail
cd "$(dirname "$0")/../.."
drive() { node dev/drive.mjs "$1" --brain image-limit "${@:2}"; }
root=$PWD/target/image-limit
rm -rf "$root"
mkdir -p "$root/imgs"
python3 - "$root" <<'EOF'
import base64, json, sys
from PIL import Image, ImageDraw, ImageFont
root = sys.argv[1]
names = [f"small{i:02}.png" for i in range(20)] + ["big.png"]
for i, name in enumerate(names[:20]):
    Image.new("RGB", (64, 64), (i * 12, 100, 200)).save(f"{root}/imgs/{name}")
big = Image.new("RGB", (3600, 900), "white")
ImageDraw.Draw(big).text((100, 250), "PELICAN 42", fill="black", font=ImageFont.load_default(size=300))
big.save(f"{root}/imgs/big.png")
def image(name):
    data = base64.b64encode(open(f"{root}/imgs/{name}", "rb").read()).decode()
    return {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": data}}
content = [image(n) for n in names] + [{"type": "text", "text": "What text does the last image show?"}]
system = [{"type": "text", "text": "You are Claude Code, Anthropic's official CLI for Claude."}]
body = {"model": "claude-haiku-5-5", "max_tokens": 50, "system": system, "messages": [{"role": "user", "content": content}]}
json.dump(body, open(f"{root}/raw.json", "w"))
EOF
echo "== the 21 images sent as they are"
cargo run -q -p harness-provider --bin harness-api -- POST /v1/messages "$root/raw.json" 2>/dev/null || true
echo
echo
echo "== the same images read through the harness"
drive down --wipe
drive up "${MODEL:-sonnet/low}"
chat=$(drive chat chat "$root/imgs" "Use the read tool on every .png file in this directory: the 20 small*.png files first, one read each, then big.png last. Then tell me, in one line, what text big.png shows.")
drive settle "$chat" --timeout 400
drive log "$chat" | tail -4
echo "end it with: node dev/drive.mjs down --brain image-limit"
