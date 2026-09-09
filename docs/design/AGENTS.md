# Writing design docs

One file per design aspect, so the agents who build the harness read the design and Max can trust what gets built. A design doc is Max's statements, his answers, and the questions still open. Agents write no design prose in it — the only agent words are headings, a one-line intro, and the `Question:` / `Max:` labels.

## Method

Per aspect: assemble his statements and the gap questions, put the questions to him, record his answers verbatim. Three sections, in this order.

**`## Max's statements`** — his words only, quoted, each with its source, grouped by topic. Sources are `docs/source-notes/` and material quoted from him inside `docs/process/`. A design doc is agent output and never a source. Anything not traceable was introduced without cause — hunt it and remove it. Where two of his statements conflict, keep both under a `### Tension:` heading and say whether an ordering between them is recoverable; where later wording supersedes earlier, mark the supersession and keep both. Where a claim rests on agent-written text with no source of his behind it, say so at the quote.

**`## Answers`** — his answer verbatim, under the question it answers.

**`## Open questions`** — one per gap, saying only what his notes say either way. Never a proposal: options read to him as rules the agent wrote, and he will answer the rule instead of the question. An answer of "needs thinking" leaves the question here with his answer attached.

Max, 2026-09-09: "BTW questions in these design dos should not be too open, or they will be useless - they need to jog my memory." So a question quotes the specific claim, choice or example in front of him and asks him to confirm, deny or refine it — "the doc claimed X; your note leans Y; which?" — never "how should Z work?".

```text
Bad, and he has to invent the whole answer:
  How should notice policy be represented?

Good, because it hands him both sides:
  The doc said policy and thresholds are "values the code reads, not branches
  the code hard-codes". Toward data: you want a meta-agent to A/B tune them.
  Toward code: your utility-model classifier consults a model, which is an
  effect rather than a value. Data with the classifier as a declared effect?
```

Never paraphrase him. Never remove a hedge. Never sharpen a conditional into a rule.

A statement that bears on several aspects goes in all of them (Max, 2026-08-12: "I don't really care about saying the same things twice unless they conflict"). Hunt conflicts, not repetition.

## Check the premise before you ask

An invented constraint produces a question with no answer: a priced statement read as a prohibition, or two decouplable concerns fused into a forced trade-off. For example, "credentials must live outside the database" came from an over-broad replication premise.

## Rules

- Markdown is never hard-wrapped.
- No READMEs for agent consumption. AGENTS.md always.
