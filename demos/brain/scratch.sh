#!/bin/sh
# Make the demo's scratch repository at $1: a Python package to be written.
set -eu
rm -rf "$1"
mkdir -p "$1/textstats" "$1/tests"
cd "$1"
cat > README.md <<'MD'
# textstats

Small text statistics, standard library only, Python 3.

- `textstats.wordcount.count_words(text: str) -> dict[str, int]`: lower-cased word counts; a word is a run of letters, digits or apostrophes.
- `textstats.wordcount.top(text: str, n: int) -> list[tuple[str, int]]`: the `n` most common words, ties broken alphabetically.
- `textstats.palindromes.is_palindrome(s: str) -> bool`: ignoring case and anything that is not a letter or digit; the empty string is not one.
- `textstats.palindromes.find_palindromes(text: str, min_len: int = 3) -> list[str]`: the distinct palindromic words of at least `min_len` characters, lower-cased, in order of first appearance.

Tests live in `tests/`, use `unittest`, and run with `./check.sh`.
MD
cat > AGENTS.md <<'MD'
# textstats

- Run `./check.sh` before you end your turn. It runs every test and then the slow style check; it takes about 15 seconds.
- Standard library only.
MD
cat > check.sh <<'SH'
#!/bin/sh
# Every test, then the (slow) style check.
set -e
python3 -m unittest discover -s tests -t . -v 2>&1
echo "style: checking..."
sleep 15
echo "style: ok"
SH
chmod +x check.sh
touch textstats/__init__.py tests/__init__.py
printf '__pycache__/\n' > .gitignore
git init -q -b main
git config user.name "textstats agents"
git config user.email agents@example.com
git add -A
git commit -qm "Start textstats"
