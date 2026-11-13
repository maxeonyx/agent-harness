#!/usr/bin/env bash
# Dictation across a relaunch. A headless page dictates speech.wav (27 s
# of Piper speech, through Chromium's microphone, as Max's voice reaches
# it) into a chat's box, with the dictation endpoint on the home brain's
# login. Ten seconds in, the brain is stopped, and started again 20 s
# later (a relaunch onto a new build is down 17-34 s), while the speech
# goes on and ends. The page keeps the audio, reconnects once the brain is
# back, and the words catch up. It prints the status as it changed, the
# transcript, and the words of speech.txt the transcript lacks; it fails
# if more than a few are missing (the endpoint rewords a little).
set -euo pipefail
cd "$(dirname "$0")/../.."
B=(--brain dictation-survives)
here=demos/dictation-survives
node dev/drive.mjs down "${B[@]}" --wipe
node dev/drive.mjs up "${B[@]}"
dir=$PWD/.drive.ignore/dictation-survives/wd
mkdir -p "$dir"
chat=$(node dev/drive.mjs chat "${B[@]}" chat "$dir" "hello")
node dev/drive.mjs settle "${B[@]}" "$chat"
log=$PWD/.drive.ignore/dictation-survives/dictation.log
watch='(() => { const s = document.querySelector(".compose .status"), t0 = Date.now(); let was; new MutationObserver(() => { if (s.title !== was) console.log(`status at ${((Date.now() - t0) / 1000).toFixed(1)} s: ${(was = s.title) || "(none)"}`); }).observe(s, { attributes: true }); return "watching the status"; })()'
node dev/drive.mjs page "${B[@]}" --timeout 120 --mic "$here/speech.wav" --log "$log" open "$chat" wait ".compose .mic" \
  eval "$watch" click ".compose .mic" sleep 50000 click ".compose .mic" gone ".compose .mic.on" \
  eval 'document.querySelector(".compose textarea").value' > "$log.out" &
page=$!
sleep 10
echo "stopping the brain mid-dictation, for 20 s"
kill "$(cat .drive.ignore/dictation-survives/pid)"
sleep 20
node dev/drive.mjs up "${B[@]}"
wait "$page"
grep -o 'log: status at .*' "$log"
transcript=$(grep '^"' "$log.out" | tail -1)
echo "transcript: $transcript"
node -e '
const norm = (s) => s.toLowerCase().replace(/[^a-z\s]/g, "").split(/\s+/).filter(Boolean);
const said = norm(require("fs").readFileSync(process.argv[1], "utf8")), got = norm(JSON.parse(process.argv[2]));
const left = [...got], missing = said.filter((w) => { const i = left.indexOf(w); if (i < 0) return true; left.splice(0, i + 1); return false; });
console.log(`${said.length} words said, ${got.length} transcribed; missing in order: ${missing.join(" ") || "none"}`);
if (missing.length > 4) process.exit(1);
' "$here/speech.txt" "$transcript"
node dev/drive.mjs down "${B[@]}"
