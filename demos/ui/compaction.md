An agent that compacts between steps: what it did before is folded into what it carried, and its node on the map counts the compaction.
tool bash {"command": "ls", "summary": "Look around before the first step"}
tool compact_and_continue {"carry": "Looked at the repo: README.md says what to build.\ntool bash {\"command\": \"cat AGENTS.md\", \"summary\": \"Read the repo's instructions\"}\nsay I compacted once and carried on. Your turn."}
