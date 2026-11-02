#!/usr/bin/env bash
# Dictation pressed again at once: on the desktop, with Chromium's fake
# microphone (a tone) and the dictation endpoint on the home brain's
# login. In a chat waiting on Max, the box's mic starts dictation; a press
# stops it and, while its last words are still coming, a second press
# starts it again, and it is listening a moment later. The same with the
# log's blob, and a start and stop in one go, before the microphone has
# opened, which leaves it off. Each line prints the status's words and
# whether the mic shows on; it fails at the first that is wrong.
set -euo pipefail
cd "$(dirname "$0")/../.."
B=(--brain dictate-again)
node dev/drive.mjs down "${B[@]}" --wipe
node dev/drive.mjs up "${B[@]}"
dir=$PWD/.drive.ignore/dictate-again/wd
mkdir -p "$dir"
chat=$(node dev/drive.mjs chat "${B[@]}" chat "$dir" "hello")
node dev/drive.mjs settle "${B[@]}" "$chat"
mic='document.querySelector(".compose .mic")'
is() { echo "eval"; echo "(() => { const s = [document.querySelector(\".compose .status\").title, $mic.classList.contains(\"on\")]; if (s[0] !== \"$1\" || s[1] !== $2) throw new Error(\"expected $1 $2, got \" + s); return s.join(\" \"); })()"; }
mapfile -t listening < <(is Listening true)
mapfile -t again < <(is Finishing true)
mapfile -t off < <(is "" false)
node dev/drive.mjs page "${B[@]}" open "$chat" wait ".compose .mic" \
  click ".compose .mic" sleep 3000 "${listening[@]}" \
  click ".compose .mic" click ".compose .mic" "${again[@]}" sleep 3000 "${listening[@]}" \
  click ".pane > .dock .blob" click ".pane > .dock .blob" "${again[@]}" sleep 3000 "${listening[@]}" \
  click ".compose .mic" sleep 3000 "${off[@]}" \
  eval "($mic.click(), $mic.click(), 'pressed twice at once')" sleep 3000 "${off[@]}"
node dev/drive.mjs down "${B[@]}"
