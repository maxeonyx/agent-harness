# chat-fresh

`demos/chat-fresh/run.sh`. A chat in a plain directory (not a repository) spawns a fresh `chat` with no `branch`. Watch the child's `pwd; ls`: it is the chat's own directory, with `note.txt` in it, and no clone was made. Then the chat spawns a role it cannot start, and the call fails with `chat cannot start a fresh `foo`: it can start …`. The chat's transcript and its child's `bash` call are printed; the brain stays up.
