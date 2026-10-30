Tool calls made together show as one block, folded to the agent's summary of each call; open it for the calls, and a call for its input and output.
tool bash {"command": "ls -la", "summary": "See what the repo holds"} tool grep {"pattern": "def ", "path": ".", "summary": "Find any functions already written"} tool read {"path": "README.md", "summary": "Read what the package should become"}
tool read {"path": "check.sh", "summary": "See how the tests run"}
say Nothing is written yet; `README.md` says what to build. Stage two parts and send them.
