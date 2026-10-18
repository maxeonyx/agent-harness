# home: the harness for everyday work

```sh
demos/home/run.sh
```

Then open https://mx-hv-mj.local:4320. It is `dev/serve home 4320`: one long-lived brain, the durable user unit `harness-home` (it comes back after a crash and at boot), on the build of this clone's commit (a tree with uncommitted changes is refused), with one database kept in `~/.local/share/agent-harness/home/`. Run it again after a commit and the new build takes over the same database, so everything you started carries on. Another brain: `NAME=try PORT=4460 demos/home/run.sh`; its log: `journalctl --user -u harness-home`; stop it: `systemctl --user stop harness-home` (until the next boot; `disable --now` for good). A brain already serving 4320 under another unit has to stop first, or `dev/serve` refuses the port.

What to do:

1. **+** (bottom right; or double-click anywhere) opens a new chat. Pick its role (`chat` is the everyday one) and the directory it works in, then write, dictate or attach in the box as in any chat. **Start** (Ctrl+Enter) starts it: your words are what it is for, and it is named from its first words unless you name it.
2. The chat opens and the agent works. It searches the web itself when the answer depends on anything current; each search shows as a step with what it found, and its answer links each source.
3. The box is a staging area: Enter is a new line, and nothing is sent until you send it. While the agent works, **Send now** (Ctrl+Enter) reaches it at its next request and **At turn end** (Ctrl+Shift+Enter) waits for its turn to end. Either way it is told how far into its work you wrote.
4. A chat is done when you press **Done**. Until then it waits on you, and the **needs you** pill at the top right lists every chat whose move is yours.
5. The context gauge in an open chat's header says how full its context is; click it for what fills it. The plan's 5-hour and weekly usage are at the bottom.
6. The **Opus · high** pill under the send is the model and effort the chat's next request runs at; a new chat's card has it too, starting from its role's. Change either at any time, from the PC or the phone. While the chat's cache is warm (an hour after its last response), each other choice says how many tokens it rereads without the cache. Each response's line says what it ran at: `02:27 · 1.1s · Sonnet medium · 8.6k context · 4 out`.
7. **Ctrl+K** (or **/**) finds a chat by its name or by anything said in it, yours or the agent's. The matching chats list under the box, newest first, each with the words that matched; the arrows choose, and Enter or a click opens the chat scrolled to that place.

What the agents get is decided by the files in `roles/`: each role's prompt, model, effort and limbs (`web` is the provider's web search and fetch; `user` is your `~/.claude`). `roles/skills.txt` lists which of your skills they are offered; edit it and running agents see the new list at their next turn.

The model and dictation use the claude.ai login in `~/.claude/.credentials.json`. Nothing here refreshes the token, so if it has expired, use Claude Code once.

It serves with the shared wildcard certificate in `~/.config/dev-certs/shared-local/` and accepts only this machine and the Windows host (the default route's gateway).

On a machine with no microphone, `SAMPLE=1 demos/home/run.sh` offers **Dictate the sample**, which streams `sample.wav` (13 s of Piper speech) to the dictation endpoint at real time (`demos/phone` dictates it in a headless page), and `speak.sh` regenerates the recording from `sample.txt`.

## A real run

A fresh database, a chat started from **+** in a scratch directory, asked: "Use your web_search tool to find what happened in New Zealand news this week: three items with sources. Then, in one response, write them to news2.md and commit that file, as two tool calls." The brain's log, trimmed:

```
   17.4s    25  words delivered to try
   18.4s    27  try: first byte
   29.8s    28  try's write made {"path": "news2.md", "content": "# New Zealand news, week to 17 October 2026\n\n1. **Election campaign: the Opportunity Party holds the balance of power.** …
   29.8s    29  try's bash made {"command": "… git add news2.md && git diff --cached --stat && git commit -q -m \"…
   29.8s    30  try: responded, ToolUse, 2 calls (6 in, 21054 cached, 1102 out)
   29.8s    31  try's write dispatched
   29.8s    32  try's bash dispatched
   29.8s    33  try's write completed
   29.8s    34  file news2.md changed by try's write
   29.9s    35  try's bash completed
   38.5s    44  try: responded, EndTurn, 0 calls (2 in, 19352 cached, 577 out)
```

The search ran inside the first response; the write and the commit ran one after the other, in the order the model wrote them. On the page, the search step listed its ten results (NZ Herald, RNZ, Stuff, The Press …) and each item of the answer ended with pills linking its sources.
