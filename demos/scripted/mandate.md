A chat on the scripted model: each line below that starts with a verb is one response.
tool bash {"command": "git log --oneline; ls", "grep": ""}
tool spawn {"tasks": [{"name": "child", "mandate": "slow 100 say Hello from a fork, streamed a word at a time."}]}
slow 20 long 60
