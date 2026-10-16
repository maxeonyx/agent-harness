#!/bin/sh
# Regenerate the test clips from their text with Piper (offline TTS).
# Needs uv and ffmpeg; downloads the 60 MB lessac voice once into voice.ignore/.
set -eu
cd "$(dirname "$0")"
voice=voice.ignore/en_US-lessac-medium.onnx
base=https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/lessac/medium
mkdir -p voice.ignore
[ -f "$voice" ] || { curl -sLo "$voice" "$base/en_US-lessac-medium.onnx"; curl -sLo "$voice.json" "$base/en_US-lessac-medium.onnx.json"; }
for name in short long; do
  uv run -q --with piper-tts python -m piper -m "$voice" -f voice.ignore/$name.wav < $name.txt
  ffmpeg -loglevel error -y -i voice.ignore/$name.wav -ar 16000 -ac 1 -sample_fmt s16 $name.wav
done
