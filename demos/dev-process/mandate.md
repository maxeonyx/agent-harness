textstats needs two things, which can go in parallel, so start them in one scope:

1. An engineer builds `textstats/wordcount.py` and its tests as the README says, on the branch `wordcount`, runs `./check.sh`, commits and pushes.
2. A designer decides how `count_words` should treat non-ASCII text (accented letters, curly apostrophes, scripts without spaces). It writes the decision, and the alternatives it rejected, to `docs/unicode.md` on the branch `unicode-design`, commits and pushes. One page; this is a small decision.

Quote this mandate to both of them: it is my own words. Keep it small and cheap: this is a demonstration of the process, not a product. And I type curly apostrophes, so “don’t” has to count as one word.

One choice is Max's, and the README doesn't settle it: whether a lone apostrophe (`'` on its own) counts as a word. Don't wait for him: decide what you think best, tell the engineer, and put the choice and why in your report so he can overturn it.

When both are back, check that each branch is on origin (`git fetch` then `git log --oneline origin/<branch>`), and report in a few lines.
