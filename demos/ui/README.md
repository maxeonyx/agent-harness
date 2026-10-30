# ui

`demos/ui/run.sh`: the face after Max's UI feedback of 29 Oct, on the scripted model (free, about 25 s plus the build). It leaves the brain `ui` up with five chats, each waiting on Max, and an orchestrator's team of eleven agents.

Watch, in the shots (`demos/ui/*.ignore.png`, in the order the run takes them) and then in the open page:

- `folded`, `calls`, `call`: four calls made together are one block, its head the agents' summaries joined (`See what the repo holds → Find any functions already written → …`); opened, each call is a row of its tool and summary, and a row opens to its input and output. The scripts are `tools.md`, `compaction.md` and `busy.md`: each line starting with a verb is one response, and each call carries `summary` as its last field.
- `staged`, `sent`: Send with words in the box stages them above a divider; twice gives two parts; Send with the box empty sends both as one message, and the scripted reply answers the second.
- `compacted`: what the agent carried, with what it replaced folded away, and its node's dot for the compaction.
- `card`, `edges`: an open card pushes the map apart around it, and the arrows at the screen's edge point at the chats that wait on Max off screen; clicking one pans to it.
- `map`: the whole map with the card closed.

End it with `node dev/drive.mjs down --brain ui`.
