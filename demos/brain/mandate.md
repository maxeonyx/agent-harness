This repository is a tiny Python package with no code yet; `README.md` says what it should become.

Build it with two agents working in parallel, in one `spawn` call:
- a fork of yourself writes `textstats/wordcount.py` and `tests/test_wordcount.py`;
- a fresh engineer writes `textstats/palindromes.py` and `tests/test_palindromes.py`.

Read the repository first, so the fork knows what you know; give the fresh engineer everything it needs in its mandate. Each agent runs `./check.sh` before it ends its turn.

When both have returned, run `./check.sh` yourself, fix anything small, commit everything with git, and end your turn with a short report of what each agent did.
