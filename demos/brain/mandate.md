This repository is a tiny Python package with no code yet; `README.md` says what it should become.

Build it with two agents working in parallel, in one `spawn` call:
- a fork of yourself writes `textstats/wordcount.py` and `tests/test_wordcount.py`;
- a fresh engineer, on the branch `palindromes` in its own clone, writes `textstats/palindromes.py` and `tests/test_palindromes.py`, commits them and pushes the branch.

Read the repository first, so the fork knows what you know; give the fresh engineer everything it needs in its mandate. Each agent runs `./check.sh` before it ends its turn.

When both have returned, merge the engineer's branch (`git merge palindromes`), run `./check.sh` yourself, fix anything small, commit everything with git, and end your turn with a short report of what each agent did.
