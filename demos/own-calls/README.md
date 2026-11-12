# own-calls: a write refused only for someone else's change, and an edit checked by its text

```sh
demos/own-calls/run.sh
```

Two engineers on the scripted model (no cost) work in one directory: A runs `mandate.md` and B runs `other.md`, one response per line. The run prints both transcripts, each tool call with its whole result.

- **Someone else's change.** A reads `shared.txt`, then makes two `write`s of it in one slowly streamed response. Meanwhile B writes it. Both writes are refused, with no effect:
  - The first refusal shows `shared.txt` as B left it, and that now counts as A having read it.
  - The second refusal is the same response's, so it only points to the first. A's second write was written before A saw the file, so neither goes through.
  - A's retry then completes.
- **Never seen through a file tool.** A runs `cat other.txt`, then `edit`s it: the text it names is there, so the edit completes. A `write` over `unseen.txt`, which A never looked at, completes and says `replacing a file of 1 line you had not seen`.
- **Its own calls.** These all complete:
  - two `edit`s of `f.txt` in one response;
  - an `edit`, a `sed -i` through `bash`, and another `edit`, in one response;
  - a `write` to `new.txt`, which A's own command made;
  - a `write` to `f.txt` after A's own `rm`.
- **A big file.** `big.txt` has 20000 lines, too many to show whole. An `edit` of line 15000 completes. One whose `old` does not occur fails and shows the file from 20 lines before where its first line is, up to `read`'s 64 KiB.
- **Several replacements in one `edit`.** The first edit of `three.txt` makes three replacements and writes them together. The second fails on its second replacement, because `beta` is already `B`. The result names that replacement and shows the file, and nothing is written: the file stays `A`/`B`/`C`, with the first replacement not applied either.
