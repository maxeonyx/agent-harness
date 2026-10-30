# Demos

Each runs with `demos/NAME/run.sh` unless its README says otherwise, and its README (or, without one, its `run.sh`'s top comment) says what to watch. A shell demo starts its brain as a test brain (`node dev/drive.mjs up --brain NAME`), which prints its link, unless what it shows is how a brain starts (`home`, `reboot`, `relaunch`, parts of `cancel`); a real-model demo runs on Sonnet at low effort, and `MODEL=…` (`haiku/low`, `scripted`) picks another. Costs are on Max's subscription.

| Demo | What it shows | Model | Cost, time |
|---|---|---|---|
| brain | an orchestrator, a fork and an engineer build a small repo; the brain is killed -9 mid-command and relaunched, and carries on with the same limbs | scripted (`MODEL=sonnet/low`: real agents) | free, 30 s (Sonnet: a few dozen requests, 2 min) |
| attach | a fork attaches its `bash` call to its report, and the chat receives the real output after the fork's words | Sonnet low | 4 requests, under 1 min |
| browser | a chat hands a browser agent an expense claim in real pages; the tab is passed to Max, who stops it | Sonnet low | about 10 requests, under 1 min |
| cancel | abandoning tasks (with time to tidy, and past a deadline), Ctrl-C once and twice, a worker passed to Max, a systemd restart, and what still runs after each | scripted (`MODEL=sonnet/low`) | free, 3 min |
| chat-engineer | a chat in a workspace directory starts an engineer `from` the clone there, which commits in its own clone made beside it | Sonnet low | 3 requests, under 1 min |
| chat-fresh | a chat in a plain directory hands a fresh `chat` a task: it works in the same directory, no clone; a role it cannot start is refused, naming the roles it can | scripted | free, 20 s |
| chat-moves | retry, a reply's versions, going back, and a retry that gave the same words; leaves the brain up to try edit and back | scripted | free, 30 s |
| composer | the composer: stacked dictate and send with the scrollbar at the box's edge, the one model and effort pill with its menus, styled hints; leaves the brain up | scripted | free, 30 s |
| compaction | an agent compacts between steps (`compact_and_continue`) and ends with its report (`compact_and_report`); Max's words and the reports are kept word for word; a compaction loop is refused | scripted (`MODEL=sonnet/low`) | free, 1 min (Sonnet: a few dozen requests, 2 min) |
| deploy | the face as Max's phone reaches it, behind a stand-in for Cloudflare Access: installable as an app, and an expired session reloading into the login; its README says how `home` is deployed | scripted | free, 30 s |
| dev-process | an orchestrator spawns an engineer and a designer in their own clones, decides the question left to Max, and reports; the brain serves on after | Sonnet low | about 12 requests, under 1 min to the report |
| face-controls | pause, park, resume, interrupt, rank, focus and abandon on a live run, driven as a tab drives them (Rust example; it owns its brain process) | Sonnet low | a few dozen requests, 1.5 min |
| home | Max's everyday harness: `dev/serve home 4320`, a long-lived brain on his home database | the roles' own | what he does in it |
| image-limit | 21 images in one request, one 3600 px wide: refused as stored (a 400), then read through the harness, which shrinks it, and the agent reads its text | Sonnet low (and one Haiku request) | about 22 requests, 1 min |
| limb-workspace | the workspace limb's tools run as calls, with the demo playing the brain (`cargo run -q -p harness-limb-workspace --example demo`) | none | free, seconds |
| reboot | `dev/serve` makes a brain a durable, enabled user unit: it comes back from its unit file alone, as at boot, and after a SIGKILL | scripted | free, 30 s (a release build if HEAD has none) |
| relaunch | a session relaunches its own brain (`dev/serve`, a throwaway unit it retires) onto a good commit, which is proven; onto one that panics as it starts, refused by its rehearsal with the old build still serving; and onto one whose brain dies 20 s in, and the harness goes back and tells the session why | scripted (each rehearsal: a token count per live session and one short real response) | a few real requests, 6 min (three incremental release builds) |
| own-calls | an agent's edits refused only for another's change, and shown that change | scripted | free, 30 s |
| phone | Max's day at phone size: + in the last directory, dictation, the two-tap send, a reload, and finding a chat with the find button | scripted | free, 40 s |
| provider | a fork shares its parent's prompt cache on the provider (`examples/fork`) | Haiku | 4 small requests, seconds |
| scripted | the scripted model: a tool call, a spawned child, a long streamed reply | scripted | free, 20 s |
| spine | a chat across Ctrl-C and relaunches: a call finishing, a response cut, the chat carrying on (Rust example; it owns its brain process) | scripted (`MODEL=sonnet/low`: the cache reads) | free, 1 min |
| store | a brain killed mid-commit, reopened with every record intact | none | free, seconds |
| substrate | two devices and one brain on one data flow | none | free, seconds |
| unread | a brain opens a database holding a later build's record, skips it and keeps it byte for byte | none | free, 10 s |
