# Dictation spike

Thesis: speech can be transcribed through Anthropic's dictation endpoint on Max's claude.ai login, from a program we own.

It can. `dictation` streams a WAV file to the endpoint Claude Code's `/voice` uses and prints the transcript.

## Run it

Needs a claude.ai login in Claude Code (`~/.claude/.credentials.json`). The program reads the access token from it and never refreshes it.

```sh
cargo run --release -- audio/long.wav              # real-time pace, like a microphone
cargo run --release -- --interims audio/short.wav  # also ask for TranscriptInterim frames
cargo run --release -- --fast audio/long.wav       # send as fast as the socket takes it
```

Every server frame goes to stderr with its arrival time; the final transcript goes to stdout. Any WAV works once converted: `ffmpeg -i IN -ar 16000 -ac 1 -sample_fmt s16 OUT.wav`. `audio/speak.sh` regenerates the clips from their `.txt` with Piper, offline.

## The protocol

`wss://api.anthropic.com/api/ws/speech_to_text/voice_stream?encoding=linear16&sample_rate=16000&channels=1&endpointing_ms=300&utterance_end_ms=1000&language=en&use_conversation_engine=true`

- Auth: `Authorization: Bearer <claude.ai OAuth access token>`. That is the only header the server checks; Claude Code also sends `User-Agent`, `x-app: cli` and `anthropic-client-platform`, and optionally `x-config-keyterms` (comma-separated recognition hints, at most 1024 chars).
- Query: only `encoding` is required. It accepts `linear16`, `opus`, `webm` and `flac`; only `linear16` produced a transcript here (the compressed ones close normally with no text). `sample_rate=48000` with 48 kHz PCM works too.
- Client → server: binary frames of raw little-endian PCM, any size (Claude Code's are whatever its native recorder yields; this sends 100 ms). Text frames `{"type":"KeepAlive"}` on open and every 8 s, and `{"type":"CloseStream"}` to finish.
- Server → client, JSON text frames:
  - `{"type":"TranscriptText","data":"..."}` — the whole session's text so far, not a delta. Later frames can rewrite earlier words (the last frame of the long clip changed "All right" to "Alright").
  - `{"type":"TranscriptInterim","data":"..."}` — sent instead of `TranscriptText` while speaking when the query has `forward_interims=typed`; one `TranscriptText` then carries the final.
  - `{"type":"TranscriptEndpoint"}` — the pending text is final. In these runs it came only once, after `CloseStream`, even across sentence pauses.
  - `{"type":"TranscriptError","description"|"error_code":...}` and `{"type":"error","error":{"type":"invalid_request_error","message":"query.encoding: Field required"},"request_id":"req_..."}`.
- Rejection is an HTTP status on the upgrade (Claude Code also reads `cf-mitigated` for Cloudflare bot protection).

## Measured (2026-10-16, Max's work Team seat, this VM)

| Clip | Mode | Result |
| :- | :- | :- |
| `short.wav`, 4.8 s | real time | exact; first word 0.4 s after connect; final 0.6 s after `CloseStream` |
| `long.wav`, 50.7 s, 172 words | real time | one word changed ("All right" → "Alright"); 157 frames; final 0.54 s after `CloseStream` |
| `long.wav` | `--fast` | same text; the upload took 7.6 s and the final came 6.2 s after `CloseStream` |
| `long.wav` looped to 12 min | real time | all 2465 words; frames every 0.26 s (median, max 1.2 s); final 0.58 s after `CloseStream`; no server limit hit |
| the 12 min loop | `--fast` | upload took 108 s; the server was still behind 10 s after `CloseStream`, so it was cut off |

Claude Code caps a tap-mode recording at two minutes and 15 s of silence; that is its own client, not the server. Every frame carries the whole text so far, so a 12-minute session received 15 MB of frames: quadratic in length.

## Browser → brain → endpoint

Nothing found blocks it. The browser cannot connect directly: a browser WebSocket cannot set `Authorization`, and the token must not reach the page anyway. So the brain relays: the page captures the microphone (`getUserMedia` into an AudioWorklet, emitting 16-bit PCM at 16 or 48 kHz, both accepted) and sends binary frames to the brain; the brain opens this socket with the token and forwards frames both ways. What the brain has to own:

- **The token's lifetime.** This program reads Claude Code's access token and refuses an expired one. A harness that refreshes it itself can sign Claude Code out (the refresh token rotates), so the harness needs one owner of the claude.ai login. That is the provider spike's question, and dictation should use the same credential, not a second one.
- **Turning snapshots into edits.** The server sends whole-text snapshots that can rewrite earlier words; the brain should send the page only what changed, which also removes the quadratic traffic.
- **Long dictations.** No limit appeared at 12 minutes. Untested: longer, idle sockets, and what happens across a reconnect (each socket is a fresh session, so text from a dropped one must be kept as final).
