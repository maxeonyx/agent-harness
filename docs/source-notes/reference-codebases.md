# We should use *liberal* inspiration from open-source alternatives:

- OpenCode (my daily driver, my main UX habits, lots of experience in its internals via my existing fork of it)
- OpenAI Codex (rust based, good "unified" codebase when compared to opencode)
- Pi (minimal, interesting plugin system, very good "meta workflow" - agent gets harness docs out of the box, not so much a fan of the UI and no good "plugin edit safety" - sometimes agents brick themselves)

We should also look at these libraries' primitives:

- OpenTUI
- Agent SDK

However, I don't want to straight up use their code. We are better to simply pull it across, rewrite it in our style.

Primitives of my own that I am keen to use:

- deconfuse (python "modular config" library - port to rust first)
- trunc (head + tail + grep for avoiding context overload) - make a library mode and use it by default for commands. Agent would be required to write in a grep term (in trunc, grep terms only *add* to the results in addition to the head + tail)

And as stated earlier in "tech.md", I'm excited to try embedding Deno.