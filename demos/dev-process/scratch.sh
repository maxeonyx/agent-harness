#!/bin/sh
# Make the demo's scratch repository under $1: a bare `textstats.git` as its
# origin, so nothing the agents push leaves the machine, and Max's own clone
# of it, `textstats`, where the orchestrator works.
set -eu
rm -rf "$1"
mkdir -p "$1/seed/textstats" "$1/seed/tests"
cd "$1/seed"
cat > README.md <<'MD'
# textstats

Small text statistics, standard library only, Python 3.

- `textstats.wordcount.count_words(text: str) -> dict[str, int]`: lower-cased word counts; a word is a run of letters, digits or apostrophes.
- `textstats.wordcount.top(text: str, n: int) -> list[tuple[str, int]]`: the `n` most common words, ties broken alphabetically.

Tests live in `tests/`, use `unittest`, and run with `./check.sh`.
MD
cat > AGENTS.md <<'MD'
# textstats

- Run `./check.sh` before you end your turn.
- Standard library only.
- Commit to your own branch and push it to `origin`; never push to `main`.
MD
cat > check.sh <<'SH2'
#!/bin/sh
set -e
python3 -m unittest discover -s tests -t . -v 2>&1
SH2
chmod +x check.sh
touch textstats/__init__.py tests/__init__.py
printf '__pycache__/\n' > .gitignore
git init -q -b main
git -c user.name=Max -c user.email=max@example.com add -A
git -c user.name=Max -c user.email=max@example.com commit -qm "Start textstats"
cd ..
git clone -q --bare seed textstats.git
rm -rf seed
git clone -q textstats.git textstats
git -C textstats config user.name "textstats agents"
git -C textstats config user.email agents@example.com
