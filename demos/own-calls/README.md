# own-calls: refused only for someone else's change

```sh
demos/own-calls/run.sh
```

An engineer on the scripted model (no cost) runs `mandate.md`, one response per line. Watch the outcomes it prints:

- `cat other.txt`, then `write` it: **refused**. Seeing a file in a command's output does not count as seeing it, so the write could replace someone else's work; the refusal says to `read` it first.
- `read f.txt`, then two `edit`s of it in one response: **both complete**. Each call expects what the earlier calls of its response left.
- `edit`, `sed -i` through `bash`, `edit`, in one response: **all complete**.
- `echo a > new.txt`, then `write` it: **completes**. The file was last changed while the agent's own command ran.
- `old.txt` made by a command but dated an hour back, then `write` it: **refused**. It did not change while one of the agent's commands ran.

The files end as `f.txt` = `uno`/`2`, `new.txt` = `b`, and `other.txt` and `old.txt` untouched.
