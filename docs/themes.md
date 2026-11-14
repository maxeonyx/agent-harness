# Making a theme

You make the next theme for Max's agent pages and the map, or take one further. A theme is one area's art: every agent in the area gets its own painting of it, and the map paints the area faintly in it. There are more areas than themes, and Max wants "to be able to spin up additional themes at will", each as good as the set he called "Absolute beast. I love it."

The contract is the top comment of `crates/face-web/src/page/theme.js`, the concept is `Theme` in `crates/core/src/design.rs`, and the bars are in `dev/theme-check.mjs`. The set is every `theme-*.js`: each opens with what it depicts, who each agent is, its technique, and what was tried and dropped and why. Read them all, and look at the whole set side by side before choosing a direction.

## What a theme must be

A picture worth hanging, not a decoration or a stock effect: depth, light, material, composition, a restrained palette, made with real generative technique. Max asked for art that is "really elegant and artistic", and of a round that fell short: "They're like 10% better, where I wanted like 5 times better." Calm where text sits: the ground sits behind whole pages of text, so its drama can't come from deep darks where text runs. Distinct from every theme in the set in what it looks at and how it's made, not only colour, so it reads as itself at thumbnail size (of the first set: "Harbor and Cowrie seem very similar right now"). Agents in one area told apart at a glance by colour as well as composition: of Pāua, "it needs inner variation in colour and identity, not just layout, so that individual agents within an area are distinguishable at a glance."

Tried and not taken, so start somewhere else:

- Riso, a risograph print: "Maybe I don't really like Riso very much".
- Ledger, graph paper: "Ledger is a bit busy". Anything ruled across the whole page reads the same way.
- Nocturne, a night sky: "Nocturne is a bit much as a background."
- Kapua, only sky as an oil sketch: the critic found it read as a paint filter, and sat too close to Tussock's sky.
- Bach, the sunlit weatherboard wall of a bach, is contested: its beauty was strong sun against dark shade, and lifting the shade enough to read text over it took the light with it. Whether it ships is one of Max's open questions in [plan.md](plan.md#questions-only-max-can-answer), with where its study lives, so don't remake it.

## How a theme gets made

Nobody judges their own work: an artist's eye on its own piece is a conflict of interest.

1. Studies. Two or three artists (fresh `designer` agents), each on a materially different direction from the others and from the set. Give each this file, the contract, the set's renders side by side, Max's words on the art (find them with `find`, and paste them in verbatim, since a `spawn` quote carries only words heard directly), and "How to design this" verbatim. Each makes a working `theme-<name>.js` that meets every bar: a study, not a sketch. Artists don't commit.
2. Rounds. A critic that made none of them judges them all beside the set, against Max's words, and picks one or none, with ranked findings an artist can act on in a round. Resume the artist that made the pick (it knows its code) with the findings, and have the critic judge again. A study the critic turns down twice is dropped, unless max-advocate thinks Max might love it anyway: then it comes to him as contested, and nothing wears it until he says. His eye can differ from a critic's: of Pāua, unfinished, he said "I actually like paua a lot".
3. The check, then max-advocate: "What would Max say about this?", with the renders described in words, since it can't see images. Act on it.
4. A PR adding `theme-<name>.js`, its display face, and its name to `THEMES` in `crates/core/src/design.rs`, the one list of themes. A theme nobody has worn is the one the next new area takes. Another agent reviews it, and on its clean verdict the agent that opened it merges.
5. A page for Max under `~/agent-harness-workspace/pages/`: the theme painted full-screen behind a real page, six agents of one area, and its map region beside the others'. Show the paintings, not the process.

## Bars and practicalities

- `node dev/theme-check.mjs <name> [--size WxH]...` paints every agent slot and holds it to the bars, each bar's reason in its top comment, and writes the PNGs to look at; `--size` paints every slot at another window shape. Art is moving behind the whole chat window, from a phone (about 0.45 wide to high) to a desktop card (about 1.05), so look at those shapes too, and keep body text readable over all of it: only a few percent of the page should fall under 4.5:1 for the theme's body ink. The check holds only the lead's box to its bar, so judge the rest of the page from the `--size` shots. Run with no theme named, it first fails when `THEMES` and the `theme-*.js` files differ, naming the extras on each side.
- Paint time moves with machine load, and several artists on one machine push it past the bar. Rerun when `/proc/loadavg` is low before trusting a miss.
- A new theme's display face is an OFL upright face no other theme uses (Harbour and Marama share Newsreader; a new theme needn't change that), vendored as `crates/face-web/src/page/font-<family>.woff2`, with `font-<family>.OFL.txt` beside it. It sets the page's lead, so it is subset to Basic Latin, Latin-1, Latin Extended-A (for macrons) and General Punctuation (dashes, curly quotes, the ellipsis: without them a lead's "—" falls back to Georgia mid-line), keeping fonttools' default layout features: `pyftsubset <Family>.ttf --unicodes=U+0020-007E,U+00A0-017F,U+2000-206F,U+20AC,U+2122,U+2212 --output-file=font-<family>.ttf`. There is no pip on the machine, so no brotli for `--flavor=woff2`: compress the TTF with `/usr/bin/woff2_compress`.
- Scratch work (studies, renders, critiques) goes in `~/agent-harness-workspace/pages/<name>/`, never in the repo. Only the chosen theme's files go in the PR.
