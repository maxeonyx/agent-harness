# own-calls: refused only for someone else's change, and shown it

```sh
demos/own-calls/run.sh
```

Two engineers on the scripted model (no cost) work in one directory: A runs `mandate.md` and B runs `other.md`, one response per line. The run prints both transcripts, each tool call with its whole result.

- **Someone else's change.** A reads `shared.txt`, then makes two `edit`s of it in one slowly streamed response. Meanwhile B writes it. Both edits are refused, with no effect:
  - The first refusal shows `shared.txt` as B left it, and that now counts as A having read it.
  - The second refusal is the same response's, so it only points to the first. A's second edit was written before A saw the file, so neither edit goes through.
  - A's retry of `draft` → `A's draft` then fails honestly (`old` does not occur). Its edit of what B wrote completes.
- **Seen only through a command.** A runs `cat other.txt`, then `write`s it. The write is refused, and the refusal shows the file. The retry completes.
- **Its own calls.** These all complete:
  - two `edit`s of `f.txt` in one response;
  - an `edit`, a `sed -i` through `bash`, and another `edit`, in one response;
  - a `write` to `new.txt`, which A's own command made;
  - a `write` to `f.txt` after A's own `rm`.
- **A big file.** `big.txt` has 20000 lines, too many to show whole. A refused `edit` of line 15000 shows the file from 20 lines before it, up to `read`'s 64 KiB. The retry completes.
- **A binary file.** A refused `write` to `blob.bin` shows `(a binary file of 3 bytes, not shown)`.
