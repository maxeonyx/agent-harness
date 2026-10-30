# One limb per machine

```sh
demos/limbs-per-machine/run.sh
```

Four chats on a scripted test brain (`limbs-per-machine`), working in three directories, one nested in another: A in `a/`, C in `a/nested/`, and S and D in `b/`, a git repo. All of them are run by one `harness-limb serve` process, the brain's limb on this machine; each command runs in a systemd scope of its own (`harness-call-<call>.scope`) inside its agent's slice (`harness-<task>.slice`). What it shows, in order:

- `systemctl --user list-units 'harness-*'` while three commands run: one limb scope, three call scopes, three agent slices (D's spawn runs in the brain, so D has none). A scope's description is its chat's name and its command.
- The limb's resident memory.
- S ran `setsid sleep 4242 & sleep 4243`. Max abandons S with no time to tidy: both sleeps are gone, the one that escaped with `setsid` too, because the call's scope is stopped.
- A read `nested/note.txt`; C, in `a/nested/`, overwrote it. One limb keeps the file versions of every directory, so A's next request carries a notice that the file changed.
- D spawns an engineer on branch `x` from `origin/nope`, which doesn't exist. The child waits, `starting`, with git's words, while A and C finish.

The brain stays up after; end it with `node dev/drive.mjs down --brain limbs-per-machine`.

## Real output

A run at 23:32 NZDT on 2026-10-30, with this clone's path shortened to `…/` and lines cut at 200 characters.

```
brain limbs-per-machine on https://mx-hv-mj.local:4386/ (pid 136874; every role on scripted)
log …/.drive.ignore/limbs-per-machine/brain.log
end it with: node dev/drive.mjs down --brain limbs-per-machine

== systemd: one limb scope, a scope per running command, a slice per agent
harness-call-a893e61c2a7893d4.21.0.scope     loaded active running tool bash {"command":"sleep 3: sleep 3; echo changed > note.txt; echo wrote
harness-call-a893e61c2a7893d4.23.0.scope     loaded active running tool read {"path":"nested/note.txt"}: sleep 12; echo A done
harness-call-a893e61c2a7893d4.35.0.scope     loaded active running tool bash {"command":"setsid sleep 4242…: setsid sleep 4242 & sleep 4243
harness-limb-a893e61c2a7893d4.1.2.scope      loaded active running [systemd-run] …/target/release/harness-limb serve a893e61c2a7893d4.1.2
harness-2189df306617c469.1.0.slice           loaded active active  Slice /harness/2189df306617c469.1.0
harness-51f931af78cc97f6.1.0.slice           loaded active active  Slice /harness/51f931af78cc97f6.1.0
harness-c2d380e7ed8b9534.1.0.slice           loaded active active  Slice /harness/c2d380e7ed8b9534.1.0

== the limb's memory
…/target/release/harness-limb serve a893e61c2a78 VmRSS:	    9084 kB

== stopping the chat whose command ran setsid sleep 4242 & sleep 4243
 137122 bash -c setsid sleep 4242 & sleep 4243
 137157 sleep 4242
 137158 sleep 4243
  act: tab 73a478049e3738a7 linked
  act: Max presses Abandon, with 0ns to finish
included
left running:
  no sleep 4242 or 4243

== A, in a/, read nested/note.txt; C, in a/nested/, wrote it
  → read {"path":"nested/note.txt"}
  ←      1	original
  → bash {"command":"sleep 12; echo A done"}
  ← A done
harness: [harness] Changed since you last saw them: …/.drive.ignore/limbs-per-machine/dirs/a/nested/note.txt. Read them again before relying on what you saw.
agent: A carried on
agent: C wrote

== a child whose branch can't be made waits with git's words; the others carried on
child a893e61c2a7893d4.40.1 "badbranch": starting mx-hv-mj; mx-hv-mj failed: git checkout --quiet --no-track -b x origin/nope: fatal: 'origin/nope' is not a commit and a branch 'x' cannot be created f

end it with: node dev/drive.mjs down --brain limbs-per-machine
```
