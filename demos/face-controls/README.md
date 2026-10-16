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
6. **Refusals.** Holding the stopped task, or sending it words, is refused with the core's reason, `Finished`. Failing a task isn't Max's to author, so the face refuses it.

Every control is one of Max's facts in the journal's own encoding, posted to `/act` as `{face, seen, fact}`:

```json
{"Task": ["<task>", {"Drain": "ToMax"}]}
{"Task": ["<task>", {"Drain": {"Stop": "Max"}}]}
{"Task": ["<task>", {"Held": true}]}
{"Ranking": {"order": [{"task": "<task>", "why": "placed by you"}]}}
{"Focus": {"face": "<face>", "task": "<task>"}}
{"Delivery": {"id": "<new id>", "to": "<task>", "content": {"Words": "[{\"type\":\"text\",\"text\":\"...\"}]"}}}
```

The brain folds the fact before it appends it. A 409 carries the core's `Rejected` by name.

## On the page

- **Select a task** and you focus on it: that's a `Focus` fact for this page's face.
- **Interrupt**, **Stop** and **Hold/Release** sit above its details. The answer appears beside them: done, or `refused: <reason>`.
- **While the task's request runs**, its text, thinking and tool input stream in below its context.
- **Waiting on you** lists the queue in ranked order. The ↑ ↓ buttons reorder it, and **Next** opens the top item and puts the cursor in the compose box.

## Real output

The demo's own lines:

```
   0.2s  the root task is orchestrator
   0.2s  == live text: a request of the root's as it streams, from /live
   6.1s  first connection to /live?model=6666ffe35862df44dc4f3214815d54ee, cut after 2 s: events 1 to 8
  22.1s  reconnected with Last-Event-ID: 8: events 9 to 685, then: event: end
  22.1s  the streamed text, from both connections:
          {"tasks": [{"name":"wordcount","mandate":"Implement textstats/wordcount.py and tests/test_wordcount.
          py per README.md. Requirements:\n- count_words(text: str) -> dict[str, int]: lower-cased word counts
          ; a word is a maximal run of letters, digits or apostrophes (use regex like [A-Za-z0-9']+, matched c
          ase-insensitively then lower-cased).\n- top(text: str, n: int) -> list[tuple[str, int]]: the n most 
          ...

  22.1s  == hold the root: nothing under it sends a request until it is released
          answer:  -> 200
  22.2s  held now: orchestrator-wordcount, orchestrator-palindromes
  27.2s  5 s later: 0 requests sent meanwhile; release it
          answer:  -> 200
  27.2s  == interrupt both children as soon as they exist
  27.2s  interrupt orchestrator-palindromes while it is on 'harness'
          answer:  -> 200
  27.3s  interrupt orchestrator-wordcount while it is on 'model'
          answer:  -> 200
  28.8s  both wait on Max; his queue, oldest first:
          orchestrator-palindromes: unranked
          orchestrator-wordcount: unranked
  28.9s  == reorder the queue: the second to the top
          answer:  -> 200
          orchestrator-wordcount: placed by you
          orchestrator-palindromes: placed by you
  28.9s  == focus the top item, orchestrator-wordcount, and tell it to carry on
          answer:  -> 200
          answer:  -> 200
  29.0s  the same post again, as a page whose connection dropped would retry it:
          answer: Exists -> 409
  29.0s  == stop the other, orchestrator-palindromes
          answer:  -> 200
  29.0s  orchestrator-palindromes: cancelled by Max
  29.0s  == what the core refuses
  29.0s  hold the stopped task:
          answer: Finished -> 409
  29.1s  words to the stopped task:
          answer: Finished -> 409
  29.1s  a fact that is not Max's to author (failing a task):
          answer: not a fact Max authors -> 403
  29.1s  == wait for the root's result
 174.8s  done; the brain's own log:
```

