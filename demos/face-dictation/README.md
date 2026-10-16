# face-dictation: dictating into the compose box

```sh
cargo run -p harness-face-web --example dictation
```

Then open https://mx-hv-mj.local:4322 (another port: append it, `-- 4400`). The journal is in memory and has one task, `chat`, which has had its turn and is waiting on you.

1. Click `chat` in the tree. The compose box at the bottom right now says "To chat".
2. Click **Dictate**, allow the microphone, and talk. The words arrive in the box as you speak, and you can edit them while more arrive. Click **Stop** when you have finished.
3. Press **Ctrl+Enter** to send. Enter on its own is a new line. The terminal prints the delivery that a brain would record, and the page's journal shows it arrive.

On a machine with no microphone, click **Dictate the sample** instead. The server streams `sample.wav` (11 s of Piper speech) to the dictation endpoint at real time, as if it came from a microphone, and the transcript arrives in the box through the same relay. `speak.sh` regenerates the file from `sample.txt`.

Dictation uses the claude.ai login in `~/.claude/.credentials.json` through `harness-provider`'s `Login`. Nothing here refreshes the token, so if it has expired, use Claude Code once.

## A real run

This page was driven by headless Chromium with the sample as its microphone, so the path was getUserMedia, then the AudioWorklet, then the face's WebSocket, the endpoint, and the edits back into the box. The text "Before. " was typed first. The lines show the status and the box's text every 0.3 to 1.5 s:

```
[0.3s] connecting | Before.
[3.9s] dictating the sample | Before. This is the harness's own compose box.
[5.4s] dictating the sample | Before. This is the harness's own compose box. I'm dictating straight into
[6.6s] dictating the sample | Before. This is the harness's own compose box. I'm dictating straight into it, and the words should
[7.9s] dictating the sample | Before. This is the harness's own compose box. I'm dictating straight into it, and the words should appear while I'm still
[9.2s] dictating the sample | Before. This is the harness's own compose box. I'm dictating straight into it, and the words should appear while I'm still talking. When I stop,
[10.5s] dictating the sample | Before. This is the harness's own compose box. I'm dictating straight into it, and the words should appear while I'm still talking. When I stop, the text stays
[12.2s] dictating the sample | Before. This is the harness's own compose box. I'm dictating straight into it, and the words should appear while I'm still talking. When I stop, the text stays editable, and Control-Enter sends
after Ctrl+Enter: "" sent to chat
```

The terminal:

```
   1  brain launched, build face-dictation-demo
   2  limb max defined
   3  environment chat defined
   4  chat created (assistant)
   5  chat: request sent
   6  chat: first byte
   7  chat: responded, EndTurn, 0 calls

face: https://mx-hv-mj.local:4322

   8  words delivered to chat
      [{"text":"Before. This is the harness's own compose box. I'm dictating straight into it, and the words should appear while I'm still talking. When I stop, the text stays editable, and Control-Enter sends it to the task.","type":"text"}]
```

In a second run, "AA " was typed at the start of the box 3 s in, and "compose" was selected and overtyped with "EDITED" 6 s in. Both edits survived, and the rest of the words kept arriving after them:

```
[13.0s] listening | AA Before. This is the harness's own EDITED box. I'm dictating straight into it, and the words should appear while I'm still talking. When I stop, the text stays editable, and Control-Enter sends it to the task.
```
