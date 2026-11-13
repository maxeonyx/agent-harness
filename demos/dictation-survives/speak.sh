#!/bin/sh
# Regenerate speech.wav from speech.txt with Piper (offline TTS), as
# demos/home/speak.sh does for its sample.
set -eu
cd "$(dirname "$0")"
voice=voice.ignore/en_US-lessac-medium.onnx
base=https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/lessac/medium
mkdir -p voice.ignore
[ -f "$voice" ] || { curl -sLo "$voice" "$base/en_US-lessac-medium.onnx"; curl -sLo "$voice.json" "$base/en_US-lessac-medium.onnx.json"; }
uv run -q --with piper-tts python -m piper -m "$voice" -f voice.ignore/speech.wav < speech.txt
ffmpeg -loglevel error -y -i voice.ignore/speech.wav -ar 16000 -ac 1 -sample_fmt s16 speech.wav