The brain's log, trimmed to the controls. Records 85 to 95 weren't scripted. Near the end the root ran `find /` over the whole machine looking for `check.sh` (the brain's known friction: nothing tells the model its workspace directory). I interrupted it from the face with the same `/act` posts and told it where it was; the running bash call was interrupted, and it carried on:

```
   22.1s    18  orchestrator: responded, ToolUse, 1 calls (2 in, 2587 cached, 1943 out)
   22.1s    19  orchestrator's spawn dispatched
   22.1s    20  orchestrator-wordcount created (orchestrator, forked from orchestrator)
   22.1s    21  orchestrator-palindromes created (orchestrator, forked from orchestrator)
   22.1s    22  orchestrator held
   27.2s    23  orchestrator released
   27.2s    24  orchestrator-wordcount: request sent
   27.2s    25  orchestrator-palindromes interrupted
   27.3s    26  orchestrator-wordcount interrupted
   28.6s    27  orchestrator-wordcount: first byte
   28.6s    28  orchestrator-wordcount: response abandoned
   28.9s    29  queue ranked: 2 placed
   28.9s    30  a face changed focus
   29.0s    31  words delivered to orchestrator-wordcount
   29.0s    32  orchestrator-wordcount: request sent
   29.0s    33  orchestrator-palindromes stopped by Max
   30.3s    34  orchestrator-wordcount: first byte
   33.0s    35  orchestrator-wordcount's write made {"content":"\"\"\"Word counting utilities.\"\"\"\n\nimport re\n\n_WORD_RE = re.compile(r\"[A-Za-z0-9']+\")\n\n
   33.0s    36  orchestrator-wordcount: responded, ToolUse, 1 calls (2 in, 6252 cached, 373 out)
   ...
   64.1s    55  orchestrator-wordcount: responded, EndTurn, 0 calls (2 in, 7734 cached, 227 out)
   64.1s    56  orchestrator's spawn completed
   64.3s    57  orchestrator: request sent
   ...
   91.2s    85  orchestrator's bash made {"command":"cd \"$(dirname \"$(find / -maxdepth 8 -name check.sh -path '*face-controls-demo*' 2>/dev/null | he
   91.2s    86  orchestrator: responded, ToolUse, 1 calls (2 in, 7243 cached, 160 out)
   91.2s    87  orchestrator's bash dispatched
  146.7s    88  orchestrator interrupted
  146.7s    89  orchestrator's bash interrupted
  147.2s    90  words delivered to orchestrator
  147.2s    91  orchestrator: request sent
  148.6s    92  orchestrator: first byte
  149.2s    93  orchestrator's bash made {"command":"pwd && ./check.sh","grep":"FAIL|ERROR|ok$|Ran|style"}
  149.2s    94  orchestrator: responded, ToolUse, 1 calls (2 in, 8721 cached, 95 out)
  149.2s    95  orchestrator's bash dispatched
   ...

== the root is done; its result:

Committed. Final report:

**What happened:**
- **Fork agent (wordcount)**: Implemented `textstats/wordcount.py` — `count_words` (regex `[A-Za-z0-9']+`, lower-cased) and `top` (sorted by `(-count, word)`, alphabetical tie-break). Wrote 15 unit tests in `tests/test_wordcount.py` covering punctuation, case, digits, apostrophes, ties, and edge cases (`n=0`, `n` too large, empty text). `./check.sh` passed for its part.
- **Fresh engineer (palindromes)**: Was cancelled before producing any output, so I (fork/orchestrator) implemented this part myself: `textstats/palindromes.py` with `is_palindrome` (strips non-alphanumerics, lower-cases, rejects empty) and `find_palindromes` (tokenizes on `[A-Za-z0-9]+`, dedups by first appearance, filters by `min_len` and palindrome check), plus 18 unit tests in `tests/test_palindromes.py` covering phrases with punctuation, digits, case-insensitivity, dedup ordering, and `min_len` variations.

**Verification:** Ran `./check.sh` from the repo root — all 33 tests pass, style check ok.

**Commit:** `f9ef759 Implement wordcount and palindromes modules with tests`, adding `textstats/wordcount.py`, `textstats/palindromes.py`, `tests/test_wordcount.py`, `tests/test_palindromes.py`.
```
