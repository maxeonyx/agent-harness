textstats needs two things, which can go in parallel, so start them in one scope:

1. An engineer builds `textstats/wordcount.py` and its tests as the README says, on the branch `wordcount`, runs `./check.sh`, commits and pushes.
2. A designer decides how `count_words` should treat non-ASCII text (accented letters, curly apostrophes, scripts without spaces). It writes the decision, and the alternatives it rejected, to `docs/unicode.md` on the branch `unicode-design`, commits and pushes. One page; this is a small decision.

Keep it small and cheap: this is a demonstration of the process, not a product. When both are back, check that each branch is on origin (`git fetch` then `git log --oneline origin/<branch>`), and report in a few lines.
