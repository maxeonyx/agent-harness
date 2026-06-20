See OpenCode's TUI, basically. But we'll do a different color scheme - anthropic web style. I think we should liberally take from opencode

We'll try to use opentui rust.

Here are the style principles:

1 row height = 2 col width, everywhere.

Where we can, ie. padding rows with a color transition, we use *half block transitions* to increase content density. that is effectively 1/2 row padding for the elements both above and below. 1/2 row = 1 col width, so containers with 1/2 row above/below have 1 col left/right padding.

Background: anthropic beige. Inactive editable areas off-white. Active editable areas white. Most text black. Important text kinda dark maroon. Look at anthropic web colors tho.

Scrollbar:
- opentui has a scrollbar primitive. I like it a LOT but it's not quite perfect. It needs configurable *half block end caps* - ie. the scrollbar can optionally extend by an additional half block (taking the space where arrows would be). That allows it to fit with our styling.

This is all captured in my opencode fork UI compactness change.

I would like to have windows-style persistent select, right click to copy, right click to paste. I never got that working in opencode.

