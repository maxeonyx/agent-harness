# face-controls: Max's controls, his queue and live text, from the face

```sh
demos/face-controls/run.sh
```

It starts a real brain run (Sonnet) on the brain demo's scratch repo and mandate, then drives it only through the face's HTTP endpoints, the way the page does. Watch it at https://mx-hv-mj.local:4323 while it runs (another port: `PORT=4400`). It takes a few minutes and ends when the root has its result.

What it does, in order:

1. **Live text.** It reads one of the root's requests from `/live?model=<id>` for 2 seconds, cuts the connection, and reconnects with `Last-Event-ID` the way a browser does. The second connection carries on from the next event to the end of the request.
2. **Hold.** It holds the root. Nothing under it sends a request, and the children show "held". Then it releases the root.
3. **Interrupt.** It interrupts both children. The one that was streaming has its response abandoned, and both wait on Max: they are in his queue.
4. **The queue.** It reorders the queue, focuses the new top item, and sends it words. It then posts the same words again, as a page would after a dropped connection, and the core refuses the copy as `Exists`.
5. **Stop.** It stops the other child, which ends cancelled by Max.
6. **Refusals.** Holding the stopped task, or sending it words, is refused with the core's reason, `Finished`. Failing a task isn't Max's to author, and the core refuses it as `Author`.

Every control is a batch of Max's facts in the journal's own encoding, posted to `/act` as `{face, seen, batch: {values, facts}}`. Max's words are a value: the page posts their bytes to `/digest` for their id, and sends the bytes in `values` beside the fact that names them:

```json
{"Task": ["<task>", {"Drain": "ToMax"}]}
{"Task": ["<task>", {"Drain": {"Stop": "Max"}}]}
{"Task": ["<task>", {"Held": true}]}
{"Ranking": {"order": [{"task": "<task>", "why": "placed by you"}]}}
{"Focus": {"face": "<face>", "task": "<task>"}}
{"Delivery": {"id": "<new id>", "to": "<task>", "content": {"Words": "<digest of the words' bytes>"}}}
```

The brain commits the batch as Max's: the core decides what he may author, and a 409 carries its `Rejected` by name.

## On the page

- **Select a task** and you focus on it: that's a `Focus` fact for this page's face.
- **Interrupt**, **Stop** and **Hold/Release** sit above its details. The answer appears beside them: done, or `refused: <reason>`.
- **While the task's request runs**, its text, thinking and tool input stream in below its context.
- **Waiting on you** lists the queue in ranked order. The ↑ ↓ buttons reorder it, and **Next** opens the top item and puts the cursor in the compose box.

## Real output

The demo's own lines:

```
   0.0s  the root task is orchestrator
   0.0s  == live text: a request of the root's as it streams, from /live
   7.0s  first connection to /live?model=7d2d7ab3994275e2ad4aa6bb36b13a70, cut after 2 s: events 1 to 9
  16.6s  reconnected with Last-Event-ID: 9: events 10 to 484, then: event: end
  16.6s  the streamed text, from both connections:
          Both are empty. Now spawning the two agents in parallel.{"tasks": [{"name":"wordcount","mandate":"In
           this repo (textstats package), write `textstats/wordcount.py` implementing:\n- `count_words(text: s
          tr) -> dict[str, int]`: lower-cased word counts; a word is a maximal run of letters, digits or apost
          rophes (use regex like `[A-Za-z0-9']+` on lower-cased text, or find runs manually - standard library
          ...

  16.6s  == hold the root: nothing under it sends a request until it is released
          answer:  -> 200
  20.2s  held now: orchestrator-wordcount
  25.2s  5 s later: 0 requests sent meanwhile; release it
          answer:  -> 200
  25.3s  == interrupt both children as soon as they exist
  25.3s  interrupt orchestrator-palindromes while it is on 'model'
          answer:  -> 200
  25.3s  interrupt orchestrator-wordcount while it is on 'model'
          answer:  -> 200
  26.4s  both wait on Max; his queue, oldest first:
          orchestrator-wordcount: unranked
          orchestrator-palindromes: unranked
  26.4s  == reorder the queue: the second to the top
          answer:  -> 200
          orchestrator-palindromes: placed by you
          orchestrator-wordcount: placed by you
  26.5s  == focus the top item, orchestrator-palindromes, and tell it to carry on
          answer:  -> 200
          answer:  -> 200
  26.6s  the same post again, as a page whose connection dropped would retry it:
          answer: Exists -> 409
  26.6s  == stop the other, orchestrator-wordcount
          answer:  -> 200
  26.6s  orchestrator-wordcount: cancelled by Max
  26.6s  == what the core refuses
  26.6s  hold the stopped task:
          answer: Finished -> 409
  26.7s  words to the stopped task:
          answer: Finished -> 409
  26.7s  a fact that is not Max's to author (failing a task):
          answer: Author -> 409
  26.7s  == wait for the root's result
```

The brain's log, trimmed to the controls. The hold landed just after both children's first requests went out; each finished that response and then sent nothing until the release:

```
   16.6s    21  orchestrator's spawn made
   16.6s    22  orchestrator: responded, ToolUse, 1 calls (2 in, 3290 cached, 1295 out)
   16.6s    23  orchestrator's spawn dispatched
   16.6s    24  orchestrator-wordcount created (forked from orchestrator)
   16.6s    25  orchestrator-palindromes created (engineer)
   16.6s    26  orchestrator-palindromes: request sent
   16.6s    27  orchestrator-wordcount: request sent
   16.6s    28  orchestrator held
   17.6s    29  orchestrator-palindromes: first byte
   17.6s    30  orchestrator-wordcount: first byte
   20.1s    31  orchestrator-wordcount's write made {"content":"\"\"\"Word counting utilities.\"\"\"\n\nimport re\n\n_WORD_RE = re.compile(r\"[A-Za-z0
   20.1s    32  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 3412 cached, 356 out)
   20.1s    33  orchestrator-wordcount's write dispatched
   20.1s    34  orchestrator-wordcount's write completed
   20.1s    35  file textstats/wordcount.py changed by orchestrator-wordcount's write
   22.7s    36  orchestrator-palindromes's write made {"content":"import re\n\n_WORD_RE = re.compile(r\"[A-Za-z0-9']+\")\n\n\ndef is_palindrome(s: str
   22.7s    37  orchestrator-palindromes: responded, ToolUse, 1 calls (2 in, 0 cached, 640 out)
   22.7s    38  orchestrator-palindromes's write dispatched
   22.7s    39  orchestrator-palindromes's write completed
   22.7s    40  file textstats/palindromes.py changed by orchestrator-palindromes's write
   25.3s    41  orchestrator released
   25.3s    42  orchestrator-palindromes: request sent
   25.3s    43  orchestrator-wordcount: request sent
   25.3s    44  orchestrator-palindromes interrupted
   25.4s    45  orchestrator-wordcount interrupted
   26.2s    46  orchestrator-wordcount: first byte
   26.2s    47  orchestrator-wordcount: response abandoned
   26.2s    48  orchestrator-palindromes: first byte
   26.2s    49  orchestrator-palindromes: response abandoned
   26.4s    50  queue ranked: 2 placed
   26.5s    51  a face changed focus
   26.5s    52  words delivered to orchestrator-palindromes
   26.6s    53  orchestrator-palindromes: request sent
   26.6s    54  orchestrator-wordcount stopped by Max
   27.5s    55  orchestrator-palindromes: first byte
   ...
```

```
== the root is done; its result:

Done. Report:

**What happened:**
- The fresh engineer agent implemented `textstats/palindromes.py` (`is_palindrome`, `find_palindromes`) and `tests/test_palindromes.py` (10 unittest cases covering empty strings, case-insensitivity, punctuation stripping, min_len filtering, order-of-first-appearance dedup, and no-match cases). Its own `./check.sh` run passed.
- My forked task to write `textstats/wordcount.py` and its tests was cancelled mid-way: `wordcount.py` (`count_words`, `top`) had already been written and was complete/correct, but `tests/test_wordcount.py` was never created.
- I wrote `tests/test_wordcount.py` myself (14 cases covering basic counting, lower-casing, apostrophes, digits, punctuation splitting, empty input, `top()` ordering, alphabetical tie-breaking, `n=0`, and `n` larger than the vocabulary).

**Verification:** Ran `./check.sh` at the repo root — all 20 tests pass (10 palindrome tests + 10 wordcount tests) and the style check reports ok.

**Committed:** `df7c265 Implement wordcount and palindromes modules with tests`, adding `textstats/wordcount.py`, `textstats/palindromes.py`, `tests/test_wordcount.py`, `tests/test_palindromes.py`.
```
